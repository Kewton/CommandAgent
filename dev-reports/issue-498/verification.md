# Issue #498 verification

- Status: `passed`

## Checks

- `cargo test --lib recovery_step_plan_binding`: `passed`
- `cargo test --test corpus_regression --test generality_guardrails`: `passed`
- `COMMANDAGENT_ISSUE498_REPLAY_ROOT=/Volumes/SSD_NX/tmp/commandagent-issue498-worker-replay-20260919 cargo test --lib issue498_saved_capacity_failure_stops_without_another_request_or_fallback -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

The focused binding suite passes 91 tests, including nine Issue #498 tests and
the retained #496, package split, Recovery and formation regressions. Corpus
regression passes seven tests; all ten generality guardrails pass. The full
`cargo test` exits 0: 2676 library tests pass with 20 existing ignored tests,
followed by successful binary, integration and documentation test suites.
Existing ignored tests retain their prior status. Full verification ran outside
the sandbox because existing tests bind local HTTP listeners.

## Saved-fixture evidence

The model/host hashes and 529/2237 scalar counts are checked against the committed
fixture. An independent exhaustive overlap check confirms no containment or
overlap, hence a 2766-scalar minimum (266 over the unchanged limit). Admission
retains the complete 2786-character augmented owner and stops on attempt 1.
The Replay client receives only one proposal and makes exactly one request.
Application commands, verifiers, setup fallback and last-valid fallback do not
run. Preexisting disposable app bytes and absent paths remain unchanged.

Old-field event deserialization and new terminal control are both asserted.
The event reports `stopped`, two remaining attempts and unchanged recovery
budget. Frozen source hashes, original scope, preservation view, registered
contract/package evidence and marker state accompany the proof. The full final
error chain contains neither exhausted-budget nor ProposalRepairable wording.
No downstream ordering conclusion is inferred.

The isolated replay, evidence hashes and exact new SSD directory are recorded
in `workspace/tmp/issue-498-worker-replay-20260919/report.md`. It is a
parsed-and-later reconstruction, not a saved raw-response or live-provider run.

## Development findings

An initial source comparison included the goal, which existing canonicalization
shortens. The proof now compares complete frozen owner duties and retains both
goal views as evidence; it still requires exact preservation-view reconstruction
and immutable captured source hashes. Negative controls preserve strict failure
for missing/ambiguous provenance and inconsistent acquisition views.

The saved-fixture retry-dependent loss tests were moved to direct preservation
checks. Schema/empty/mixed/fallback controls now use a repairable overlapping
fixture and assert the actual downstream boundary reached. Early development
assertion failures were corrected before the successful focused, corpus, clippy
and full runs above; no required check remains failed or unrun.
