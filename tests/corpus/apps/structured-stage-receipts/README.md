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

The row `ultra-plan-two-phases` runs with `profile_explicit = false`. An explicit profile plans its
next phase on another thread, whose provider events can land anywhere among the main thread's, so
that order was not a property of the code. This setting plans in line and gives one order; no event
is left out and no order is normalized. Against a recording with speculation, the difference is the
three `speculative_phase_*` events and where one `provider_request` falls.

The row `investigate-end` ends at `investigation reproducer was not identified`, before that intent's
end, on the base and now alike. The row `fix-end` reaches its end.

`dummy-secret-next-to-reserved-words` is there for the four words this feature made fixed identifiers.

The comparison also asserts that no row leaves `state/structured-stages` in its state directory.

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
