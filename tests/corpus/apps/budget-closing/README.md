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
and elapsed milliseconds normalized), the event-name sequence, one digest of
every event payload in order (`event_digest`), and the number of unused
replies. The digest compares every key and value as written except: the
numbers of exactly five keys (`occurred_at_epoch_ms`, `duration_ms`,
`elapsed_ms`, the provider-turn `estimated_prompt_tokens_sent`, which counts
the system prompt that names the temporary root and so changes with its
length, and `estimated_stable_prefix_chars`, which is measured against the
previous request in a process-wide cache and so depends on the tests that
ran before), and, inside strings, the temporary root, generated ids (UUIDs),
`elapsed_ms: N`, and `total_duration_ms=N`. The test separately requires
every provider turn to report a positive prompt size estimate. It is
therefore a comparison of the event payloads apart from those five numbers
and those string normalizations, not a proof that the raw payload bytes are
unchanged. The five rows added in the review round
(`unchanged-write-after-ambiguous-edit`, `hidden-path-bash-rejected`,
`cancel-before-notice-request`, `post-write-read-after-failed-write`,
`post-write-read-accepted`) and the `event_digest` of every row were recorded
the same way, by checking out `src/` at that commit and running the same
scripts, then restoring `src/`. `tests/budget_closing.rs` runs every row with the
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
| `notice` | the request sent with three calls left, counting itself, carried the notice and was handed to the client (also when the call was interrupted afterwards); 0 or 1 per run, never when the limit is 3 or less | `remaining_calls_reached` |
| `final_deferred` | after the notice, a normal final passed every existing check, or the existing post-write read confirmation was about to end the run, while an unresolved failure remained; 0 or more | `unresolved_failure` |
| `final_accepted` | after the notice, the same with no unresolved failure; at most once, and the run ends with it | `no_unresolved_failure` |
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
  contains `new_string` and no longer contains `old_string`. When
  `new_string` contains `old_string` no Write resolves it; rewriting the same
  bytes after an ambiguous edit does not (`unchanged-write-after-ambiguous-edit`).
  Another anchor in the same file (`other-anchor-edit`) or the success of one
  of two failed edits (`two-failed-edits-one-resolved`) leaves the other
  unresolved. Paths are compared after the tools' normalization, rebuilt
  from their components, so `a//b` and `a/./b` are `a/b`; `..` is not
  resolved.
- A Bash call the tool layer refused before running it (here an
  engine-private path) is counted in `policy_rejected_bash` and is not a
  failure (`hidden-path-bash-rejected`); a Bash that ran and failed is.
- The existing post-write read confirmation does not end the run after the
  notice while a failure remains (`post-write-read-after-failed-write`);
  without one it completes as before (`post-write-read-accepted`).
- A cancel before the notice request reaches the client records no notice
  (`cancel-before-notice-request`); the same script interrupted after the
  client received it records the notice and no terminal phase. The notice
  request and the run share one dispatch state that moves once: the
  provider call's worker dispatches it or the returned call closes it, so a
  worker that reaches the client after the call already returned
  interrupted does not send that request (a leaf test holds the worker at
  that point). That is the only cancel behavior that changes, and only for
  the notice request of an enabled run (the test
  derives that variant from this row).
- A recoverable Bash error is a refusal only when it is a typed
  engine-private or sensitive path refusal or starts with the host's own
  refusal words; a timeout whose command or output contains those words is
  an executed failure (leaf tests; the tool's 180-second timeout cannot be
  reached from a scripted run).
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
