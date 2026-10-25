#!/usr/bin/env bash
set -euo pipefail
task_root=/Users/user/Documents/Codex/2026-10-24/ok-x20
phase="$1"
repo_dir="$task_root/work/Cap"
out_dir="$task_root/work/pr/$phase"
mkdir -p "$out_dir"
export PATH="/Users/user/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin:$PATH"
export RUSTC=/Users/user/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin/rustc
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR="$task_root/work/gpui-target"
cd "$repo_dir"
app_opt=3
if [[ "$phase" == committed ]]; then app_opt=0; fi
python3 - "$task_root" "$phase" <<'PY'
from pathlib import Path
import sys,hashlib,json,subprocess
r=Path(sys.argv[1]); phase=sys.argv[2]
source=(r/'work/pr'/('current-baseline.rs' if phase=='baseline' else 'final-candidate.rs')).read_text()
diagnostic=r/('work/review/layout-cache-diagnostic-baseline.rs' if phase=='baseline' else 'work/cap-gpui-long-script-performance/evidence/layout-cache-diagnostic.rs')
hooks=[r/'work/review/independent-text-input.rs',r/'work/cap-gpui-long-script-performance/evidence/text-input-viewport-benchmark.rs',diagnostic]
if phase=='candidate': hooks.insert(0,r/'work/minimize/exhaustive-regressions.rs')
text=source+'\n'+''.join(f'#[cfg(test)]\ninclude!({json.dumps(str(p))});\n' for p in hooks)
text+=f'#[cfg(not(test))]\ninclude!({json.dumps(str(r/"work/review/native-module.rs"))});\n'
if phase=='committed':
 text=source
 hooks=[]
(r/'work/Cap/apps/desktop-gpui/src/ui/text_input.rs').write_text(text)
(r/'work/pr'/phase/'source.json').write_text(json.dumps({'base':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'production_sha256':hashlib.sha256(source.encode()).hexdigest(),'instrumented_sha256':hashlib.sha256(text.encode()).hexdigest(),'hooks':[str(p) for p in hooks]},indent=2)+'\n')
PY
cargo test --manifest-path apps/desktop-gpui/Cargo.toml --locked --offline --bin cap-gpui --no-run \
 --config profile.dev.package.cap-desktop-gpui.opt-level="$app_opt" \
 --config profile.dev.package.cap-desktop-gpui.debug=0 \
 --config 'profile.dev.package."*".debug=0' --message-format=json > "$out_dir/build.jsonl" 2> "$out_dir/build.log"
python3 - "$out_dir" <<'PY'
import sys,json,shutil,hashlib
from pathlib import Path
p=Path(sys.argv[1]); rows=[json.loads(s) for s in (p/'build.jsonl').read_text().splitlines()]
a=next(x for x in reversed(rows) if x.get('reason')=='compiler-artifact' and x.get('executable') and x['target']['name']=='cap-gpui')
shutil.copy2(a['executable'],p/'gpui-tests')
(p/'profile.json').write_text(json.dumps(a,indent=2)+'\n')
print('Built',p,hashlib.sha256((p/'gpui-tests').read_bytes()).hexdigest())
PY
