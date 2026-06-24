# Loadmap2 Phase27 Plan

Date: 2026-06-23 JST

Status: completed / closed_proven

## Scope

Phase27 closes `P20-COV-003` / KI-006:

| row | responsibility |
| --- | --- |
| C21 | Repair target decision/admission across route, source, test, docs, setup, and evidence-binding cases. |
| C22 | Target prioritization by failure kind, authority, role, and progress history. |
| C23 | Repair job state machine lifecycle and verifier rerun transition reporting. |
| C24 | Repair attempt ledger across profile families and attempt outcomes. |
| C25 | No-progress recovery strategy branches across target, role, evidence binding, contract conflict, scaffold, and explicit stop. |
| C26 | Verifier diagnostic assessment with language-specific diagnostics and weak target filters. |
| C27 | Verifier orchestration, failure-attempt limits, rerun outcome events, binding scope, and safe-stop report. |
| C28 | Verifier command policy for generated tests, self-reference, unsupported assertions, and expectation audit. |
| C29 | Artifact completion job bound to ledger, ownership, freshness, and missing-evidence distinction. |
| C30 | Focused edit recovery after target admission with current excerpt and stale-target rejection. |
| C31 | Forced small edit / deterministic fallback admission and safe mutation proof. |
| C32 | Patch executor/validation, unsafe/noop/duplicate/test-weakening rejection, rollback proof. |

Phase27 is the target/verifier/repair lifecycle layer after Phase25 dispatch
and Phase26 recovery-task rendering. It should make the selected target,
verifier rerun, attempt result, and patch/admission outcome observable before a
repair can claim progress.

## Non-goals

- Do not implement C33 full contract-conflict resolution. Phase27 may record a
  no-progress branch that defers to Phase28, but source-of-truth conflict
  resolution remains Phase28.
- Do not import Anvil hidden continuation, large autonomous workflow control,
  or retry-until-success behavior.
- Do not weaken verifier commands, generated-test integrity checks, or patch
  validation in order to pass eval.
- Do not let profiles become workflow engines. Profiles can provide facts;
  shared target admission and verifier/repair lifecycle own decisions.
- Do not add provider/model-specific behavior.

## Design Alignment

Phase27 follows the current CommandAgent contract stack:

```text
FailureEvidence / ArtifactGraph / ArtifactLedger
  -> ActiveJob + RecoveryAction from Phase25
  -> RecoveryTask / action envelope from Phase26
  -> Phase27 target admission, verifier orchestration, repair lifecycle,
     attempt ledger, completion job, focused edit, mechanical fallback,
     patch validation
  -> original guard/verifier rerun
  -> success, explicit stop, or row-owned no-progress evidence
```

The minimal loop remains the executor. Phase27 adds deterministic gates and
reports around the existing execution, not another execution engine.

## Architecture Notes

- Add small shared structs/enums when multiple rows need the same concepts:
  target candidate/admission, target priority component, verifier rerun event,
  attempt outcome, patch validation outcome, and focused edit evidence.
- Keep row-specific producers thin. A verifier diagnostic producer should not
  also choose a patch; a patch validator should not decide the active job.
- Prefer typed report fields over prose parsing. Eval and repair packets should
  read the same structured fields.
- If an Anvil behavior is useful but too broad, preserve the useful contract
  data and explicitly omit hidden execution or advisory behavior.

## Cross-phase Boundaries

| adjacent phase | boundary |
| --- | --- |
| Phase23 | Artifact role, scope, and ownership are inputs to Phase27 target admission. Do not redefine them. |
| Phase24 | Artifact ledger, completion evidence, evidence binding, and freshness are inputs. Do not recreate producer logic. |
| Phase25 | Active-job dispatch has selected owner/action. Phase27 must not create a parallel dispatcher. |
| Phase26 | Recovery task and action envelope describe the allowed repair. Phase27 must not bypass them. |
| Phase28 | Contract-conflict source-of-truth decision is not Phase27. Phase27 only records deferral or safe stop for conflict branches. |
| Phase29 | Language/profile/tool/runtime support expansion is not Phase27 except where a row needs representative focused proof. |

## Horizontal Expansion

Phase27 must not be Next.js-only. Required proof families:

- route/source/test/docs/setup/evidence-binding target admission;
- Rust, Python, Next.js, and generic verifier diagnostics;
- completion and missing-evidence artifact jobs;
- focused edit and stale-target rejection;
- mechanical fallback admission for at least one compile/import-style
  diagnostic;
- patch validation for unsafe, noop, duplicate, protected, generated/cache,
  and test-weakening mutations.

Profile-specific facts should flow through common target/verifier/patch
contracts, not through separate profile workflows.

## Documentation Updates

Runtime changes in Phase27 must update:

