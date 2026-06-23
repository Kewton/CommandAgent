# Phase26 Implementation Tasks

Date: 2026-06-23 JST

## Task Status Legend

- `[ ]` not started
- `[~]` started / blocked on proof
- `[x]` complete

## Preparation

- [x] Confirm the current branch, commit, and dirty state before
      implementation.
- [x] Re-read `AGENTS.md`, `docs/philosophy.md`, `docs/architecture.md`,
      `docs/adr/0002-contract-recovery.md`, `docs/evaluation.md`, and
      `docs/profiles.md`.
- [x] Confirm Anvil baseline against
      `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`.
- [x] Confirm C13-C20 still map to `P20-COV-002` and KI-005.
- [x] Inspect Phase22-Phase25 outputs and avoid reopening closed rows unless a
      narrow regression is proven.
- [x] Inventory current modules:
      - `recovery_task.rs`
      - `repair.rs`
      - `repair_job.rs`
      - `repair_brief.rs`
      - `repair_action_plan.rs`
      - `setup_lifecycle.rs`
      - `setup_artifact_validation.rs`
      - `profiles.rs`
      - `profile_artifact.rs`
      - `semantic_failure.rs`
      - `recovery_orchestration.rs`
      - `recovery_policy.rs`
      - `runtime/setup.rs`
      - `runtime/repair_loop.rs`
- [x] Inventory current eval fields and focused fixtures for recovery task,
      setup, profile, semantic failure, repair brief, and action envelope.

## C13: Recovery Messages / Packets / Safe Stop

- [x] Inventory current repair packet and safe-stop payload fields.
- [x] Define required safe-stop fields for evidence binding, completion
      authority, setup, profile, semantic failure, and action-envelope
      rejection.
- [x] Ensure repair packet rendering carries owner, job, action, target,
      cluster, attempt outcome, required action, disallowed actions, and rerun
      authority when present.
- [x] Add tests for safe-stop payload rendering across evidence/completion and
      recovery task failures.
- [x] Ensure final error surfaces remain explicit and do not claim success.

## C14: Setup Bootstrap Lifecycle

- [x] Inventory setup lifecycle, setup validation, setup runtime, and setup
      eval fields.
- [x] Define setup candidate validation inputs:
      - manifest kind/path;
      - dependency/toolchain evidence;
      - setup readiness;
      - command authority;
      - attempt key/fingerprint;
      - stale setup reason;
      - setup result and failure signature.
- [x] Add non-Node setup policy for Rust and Python where verifier evidence
      can deterministically identify manifest/toolchain setup blockers.
- [x] Ensure setup lifecycle does not execute dependency setup implicitly from
      normal repair.
- [x] Add setup lifecycle/setup artifact validation tests.
- [x] Add focused setup fixtures for Node, Rust, Python, stale setup, and
      setup command authority.

## C15: Project Probe / Profile / Scaffold Facts

- [x] Inventory common profile output schema and profile artifact projection.
- [x] Add missing project/profile facts for root hints, manifests,
      entrypoints, integration artifacts, setup artifacts, scaffold artifacts,
      protected paths, verifier commands, and behavior obligations.
- [x] Define bounded scaffold materialization evidence as an explicit artifact
      contract, not a hidden scaffold workflow.
- [x] Add scaffold completion evidence and ownership fields.
- [x] Add profile output and scaffold-focused tests.
- [x] Add focused scaffold/profile fixture.

## C16: Profile Failure To Typed Recovery Job

- [x] Inventory profile verification failure reason codes across Next.js,
      Rust, Python, docs, and data profiles.
- [x] Map profile failures to typed recovery facts:
      - route integration;
      - manifest/config;
      - setup/dependency/toolchain;
      - source implementation;
      - scaffold/project shape;
      - explicit stop for unsupported or ambiguous failures.
- [x] Ensure profiles emit candidate hints only; Phase25 dispatch still owns
      final selection.
- [x] Add profile mapping tests for each profile family.
- [x] Add focused profile-failure matrix.

## C17: Semantic Failure Report

- [x] Inventory current semantic failure fields and verifier diagnostic fields.
- [x] Add conflict object fields without implementing Phase28 conflict job
      resolution.
- [x] Add cluster target ranking inputs:
      - failure kind;
      - source of truth;
      - observed/expected;
      - affected cases;
      - candidate artifacts;
      - preferred repair role;
      - weak verifier reason.
- [x] Ensure unknown diagnostics remain observable and not silently mapped to
      source repair.
- [x] Add semantic failure tests and verifier focused fixtures.
- [x] Record live/focused eval evidence before promoting C17.

## C18: Semantic Repair Plan

