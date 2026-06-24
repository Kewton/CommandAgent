# Loadmap2 Phase26 Plan

Date: 2026-06-23 JST

Status: completed / closed_proven

## Objective

Phase26 closes the `P20-COV-002` rows:

| coverage id | responsibility |
| --- | --- |
| C13 | Recovery messages, repair packets, and safe-stop payload coverage for evidence/completion failures. |
| C14 | Setup bootstrap lifecycle, candidate validation, setup result ledger, and non-Node setup policies. |
| C15 | Project probe/profile/scaffold profile facts, bounded scaffold materialization, and scaffold completion evidence. |
| C16 | Profile failure to typed recovery job mapping across route, manifest, setup, source, scaffold, and explicit stop. |
| C17 | Semantic failure report conflict objects, cluster target ranking, and live eval evidence. |
| C18 | Semantic repair plan cluster exhaustion and role-strategy transitions from repair outcomes. |
| C19 | Repair brief target, root cause, constraints, allowed/disallowed actions, and E2E evidence. |
| C20 | Repair action space lifecycle transitions and all action-family proof. |

The goal is to turn the current recovery task/setup/profile/semantic-repair
surface from partially structured hints into row-level contracts that can feed
the existing Phase25 dispatch boundary:

```text
deterministic failure evidence
  -> profile/setup/semantic facts
  -> selected dispatch decision from Phase25
  -> recovery task / setup task / safe stop packet with explicit semantics
  -> bounded minimal-loop execution or explicit stop
  -> eval-visible proof
```

Phase26 must make the repair task clear enough that the minimal loop is not
asked to infer root cause, setup readiness, action family, or disallowed
changes from prose.

## Inputs

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_22/`
- `workspace/mvp/logic/anvil/loadmap2/phase_23/`
- `workspace/mvp/logic/anvil/loadmap2/phase_24/`
- `workspace/mvp/logic/anvil/loadmap2/phase_25/`
- current CommandAgent modules:
  - `src/agent/step_runner/recovery_task.rs`
  - `src/agent/step_runner/repair.rs`
  - `src/agent/step_runner/repair_job.rs`
  - `src/agent/step_runner/repair_brief.rs`
  - `src/agent/step_runner/repair_action_plan.rs`
  - `src/agent/step_runner/setup_lifecycle.rs`
  - `src/agent/step_runner/setup_artifact_validation.rs`
  - `src/agent/step_runner/profiles.rs`
  - `src/agent/step_runner/profile_artifact.rs`
  - `src/agent/step_runner/semantic_failure.rs`
  - `src/agent/step_runner/recovery_orchestration.rs`
  - `src/agent/step_runner/recovery_policy.rs`
  - `src/agent/step_runner/runtime/setup.rs`
  - `src/agent/step_runner/runtime/repair_loop.rs`
- eval/report modules:
  - `scripts/eval_agent_slice.sh`
  - `scripts/eval_report.py`
  - `tests/test_eval_report.py`
  - focused cases under `eval/cases/focused/control-recovery/`

## Non-goals

- Do not implement Phase27 target prioritization, verifier orchestration,
  repair lifecycle retry strategy, patch validation, or completion job
  lifecycle.
- Do not implement Phase28 full contract conflict resolution. Phase26 may
  emit structured conflict inputs, but C33 owns conflict job resolution.
- Do not turn profiles into workflow engines. Profiles may expose typed facts
  and candidate hints; Recovery Orchestration and Recovery Task Contract still
  own job/action rendering.
- Do not run dependency setup implicitly from ordinary repair. Setup remains a
  visible, policy-gated action with command authority and ledger evidence.
- Do not add hidden repair loops, retry expansion, hidden continuation, or
  provider/model-specific branches.
- Do not weaken verifiers or success checks to make Phase26 pass.

## Design Alignment

Phase26 follows the current CommandAgent contract stack:

```text
Task/Profile/Artifact/Evidence facts
  -> Setup/Profile/Semantic recovery facts
  -> Phase25 dispatch gate
  -> Recovery Task Contract / setup action / safe stop packet
  -> minimal loop execution
  -> evidence-bound verification
