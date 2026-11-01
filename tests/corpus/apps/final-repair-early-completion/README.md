# final-repair-early-completion

Issue #956. A final-acceptance repair of an existing invalid artifact must not
end after a read-only Bash command, a passing inspection, or the mere presence
of its paths. A failed `artifact_contract_check` receipt binds its request and
inputs to the repair session that follows; that session holds a completion
candidate (after a tool batch, a final answer without tools, or a contract
short-circuit) until a bound artifact's raw bytes change or a Write/Edit hands
the decision to the repair loop's raw-byte snapshot. The same check then
reruns inside the existing bounded repair.

## Provenance

Synthetic. `tests/final_repair_early_completion.rs` builds every case at run
time from generic draft profiles, a POSIX shell adapter run by the
operator-pinned `/bin/sh`, and scripted fake planner and executor clients, all
in temporary directories. Each `base_sequence` and `base_executor_calls` was
observed by running that file against the unchanged sources of develop
3ec27982. No smoke task, grader input, expected value, model output, generated
code, host path, or credential is stored here.

## Event contract

- No event name, schema version, field type, or stop reason changes. A held
  completion is the existing `step_completion_blocked` event with the new
  reason `final_repair_artifact_unchanged` (reserved in
  `src/sensitive_data/fixed_identifiers.rs`) and the additive object
  `repair_binding` of target counts (`targets`, `unchanged`, `absent`,
  `unknown`). It carries no file content.
- The order a case proves is: repair start, the read or inspection
  (`tool:Bash:ok`), the held completion (`blocked:...`), the later Write, the
  same check (`receipt:...`), and the result.

## What this is not

These cases measure a control path with fake clients. They are not a
measurement of a model's ability, of a smoke success rate, or of any CA bench,
held-out, or campaign result, and they open none of those.
