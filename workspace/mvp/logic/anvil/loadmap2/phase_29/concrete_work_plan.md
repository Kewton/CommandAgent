# Phase29 Concrete Work Plan

Date: 2026-06-23 JST

Status: completed / closed_proven

Completion proof:

- Focused fixture root:
  `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335`
- Focused recheck: `passed_recheck: 11`
- Broad sign-off: `status: pass`

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and exclude them from Phase29 changes.
3. Re-read:
   - `workspace/mvp/logic/anvil/loadmap2/README.md`
   - `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
   - `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
   - `docs/eval/legacy-control-stack-coverage-20260621.md`
4. Confirm Phase29 owns only C34-C44.

Exit criteria:

- C34-C44 are the only selected rows.
- C49/C50, Phase31 timeout proof, and final migration declaration remain out
  of scope.

Result: completed. Unrelated dirty file
`workspace/mvp/logic/anvil/loadmap2/phase_21/implementation_report.md` was
left untouched.

## Step 1: Source Alignment And Row Ledger

1. Inspect source families for C34-C44 in the Anvil baseline recorded by the
   coverage table.
2. Update `source_alignment_matrix.md` if any source file or adopted behavior
   is wrong.
3. Split `blocking_ledger.md` into one or more blockers per row.
4. Confirm `row_closure_matrix.md` has disposition and proof columns for all
   eleven rows.

Exit criteria:

- No row is represented only by the phrase "runtime support".
- Every row has an owner layer and a proof family before runtime changes.

Result: completed. Row dispositions and blockers are reconciled in
`row_closure_matrix.md`, `blocking_ledger.md`, and `reconciliation.md`.

## Step 2: Plan Implementation Order

Implement Phase29 in dependency order so support contracts are available before
row-specific adapters consume them:

1. C38 workspace candidate and ignored-dir single source of truth.
2. C35 effective tool policy projection.
3. C37 Bash/setup command classification.
4. C36 tool failure recovery branches.
5. C39 job/progress report fields.
6. C34 language mechanical repair adapters.
7. C40 scaffold setup/artifact contract.
8. C41 generic non-coding evidence producers.
9. C42 answer-only/work-mode gating.
10. C43 lifecycle/turn state projection.
11. C44 provider request boundary audit and tests.

Rationale:

- Workspace and policy facts are inputs to later recovery and proof surfaces.
- Tool/setup/report facts should be stable before adding more adapter rows.
- Provider boundary audit comes last so it can verify no earlier row leaked
  policy into transports.

Result: implemented as shared report projections and deterministic support
facts. No hidden runtime controller was added.

## Step 3: C38 Workspace Candidate Discovery

1. Consolidate ignored-dir rules used by workspace snapshot, artifact graph,
   target admission, profiles, and eval scans.
2. Add candidate discovery that records source, role hint, scope status, and
   exclusion reason.
3. Feed candidates into artifact/recovery contracts without treating candidate
   paths as owned deliverables.
4. Add tests for nested projects, dependency/cache/build output, VCS dirs,
   greenfield projects, and raw data paths.

Exit criteria:

- Scope-aware candidates are deterministic and rejected paths explain why.
- Candidate discovery does not bypass ownership or target admission.

## Step 4: C35 Effective Tool Policy

1. Inventory all selected recovery owners/actions.
2. Define a common projection from owner/action/job to allowed tool category,
   disallowed actions, and policy reason.
3. Connect setup, evidence-binding, source/docs/test, verifier-contract,
   scaffold, and tool-protocol jobs.
4. Add tests that prove both admitted and rejected tool categories.

Exit criteria:

- Effective policy is derived from the selected contract, not from provider or
  model identity.
- Policy failures are visible in evidence/eval fields.

## Step 5: C37 Bash And Setup Command Classification

1. Classify command intent before admission:
   verifier, setup check, setup execution, inspection, mutation,
   network/dependency, or blocked.
2. Bind setup command authority to setup lifecycle and evidence binding.
3. Keep dependency installation explicit; `--yes` may admit a policy-approved
   visible setup action, but the model must not issue dependency setup
   implicitly.
4. Add Node, Cargo, and Python command tests.

Exit criteria:

- Setup command authority is deterministic and visible.
- Blocked/network/dependency cases stop or require explicit setup policy rather
  than becoming ordinary Bash repair.

## Step 6: C36 Tool Failure Recovery

1. Normalize provider parse, schema mismatch, prose-only, stale edit, and
   invalid tool input failures into common correction facts.
2. Connect those facts to bounded correction packets and safe-stop exhaustion.
3. Add tests and focused fixtures for branches that alter recovery task text or
   expected eval fields.

Exit criteria:

- Tool failure recovery is bounded and observable.
- A spent correction cannot loop or fall back to unrelated source repair.

## Step 7: C39 Job Report And Progress Events

1. Add job report fields for active owner, selected action, action-plan
   status, attempt outcome, lifecycle transition, and stop reason.
2. Feed the report from existing active-job/recovery/repair states.
3. Update eval report tests and docs.

Exit criteria:

- Eval can assert job/progress state without parsing prose.
- UI-only progress behavior is not required for row closure.

## Step 8: C34 Language Mechanical Repair Adapters

1. Complete deterministic adapter facts for supported Rust, Python,
   TypeScript, and Next.js diagnostic families.
2. Keep outputs as admitted proposals/hints consumed by recovery contracts.
3. Reject adapter output when target, verifier authority, or allowed change
   kind is missing.
4. Add targeted tests and focused language matrix if the model-facing task
   changes.

Exit criteria:

