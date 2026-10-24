#!/usr/bin/env python3
"""Scratch (ci/idle-win-busy-ab, not for main): launch cap-gpui, let it settle,
then report per-thread CPU cycles, context switches, thread names, start
modules and leaf-module samples; optionally attach samply."""

import argparse
import ctypes
import json
import os
import subprocess
import tempfile
import time
from ctypes import wintypes

ntdll = ctypes.WinDLL("ntdll")
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)

ntdll.NtQuerySystemInformation.argtypes = [ctypes.c_ulong, ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong)]
ntdll.NtQuerySystemInformation.restype = ctypes.c_long
ntdll.NtQueryInformationThread.argtypes = [wintypes.HANDLE, ctypes.c_ulong, ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong)]
ntdll.NtQueryInformationThread.restype = ctypes.c_long
kernel32.OpenThread.restype = wintypes.HANDLE
kernel32.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel32.OpenProcess.restype = wintypes.HANDLE
kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel32.QueryThreadCycleTime.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_ulonglong)]
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
kernel32.SuspendThread.argtypes = [wintypes.HANDLE]
kernel32.SuspendThread.restype = wintypes.DWORD
kernel32.ResumeThread.argtypes = [wintypes.HANDLE]
kernel32.ResumeThread.restype = wintypes.DWORD
kernel32.GetThreadContext.argtypes = [wintypes.HANDLE, ctypes.c_void_p]
kernel32.GetThreadContext.restype = wintypes.BOOL
kernel32.GetThreadDescription.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_wchar_p)]
kernel32.GetThreadDescription.restype = ctypes.c_long
kernel32.LocalFree.argtypes = [ctypes.c_void_p]
psapi.EnumProcessModulesEx.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_void_p), wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), wintypes.DWORD]
psapi.GetModuleBaseNameW.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.LPWSTR, wintypes.DWORD]


class MODULEINFO(ctypes.Structure):
    _fields_ = [("lpBaseOfDll", ctypes.c_void_p), ("SizeOfImage", wintypes.DWORD), ("EntryPoint", ctypes.c_void_p)]


psapi.GetModuleInformation.argtypes = [wintypes.HANDLE, ctypes.c_void_p, ctypes.POINTER(MODULEINFO), wintypes.DWORD]

THREAD_ALL = 0x001F03FF
PROCESS_QUERY = 0x0400 | 0x0010


def snapshot(pid):
    size = 1 << 22
    for _ in range(8):
        buf = ctypes.create_string_buffer(size)
        needed = ctypes.c_ulong(0)
        status = ntdll.NtQuerySystemInformation(5, buf, size, ctypes.byref(needed))
        if status == 0:
            break
        if status & 0xFFFFFFFF == 0xC0000004:
            size = max(size * 2, needed.value + 65536)
            continue
        raise OSError(hex(status & 0xFFFFFFFF))
    raw = buf.raw
    offset = 0
    for _ in range(100000):
        next_offset = int.from_bytes(raw[offset:offset + 4], "little")
        threads = int.from_bytes(raw[offset + 4:offset + 8], "little")
        process_id = int.from_bytes(raw[offset + 80:offset + 88], "little")
        if process_id == pid:
            cycles = int.from_bytes(raw[offset + 24:offset + 32], "little")
            out = {"time": time.perf_counter(), "cycles": cycles, "threads": {}}
            base = offset + 256
            for index in range(threads):
                t = base + index * 80
                create = int.from_bytes(raw[t + 16:t + 24], "little")
                tid = int.from_bytes(raw[t + 48:t + 56], "little")
                switches = int.from_bytes(raw[t + 64:t + 68], "little")
                out["threads"][tid] = {"create": create, "switches": switches, "cycles": thread_cycles(tid)}
            return out
        if next_offset == 0:
            break
        offset += next_offset
    raise LookupError(pid)


def thread_cycles(tid):
    h = kernel32.OpenThread(0x0800, False, tid)
    if not h:
        return 0
    c = ctypes.c_ulonglong(0)
    kernel32.QueryThreadCycleTime(h, ctypes.byref(c))
    kernel32.CloseHandle(h)
    return c.value


