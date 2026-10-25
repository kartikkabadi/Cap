# GPUI long-script text-input evidence

Code commit: [`8f14e93e92100144605f37dcf132a4dd36fd38d8`](https://github.com/kartikkabadi/Cap/commit/8f14e93e92100144605f37dcf132a4dd36fd38d8).

The PR code diff contains one file, +487/-72. This separate evidence snapshot contains no application executables, dependency caches, app profiles or credentials.

## Current-main validation

The timing baseline is `ec15b98403441aee7780ff7734fef93d2e1a323f`. The final committed suite was rebuilt at `900dd827b9626b5cb312cc993f715dfd465a53a9` after reviewing its unrelated Windows-picker changes. Last-checked main `100f43a0d3593f60e314d806f6ceef481509d39c` then added web/CLI-only changes. Its GPUI workspace, Rust crates and build configuration are identical to that tested tip; see `current/latest-main-equivalence.json`. Both measured text-input source files remain identical throughout. The final uninstrumented source SHA-256 is `a14085e4059c6e026c108ebc9293d2a53fa95b76b1debaa2f919a0b22c5516e6`. See `current/validation.json`, `current/performance-analysis.json`, the complete 66-case `current/performance-results/summary.csv`, raw samples and per-process metadata.

- Committed suite at app opt-level 0: 1,430 passed, 0 failed, 24 pre-existing opt-in tests ignored.
- Extended audit at app opt-level 3: 1,439 passed, 0 failed, 26 ignored. The cache probe and viewport benchmark were also run explicitly; the other 24 remain unexecuted.
- 480 matching geometry/selection/hit-test snapshots and 108 matching native font/Metal PNG pairs, plus four native cached/fresh font/color checks per build.
- Current native images are 560x320 at the same backing scale in both builds. Historical snapshots used other display scales; compare within each matched pair of builds.
- Clippy completed with the same 49 existing app diagnostics, zero new diagnostics and none in the changed file. Full-app rustfmt reports four unchanged upstream files; both the baseline target and changed file pass individually. See the formatting comparison and full log.
- Fresh Unicode verification covers 374,742 offsets in both directions and the official Unicode 17 expectations. The exact helper bodies and source hashes are included.

The first baseline audit probe used the optimized layout's private field path and failed to compile. Its log is retained. The corrected baseline probe accesses the original layout structure; all reported measurements and test results use successful builds. The baseline cache diagnostic intentionally fails to demonstrate replacement during right-arrow, Shift-right and scroll; the candidate passes all three.

## Reproduction

Use a disposable Cap checkout at the recorded base or code commit, with Rust 1.95.0 and the repository's normal native setup, including `scripts/prepare-gpui-dependency.sh` and `node scripts/setup.js`. `current/current-baseline.rs` and `current/final-candidate.rs` are exact pure whole-file snapshots. `current/build-phase.sh` shows the source hooks and actual compiler flags. `source.json`, `profile.json` and build JSON/logs record provenance for each binary. Adjust the recorded workspace prefix in scripts and the native example's paths for your checkout. Audit fixtures are under `current/fixtures`; they are excluded from the PR code diff.

Build the baseline and candidate test binaries with app opt-level 3 and debug info disabled, leaving debug assertions and overflow checks enabled. Execute the regular tests and cache diagnostic with the native runtime paths from `test-env.sh`. Use the normal app opt-level 0 for the uninstrumented committed suite. Build the native example separately with `gpui_platform/test-support`; run baseline and candidate captures at the same display scale.

Run `current/run-viewport-pairs.py --repo <checkout> --baseline <baseline-test-binary> --candidate <candidate-test-binary> --pairs 3 --matrix core --output <results> --build-profile <candidate-profile.json>`. It runs 66 cases with seven samples per case and 120 operations per sample. Process order alternates. `current/analyze-current.py` independently recomputes every summary from all 2,772 samples and checks the test, diagnostic, geometry, PNG and profile comparisons.

Timing uses GPUI TestPlatform/NoopTextSystem on a shared Apple M1 Pro host. It measures elapsed time for synchronous warm CPU handlers, binding dispatch/scene preparation and scroll redraw. Native font shaping, OS input latency, GPU presentation, FPS, release-profile performance and export throughput are unmeasured. Native captures validate correctness separately. No full signed-in acceptance, system IME event entry or native Windows/Linux execution is claimed. Cold shaping and cache misses can still scale with content length.

## Historical evidence

The two ZIPs under `historical/` preserve the independent original-archive audit and the subsequent minimization audit at the earlier fixed base. Each has its own per-file checksum manifest and report. Their historical “no push or PR” statements describe the state when those snapshots were produced. In the minimization performance files, “baseline” means the earlier optimized candidate, not upstream. They include three/five-pair comparisons and a five-pair identical-binary control. The current-main comparison above is the source of the new PR's headline numbers.

`MANIFEST.json` records every published evidence file's size and SHA-256, excluding itself. Source is accompanied by the Cap license; third-party Unicode test licenses are retained in the historical audit ZIP.