- Mechanical repair support is language-aware but not a direct patch executor.
- Unsupported language families fail as explicit unsupported evidence, not
  generic source repair.

## Step 9: C40 Scaffold Contract

1. Represent scaffold requirements as setup/artifact obligations with required
   paths, role, ownership, completion evidence, and freshness.
2. Route scaffold failures through existing active-job and recovery task
   contracts.
3. Add focused proof only if scaffold repair prompts change.

Exit criteria:

- Scaffold support is not an independent workflow engine.
- Missing scaffold and route/integration ownership are distinguishable.

## Step 10: C41 Non-coding Evidence Producers

1. Add or complete generic evidence producers for docs literal, data/schema,
   research citation/summary, and ops command/report deliverables.
2. Reuse completion evidence, evidence binding, deliverable obligation, and
   artifact ownership.
3. Avoid profile-specific branches until the generic producer cannot express
   the requirement.

Exit criteria:

- Non-coding deliverables can close with structured evidence rather than final
  answer prose alone.
- Generic producer coverage is documented.

## Step 11: C42 Answer-only And Work-mode Gate

1. Add deterministic task admission facts for answer-only and work-mode
   requests.
2. Prevent mutation only when a request is admitted as answer-only or when
   work mode is explicitly not admitted.
3. Add tests for coding, docs/report, answer-only, and ambiguous requests.

Exit criteria:

- Normal coding repair is not suppressed by final-answer contract logic.
- Answer-only behavior does not broaden no-tool guards for ordinary tasks.

## Step 12: C43 Lifecycle And Turn State

1. Project only recovery-relevant lifecycle states:
   interrupted, completed, stopped, repair exhausted, awaiting user, or
   explicit stop.
2. Connect these states to eval/report output where useful.
3. Exclude actor-loop details that do not affect explicit recovery contracts.

Exit criteria:

- Lifecycle state is observable where recovery decisions need it.
- No hidden actor-loop control stack is introduced.

## Step 13: C44 Provider Request Boundary

1. Audit provider request construction for Ollama, Gemini, OpenAI, planner,
   and XML fallback.
2. Add or update tests proving providers own transport/tool-call parsing only.
3. Ensure recovery/profile/planning policy is not embedded in provider
   modules.
4. Update `docs/providers.md` if the boundary is clarified.

Exit criteria:

- Provider request plumbing is policy-free.
- Native and fallback tool-call handling remains transport-level only.

## Step 14: Focused Proof

Create focused fixture families only for rows with model-facing or
recovery-facing behavior changes. Implemented directory:

```text
eval/cases/focused/control-recovery/runtime-support/
```

Minimum candidate cases:

1. language mechanical repair unsupported/admitted family;
2. effective tool policy admitted/rejected category;
3. tool failure stale edit or prose-only correction;
4. setup command classification;
5. workspace candidate ignored-dir rejection;
6. scaffold artifact contract;
7. docs/data completion evidence;
8. answer-only gating;
9. provider boundary transport-only fixture if reportable offline.

Exit criteria:

- Every added fixture passes recheck.
- Any omitted fixture is justified in `focused_worklist.md`.

## Step 15: Documentation And Coverage

After row proof passes:

1. Update `docs/eval/legacy-control-stack-coverage-20260621.md` for proven
   C34-C44 rows.
2. Update KI-008 in `current_issue_phase_map.md`.
3. Update `recovery_plan.md` Phase29 exit status.
4. Update the loadmap2 `README.md` Phase29 row.
5. Add `implementation_report.md`.

Exit criteria:

- Documentation changes are proof-backed.
- Partial rows are not silently promoted.

## Step 16: Verification

Run:

```bash
cargo fmt --check
cargo test mechanical_repair
cargo test recovery_policy
cargo test recovery_orchestration
cargo test recovery_task
cargo test setup_lifecycle
cargo test setup_artifact_validation
cargo test workspace_snapshot
cargo test workspace_scope
cargo test artifact_graph
cargo test evidence_binding
cargo test completion_evidence
cargo test providers
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/runtime-support --out eval/runs/loadmap2-phase29-runtime-support-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py <phase29-focused-root> --cases-dir eval/cases/focused/control-recovery/runtime-support --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=<existing-smoke-root> --root focused=<existing-focused-root> --root supplemental=<phase29-focused-root> --root large=<existing-large-root>
cargo test
cargo build --release
```

If no focused fixtures are required for a row, record the reason in
`row_closure_matrix.md` and `focused_worklist.md`.

Exit criteria:

- Targeted tests pass.
- Focused recheck passes when fixtures are added.
- Broad sign-off passes or every new finding is mapped to a later phase with
  owner, proof, and rationale.
- Full local Rust verification passes.

## Step 17: Exit Review

1. Verify every C34-C44 row has final disposition.
2. Verify every split-forward has failed proof, owner, downstream phase, and
   closure condition.
3. Verify every exclusion has design rationale and no accepted migration gap.
4. Verify provider/profile/tool boundaries still match AGENTS.md.
5. Verify docs, coverage, and issue maps agree.

Exit criteria:

- Phase29 either closes KI-008 or narrows it into explicit downstream blockers.
- No row is left as unowned support work.

## Plan Review

Review findings applied:

- Reordered implementation so shared workspace/policy/setup/report contracts
  precede language/scaffold/non-coding adapters.
- Added explicit focused-fixture criteria instead of requiring fixtures for
  purely internal report/audit changes.
- Added provider boundary audit at the end to catch accidental policy leakage
  from earlier rows.
- Added split-forward and exclusion exit review rules.
