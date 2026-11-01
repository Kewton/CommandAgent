# budget-closing

Synthetic corpus for Issue #957, the opt-in `COMMANDAGENT_BUDGET_CLOSING=once`.
It is not a bench measurement and contains no task text, expected answers,
generated code, or run records from any bench or smoke run.

## Origin

Every row in `fixtures/events.jsonl` is a scripted run written for this case:
a fake `ChatClient` returns fixed replies (`Write`/`Edit` of `notes.txt`,
`.env`, or `app.js`, `Bash` with `true`, `test -f notes.txt`, `test -d .`,
`grep -q vN notes.txt`, or a refused path, `Read`, empty replies, and short
final texts) in a temporary workspace. No model, key, or provider is used.
Two prompts are used: an edit prompt and a question prompt that does not
ask for a write.

Each row's `base` was recorded from the base commit
`c141d74028d2384d65668dabe278361596c9d67e`, before the closing existed: the
outcome (stop reason and final text, or the error text), the request digests
(SHA-256 of the serialized request messages, first 8 bytes, workspace root
and elapsed milliseconds normalized), the event-name sequence, and the
number of unused replies. `tests/budget_closing.rs` runs every row with the
variable unset, empty, and `off` and requires exactly that `base`. The `once`
expectations are written in the test from the Issue's rules, not from the
new implementation's output.

The workspace root is the canonicalized temporary directory and no row
depends on wall-clock time, so macOS and Linux produce the same bytes.

Plan steps, the final acceptance repair, and a phase-scoped run need
crate-internal options; the leaf tests in
`src/minimal_loop/budget_closing/tests.rs` run them and require the same
requests, events, and outcome as with the variable unset.

## Event contract

`budget_closing` is emitted only for a generic direct run (MinimalLoop, no
step kind, no phase scope, no recovery obligation) without a completion
contract, and only with `once`.

| Phase | When | `guard_reason` |
|---|---|---|
| `notice` | the request sent with three calls left, counting itself, carried the notice; 0 or 1 per run, never when the limit is 3 or less | `remaining_calls_reached` |
| `final_deferred` | after the notice, a normal final passed every existing check while an unresolved failure remained; 0 or more | `unresolved_failure` |
| `final_accepted` | after the notice, a normal final passed every existing check with no unresolved failure; at most once, and the run ends with it | `no_unresolved_failure` |
| `exhausted_after_notice` | after the notice, the run reached the existing iteration-budget exit; at most once | `iteration_budget_exhausted` |

Other exits (provider errors, timeouts, max length, interrupts, other stops)
emit neither `final_accepted` nor `exhausted_after_notice`.

Every event carries the same numeric fields: `prompt_version` (1),
`iteration`, `iteration_limit`, `remaining_calls`,
`unresolved_bash_failures`, `unresolved_write_failures`,
`resolved_failures`, `policy_rejected_bash`, and `final_deferrals`. No
command, path, content, model text, tool output, or secret is copied.

## Ledger rules the rows exercise

- A Bash call that ran and was not successful is resolved only by a later
  success (new or repeated) of the same command string
  (`accept-after-same-check`); an unrelated success (`unrelated-bash-success`),
  a write (`write-without-rerun`), a read or a self-report
  (`read-and-self-report`) does not resolve it.
- A failed Write is resolved only by a successful Write of the same path and
  content; a write to another path does not (`other-path-write`).
- A failed Edit is resolved only by a successful Edit with the same path,
  `old_string`, `new_string`, and `replace_all`
  (`matching-edit-resolves`), or by a Write to the same path whose content
  shows the edit's effect. Another anchor in the same file
  (`other-anchor-edit`) or the success of one of two failed edits
  (`two-failed-edits-one-resolved`) leaves the other unresolved.
- Failures before the notice count (`failure-before-notice`), and a run
  without writes is guarded too (`no-write-bash-failure`).
- A Bash call refused by policy is not a failure; only its count is kept
  (`policy-rejected-bash`).
- Consecutive finals are all deferred (`consecutive-finals`); the run then
  ends through the existing budget exit with the reason its existing order
  chooses.

## Limits of meaning

The notice and the guard add no call, budget, time, or retry. A
`final_accepted` event means only that the ledger had no unresolved entry;
it is not verification, acceptance, or execution evidence, and a successful
Bash result means only that the command exited successfully.
