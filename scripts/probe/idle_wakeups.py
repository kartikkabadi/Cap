#!/usr/bin/env python3
"""Scratch harness for ci/idle-vblank-ab. Not for main.

Launches each probe binary, lets its window settle, then counts the main
thread's context switches (each one is a wakeup) and the process CPU time over
a fixed window. Variants alternate order every round so drift hits both.

    idle_wakeups.py --label idle --seconds 60 --rounds 3 base=path head=path
"""

import argparse
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time

IS_WINDOWS = sys.platform == "win32"
# Precise CPU cost: Windows reports CPU cycles, Linux reports run time.
WORK_KEY = "mcycles_per_s" if IS_WINDOWS else "cpu_ms_per_s"


if IS_WINDOWS:
    import ctypes
    from ctypes import wintypes

    ntdll = ctypes.WinDLL("ntdll")
    ntdll.NtQuerySystemInformation.argtypes = [
        ctypes.c_ulong,
        ctypes.c_void_p,
        ctypes.c_ulong,
        ctypes.POINTER(ctypes.c_ulong),
    ]
    ntdll.NtQuerySystemInformation.restype = ctypes.c_long
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.OpenThread.restype = wintypes.HANDLE
    kernel32.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.QueryThreadCycleTime.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_ulonglong)]
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    THREAD_QUERY_LIMITED_INFORMATION = 0x0800

    def thread_cycles(tid):
        handle = kernel32.OpenThread(THREAD_QUERY_LIMITED_INFORMATION, False, tid)
        if not handle:
            return 0
        cycles = ctypes.c_ulonglong(0)
        kernel32.QueryThreadCycleTime(handle, ctypes.byref(cycles))
        kernel32.CloseHandle(handle)
        return cycles.value
    SYSTEM_PROCESS_INFORMATION = 5
    PROCESS_HEADER = 256  # sizeof(SYSTEM_PROCESS_INFORMATION) on x64
    THREAD_SIZE = 80  # sizeof(SYSTEM_THREAD_INFORMATION) on x64

    def snapshot(pid):
        size = 1 << 22
        while True:
            buf = ctypes.create_string_buffer(size)
            needed = ctypes.c_ulong(0)
            status = ntdll.NtQuerySystemInformation(
                SYSTEM_PROCESS_INFORMATION, buf, size, ctypes.byref(needed)
            )
            if status == 0:
                break
            if status & 0xFFFFFFFF == 0xC0000004:  # STATUS_INFO_LENGTH_MISMATCH
                size = max(size * 2, needed.value + 65536)
                continue
            raise OSError(f"NtQuerySystemInformation failed: {status & 0xFFFFFFFF:#x}")
        raw = buf.raw
        offset = 0
        while True:
            next_offset = int.from_bytes(raw[offset : offset + 4], "little")
            threads = int.from_bytes(raw[offset + 4 : offset + 8], "little")
            process_id = int.from_bytes(raw[offset + 80 : offset + 88], "little")
            if process_id == pid:
                cycles = int.from_bytes(raw[offset + 24 : offset + 32], "little")
                user = int.from_bytes(raw[offset + 40 : offset + 48], "little")
                kernel = int.from_bytes(raw[offset + 48 : offset + 56], "little")
                per_thread = []
                base = offset + PROCESS_HEADER
                for index in range(threads):
                    t = base + index * THREAD_SIZE
                    t_kernel = int.from_bytes(raw[t : t + 8], "little")
                    t_user = int.from_bytes(raw[t + 8 : t + 16], "little")
                    t_create = int.from_bytes(raw[t + 16 : t + 24], "little")
                    t_id = int.from_bytes(raw[t + 48 : t + 56], "little")
                    t_switches = int.from_bytes(raw[t + 64 : t + 68], "little")
                    per_thread.append((t_create, t_id, t_switches, (t_user + t_kernel) / 1e7))
                # The main thread is the process's first thread.
                per_thread.sort()
                main = per_thread[0]
                return {
                    "time": time.perf_counter(),
                    "cpu": (user + kernel) / 1e7,
                    "switches": sum(t[2] for t in per_thread),
                    "main_switches": main[2],
                    "main_cpu": main[3],
                    # Cycle counts are exact; CPU times only advance per clock tick.
                    "work": cycles / 1e6,
                    "main_work": thread_cycles(main[1]) / 1e6,
                    "threads": {t[1]: t[2] for t in per_thread},
                }
            if next_offset == 0:
                raise LookupError(f"process {pid} not found")
            offset += next_offset

