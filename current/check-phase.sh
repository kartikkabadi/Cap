#!/usr/bin/env bash
set -euo pipefail
task_root=/Users/user/Documents/Codex/2026-10-24/ok-x20
phase="$1"
out_dir="$task_root/work/pr/$phase"
repo_dir="$task_root/work/Cap"
source "$task_root/work/review/test-env.sh"
export CAP_GPUI_APP_DATA_DIR="$out_dir/profile"
export CAP_GPUI_TAURI_STORE="$out_dir/profile/store"
export CAP_GPUI_LOG_DIR="$out_dir/profile/logs"
export CAP_REVIEW_SNAPSHOT="$out_dir/geometry.json"
mkdir -p "$CAP_GPUI_APP_DATA_DIR"
ln -sfn "$repo_dir/target/Frameworks" "$out_dir/Frameworks"
if [[ "$phase" == baseline ]]; then
 "$out_dir/gpui-tests" ui::text_input --test-threads=1 > "$out_dir/tests.log" 2>&1
else
 "$out_dir/gpui-tests" --test-threads=1 > "$out_dir/tests.log" 2>&1
fi
set +e
"$out_dir/gpui-tests" --exact ui::text_input::layout_cache_diagnostic::diagnose_text_input_layout_cache --ignored --nocapture --test-threads=1 > "$out_dir/cache-diagnostic.log" 2>&1
cache_status=$?
set -e
if [[ "$phase" == baseline ]]; then
 [[ "$cache_status" == 101 ]]
else
 [[ "$cache_status" == 0 ]]
fi
printf '%s\n' "$cache_status" > "$out_dir/cache-status.txt"
tail -n 4 "$out_dir/tests.log"
