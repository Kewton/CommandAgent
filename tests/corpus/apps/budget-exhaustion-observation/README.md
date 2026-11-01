# budget-exhaustion-observation

Synthetic corpus for Issue #959. It is not a bench measurement and contains
no task text, expected answers, or generated code from any bench or smoke run.

## Origin

Every row in `fixtures/events.jsonl` is a scripted run written for this case:
a fake `ChatClient` returns fixed tool calls (`Write`/`Edit` of `notes.txt`,
`Bash` with `true`, `false`, `test -f notes.txt`, `exit 127`, a refused
path, `Read`, `Glob`) in a temporary workspace. No model, key, or provider is
used. One row (`contract-allowance-budget`) carries a synthetic completion
contract with one required path, so its limit includes the existing contract
allowance (`--max-iterations` 1 + 3 + 1 + 3 = 8).

No row depends on how fast the machine is. The step wall-clock row uses a cap
of 0, so the cap has expired before the first provider call: no request is
sent and no time sink is recorded. The other rows keep the default cap and
never sleep.

No row depends on the platform either. The workspace root is the
canonicalized temporary directory: `Read` prints a path relative to the root
only when the resolved file path starts with the root
(`src/tools/path_guard.rs` `relative_display`), and macOS keeps its temporary
directory behind a symlink, so a non-canonical root would make the `Read`
result (and the request digest) differ between macOS and Linux.

Each row's `expect` was recorded from the base commit
`20b549fc6269217bd0161d2038326480c7605d5f`, before the observation existed:
the error text, the stop reason, the request digests, the event-name
sequence, and the exact `loop_stop` line (`base_loop_stop`). Wall-clock
durations and generated recovery-file ids are normalized. Only
`budget_fields` comes from the Issue's rules, worked out by hand for each
script.

Rows without `entry` run through the public entry
(`tests/budget_exhaustion_observation.rs`). Rows with `"entry": "plan_step"`
need crate-internal plan-step options and run in the leaf tests
(`src/minimal_loop/loop_run/budget_exhaustion_observation/tests.rs`).

## Field contract

Only the `loop_stop` emitted after the iteration budget is spent carries the
nine fields, whatever its reason (`artifact_follow_through_exhausted`, a
capability reason, `model_stagnation:read_only_loop`,
`model_stagnation:no_progress_recorded`, `loop_progress_exhausted`, or the
scaffold reason). Other `loop_stop` sites keep their base bytes.

| Field | Value |
|---|---|
| `iteration_limit` | the run's iteration limit (`--max-iterations` + 3, plus the existing contract allowance) |
| `iterations_used` | iterations started; equal to the limit on this path |
| `iteration_budget_exhausted` | `true` |
| `successful_write_count` | Write/Edit tool calls that executed successfully |
| `last_successful_write_iteration` | 1-based iteration of the last one, or `null` |
| `bash_results_after_last_write` | Bash results that reached the execution-progress observation after it |
| `bash_failures_after_last_write` | how many of those were not successful |
| `last_bash_after_last_write_succeeded` | the last of those results, or `null` |
| `write_failure_after_last_write` | a Write/Edit recoverable error or rejection after it |

A successful Write/Edit resets the three Bash fields and the failure flag, in
tool order within one reply. A run without a write counts from its start. A
repeated successful command counts as a success. Bash refused before
execution or replaced by a policy substitute is not counted. Values are
numbers, bools, or null; no path, command, tool output, or model text is
copied. The reason, its selection order, and the existing fields are
unchanged.

## Limits of meaning

These fields describe the run only when its budget ran out. They are not a
ledger of unresolved obligations, not a record of which checks ran, and not
execution evidence. A successful Bash result means only that the command
exited successfully, not that it verified anything. Host-side writes and
files changed through Bash are not Write/Edit calls and are not counted. A run
that ends with a normal final answer never carries these fields.
