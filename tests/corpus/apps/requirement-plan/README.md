# Issue #945 requirement plan fixtures

`fixtures/plan.json` holds synthetic scripted runs for the experimental
`COMMANDAGENT_REQUIREMENT_PLAN` opt-in. Every row is synthetic: the request
text, the files, the commands, the notes, and the model replies were written
for this corpus. No benchmark task text, grader, expected value, or generated
code is included.

`tests/requirement_plan.rs` reads each row, replays the script through the
public minimal loop with a mock `ChatClient`, and compares the keys the row's
`expect` object sets.

- `default-off-*` rows (unset, empty, `off`) pin the behavior of source
  `c9f88d35`, recorded before the requirement plan existed: the provider
  request digests (serialized messages plus the native-tool flag, with the
  workspace path and `elapsed_ms` normalized), the event-name sequence, the
  provider call count, and the outcome. No `requirement_plan` event appears.
