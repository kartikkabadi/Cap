#!/usr/bin/env bash
set -euo pipefail
task_root=/Users/user/Documents/Codex/2026-10-24/ok-x20
phase="$1"
repo_dir="$task_root/work/Cap"
out_dir="$task_root/work/pr/$phase"
export PATH="/Users/user/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin:$PATH"
export RUSTC=/Users/user/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin/rustc
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR="$task_root/work/gpui-target"
cd "$repo_dir"
mkdir -p apps/desktop-gpui/examples
cp "$task_root/work/review/native-main.rs" apps/desktop-gpui/examples/review-native.rs
cargo build --manifest-path apps/desktop-gpui/Cargo.toml --locked --offline --example review-native --features gpui_platform/test-support \
 --config profile.dev.package.cap-desktop-gpui.opt-level=3 \
 --config profile.dev.package.cap-desktop-gpui.debug=0 \
 --config 'profile.dev.package."*".debug=0' --message-format=json > "$out_dir/native-build.jsonl" 2> "$out_dir/native-build.log"
cp "$CARGO_TARGET_DIR/debug/examples/review-native" "$out_dir/native-capture"
source "$task_root/work/review/test-env.sh"
export CAP_NATIVE_OUTPUT="$out_dir/native"
"$out_dir/native-capture" > "$out_dir/native-run.log" 2>&1