def modules(pid):
    hp = kernel32.OpenProcess(PROCESS_QUERY, False, pid)
    arr = (ctypes.c_void_p * 2048)()
    needed = wintypes.DWORD(0)
    psapi.EnumProcessModulesEx(hp, arr, ctypes.sizeof(arr), ctypes.byref(needed), 3)
    out = []
    for i in range(needed.value // ctypes.sizeof(ctypes.c_void_p)):
        info = MODULEINFO()
        psapi.GetModuleInformation(hp, arr[i], ctypes.byref(info), ctypes.sizeof(info))
        name = ctypes.create_unicode_buffer(260)
        psapi.GetModuleBaseNameW(hp, arr[i], name, 260)
        out.append((info.lpBaseOfDll or 0, info.SizeOfImage, name.value))
    kernel32.CloseHandle(hp)
    return out


def where(mods, addr):
    for base, size, name in mods:
        if base <= addr < base + size:
            return f"{name}+{addr - base:#x}"
    return f"{addr:#x}"


def thread_info(tid, mods):
    h = kernel32.OpenThread(THREAD_ALL, False, tid)
    if not h:
        return "?", "?"
    desc = ctypes.c_wchar_p()
    name = ""
    if kernel32.GetThreadDescription(h, ctypes.byref(desc)) >= 0 and desc.value:
        name = desc.value
    start = ctypes.c_void_p()
    ntdll.NtQueryInformationThread(h, 9, ctypes.byref(start), 8, None)
    kernel32.CloseHandle(h)
    return name, where(mods, start.value or 0)


def leaf_samples(tids, mods, seconds, hz=100):
    buf = ctypes.create_string_buffer(1232 + 16)
    addr = (ctypes.addressof(buf) + 15) & ~15
    handles = {tid: kernel32.OpenThread(THREAD_ALL, False, tid) for tid in tids}
    counts = {tid: {} for tid in tids}
    end = time.perf_counter() + seconds
    for _ in range(int(seconds * hz)):
        if time.perf_counter() >= end:
            break
        for tid, h in handles.items():
            if not h:
                continue
            if kernel32.SuspendThread(h) == 0xFFFFFFFF:
                continue
            ctypes.memset(addr, 0, 1232)
            ctypes.c_uint32.from_address(addr + 0x30).value = 0x100001
            ok = kernel32.GetThreadContext(h, addr)
            kernel32.ResumeThread(h)
            if ok:
                rip = ctypes.c_uint64.from_address(addr + 0xF8).value
                key = where(mods, rip).split("+")[0]
                counts[tid][key] = counts[tid].get(key, 0) + 1
        time.sleep(1 / hz)
    for h in handles.values():
        if h:
            kernel32.CloseHandle(h)
    return counts


def screenshot(path):
    ps = (
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing;"
        "$b=[System.Windows.Forms.SystemInformation]::VirtualScreen;"
        "$bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height;"
        "$g=[System.Drawing.Graphics]::FromImage($bmp);"
        "$g.CopyFromScreen($b.Left,$b.Top,0,0,$bmp.Size);"
        f"$bmp.Save('{path}')"
    )
    subprocess.run(["powershell", "-NoProfile", "-Command", ps], check=False)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--label", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--warmup", type=float, default=25)
    p.add_argument("--seconds", type=float, default=30)
    p.add_argument("--samply")
    p.add_argument("--env", action="append", default=[])
    p.add_argument("binary")
    a = p.parse_args()
    os.makedirs(a.out, exist_ok=True)
    env = dict(os.environ)
    env["CAP_GPUI_APP_DATA_DIR"] = tempfile.mkdtemp(prefix="cap-app-")
    for kv in a.env:
        k, v = kv.split("=", 1)
        env[k] = v
    log = open(os.path.join(a.out, f"{a.label}-app.log"), "w")
    proc = subprocess.Popen([a.binary], stdout=log, stderr=subprocess.STDOUT, env=env)
    report = {}
    try:
        time.sleep(a.warmup)
        if proc.poll() is not None:
            raise RuntimeError(f"exited early {proc.returncode}")
        screenshot(os.path.join(a.out, f"{a.label}-screen.png"))
        mods = modules(proc.pid)
        s0 = snapshot(proc.pid)
        time.sleep(a.seconds)
        s1 = snapshot(proc.pid)
        wall = s1["time"] - s0["time"]
        main_tid = min(s1["threads"].items(), key=lambda kv: kv[1]["create"])[0]
        rows = []
        for tid, t1 in s1["threads"].items():
            t0 = s0["threads"].get(tid, {"switches": 0, "cycles": 0})
            name, start = thread_info(tid, mods)
            rows.append({
                "tid": tid, "main": tid == main_tid, "name": name, "start": start,
                "mcycles_per_s": (t1["cycles"] - t0["cycles"]) / wall / 1e6,
                "switches_per_s": (t1["switches"] - t0["switches"]) / wall,
            })
        rows.sort(key=lambda r: -r["mcycles_per_s"])
        total = (s1["cycles"] - s0["cycles"]) / wall / 1e6
        print(f"## {a.label}: process {total:.1f} Mcycles/s, {len(rows)} threads", flush=True)
        for r in rows:
            print(f"  tid={r['tid']:>6} main={int(r['main'])} {r['mcycles_per_s']:9.1f} Mc/s {r['switches_per_s']:8.1f} sw/s name={r['name']!r} start={r['start']}", flush=True)
        top = [r["tid"] for r in rows[:10]]
        leaves = leaf_samples(top, mods, 10)
        for r in rows[:10]:
            c = leaves[r["tid"]]
            n = sum(c.values()) or 1
            parts = ", ".join(f"{k} {100 * v / n:.0f}%" for k, v in sorted(c.items(), key=lambda kv: -kv[1])[:5])
            print(f"  leaf tid={r['tid']} {r['name']!r}: {parts}", flush=True)
            r["leaf"] = c
        report = {"process_mcycles_per_s": total, "threads": rows}
        if a.samply:
            subprocess.run([a.samply, "record", "-p", str(proc.pid), "-d", "15", "--save-only",
                            "--unstable-presymbolicate", "-o", os.path.join(a.out, f"{a.label}-samply.json.gz")],
                           check=False, timeout=600)
    finally:
        proc.kill()
        proc.wait()
        log.close()
    with open(os.path.join(a.out, f"{a.label}-threads.json"), "w") as f:
        json.dump(report, f, indent=1)


if __name__ == "__main__":
    main()