else:
    TICKS = os.sysconf("SC_CLK_TCK")

    def _switches(path):
        total = 0
        with open(path) as status:
            for line in status:
                if line.startswith(("voluntary_ctxt_switches", "nonvoluntary_ctxt_switches")):
                    total += int(line.split()[1])
        return total

    def _cpu(path):
        with open(path) as stat:
            fields = stat.read().rsplit(")", 1)[1].split()
        return (int(fields[11]) + int(fields[12])) / TICKS

    def _run_ms(path):
        with open(path) as schedstat:
            return int(schedstat.read().split()[0]) / 1e6

    def snapshot(pid):
        work = 0.0
        for tid in os.listdir(f"/proc/{pid}/task"):
            try:
                work += _run_ms(f"/proc/{pid}/task/{tid}/schedstat")
            except FileNotFoundError:
                pass
        threads = {}
        for tid in os.listdir(f"/proc/{pid}/task"):
            try:
                threads[int(tid)] = _switches(f"/proc/{pid}/task/{tid}/status")
            except FileNotFoundError:
                pass
        return {
            "time": time.perf_counter(),
            "cpu": _cpu(f"/proc/{pid}/stat"),
            "switches": sum(threads.values()),
            "main_switches": threads[pid],
            "main_cpu": _cpu(f"/proc/{pid}/task/{pid}/stat"),
            # Run time from schedstat is in ns; utime/stime only count 10 ms ticks.
            "work": work,
            "main_work": _run_ms(f"/proc/{pid}/task/{pid}/schedstat"),
            "threads": threads,
        }


def measure(binary, mode, warmup, seconds, fresh_env):
    env = dict(os.environ)
    if fresh_env:
        env[fresh_env] = tempfile.mkdtemp(prefix="idle-probe-")
    process = subprocess.Popen(
        [binary] + ([mode] if mode != "none" else []),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=env,
    )
    try:
        time.sleep(warmup)
        if process.poll() is not None:
            raise RuntimeError(f"{binary} exited early with {process.returncode}")
        start = snapshot(process.pid)
        time.sleep(seconds)
        end = snapshot(process.pid)
        if process.poll() is not None:
            raise RuntimeError(f"{binary} exited during the window with {process.returncode}")
    finally:
        process.kill()
        process.wait()
    wall = end["time"] - start["time"]
    busiest = sorted(
        (
            (end["threads"][tid] - start["threads"].get(tid, 0)) / wall
            for tid in end["threads"]
        ),
        reverse=True,
    )
    return {
        "main_wakeups_per_s": (end["main_switches"] - start["main_switches"]) / wall,
        "process_wakeups_per_s": (end["switches"] - start["switches"]) / wall,
        "cpu_pct": 100 * (end["cpu"] - start["cpu"]) / wall,
        "main_cpu_pct": 100 * (end["main_cpu"] - start["main_cpu"]) / wall,
        WORK_KEY: (end["work"] - start["work"]) / wall,
        "main_" + WORK_KEY: (end["main_work"] - start["main_work"]) / wall,
        "threads": len(end["threads"]),
        "top_thread_wakeups_per_s": [round(rate, 1) for rate in busiest[:5]],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--label", required=True)
    parser.add_argument("--mode", default="idle")
    parser.add_argument("--seconds", type=float, default=60)
    parser.add_argument("--warmup", type=float, default=8)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--fresh-env", help="env var set to a new empty directory per launch")
    parser.add_argument("--json", required=True)
    parser.add_argument("--summary", required=True)
    parser.add_argument("variants", nargs="+", help="name=binary")
    args = parser.parse_args()

    variants = [variant.split("=", 1) for variant in args.variants]
    results = {name: [] for name, _ in variants}
    for round_index in range(args.rounds):
        order = variants if round_index % 2 == 0 else list(reversed(variants))
        for name, binary in order:
            result = measure(binary, args.mode, args.warmup, args.seconds, args.fresh_env)
            results[name].append(result)
            print(f"{args.label} round {round_index + 1} {name}: {json.dumps(result)}", flush=True)

    with open(args.json, "w") as out:
        json.dump(results, out, indent=2)

    keys = [
        "main_wakeups_per_s",
        "process_wakeups_per_s",
        WORK_KEY,
        "main_" + WORK_KEY,
        "cpu_pct",
    ]
    lines = [
        f"### {args.label} ({args.mode}, {args.seconds:.0f} s x {args.rounds} rounds)",
        "",
        "| variant | " + " | ".join(f"{key} median (min-max)" for key in keys) + " |",
        "|---|" + "---|" * len(keys),
    ]
    for name, runs in results.items():
        cells = []
        for key in keys:
            values = [run[key] for run in runs]
            cells.append(f"{statistics.median(values):.3f} ({min(values):.3f}-{max(values):.3f})")
        lines.append(f"| {name} | " + " | ".join(cells) + " |")
    lines.append("")
    with open(args.summary, "a") as out:
        out.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
