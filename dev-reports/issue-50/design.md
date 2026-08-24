# Issue #50 Design

## Context

The TUI currently formats footer totals as `1m01s` but renders spinner and live
footer durations as raw seconds. Unicode presentation marks are selected only
for the spinner; banner art, activity breadcrumbs, and live-footer separators
remain Unicode in a non-UTF-8 locale. Active footer rows are also uniformly
dimmed, which can make the current phase and operation hard to read.

The predecessor review found that Issue #47 already contains the committed
Issue #43, #46, and #49 presentation work that this issue must preserve. Issue
#51 is a documentation-only REPL continuation change and does not overlap the
planned implementation surface. This branch will therefore build from the
Issue #47 head and leave Issue #51's independent commit for normal integration.

## Design

1. Add a small TUI elapsed-time formatter and use it from both spinner rendering
   and all live-footer duration segments. Keep the existing compact contract:
   seconds below one minute, then `XmYYs`.
2. Add a presentation-glyph helper that converts only CommandAgent's decorative
   Unicode marks to stable ASCII equivalents. Locale detection remains in the
   terminal layer, gains an injectable helper for tests, and continues to honor
   `LC_ALL` before `LANG`.
3. Apply glyph conversion only at display boundaries: startup banner output,
   emitted presentation markdown/breadcrumbs, scripted UX-demo output, and live
   footer rendering. Event values and `events.jsonl` are not changed.
4. Preserve the public pure presentation snapshots in UTF-8 and add focused
   ASCII-locale snapshots. The UTF-8 banner art remains unchanged; non-UTF-8 art
   uses an ASCII box.
5. Keep the settings-only footer row dim. For active status, render the primary
   row without dim styling; on a narrow two-row footer, dim only the secondary
   detail row. Color-disabled output remains escape-free.

## Verification

Run focused TUI module tests first, then the scripted demo and locale-facing
integration checks. Because shared Rust presentation behavior is affected, run
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test` before handoff. Do not regenerate recorded SVG/GIF assets here;
record that capture work remains delegated to Issue #43 item D.

If the global Markdown test capture observes output from concurrent tests,
scope capture ownership to its creating thread. This preserves the captured
rendering contract while preventing unrelated presentation output from making
verification nondeterministic.
