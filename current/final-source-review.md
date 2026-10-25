# Final source review

Reviewed the complete minimized diff and the shared TextInput callers before publication. No unresolved correctness issue was found in this pass.

## Root cause and behavior

The old previous/next helpers restart grapheme iteration from a string endpoint. Flex width probes overwrite the same layout used for interaction. WrappedLine painting enters paragraph glyph loops even when the paragraph is fully outside the content mask. The patch addresses these three paths within the existing component.

Navigation starts GraphemeCursor at the caret while supplying the full string. Rounding interior byte offsets up for previous and down for next preserves the old iterator semantics, including offsets beyond the end. Both helper bodies exactly match the freshly executed Unicode verification sources.

Measurement caches sizes separately from the active interaction rows. The retained shaped layout is keyed by text, font size, line height, complete inherited font, text color and marked range, with its active wrap width checked separately. At most four alternate width/size pairs are retained. Deferred measurement captures the inherited font while the parent text style is in scope. Existing text edits still invalidate the shared layout. Selection and caret movement can reuse it.

Paragraph culling uses GPUI's content mask and font bounding-box height. For paragraphs above the viewport, the fallback reproduces repeated f32 wrapped-row advancement instead of assuming multiplication gives the same coordinate. Marked composition bypasses this optimization. Selection and caret painting are unchanged. The culling predicate mirrors GPUI's own glyph bounds test and may conservatively paint extra content.

## Affected callers and contracts

Traced teleprompter multiline editing and scrolling, settings and search, renaming, editor fields, screenshot hex fields and annotation text. Public TextInput APIs, focus behavior, text storage, events, UTF-16 conversion, undo history, recording and export formats are unchanged. Current main's added is_focused accessor is preserved.

Checked cache hits, intrinsic and alternate-width probes, resize/text/style/composition invalidation, empty lines, single-line horizontal scrolling, multiline and fit-content measurements, selection geometry and hit testing. Runtime results are recorded separately in the test logs and comparison JSON files.

## Tests and limits

The repository patch retains all 25 existing component tests and adds five focused regressions. Seven original exhaustive audit regressions and two independent checks are executed separately and do not enlarge the PR diff. Native capture tests use the production component with a fixed test theme and the real platform text/graphics system.

The timing benchmark uses TestPlatform/NoopTextSystem and measures warm CPU work. It does not establish native input latency, native font-shaping speed, GPU presentation or FPS. Full signed-in app acceptance, system IME event entry and native Windows/Linux execution are not covered locally. Cold shaping and cache misses still scale with content; Unicode context may require scanning. The new cache is bounded, but this is not full text virtualization.
