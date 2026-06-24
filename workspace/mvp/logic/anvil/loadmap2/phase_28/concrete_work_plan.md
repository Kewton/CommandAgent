# Phase28 Concrete Work Plan

Date: 2026-06-23 JST

Status: completed / closed_proven

## Completion Result

All Phase28 C33 work items completed. The final focused proof root is
`eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`.
Focused assertions and recheck assertions passed for all six C33 fixtures.
Broad sign-off also passed with the Phase28 root included as supplemental
evidence.

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Confirm unrelated dirty files and exclude them from Phase28 changes.
3. Re-read:
   - `workspace/mvp/logic/anvil/loadmap2/README.md`
   - `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
   - `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
   - `docs/eval/legacy-control-stack-coverage-20260621.md`
4. Confirm C33 is the only Phase28 row.

Exit criteria:

- Phase28 scope is C33 only.
- Phase27 C25 conflict branch is treated as an input/handoff, not as proof of
  C33.

## Step 1: Source Alignment

1. Inspect Anvil baseline files:
   - `contract_conflict_job.rs`
   - `spec_authority.rs`
   - `api_contract_expectation.rs`
2. Map each behavior to CommandAgent contracts:
   - conflict object;
   - authority input;
   - authority decision;
   - selected conflict action;
   - safe stop.
3. Update `source_alignment_matrix.md` if implementation discoveries narrow
   or split C33.

Exit criteria:

- Adopted and intentionally omitted behavior is explicit.
- No hidden Anvil control loop is admitted by accident.

## Step 2: Add Contract Conflict Boundary

1. Add a small module, likely `src/agent/step_runner/contract_conflict.rs`.
2. Define:
   - `ContractConflictObject`;
   - `ConflictSide`;
   - `ConflictAuthorityInput`;
   - `ConflictAuthorityDecision`;
   - `ContractConflictResolution`.
3. Keep fields bounded and serializable/renderable for evidence:
   - paths;
   - roles;
   - source layers;
   - source-of-truth values;
   - observed/expected pairs;
   - affected cases;
   - missing evidence.
4. Keep the authority side and repair target/action as separate fields.
5. Add unit tests for object rendering and authority input collection.

Exit criteria:

- Conflict data is typed before it is rendered into recovery/eval output.
- The module does not run tools or select retry behavior.

## Step 3: Define Deterministic Authority Rules

1. Implement precedence rules from `README.md`.
2. Explicitly reject these shortcuts:
   - generated test beats user/profile/docs/API contract;
   - source implementation wins by default;
   - preserving authoritative implementation is confused with source repair;
   - path order chooses authority;
   - verifier command authority exceeds what it checks.
3. Return `ambiguous_authority` or `insufficient_evidence` when deterministic
   authority cannot be established.
4. Add unit tests for:
   - user/profile/docs/API authority over generated tests;
   - pre-existing test authority;
   - generated test without binding;
   - weak verifier command;
   - equal-authority ambiguity.

Exit criteria:

- Authority decision is deterministic and observable.
- Ambiguous cases do not enter source repair.

## Step 4: Connect Semantic Failure Inputs

1. Use existing `SemanticFailureReport` conflict fields as inputs.
2. Preserve existing Phase26 behavior: semantic reports collect evidence; they
   do not resolve conflicts.
3. Add conversion helpers only if needed:
   - verifier observed/expected -> conflict side;
   - affected case -> conflict evidence;
   - source-of-truth payload -> authority candidate;
   - weak verifier reason -> verifier authority limitation.
4. Add `cargo test semantic_failure` coverage.

Exit criteria:

- C33 consumes Phase26 conflict inputs without moving C33 policy into
  `semantic_failure.rs`.

## Step 5: Connect Recovery Orchestration

1. Add a `contract_conflict` active-job candidate when the conflict object
   exists and authority decision is needed.
2. Map decisions to existing action envelopes:
   - implementation authoritative -> repair or reject non-authoritative test,
     docs/API, or verifier artifact;
   - test authoritative -> repair implementation unless a stronger user,
     profile, docs, or API authority contradicts the test;
   - docs/API authoritative -> repair implementation/test/verifier side that
     violates the docs/API contract;
   - verifier contract limited -> verifier contract correction or safe stop;
   - ambiguous/insufficient -> explicit stop.
3. Ensure selected action carries:
   - authoritative side;
   - repair target side;
   - `source_of_truth`;
   - `allowed_change_kind`;
   - `rerun_authority`;
   - `tool_policy_projection`.
4. Add `cargo test recovery_orchestration` coverage.

Exit criteria:

