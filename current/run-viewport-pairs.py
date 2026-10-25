import argparse
from collections import Counter
import csv
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time


parser = argparse.ArgumentParser()
parser.add_argument("--repo", type=Path)
parser.add_argument("--baseline", type=Path)
parser.add_argument("--candidate", type=Path)
parser.add_argument("--pairs", type=int, default=5)
parser.add_argument("--matrix", choices=["core", "all"], default="core")
parser.add_argument("--output", type=Path)
parser.add_argument("--build-profile", type=Path)
args = parser.parse_args()
assert args.pairs > 0
audit = Path(__file__).resolve().parent
root = audit.parent
repo = (args.repo or root / "cap-gpui-next").resolve()
baseline = args.baseline or audit / "bin/gpui-viewport-baseline"
candidate = args.candidate or audit / "bin/gpui-viewport-candidate"
output = args.output or audit / f"viewport-{args.matrix}-results"
output.mkdir(parents=True, exist_ok=True)
executables = {"baseline": baseline.resolve(), "candidate": candidate.resolve()}
test_name = "ui::text_input::navigation_viewport_benchmark::benchmark_text_input_navigation_viewport"
build_profile = (args.build_profile or audit / "app-benchmark-build-profile.json").resolve()
benchmark_source = audit / "text-input-viewport-benchmark.rs"
environment = os.environ.copy()
native = repo / "target/native-deps"
if platform.system() == "Linux" and native.is_dir():
    environment["LD_LIBRARY_PATH"] = ":".join(
        [str(native / "lib"), str(native / "onnxruntime/lib"), environment.get("LD_LIBRARY_PATH", "")]
    )
    if (native / "onnxruntime/lib/libonnxruntime.so").is_file():
        environment.setdefault("ORT_DYLIB_PATH", str(native / "onnxruntime/lib/libonnxruntime.so"))
if (root / ".tooling/xdg").is_dir():
    environment.setdefault("XDG_RUNTIME_DIR", str(root / ".tooling/xdg"))
environment["CAP_GRAPHEME_BENCH_MATRIX"] = args.matrix


def utc_now():
    return datetime.now(timezone.utc).isoformat(timespec="microseconds")


def reference(path):
    return {"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


cpu_info = Path("/proc/cpuinfo")
cpu = next(
    (line.split(":", 1)[1].strip() for line in (cpu_info.read_text().splitlines() if cpu_info.exists() else [])
     if line.startswith("model name")),
    platform.processor() or "unknown",
)
metadata = {
    "started_utc": utc_now(),
    "finished_utc": None,
    "matrix": args.matrix,
    "pairs": args.pairs,
    "test_name": test_name,
    "order": "Pair 1 baseline/candidate, pair 2 candidate/baseline, alternating thereafter.",
    "samples_per_case": 7,
    "expected_rows_per_process": 462 if args.matrix == "core" else 1701,
    "expected_cases_per_process": 66 if args.matrix == "core" else 243,
    "benchmark_source": reference(benchmark_source),
    "runner_source": reference(Path(__file__).resolve()),
    "executables": {name: reference(path) for name, path in executables.items()},
    "build_profile_reference": {
        **reference(build_profile),
        "contents": json.loads(build_profile.read_text()),
    },
    "build_log_references": {
        name: reference(path)
        for name in executables
        if (path := audit / f"gpui-viewport-{name}-build.log").exists()
    },
    "environment": {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "cpu": cpu,
        "cpu_affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "runtime_paths": {
            name: environment.get(name)
            for name in ["LD_LIBRARY_PATH", "ORT_DYLIB_PATH", "XDG_RUNTIME_DIR"]
        },
    },
    "viewport": {
        "window": [560, 320],
        "scroll_viewport": [560, 240],
        "text_width": 496,
        "font_size": 30,
        "line_height": 45,
        "alignment": "center",
        "navigation_scroll": "Centers the 120-step midpoint once before timing; no per-key autoscroll.",
        "row_range": "Visible row end is exclusive. Rows and offsets are included in each case key.",
        "scroll_sequence": "120 redraws over a two-row round trip; positions prepared outside timing.",
    },
    "scope": {
        "action_handler": "Production handler, state changes and notify; timer inside one update closure, before effect flush.",
        "key_dispatch": "Per-key GPUI binding dispatch plus synchronous TestPlatform CPU draw; no native font shaping, OS input or GPU presentation.",
        "scroll_redraw": "ScrollHandle offset changes plus synchronous TestPlatform CPU redraw; no navigation-helper calls, native font shaping or GPU presentation.",
    },
    "processes": [],
}


def save_metadata():
    temporary = output / "metadata.json.tmp"
    temporary.write_text(json.dumps(metadata, indent=2) + "\n")
    temporary.replace(output / "metadata.json")


