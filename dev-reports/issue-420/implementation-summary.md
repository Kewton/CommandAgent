# Issue #420 reopened implementation

Implement steps now reject an engine-owned page in their own explicit expected
paths, even when earlier backend routes satisfy the run-wide contract. The
check applies to iteration short-circuit, assistant final responses, and
post-tool completion under both enforce and observe modes. Reading the scaffold,
writing it unchanged, or writing an unrelated file cannot discharge the UI step.
The bounded loop continues with targeted feedback and fails honestly if the
placeholder remains.

The new `implementation_completion` leaf captures expected-path content hashes
before run-contract paths are merged. It preserves actual changes made through
Bash and existing tool-path tracking. The extracted `short_circuit` leaf retains
contract verification and existing setup/inspect/verify eligibility. Iteration
outcomes now retain changed paths, iteration count, and tool count instead of
resetting them to zero.

Existing non-placeholder artifacts can still succeed without a write after
contract verification; runner step verification remains in force. Required
Recovery/synthetic-precheck mutation options retain their existing policy.
The runner carries the real step ID in session options. The model probe sets
that optional ID to `None` because it has no planner step identity.

`step_short_circuited` retains its existing name and fields and adds `step_id`,
`write_or_edit_seen`, `scaffold_placeholder_paths`, `expected_path_hashes`,
`changed_paths`, `completion_requirements`, and `satisfaction_basis` in the
minimal-loop emitter. The hashes identify the step's explicit expected paths;
`verified_existing_artifacts` distinguishes verified no-op success from source
changes. Setup without a contract reports `setup_artifacts_present`. A new
`step_completion_blocked` event records `unsatisfied_scaffold_placeholder` and
the unresolved paths. Runner start/pre-satisfied event shapes are unchanged.

## Tests and fixture

Ten new tests cover all 19 historical events, API-before-UI completion, unrelated
Write and textual-final bypass attempts, no-contract behavior, read-only
exhaustion, verified existing satisfaction, failing verification, Bash mutation,
Recovery/synthetic options, and planner terminal-event correlation. Existing
#422 obligation classification and non-implement short-circuit regressions remain
in the required suite.

`tests/corpus/apps/issue420-step-completion` records all nine source event hashes
and exact line provenance. Historical outcomes stay distinct: 8 skipped,
2 failed, 3 completed with changes, 6 completed without changes; 11 shortcut
objects lack step IDs. The iteration replay rejects all 11 scaffold probes.
The fixture intentionally preserves the #422-only run-wide contract pass with
an API implementation, proving that #422 alone cannot close the reopened scope.
S3/E3's identical page and S3's package are copied unchanged; the small backend
route is a documented minimized stand-in.

## Integration and limits

Predecessor #435 commit `0384999cb3a74c2603f035cadb3d0b0faddae9bc` and its
passing report were inspected and incorporated by fast-forward before edits.
Its command-registration, weak-evidence rejection, and repair targeting remain
intact. No growth baseline, source/release/promotion gate, data-directory
allow-list, historical evidence, or live runtime namespace changed.

These deterministic fixtures do not constitute a new nine-run campaign, Next.js
production build, browser measurement, or proof of arbitrary domain semantics.
Exact engine-page recognition has the existing #422 matching limits; normal
contract and final acceptance checks retain responsibility for other unfinished
or incorrect applications. No Python harness changed. No push, PR, or external
Issue mutation was performed.
