# Phase26 Concrete Work Plan

Date: 2026-06-23 JST

Status: executed / closed_proven

## Step 0: Baseline And Scope Confirmation

Inputs:

- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- Phase22 through Phase25 packages

Actions:

1. Confirm KI-005 still owns C13-C20.
2. Confirm Phase22-C01 through Phase25-C12 are not reopened.
3. Capture current commit and dirty state.
4. Record any unrelated dirty files and keep them out of Phase26 commits.

Exit condition:

- Phase26 work is scoped to C13-C20 only.

## Step 1: Source Alignment And Row Ledger

Actions:

1. Reconcile C13-C20 against Anvil source files in
   `source_alignment_matrix.md`.
2. For each row, define adopted behavior, intentionally omitted behavior,
   CommandAgent target modules, and proof method.
3. Fill `row_closure_matrix.md` with current status `Partial`, adoption
   decision, owner layer, missing contract, target modules, required proof,
   closure condition, and initial disposition.
4. Fill `blocking_ledger.md` with one or more blockers per row.
5. Keep all blockers same-surface. If a blocker belongs to Phase27 or Phase28,
   record it as a boundary, not as Phase26 work.

Exit condition:

- Every C13-C20 row has at least one blocker and one proof command.

## Step 2: C13 Recovery Messages And Safe Stop

Actions:

1. Inspect `recovery_task.rs`, `repair.rs`, `repair_job.rs`, and any repair
   packet/failure packet rendering.
2. Define a common safe-stop payload shape for:
   - evidence binding failure;
   - completion authority failure;
   - setup/setup readiness failure;
   - profile verification failure;
   - semantic failure with no admissible repair;
   - action-envelope rejection.
3. Ensure rendered recovery task/repair packet fields include owner, job,
   action, target, cluster, required action, disallowed actions, rerun
   authority, and attempt outcome where present.
4. Add unit tests for every safe-stop family.
5. Add focused fixtures that assert safe-stop fields rather than only final
   reason text.

Do not:

- convert safe-stop into hidden continuation;
- hide a safe-stop behind successful final answer prose.

## Step 3: C14 Setup Bootstrap Lifecycle

Actions:

1. Inspect `setup_lifecycle.rs`, `setup_artifact_validation.rs`,
   `runtime/setup.rs`, verifier dependency classification, and setup eval
   fields.
2. Define setup candidate validation for setup manifest identity, setup
   readiness, command authority, setup attempt key, manifest fingerprint,
   stale setup reason, setup result, and failure signature.
3. Add or complete Rust and Python setup policy paths where deterministic
   verifier evidence points to manifest/toolchain setup blockers.
4. Ensure setup does not execute from ordinary repair without visible setup
   authority.
5. Add tests for stale setup, invalid setup manifest, valid setup manifest,
   non-Node setup blockers, and command authority.
6. Add focused setup fixtures.

Do not:

- allow arbitrary dependency install in normal repair;
- bypass Bash/tool policy.

## Step 4: C15 Profile / Project / Scaffold Facts

Actions:

1. Inspect `profiles.rs`, `profile_artifact.rs`, `artifact_graph.rs`, and
   scaffold-related fields.
2. Extend the common profile output schema for root hints, manifests,
   entrypoints, integration artifacts, setup artifacts, scaffold artifacts,
   protected paths, verifier commands, and behavior obligations.
3. Represent bounded scaffold materialization as explicit artifact contract
   facts.
4. Add scaffold completion evidence and ownership facts.
5. Add profile output tests and focused scaffold/profile fixtures.

Do not:

- add a profile-specific scaffold workflow engine;
- make profile verification mutate files.

## Step 5: C16 Profile Failure Mapping

Actions:

1. Inventory profile verification failure reason codes across Next.js, Rust,
   Python, docs, and data profiles.
2. Map failures to typed recovery facts:
   - route integration;
   - manifest/config;
   - setup/dependency/toolchain;
   - source implementation;
   - scaffold/project shape;
   - explicit stop.
3. Feed those facts to the Phase25 dispatch gate as candidate hints.
4. Add tests for each profile family and failure family.
5. Add focused profile-failure matrix.

Do not:

- let profiles select final recovery job/action;
- add provider/model-specific profile behavior.

## Step 6: C17 Semantic Failure Report

Actions:

1. Inspect `semantic_failure.rs`, `verifier_diagnostic.rs`, and current
   semantic/eval fields.
2. Add conflict-object inputs as data-only evidence:
   - conflicting sources of truth;
   - incompatible expected outcomes;
   - ambiguous target authority;
   - evidence needed by Phase28.
3. Add cluster target ranking inputs:
   - diagnostic kind;
   - source of truth;
   - observed/expected pairs;
   - affected cases;
   - candidate artifacts;
   - preferred repair role;
   - weak verifier reason.
4. Ensure unknown diagnostics stay visible.
5. Add semantic failure tests and verifier-focused fixtures.

Do not:

- resolve full contract conflicts; that is Phase28.