- [x] Define semantic repair plan fields:
      - selected cluster;
      - authority;
      - repair role;
      - hypothesis;
      - expected improvement;
      - expected evidence delta;
      - success check;
      - exhausted cluster/role/target handoff.
- [x] Connect attempt outcome facts to cluster exhaustion and role-strategy
      transition inputs without adding retry expansion.
- [x] Add recovery task and repair state tests.
- [x] Add focused semantic repair fixture.

## C19: Repair Brief

- [x] Expand repair brief rendering to include root cause, target, allowed
      change kind, disallowed actions, preservation constraints, confidence,
      success check, and evidence delta.
- [x] Ensure repair brief consumes selected dispatch/action facts rather than
      recomputing owner/action from prose.
- [x] Add tests for repair brief rendering, rejection, and E2E-visible fields.
- [x] Add focused repair brief fixture.

## C20: Repair Action Space / Action Envelope

- [x] Inventory current repair action labels and action envelope checks.
- [x] Define lifecycle states for admitted, rejected, explicit stop,
      setup-owned, tool-protocol-owned, and unsupported actions.
- [x] Validate job/action compatibility, authority, target role, tool policy,
      source of truth, no-change contracts, and disallowed action families.
- [x] Add action-family tests for setup, manifest, route, source, docs,
      evidence binding, verifier contract, tool protocol, scaffold, and safe
      stop.
- [x] Add focused action-envelope matrix.

## Documentation

- [x] Update `docs/architecture.md` for recovery task/setup/profile/semantic
      repair/action-envelope boundary changes.
- [x] Update `docs/adr/0002-contract-recovery.md` if Recovery Task Contract or
      setup recovery semantics change.
- [x] Update `docs/evaluation.md` for any eval field or sign-off changes.
- [x] Update `docs/profiles.md` for profile output, scaffold, or profile
      failure mapping changes.
- [x] Update `docs/known-limitations.md` if rows split forward or carry an
      explicit limitation.
- [x] Add Phase26 eval report under `docs/eval/`.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
      row proof supports status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`,
      `recovery_plan.md`, and `README.md` if KI-005 status changes.

## Evaluation

- [x] Run `cargo fmt --check`.
- [x] Run targeted Rust tests:
      - `cargo test recovery_task`
      - `cargo test repair_job`
      - `cargo test repair_brief`
      - `cargo test repair_action_plan`
      - `cargo test setup_lifecycle`
      - `cargo test setup_artifact_validation`
      - `cargo test profile`
      - `cargo test semantic_failure`
      - `cargo test recovery_orchestration`
      - `cargo test recovery_policy`
- [x] Run `python3 tests/test_eval_report.py`.
- [x] Run focused Phase26 fixture root with recheck.
- [x] Run broad sign-off with Phase26 focused root.
- [x] Run `cargo test`.
- [x] Run `cargo build --release`.

Result roots and checks:

- focused root:
  `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`
- focused recheck assertions: `passed_recheck: 11`
- broad sign-off: pass
- no Phase26 row was split forward

## Review Checklist

- [x] Every C13-C20 task has an owner layer and proof command.
- [x] No row is closed by docs alone, CI alone, broad sign-off alone, or field
      existence alone.
- [x] Recovery task rendering is clear before minimal-loop execution.
- [x] Setup remains visible and policy-gated; no implicit dependency setup is
      introduced.
- [x] Profiles produce facts and candidate hints only; they do not arbitrate
      workflow behavior.
- [x] Semantic failure reports are data-only and do not execute repair.
- [x] Action envelopes reject unsupported action/target/authority combinations
      before prompt rendering.
- [x] No provider/model-specific runtime branch is introduced.
- [x] No hidden retry, hidden continuation, or hidden repair loop is
      introduced.
- [x] No Phase27 target/verifier/patch lifecycle behavior is claimed in
      Phase26.
- [x] No Phase28 contract conflict resolution is claimed in Phase26.

## Completion Result

Phase26 closes C13-C20 as row-level `closed_proven` work. Documentation tasks
were completed by updating the Phase26 package, the recovery map, the coverage
table, and the Phase26 eval report. Existing architecture, recovery ADR,
evaluation, and profile docs already describe the admitted contract boundaries;
no known-limitations update was needed because no row was split forward or
externally limited.

## Review Result

Review findings applied:

- Split Phase26 into eight row-owned workstreams so broad recovery-task work
  cannot hide setup/profile/semantic/action-envelope gaps.
- Added explicit non-goals for Phase27 and Phase28 to keep the scope stable.
- Required common contract structures and focused fixtures before
  profile-specific expansion.
- Required safe-stop proof as a first-class path, not only selected repair
  success.
- Required action-envelope admission/rejection proof before Recovery Task
  Contract rendering.
