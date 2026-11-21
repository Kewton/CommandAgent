# event-line-atomicity (Issue #987)

Synthetic byte shapes for the events file line contract: one event is one
JSON object followed by one LF, and no other writer's bytes land inside it.

## What the cases pin

- Valid files: small lines, a line with an escaped newline, quotes, a tab,
  emoji, and a combining character, and a large line. The product writer
  (`src/eval_events/append_writer.rs`) rewrites each valid case byte for byte.
- Invalid files a strict line reader refuses: a record split by another
  record (the shape the base writer could leave when two threads appended to
  one file), two objects on one line, a last line without LF, an empty line,
  and a line that is not an object.
- Reader line cases (Issue #1059) in `fixtures/reader-line-cases.json`: a whole
  line, a broken line, and a whole line, in that order, with the broken line
  holding a short key/value pair, a multi-byte value, a marker-like
  `＜masked＞`, and an identifier-like `EVENT_CANARY_31`. The broken line is
  either cut inside a multi-byte character or malformed JSON. One case puts the
  only success and terminal values inside the broken line. The bytes are pinned
  as hex so the cut is byte-exact.

## How it is checked

`eval_events::append_writer::tests::corpus_line_cases_get_their_recorded_verdicts`
reads `fixtures/line-cases.json`, checks its sha256 against
`source-manifest.json`, applies the strict check to each case's bytes, and
compares the verdict and event ids. The concurrent orderings themselves
(a writer stopped inside a record, between a provider turn's event line and
its tail, and inside a large tail) are made by the stopped-writer tests in the
same module, not recorded here. The refusal after a failed write leaves a
line open is checked by
`a_line_left_open_refuses_every_later_append_to_that_file`
and `a_tail_reopened_onto_a_regular_file_meets_the_mark`.

`eval_events::append_writer::tests::broken_lines_do_not_hide_or_forge_neighbouring_events`
reads `fixtures/reader-line-cases.json` and drives the four reader entrances
over each case's bytes:

- the strict `runs <id> --events` path fails, naming the file and the broken
  line's number, and its message never repeats the broken line's content;
- the terminal report and the completion snapshot skip only the broken line and
  keep the valid neighbours, in order and by value, with the profile,
  prompt layout, and acceptance status of the valid lines;
- the lenient run inventory reads the neighbours' status and stop reason,
  applies the existing display masking, and never shows a value that only the
  broken line held;
- the case whose only success and terminal values sit inside the broken line
  produces no success evidence: no terminal status, no acceptance.

`a_line_cut_inside_a_character_keeps_the_earlier_lines_visible` is the former
V3-B1 constraint, rewritten to the intended "the earlier line X stays visible"
and checked at the same four entrances.

## Regenerating

The fixtures are hand-authored. After editing `fixtures/line-cases.json`,
regenerate the digest with `shasum -a 256 fixtures/line-cases.json` and update
`source-manifest.json`. `fixtures/reader-line-cases.json` pins its bytes in the
file itself and has no digest entry. Historical evidence and recordings are not
rewritten.