## Step 7: C18 Semantic Repair Plan

Actions:

1. Define semantic repair plan fields for selected cluster, authority, repair
   role, hypothesis, expected improvement, expected evidence delta, success
   check, and exhausted cluster/role/target facts.
2. Connect attempt outcome facts to role-strategy transition inputs.
3. Render those facts into recovery task/repair brief without adding retry
   expansion.
4. Add tests for cluster exhaustion, role transition, and expected evidence
   delta rendering.
5. Add focused semantic repair fixture.

Do not:

- silently switch targets or roles without visible evidence. Phase27 owns
  deeper target prioritization and no-progress strategy.

## Step 8: C19 Repair Brief

Actions:

1. Inspect `repair_brief.rs` and Recovery Task Contract rendering.
2. Ensure repair brief includes root cause, target, allowed change kind,
   disallowed actions, preservation constraints, confidence, success check,
   and expected evidence delta.
3. Ensure repair brief consumes selected dispatch/action facts.
4. Add tests for selected repair, explicit stop, and rejected repair brief
   paths.
5. Add focused repair brief fixture.

## Step 9: C20 Repair Action Space

Actions:

1. Inspect `repair_action_plan.rs`, `recovery_orchestration.rs`,
   `recovery_policy.rs`, and current action labels.
2. Define action-envelope lifecycle states:
   - admitted;
   - rejected;
   - explicit_stop;
   - setup_owned;
   - tool_protocol_owned;
   - unsupported.
3. Validate action/job compatibility, authority, target role, source of truth,
   no-change contract, tool policy, and disallowed action families.
4. Add action-family tests for setup, manifest, route, source, docs,
   evidence-binding, verifier-contract, tool-protocol, scaffold, and safe
   stop.
5. Add focused action-envelope matrix.

Do not:

- add patch execution or rollback execution. Phase27 owns patch validation and
  repair lifecycle.

## Step 10: Focused Eval Package

Suggested focused fixture directory:

```text
eval/cases/focused/control-recovery/recovery-task/
```

Minimum fixture families:

- safe-stop evidence binding failure;
- safe-stop completion authority failure;
- setup Node dependency readiness;
- setup Rust manifest/toolchain blocker;
- setup Python dependency/import blocker;
- profile route/manifest/setup/source/scaffold failure mapping;
- semantic conflict object;
- semantic repair cluster exhaustion;
- repair brief rendering;
- action envelope admission and rejection.

Suggested execution:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/recovery-task \
  --out eval/runs/loadmap2-phase26-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase26-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/recovery-task \
  --recheck
```

## Step 11: Documentation And Coverage

Update only after row proof:

- `docs/architecture.md`
- `docs/adr/0002-contract-recovery.md`
- `docs/evaluation.md`
- `docs/profiles.md`
- `docs/known-limitations.md`, if needed
- `docs/eval/loadmap2-phase26-recovery-task-setup-profile-20260623.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`

Coverage status rule:

- C13-C20 can become `Implemented` only after unit, focused, report, and
  sign-off proof.
- Any row below proof remains `Partial` and must be split with owner,
  downstream phase, failed proof, and closure condition.

## Step 12: Verification

Run:

```bash
cargo fmt --check
cargo test recovery_task
cargo test repair_job
cargo test repair_brief
cargo test repair_action_plan
cargo test setup_lifecycle
cargo test setup_artifact_validation
cargo test profile
cargo test semantic_failure
cargo test recovery_orchestration
cargo test recovery_policy
python3 tests/test_eval_report.py
cargo test
cargo build --release
```

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=<phase26-focused-root> \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

If sign-off fails:

- map each finding to C13-C20 or a later same-surface phase;
- keep Phase26 open unless the finding is split with owner, downstream phase,
  proof command, and failed proof evidence.

Executed verification:

```text
cargo fmt --check
cargo test recovery_task
cargo test recovery_orchestration
cargo test setup_lifecycle
cargo test setup_artifact_validation
cargo test semantic_failure
cargo test repair_brief
cargo test repair_action_plan
cargo test profiles
cargo test profile_artifact
cargo test repair_job
cargo test recovery_policy
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/recovery-task --out eval/runs/loadmap2-phase26-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340 --cases-dir eval/cases/focused/control-recovery/recovery-task --recheck
```

Final verification before commit reran full `cargo test`, release build, and
broad sign-off.

Phase26 execution result:

- C13-C20: `closed_proven`
- focused root:
  `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`
- focused assertions: `passed_recheck: 11`
- broad sign-off: pass
- split-forward rows: none
- migration completion: still blocked by Phase27 and later assigned rows

## Review Result

Review findings applied:

- Split implementation into row-owned steps before any runtime change.
- Added setup/profile/semantic/action-envelope focused fixture requirements
  instead of relying on broad eval.
- Kept Phase27/Phase28 boundaries explicit.
- Required common contracts and eval fields before profile-specific expansion.
- Added clear no-go rules for hidden retries, implicit setup, and workflow
  engines.
