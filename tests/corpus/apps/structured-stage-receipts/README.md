# structured-stage-receipts

Synthetic corpus for Issue #975 (G4): stages declared over pinned `propose-artifacts` tools, where
only the host's record of an accepted submission completes a stage. This is not a model-capability
or bench measurement, and it holds no task text, expected answer, generated code, or run record from
any bench or smoke run. No live model, real key, external provider, or Jev call is used. Every secret
is a dummy.

## Origin

Base commit: `d2360125210564351f30e42bbc1a9f5e51f2b3cc`.

The two profiles are generic synthetics: one proposes a settings object and one a document index.
They go through the same state machine, with the real Chat Completions client against a disposable
fake on 127.0.0.1 and a fixed script as the pinned process.

## Files

- `fixtures/cases.json`: the profiles; the events and refusals a stage run can end in; the public
  entries; the acceptance rows with the tests that stand for each; the off rows.
  `tests/structured_stage_receipts.rs` asserts each named test exists and that the off rows are the
  ones it runs.
- `fixtures/off-golden.json`: the off control, recorded on the base commit's production code, before
  this feature existed. Each row holds the outcome, every request body, every event line, the
  workspace artifacts, and the count of unused replies.

## How the off recording was made

The worktree stays where it is: the base is a copy.

1. `git archive -o <scratch>/base.tar d2360125210564351f30e42bbc1a9f5e51f2b3cc`, then `tar -xf` it
   into an empty directory.
2. Copy `tests/structured_stage_receipts.rs` there as `tests/stage_off_recorder.rs`, and this corpus
   directory with it, because the file includes them.
3. In that copy only: in `off_control`, replace the list of sources `the_corpus_names_what_runs`
   reads with an empty one, because the base has no such files. Add a test that takes
   `register_off_extension()`, maps `OFF_ROWS` through `record_off_row`, and writes
   `{"issue", "kind", "source": "d2360125", "rows"}` as pretty JSON with a final newline.
4. Run that one test with `cargo test --manifest-path <copy>/Cargo.toml --test stage_off_recorder`,
   through the heavy lock. Copy the result here, and update the digest in `source-manifest.json`.
5. Remove the copy.

The expected values therefore do not come from the new implementation: the code that records is the
code that compares, and only the production code under it differs.

## What the off comparison normalizes

Nothing is left out. Inside the recording, exactly these are replaced by a placeholder:

- The numbers of six keys: `epoch`, `occurred_at_epoch_ms`, `duration_ms`, `elapsed_ms`,
  `estimated_prompt_tokens_sent`, and `estimated_stable_prefix_chars`.
- Inside strings, and in the names of artifacts: the two temporary roots, the fake's loopback URL,
  generated UUIDs, `elapsed_ms: N`, `total_duration_ms=N`, and `"epoch":N` in an evidence file.

Two ultra rows differ in one thing:

- `ultra-plan-two-phases` runs with `profile_explicit = false`, which plans in line and gives one
  order. Nothing is left out and no order is normalized.
- `ultra-plan-speculating` runs as an explicit profile does: the next phase's plan is asked for on
  another thread. Its client writes `provider_request` and `provider_response`, which can land
  anywhere among the main thread's events. Those two, and only between
  `speculative_phase_plan_started` and `speculative_phase_plan_adopted`, may move among the others;
  they keep their order between themselves, because one thread writes them. Everything else is
  compared in order, byte for byte. The test
  `only_the_other_threads_events_may_move_and_only_within_their_range` holds that comparison to this.
  The requests go in two lists, by whether a request offers a tool, each in its order. A plan's request
  from the main thread goes in the plans' list too. This is for the replies: a single queue of replies
  gave a plan's request a reply meant for a step (`done`, then a `planner_error`). The order across the
  two lists is not compared.

Two threads can also leave two values on one line of the events file, so the recorder reads values
and not lines.

The row `investigate-end` ends at `investigation reproducer was not identified`, before that intent's
end, on the base and now alike. The row `fix-end` reaches its end.

`dummy-secret-next-to-reserved-words` is there for the four words this feature made fixed identifiers.

The comparison also asserts that no row leaves `state/structured-stages` in its state directory.

## Calls the schema refuses (Issue #998)

A call of a stage tool that G2 refuses spends no G4 attempt and opens no record. Each counts against
its target and against the run, apart from G4's attempts, with the same `attempts_per_target` and
`attempts_per_run` the descriptor sets. The refusal that reaches either is the last: the run ends with
`stage_run_ended: schema_refusal_limit`, nothing is sent after it, and one `stage_schema_refusal_limit`
event carries `stage`, `target`, `refusals`, `limit`, and `scope` (`target` or `run`). With
`attempts_per_target = 1`, the first refusal ends the run. A tool in no stage is not counted, and its
refusals go back to the model as data, as before.

The fixtures send refusals that differ from one call to the next, as a model's do. The same refusal
again and again is ended at the third by the loop's repeat guard, before these limits; one test keeps
that as the control. Under a run's wallet (Issue #976) each refusal is also a submission of its stage,
and whichever limit is reached first ends the run with its own reason.

The counts are the process's, kept as G4's record is: one set for each workspace and profile. They
go on over the sessions of the process, its phases and final repairs among them, and nothing starts
them over, a success or a step forward included. A `scope` of `run` is that sum: every target's
refusals in this workspace and profile in the process. Once a limit is reached, a later session of
the process ends at its first refusal. The counts are in memory only: a new process, a resume among
them, starts them at zero, and G4's ledger has no field for them: they are in the event and the run's
end, not in the ledger.

These rows are in `fixtures/cases.json` under `events`, `refusals`, and acceptance row 4.
`off_control::the_corpus_names_the_schema_refusal_limit_as_the_code_writes_it` holds the two rows to
the code. The off control is unchanged: a run with no stage never reaches this count.

## Which test runs where

Nothing here is skipped, except `the_notice_on_a_real_terminal_with_the_footer_and_the_spinner`, which
needs a terminal and someone to read it.

| Test | macOS | Linux and others |
| --- | --- | --- |
| `off_control::*` | Yes | Yes |
| Unit: `tools::profile_tools::effects::tests::stages::*`, with its `sessions` running whole sessions | Yes | Yes, with an unconfined launch only a test build can ask for |
| Unit: `planner::structured_stages::*`, `structured_stage_completion::tests::*`, `tools::profile_tools::pre_run::tests::*` | Yes | Yes |
| `where_the_strict_isolation_does_not_exist_a_stage_tool_is_unavailable` | No | Yes |
| `with_the_strict_isolation::*`, each with the real isolation | Yes | No |

## Known limits

See `docs/dev/structured-stages.md`. A stage's success end to end is exercised only locally on macOS,
so a green CI is no evidence of one. The mutation rows are measured by the leader, not here.
