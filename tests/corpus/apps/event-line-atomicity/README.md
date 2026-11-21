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

## How it is checked

`eval_events::append_writer::tests::corpus_line_cases_get_their_recorded_verdicts`
reads `fixtures/line-cases.json`, checks its sha256 against
`source-manifest.json`, applies the strict check to each case's bytes, and
compares the verdict and event ids. The concurrent orderings themselves
(a writer stopped inside a record, between a provider turn's event line and
its tail, and inside a large tail) are made by the stopped-writer tests in the
same module, not recorded here. The refusal after a failed write leaves a
line open is checked by `a_line_left_open_refuses_every_later_append_to_that_file`
and `a_tail_reopened_onto_a_regular_file_meets_the_mark`; the readers' view of
a line cut inside a character, by
`a_line_cut_inside_a_character_hides_earlier_lines_from_existing_readers`.

## Regenerating

The fixture is hand-authored. After editing it, regenerate the digest with
`shasum -a 256 fixtures/line-cases.json` and update `source-manifest.json`.
Historical evidence and recordings are not rewritten.