save_metadata()
keys = [
    "kind", "target_bytes", "actual_bytes", "graphemes", "region", "motion", "scope", "steps",
    "window_width", "window_height", "viewport_width", "viewport_height", "text_width",
    "font_size", "line_height", "scroll_y", "visible_row_start", "visible_row_end",
    "caret_row_start", "caret_row_end", "total_rows", "scroll_offset_min", "scroll_offset_max",
]
expected_rows = 462 if args.matrix == "core" else 1701
expected_scope_cases = (
    {"action_handler": 24, "key_dispatch": 24, "scroll_redraw": 18}
    if args.matrix == "core"
    else {"action_handler": 108, "key_dispatch": 108, "scroll_redraw": 27}
)
processes = {}
reference_keys = None
for pair in range(args.pairs):
    order = ["baseline", "candidate"] if pair % 2 == 0 else ["candidate", "baseline"]
    pair_keys = {}
    for variant in order:
        print(f"Running viewport pair {pair + 1}/{args.pairs}: {variant} ({args.matrix})", flush=True)
        stem = output / f"pair-{pair + 1}-{variant}"
        process_metadata = {"pair": pair + 1, "variant": variant, "started_utc": utc_now()}
        started = time.monotonic()
        result = subprocess.run(
            [
                str(executables[variant]),
                "--exact",
                test_name,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ],
            cwd=repo,
            env={**environment, "CAP_GRAPHEME_BENCH_VARIANT": variant},
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=900,
        )
        process_metadata.update({
            "finished_utc": utc_now(),
            "elapsed_seconds": time.monotonic() - started,
            "exit_code": result.returncode,
            "stdout": str(stem.with_suffix(".stdout")),
            "stderr": str(stem.with_suffix(".stderr")),
        })
        metadata["processes"].append(process_metadata)
        stem.with_suffix(".stdout").write_text(result.stdout)
        stem.with_suffix(".stderr").write_text(result.stderr)
        save_metadata()
        result.check_returncode()
        lines = []
        for line in result.stdout.splitlines():
            if "variant,kind,target_bytes," in line:
                lines.append(line[line.index("variant,kind,target_bytes,") :])
            elif line.startswith(variant + ","):
                lines.append(line)
        rows = list(csv.DictReader(lines))
        assert len(rows) == expected_rows, (variant, len(rows), expected_rows)
        text = "\n".join(lines) + "\n"
        temporary = stem.with_suffix(".csv.tmp")
        temporary.write_text(text)
        temporary.replace(stem.with_suffix(".csv"))
        grouped = {}
        for row in rows:
            assert row["variant"] == variant
            assert None not in row, ("unexpected extra CSV fields", row)
            key = tuple(row[field] for field in keys)
            nanos = float(row["ns_per_step"])
            assert math.isfinite(nanos) and nanos > 0, (key, nanos)
            grouped.setdefault(key, []).append((int(row["sample"]), nanos))
        assert len(grouped) == expected_rows // 7, (variant, len(grouped), expected_rows // 7)
        scope_index = keys.index("scope")
        scope_cases = dict(Counter(key[scope_index] for key in grouped))
        assert scope_cases == expected_scope_cases, (variant, scope_cases, expected_scope_cases)
        case_keys = set(grouped)
        if reference_keys is None:
            reference_keys = case_keys
        else:
            assert case_keys == reference_keys, {
                "pair": pair + 1,
                "variant": variant,
                "missing_keys": sorted(reference_keys - case_keys),
                "extra_keys": sorted(case_keys - reference_keys),
            }
        pair_keys[variant] = case_keys
        for key, samples in grouped.items():
            assert sorted(sample for sample, _ in samples) == list(range(1, 8)), (key, samples)
            processes.setdefault(key, {}).setdefault(variant, {})[pair] = statistics.median(
                nanos for _, nanos in samples
            )
        process_metadata["validated_rows"] = len(rows)
        process_metadata["validated_cases"] = len(grouped)
        save_metadata()
    assert pair_keys["baseline"] == pair_keys["candidate"], (pair + 1, "case keys differ")

with (output / "summary.csv").open("w") as file:
    writer = csv.writer(file)
    writer.writerow(
        [*keys, "baseline_median_ns", "candidate_median_ns", "median_paired_speedup", "paired_min", "paired_max"]
    )
    for key, variants in processes.items():
        assert set(variants) == {"baseline", "candidate"}
        assert set(variants["baseline"]) == set(range(args.pairs))
        assert set(variants["candidate"]) == set(range(args.pairs))
        before = [variants["baseline"][pair] for pair in range(args.pairs)]
        after = [variants["candidate"][pair] for pair in range(args.pairs)]
        ratios = [left / right for left, right in zip(before, after)]
        writer.writerow(
            [
                *key,
                statistics.median(before),
                statistics.median(after),
                statistics.median(ratios),
                min(ratios),
                max(ratios),
            ]
        )
metadata["finished_utc"] = utc_now()
metadata["validated_cases"] = len(processes)
metadata["summary"] = str(output / "summary.csv")
save_metadata()
print(f"Saved {len(processes)} viewport comparisons in {output}", flush=True)