- C33 integrates with the existing dispatch gate.
- No second dispatcher or hidden conflict engine is introduced.

## Step 6: Close Phase27 Handoff

1. Add or extend tests for the C25 no-progress conflict branch.
2. Verify no-progress can hand off to C33 with:
   - owner `contract_conflict`;
   - selected conflict object;
   - authority decision pending or selected;
   - explicit stop if authority is ambiguous.
3. Ensure repeated no-progress does not increase retry budget.
4. Add `cargo test repair_job` coverage.

Exit criteria:

- Phase27 conflict-dependent branch is no longer a dangling blocker.
- The handoff closes only through C33 proof.

## Step 7: Render Recovery Task And Safe Stop

1. Extend recovery task rendering with conflict fields.
2. Extend safe-stop payload for ambiguous/insufficient authority.
3. Include disallowed actions:
   - do not rewrite verifier to pass;
   - do not edit generated test unless it is the selected action;
   - do not edit source when authority is ambiguous;
   - do not run setup or unrelated commands.
4. Add `cargo test recovery_task`.

Exit criteria:

- The bounded repair prompt says what to fix, what not to fix, and which
  authority/verifier remains the success check.
- Safe-stop payload is complete without requiring prose interpretation.

## Step 8: Add Eval Fields

1. Extend eval schema and report parsing with C33 fields.
2. Add report tests for conflict authority and safe stop.
3. Document expected fields in `eval/README.md` and `docs/evaluation.md`.

Exit criteria:

- Focused fixtures can assert conflict decisions directly.
- Eval reports do not infer conflict closure from raw reason text.

## Step 9: Add Focused Fixtures

Create focused cases under:

```text
eval/cases/focused/control-recovery/contract-conflict/
```

Required cases:

1. `source-vs-generated-test.yaml`
2. `source-vs-preexisting-test.yaml`
3. `docs-api-vs-source.yaml`
4. `weak-verifier-contract.yaml`
5. `phase27-no-progress-handoff.yaml`
6. `ambiguous-authority-safe-stop.yaml`

Each case must assert:

- conflict status;
- conflict sides;
- selected authority or ambiguity;
- selected action or safe-stop reason;
- source of truth;
- missing evidence when any.

Exit criteria:

- Focused recheck passes for all C33 cases.

## Step 10: Documentation And Coverage

After proof passes:

1. Update `docs/eval/legacy-control-stack-coverage-20260621.md` C33 to
   `Implemented`.
2. Update KI-007 in `current_issue_phase_map.md` to `closed_proven`.
3. Update `recovery_plan.md` Phase28 exit gate with proof root.
4. Update `workspace/mvp/logic/anvil/loadmap2/README.md` Phase28 row status.
5. Add `implementation_report.md`.

Exit criteria:

- Coverage state changes are proof-backed.
- Phase28 does not claim Phase29 responsibilities.

## Step 11: Verification

Run:

```bash
cargo fmt --check
cargo test contract_conflict
cargo test semantic_failure
cargo test recovery_orchestration
cargo test recovery_task
cargo test repair_job
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/contract-conflict --out eval/runs/loadmap2-phase28-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py <phase28-focused-root> --cases-dir eval/cases/focused/control-recovery/contract-conflict --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=<existing-smoke-root> --root focused=<existing-focused-root> --root phase28-focused=<phase28-focused-root> --root large=<existing-large-root>
cargo test
cargo build --release
```

Exit criteria:

- Focused fixture recheck passes.
- Broad sign-off passes or every new finding is mapped to a later phase with
  owner, proof, and rationale.
- Full local Rust verification passes.

## Step 12: Exit Review

Before closing Phase28:

1. Confirm C33 has exactly one final disposition:
   - `closed_proven`;
   - `excluded_with_rationale`;
   - valid split-forward with owner and proof.
2. Confirm ambiguous authority stops with structured conflict evidence.
3. Confirm no generated artifact is treated as authority without binding.
4. Confirm no provider/model branch or hidden retry was added.
5. Confirm docs and eval fields match implementation.

Exit criteria:

- Phase28 can be reported as complete only when C33 is proof-backed.

## Plan Review

Review findings applied:

- Moved authority decision before repair action selection to avoid
  source-repair fallback.
- Separated authority side from repair target/action so the authoritative side
  is preserved rather than blindly edited.
- Added an explicit Phase27 handoff step so C25 conflict deferral cannot remain
  open.
- Required safe-stop proof, not just repairable conflict proof.
- Required eval fields before focused fixtures so assertions are structured.
- Deferred language/profile expansion to Phase29 unless C33 proof directly
  requires it.