```

Layer ownership:

| layer | Phase26 responsibility |
| --- | --- |
| `recovery_task` / repair packet | Render safe-stop and repair packet fields for evidence, completion, setup, profile, semantic, and action-envelope failures. |
| setup lifecycle | Validate setup candidates, setup manifests, setup readiness, setup result ledger, and non-Node setup policy facts without implicit execution. |
| profiles | Emit common project/profile/scaffold facts and typed profile-failure mappings without selecting final workflow behavior. |
| semantic failure | Structure diagnostic clusters, conflict inputs, observed/expected facts, affected cases, and candidate artifacts. |
| semantic repair / repair brief | Render cluster, role, root cause, hypothesis, constraints, expected delta, and success evidence into bounded repair instructions. |
| repair action plan | Validate action family, authority, allowed/disallowed changes, and lifecycle status before prompt rendering. |
| eval/report | Prove each row using expected fields, focused fixtures, and broad sign-off evidence. |

This keeps Phase26 bounded: it clarifies what the selected recovery action
means, but it does not create a hidden scheduler or run extra attempts.

## Architecture Shape

Prefer common contracts over profile-specific workflows:

1. Add shared safe-stop / repair packet fields once, then feed them from
   evidence, completion, setup, profile, and semantic failures.
2. Make setup lifecycle a typed setup state machine, not a Bash permission
   shortcut.
3. Represent profile output and scaffold facts in a common schema used by
   Next.js, Rust, Python, docs, and data profiles.
4. Map profile verification failures to typed recovery job/action/target facts
   before dispatch.
5. Expand semantic failure reports and repair briefs as data-only planning
   inputs; do not let them execute repair by themselves.
6. Validate action envelopes before Recovery Task Contract rendering.
7. Close rows only when unit tests, focused fixtures, and sign-off evidence
   prove the row-specific behavior.

## Horizontal Rollout

Phase26 must not be Next.js-only. Required coverage families:

- Next.js setup, manifest, route, source, and scaffold/profile facts.
- Rust manifest/toolchain/test diagnostic setup and source failure facts.
- Python dependency/import/test assertion setup and source failure facts.
- Docs literal and section evidence facts.
- Data/schema output evidence facts where applicable.
- Tool protocol and verifier contract failures only as inputs to safe-stop or
  repair task rendering, not as Phase26-specific workflow engines.

## Documentation Updates

Runtime changes in Phase26 must update:

- `docs/architecture.md` when recovery task, setup lifecycle, profile output,
  semantic failure, or action-envelope boundaries change.
- `docs/adr/0002-contract-recovery.md` if Recovery Task Contract or setup
  recovery semantics change.
- `docs/evaluation.md` if eval fields, focused matrix interpretation, or
  sign-off requirements change.
- `docs/profiles.md` if profile output, scaffold, or failure mapping changes.
- `docs/ultra-plan-run.md` only if planner-facing recovery/setup/profile
  contract fields change.
- `docs/known-limitations.md` if a Phase26 row is intentionally split forward
  or externally limited.
- `docs/eval/legacy-control-stack-coverage-20260621.md` only after row proof.
- a new `docs/eval/loadmap2-phase26-recovery-task-setup-profile-*.md` report
  at implementation closure.

## Required Proof

Minimum row proof before `closed_proven`:

| row | minimum proof |
| --- | --- |
| C13 | Recovery task tests and focused safe-stop fixtures proving repair packet and safe-stop payload coverage for evidence/completion/setup/profile/semantic failures. |
| C14 | Setup lifecycle/setup validation tests and focused setup matrix proving candidate validation, setup readiness, command authority, result ledger, stale setup, and non-Node setup policy. |
| C15 | Profile output/scaffold tests and focused scaffold fixture proving project/profile/scaffold facts, bounded scaffold materialization, and scaffold completion evidence. |
| C16 | Profile mapping tests and focused profile-failure matrix proving route, manifest, setup, source, scaffold, and explicit-stop typed mappings. |
| C17 | Semantic failure tests and verifier fixtures proving conflict objects, observed/expected, affected cases, cluster target ranking, and live eval visibility. |
| C18 | Semantic repair/recovery task tests proving cluster exhaustion, role-strategy transitions, and expected evidence delta rendering. |
| C19 | Repair brief tests and focused repair brief fixture proving root cause, target, constraints, allowed/disallowed actions, preservation constraints, and E2E evidence. |
| C20 | Repair action plan/action envelope tests and focused action-envelope matrix proving lifecycle transitions and action-family admission/rejection. |

Phase-level proof:

- `cargo fmt --check`
- targeted Rust tests for recovery task, setup lifecycle, setup validation,
  profiles/profile artifacts, semantic failure, repair brief, repair action
  plan, recovery orchestration, and repair loop where changed
- `python3 tests/test_eval_report.py`
- focused Phase26 eval fixture root with recheck
- broad sign-off using existing smoke/focused/large roots plus Phase26 focused
  fixture root
- `cargo test`
- `cargo build --release`

## Exit Gate

Phase26 can close only when:

- C13-C20 are each `closed_proven`, or a narrower same-surface blocker is
  split forward with failed proof evidence, owner, downstream phase, and
  closure condition.
- `source_alignment_matrix.md`, `row_closure_matrix.md`,
  `blocking_ledger.md`, `reconciliation.md`, and `focused_worklist.md` are
  updated with final results.
- coverage table status changes are made only for rows with row-specific
  proof.
- broad sign-off is pass or every finding is mapped to a later row with proof
  and owner.
- no behavior relies on hidden retries, hidden continuation, provider/model
  branches, profile workflow engines, or implicit dependency setup.

Phase26 result:

- C13-C20 are all `closed_proven` in `row_closure_matrix.md`.
- all P26 blockers are `closed_proven` in `blocking_ledger.md`.
- focused proof root:
  `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`
- focused recheck assertions: `passed_recheck: 11`
- broad sign-off is run as regression proof before commit; it does not replace
  row-level proof.
- migration remains incomplete because Phase27 and later rows remain assigned
  in the loadmap.

## Plan Review

Review findings applied:

- Split the broad Phase26 surface into eight coverage rows so setup/profile
  work does not hide semantic repair or action-envelope gaps.
- Kept Phase27 target/verifier/patch responsibilities out of scope while
  allowing Phase26 to produce data consumed by those later rows.
- Required common profile/setup/semantic contracts before profile-specific
  expansion.
- Required focused fixtures for both selected repair tasks and explicit
  safe-stop paths.
- Required row closure evidence before coverage status changes; CI or broad
  sign-off alone cannot close a row.