- `docs/architecture.md` if target admission, verifier orchestration, repair
  lifecycle, patch validation, or rollback admission boundaries change.
- `docs/adr/0002-contract-recovery.md` if Phase27 admits a new recovery
  lifecycle or patch execution contract.
- `docs/evaluation.md` if eval fields, focused matrix interpretation, or
  sign-off gates change.
- `docs/profiles.md` only if profile facts consumed by Phase27 change.
- `docs/known-limitations.md` if a row is intentionally split forward or
  externally limited.
- `docs/eval/legacy-control-stack-coverage-20260621.md` only after row proof.
- a new Phase27 eval report under `docs/eval/` at implementation closure.

## Required Proof

Minimum row proof before `closed_proven`:

| row | minimum proof |
| --- | --- |
| C21 | Target-admission tests and focused target matrix proving route/source/test/docs/setup/evidence-binding admission and rejection. |
| C22 | Target-priority tests and focused prioritization fixture proving deterministic priority and ambiguous tie stop. |
| C23 | Repair-job lifecycle tests and focused lifecycle fixture proving verifier rerun transitions and safe stop. |
| C24 | Attempt-ledger tests and focused attempt outcome matrix across profile families. |
| C25 | No-progress tests and focused matrix proving branch selection, Phase28 deferral for contract conflict, and explicit stop. |
| C26 | Verifier-diagnostic tests and focused verifier fixture proving language-specific diagnostic assessment and weak target filters. |
| C27 | Verifier-orchestration tests and focused verifier-rerun fixture proving attempt limits, rerun outcomes, binding scope, and safe-stop report. |
| C28 | Verifier-selection/integrity tests and focused verifier-policy fixture proving generated-test/self-reference/unsupported assertion rejection. |
| C29 | Artifact-completion/evidence-authority tests and focused completion-job fixture proving ledger/ownership/freshness binding. |
| C30 | Target-admission/ledger tests and focused edit fixture proving current excerpt requirement and stale target rejection. |
| C31 | Mechanical-repair/patch-admission tests and focused deterministic fallback fixture proving bounded safe mutation admission. |
| C32 | Patch-validation tests and focused patch fixture proving unsafe/noop/duplicate/test-weakening rejection and rollback proof. |

Phase-level proof:

- `cargo fmt --check`
- targeted Rust tests for target admission, repair job, verifier diagnostic,
  verifier selection/integrity, artifact completion, evidence authority,
  mechanical repair, patch validation, and recovery orchestration where
  changed
- `python3 tests/test_eval_report.py`
- focused Phase27 eval fixture root with recheck
- broad sign-off using existing smoke/focused/large roots plus Phase27 focused
  fixture root
- `cargo test`
- `cargo build --release`

## Exit Gate

Phase27 can close only when:

- C21-C32 are each `closed_proven`, or a narrower same-surface blocker is
  split forward with failed proof evidence, owner, downstream phase, and
  closure condition.
- Any C25 contract-conflict branch is explicitly recorded as Phase28-owned and
  does not claim C33 conflict resolution.
- `source_alignment_matrix.md`, `row_closure_matrix.md`,
  `blocking_ledger.md`, `reconciliation.md`, and `focused_worklist.md` are
  updated with final results.
- coverage table status changes are made only for rows with row-specific
  proof.
- broad sign-off is pass or every finding is mapped to a later row with proof
  and owner.
- no behavior relies on hidden retries, hidden continuation, provider/model
  branches, profile workflow engines, or implicit dependency setup.

## Implementation Closure

Phase27 closed C21-C32 with row-level proof:

- targeted Rust tests for target admission, repair job/no-progress,
  verifier diagnostics, verifier selection/integrity, artifact completion,
  evidence authority, mechanical repair, repair action plan, recovery
  orchestration, and repair loop;
- eval report tests for Phase27 `expected_*` fields;
- focused fixture root
  `eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917`, rechecked with
  `passed_recheck: 12`;
- broad sign-off using the existing smoke/focused/large roots plus the Phase27
  focused fixture root.

C25 proves only the contract-conflict branch selection/deferral. C33 conflict
resolution remains Phase28-owned.

## Plan Review

Review findings applied:

- Split Phase27 into twelve row-owned workstreams so target admission,
  verifier orchestration, repair lifecycle, completion, focused edit,
  mechanical fallback, and patch validation cannot hide each other.
- Made the Phase28 C33 conflict boundary explicit, especially for the C25
  no-progress branch.
- Required common target/verifier/patch contracts before any profile-specific
  expansion.
- Required focused proof for both selected repair paths and rejection/safe-stop
  paths.
- Required row closure evidence before coverage status changes; CI or broad
  sign-off alone cannot close a row.
