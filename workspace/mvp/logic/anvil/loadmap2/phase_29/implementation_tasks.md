# Phase29 Implementation Tasks

Date: 2026-06-23 JST

Status: completed / closed_proven

Implementation summary:

- Runtime support facts were added in `runtime_support.rs`.
- Shell/setup command classification was added in
  `command_classification.rs` and projected into setup lifecycle evidence.
- Workspace candidate reporting was added to `workspace_snapshot.rs`.
- Eval schema/report/recheck support and Phase29 deterministic fixtures were
  added for C34-C44.
- Roadmap and coverage docs were reconciled after focused recheck and broad
  sign-off.

## Phase Admission

- [x] Confirm KI-008 was assigned to Phase29, then closed as `closed_proven`
  in
  `current_issue_phase_map.md`.
- [x] Confirm C34-C44 were Phase29-owned in
  `docs/eval/legacy-control-stack-coverage-20260621.md`.
- [x] Record unrelated dirty files before implementation and keep them out of
  Phase29 commits.
- [x] Confirm Phase29 is not responsible for C49/C50, P17-L001, or final
  migration declaration.

## Source Alignment

- [x] Reconcile every C34-C44 row against the Anvil source modules listed in
  `source_alignment_matrix.md`.
- [x] For each row, record:
  - adopted behavior;
  - intentionally omitted behavior;
  - CommandAgent target module;
  - proof method;
  - split/exclusion condition.
- [x] Refresh `source_alignment_matrix.md` if implementation discovers that a
  row needs a narrower downstream split.

## C34 Language Mechanical Repair

- [x] Inventory current `mechanical_repair.rs`, `verifier_diagnostic.rs`,
  setup validation, and profile adapter outputs for Rust, Python, TypeScript,
  and Next.js.
- [x] Add or complete deterministic adapter outputs for compile/import/test/
  assertion/dependency diagnostics.
- [x] Keep adapters as admitted repair proposals or hints; do not mutate files
  directly.
- [x] Add targeted tests for each admitted language family.
- [x] Add focused fixture coverage if model-facing recovery instructions
  change.

## C35 Tool Policy And Effective Policy

- [x] Inventory owner/action values emitted by recovery orchestration.
- [x] Project selected owner/action/job into effective allowed tool category
  and disallowed action fields for setup, evidence binding, source/docs/test,
  verifier-contract, scaffold, and tool-protocol jobs.
- [x] Ensure policy projection is common and provider-independent.
- [x] Add tests for rejected disallowed tool categories and admitted expected
  categories.
- [x] Add eval/report fields only if new policy facts need focused assertion.

## C36 Tool Failure Recovery

- [x] Inventory current XML/native parse, schema, prose-only, and stale edit
  failure paths.
- [x] Normalize tool failure evidence into first-class correction facts.
- [x] Ensure correction remains bounded and safe-stops when spent/exhausted.
- [x] Add tests for provider parse, schema mismatch, prose-only response, and
  stale edit target branches.
- [x] Add focused fixtures for any model-facing correction packet changes.

## C37 Bash/Setup Command Classification

- [x] Inventory Bash verifier/setup policy in `tools/bash.rs`, `verify.rs`,
  setup runtime, and setup lifecycle modules.
- [x] Classify commands as verifier, setup check, setup execution, inspection,
  mutation, network/dependency, or blocked.
- [x] Bind setup command authority to setup lifecycle and evidence binding.
- [x] Preserve explicit setup policy; do not run network/dependency setup
  implicitly.
- [x] Add Node/Cargo/Python setup command tests and focused fixture if the
  Recovery Task Contract changes.

## C38 Workspace Candidates And Walk

- [x] Inventory ignored-dir rules across workspace snapshot, scope, artifact
  graph, target admission, eval scans, and profile path handling.
- [x] Introduce or complete one single source of truth for ignored dependency,
  cache, generated, build, and VCS directories.
- [x] Feed candidate discovery into artifact graph/recovery without admitting
  out-of-scope ownership.
- [x] Add tests for greenfield, single-project, nested project, ignored output,
  raw input, and dependency cache cases.

## C39 Job Report And Progress Events

- [x] Inventory evidence envelope, runtime event, repair job, active job, and
  eval report fields.
- [x] Add a job-level report schema for active owner, selected action, action
  plan status, attempt outcome, lifecycle transition, and stop reason.
- [x] Keep UI/progress rendering secondary; row proof must be structured data.
- [x] Update eval report tests and docs when fields are added.

## C40 Scaffold Pipeline Contract

- [x] Inventory profile/scaffold artifacts and setup/materialization evidence.
- [x] Represent scaffold as setup/artifact obligations with ownership,
  required paths, completion evidence, and safe-stop behavior.
- [x] Do not add an independent scaffold workflow engine.
- [x] Add tests for missing scaffold artifact, scaffold materialization
  evidence, and route/integration ownership.
