# Issue #11 Design

## Scope

Extend only the assistant-output markdown renderer. Preserve the existing
`strip_think`, sanitization, line-buffer limit, `NO_COLOR`, and
`ANVIL_NO_MARKDOWN` behavior, and do not introduce a new dependency.

## Design

- Keep streaming and safety gates in `src/tui/markdown.rs`, with small leaf
  modules under `src/tui/markdown/` for table parsing/layout and fenced-code
  highlighting.
- Delay only a pipe-delimited candidate header until the following line
  establishes whether it starts a table. Once a valid delimiter row is found,
  buffer table rows only until the table ends so column widths can be calculated
  across the complete table. Cap this extra buffer at the existing 64 KiB
  threshold; malformed, inconsistent, or oversized candidates are emitted as
  literal markdown lines.
- Parse delimiter-cell colons into left, center, and right alignment. Calculate
  padding from terminal display width, including CJK/full-width ranges, while
  ignoring renderer-generated SGR sequences. Add padding outside styled cell
  content so terminal wrapping cannot split an escape sequence.
- Track list indentation across adjacent list items. Treat either two or four
  added spaces as one nesting step, normalize rendered indentation to two
  spaces per level, rotate unordered markers by depth, and preserve ordered
  list numbers.
- Extend inline parsing with the safe baseline link representation
  `text (url)`. Sanitize link text and destination before adding any renderer
  formatting; do not emit OSC hyperlinks.
- Record a recognized fenced-code language on the opening fence. A lightweight
  scanner distinguishes keywords, quoted strings, and comments for
  JavaScript/TypeScript/TSX, Python, Rust, Bash, and JSON. Unknown or absent
  language tags keep the existing single-color rendering. All source is
  sanitized before the scanner inserts SGR sequences.

## Verification

Add focused unit tests for CJK table width, all alignments, malformed and wide
tables, two- and four-space nested unordered lists, ordered lists, links,
recognized and unknown code fences, injection resistance, streaming chunks,
and the existing renderer gates. Run the focused markdown tests first, followed
by formatting, clippy, and the full Rust test suite because shared TUI output
behavior changes.
