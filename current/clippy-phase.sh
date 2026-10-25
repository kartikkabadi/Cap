#!/usr/bin/env bash
set -euo pipefail
task_root=/Users/user/Documents/Codex/2026-10-24/ok-x20
phase="$1"
export PATH="/Users/user/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin:$PATH"
export RUSTC=/Users/user/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin/rustc
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR="$task_root/work/gpui-target"
cd "$task_root/work/Cap"
cargo clippy --manifest-path apps/desktop-gpui/Cargo.toml --locked --offline --bin cap-gpui --tests --no-deps \
 --config profile.dev.package.cap-desktop-gpui.opt-level=3 \
 --config profile.dev.package.cap-desktop-gpui.debug=0 \
 --config 'profile.dev.package."*".debug=0' --message-format=json > "$task_root/work/pr/$phase/clippy.jsonl" 2> "$task_root/work/pr/$phase/clippy.log"