- [x] Add focused scaffold fixture if prompt/recovery behavior changes.

## C41 Data/Docs/Research/Ops Evidence

- [x] Inventory docs/data profiles, deliverable obligations, completion
  evidence, evidence binding, and eval report fields.
- [x] Add generic non-coding evidence producers before profile-specific
  branches.
- [x] Cover docs literal, structured data/schema, research citation/summary,
  and ops command/report cases where current contracts can represent them.
- [x] Add focused non-coding matrix if model-facing evidence expectations
  change.

## C42 Answer-only And Work-mode Gating

- [x] Inventory final-answer guard, no-tool guard, step policy, and task
  contract admission behavior.
- [x] Add deterministic answer-only/work-mode gate fields that prevent
  mutating actions only for admitted non-work requests.
- [x] Ensure normal coding repair is not suppressed by broad final-answer
  wording.
- [x] Add tests for answer-only, docs/report, coding, and ambiguous requests.

## C43 Interruption, Lifecycle, Turn State

- [x] Inventory REPL/session/minimal-loop lifecycle and recovery task state.
- [x] Add only lifecycle facts required by explicit recovery contracts:
  interrupted, stopped, awaiting user, repair exhausted, or completed.
- [x] Do not port full actor-loop turn state or hidden continuation.
- [x] Add CLI/session/runtime tests where lifecycle facts are emitted.

## C44 Provider Request Plumbing

- [x] Inventory provider request construction for Ollama, Gemini, OpenAI,
  planner, and XML fallback.
- [x] Audit that provider modules own transport only and do not own planning,
  profiles, recovery, or behavioral policy.
- [x] Add prompt/request boundary tests for tool declarations, native/fallback
  parsing, usage attachment, and policy-free message construction where local
  tests can run offline.
- [x] Update `docs/providers.md` if request-boundary behavior is clarified.

## Coverage And Roadmap Updates

- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
  each row has proof.
- [x] Update KI-008 in `current_issue_phase_map.md` only after C34-C44 are
  closed, split, excluded, or externally blocked with accepted evidence.
- [x] Update `recovery_plan.md` Phase29 exit gate after proof.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/README.md` Phase29 status
  after proof.
- [x] Add `implementation_report.md` at closure time.

## Verification

- [x] `cargo fmt --check`
- [x] Targeted Rust tests for changed rows:
  - `cargo test mechanical_repair`
  - `cargo test recovery_policy`
  - `cargo test recovery_orchestration`
  - `cargo test recovery_task`
  - `cargo test setup_lifecycle`
  - `cargo test setup_artifact_validation`
  - `cargo test workspace_snapshot`
  - `cargo test workspace_scope`
  - `cargo test artifact_graph`
  - `cargo test evidence_binding`
  - `cargo test completion_evidence`
  - `cargo test providers`
- [x] `python3 tests/test_eval_report.py`
- [x] focused Phase29 fixture root with recheck when focused fixtures are added
- [x] broad sign-off with Phase29 root as supplemental evidence
- [x] `cargo test`
- [x] `cargo build --release`

## Review Gate

- [x] Confirm every C34-C44 row has an explicit disposition.
- [x] Confirm no row introduces hidden retry, hidden continuation, provider
  behavioral policy, or profile workflow ownership.
- [x] Confirm setup command support does not perform implicit dependency
  installation.
- [x] Confirm C40 scaffold remains a setup/artifact contract.
- [x] Confirm C41 uses generic evidence producers before profile-specific
  expansion.
- [x] Confirm C44 stays in provider transport boundaries only.
- [x] Confirm docs and eval updates match runtime behavior.

## Completion Checklist

- [x] Confirmed Phase29 owns C34-C44 and does not own C49/C50, Phase31, or
  final migration declaration.
- [x] Closed all C34-C44 rows as `closed_proven` in `row_closure_matrix.md`.
- [x] Closed all P29 blockers as `closed_proven` in `blocking_ledger.md`.
- [x] Added focused deterministic fixtures for all C34-C44 rows under
  `eval/cases/focused/control-recovery/runtime-support/`.
- [x] Ran focused recheck for
  `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335` with
  `passed_recheck: 11`.
- [x] Ran broad sign-off with the Phase29 root as supplemental evidence:
  `status: pass`.
- [x] Updated coverage, roadmap, ADR, architecture, provider/profile, eval,
  and phase-local docs.
- [x] Preserved the unrelated dirty file
  `workspace/mvp/logic/anvil/loadmap2/phase_21/implementation_report.md`
  outside Phase29 work.

## Plan Review

Review findings applied:

- Expanded Phase29 from a broad runtime-support label into row-owned task
  groups.
- Added row-specific target modules and proof commands.
- Added explicit source-alignment refresh and split/exclusion criteria.
- Added guardrails for scaffold, provider, lifecycle, and setup command
  behavior.
- Required coverage/roadmap state changes only after proof.
