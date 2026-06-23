# Anvil Migration Completion Roadmap

Date: 2026-06-21
Last updated: 2026-06-23 JST

## Purpose

This roadmap replaces the earlier Phase1-Phase8 migration roadmap as the plan
for reaching a real "migration complete" state.

The previous roadmap proved an important point:

```text
foundation type / rendered evidence
  != runtime-effective migration
  != eval-proven migration
```

Phase8 made failures more visible, but many Anvil-derived responsibilities
remained partial. This roadmap is completion-driven: after the final phase,
every Anvil responsibility that CommandAgent should adopt must be implemented,
runtime-effective, and eval-proven, or explicitly excluded by design.

## Completion Definition

Anvil migration is complete only when all of the following are true:

1. Every source responsibility in
   `docs/eval/legacy-control-stack-coverage-20260621.md` has one of these final
   states:
   - `Implemented`: equivalent responsibility exists in CommandAgent and is
     enforced by runtime contracts or deterministic evaluators.
   - `Excluded`: intentionally not migrated and justified by architecture.
2. No adopted responsibility remains `Partial` or `Missing`.
3. Every adopted responsibility reaches stage 5:

| Stage | Name | Meaning |
| --- | --- | --- |
| 0 | Not started | No CommandAgent owner exists. |
| 1 | Foundation type | Type, enum, or struct exists but does not affect behavior. |
| 2 | Rendered evidence | Fact appears in evidence, repair packets, or eval fields. |
| 3 | Runtime decision | Fact selects or changes an active job, target, action, or policy. |
| 4 | Enforced contract | Guard, verifier, setup, profile, or repair flow enforces it. |
| 5 | Eval-proven | Focused eval or E2E proves the behavior against an observed failure. |

4. Focused control-recovery eval passes every adopted control path.
5. Broad local LLM eval has no unowned failure class:
   - no `unknown`
   - no raw `rc:1` without diagnostic classification
   - no `source_implementation_repair` fallback when a more specific owner is
     available
6. Reports can identify, from eval artifacts alone:
   - failing contract layer
   - active job
   - recovery owner
   - target path and role
   - selected repair action
   - tool policy
   - evidence binding status
   - completion evidence status
   - attempt outcome
   - explicit stop reason when no repair is admitted

An approved exception to stage 5 is allowed only when the responsibility is
implemented and enforced, but final proof is blocked by provider throughput,
model throughput, network, or environment constraints. The coverage table must
still mark the row as `Implemented` with an accepted external proof limitation,
or `Excluded` if the responsibility itself is not migrated. A missing owner,
missing action, missing target, or missing evidence can never be an approved
exception.

## Authority And Status Terms

When roadmap documents disagree, use this authority order:

1. `docs/eval/legacy-control-stack-coverage-20260621.md` is the final
   authority for coverage-row adoption and final row state.
2. `recovery_plan.md` is the authority for Phase17+ continuation rules,
   recovery gates, and blocker disposition.
3. Phase-local files under `phase_<N>/` are authoritative for that phase's
   assigned rows after they are created, but they must reconcile back to the
   coverage table and `recovery_plan.md`.
4. `current_issue_phase_map.md` is an index of known issues to future phases.
   It must be kept in sync, but it does not override coverage or recovery
   rules.
5. Earlier phase sections in this README are historical when they conflict with
   the Phase21+ recovery extension or `recovery_plan.md`.

Status terms:

- Coverage `Partial` means incomplete implementation proof. It is never a
  final state.
- Adoption `Partial` means scoped adoption: the adopted subset must become
  `Implemented`, and the omitted subset must become `Excluded` with rationale.
  It is not an undecided final state.
- `blocked_external` is a ledger disposition for provider, model-throughput,
  network, or environment proof limits after owner/action/evidence already
  exist. It is not a substitute for missing functionality.
- `split_forward` is a phase-local disposition that moves a narrower
  same-surface blocker to a named later phase with owner, proof, and closure
  condition. It is not migration completion.
- Broad sign-off is a phase-level regression and ownership gate. It can
  confirm that no unowned broad failure remains, but it cannot close a coverage
  row without row-specific deterministic, unit, focused, or E2E proof.

## Source Baseline

This roadmap uses the Anvil source inventory recorded in
`docs/eval/legacy-control-stack-coverage-20260621.md`:

| Field | Value |
| --- | --- |
| Anvil repository | `/Users/maenokota/share/work/github_kewton/Anvil-develop` |
| Anvil HEAD | `b3ca3d330546a10bf90d8dd46bd3e102f1710573` |
| Dirty state | dirty at inventory clarification time; fixed in `anvil_source_baseline.md` |

If the Anvil source checkout changes, refresh the coverage table before adding
new migration phases. Do not silently compare later work against a moving
source tree.

## Scope

This roadmap adopts all Anvil responsibilities marked `Adopt` or `Partial` in
the current coverage table, plus the previously visible `Missing` rows that are
needed for parity.

The migration target is not a byte-for-byte port. The target is functional
parity for the responsibilities that matter to CommandAgent:

```text
failure observation
  -> artifact ledger / scope / ownership
  -> completion evidence and evidence binding authority
  -> active job arbitration and dispatch
  -> target admission and prioritization
  -> semantic failure report and repair plan
  -> repair brief, action envelope, and tool policy
  -> bounded repair or setup/verifier action
  -> verifier/profile/evidence rerun
  -> pass or explicit safe stop
```

The minimal loop remains the only model execution loop. The migrated Anvil
mechanisms must be explicit contract and recovery-control layers around that
loop, not hidden autonomous workers.

## Excluded Surface

The following remain excluded unless a new ADR changes the product direction:

- legacy engine selector
- hidden or unbounded repair loop
- provider/model-specific behavioral policy
- model-issued implicit dependency installation as ordinary repair work
- PAM/Photon advisory systems
- case memory / anti-pattern corpora
- general working-memory reminder systems

These are not migration gaps.

## Migration Workstreams

The roadmap is organized into workstreams. Each workstream must end at stage 5
before migration can be called complete.

| Workstream | Responsibilities |
| --- | --- |
| A | Task contract, behavior obligations, task-kind inference, plan admission |
| B | Failure observation, terminal classification, eval lifecycle funnel |
| C | Artifact graph, role taxonomy, workspace scope, artifact ownership, ledger |
| D | Completion evidence, evidence binding, deliverable obligations |
| E | Active job arbiter, recovery owner, dispatch gate, loop control action |
| F | Setup bootstrap, manifest validation, scaffold/profile/dev-server jobs |
| G | Target admission, target prioritization, focused edit recovery |
| H | Semantic failure report, verifier diagnostic assessment, semantic repair plan |
| I | Repair brief, repair action space, action envelope, tool policy projection |
| J | Repair job state, attempt ledger, no-progress recovery, safe stop |
| K | Tool failure recovery and tool protocol correction |
| L | Verifier orchestration, verifier command policy, evidence rerun authority |
| M | Profile adapters and language-specific bounded repair adapters |
| N | Eval reporting, focused matrix, broad local LLM completion proof |

## Phase21+ Recovery Extension

Phase20 could not declare migration completion because accepted coverage rows
remained `Partial` or `Missing`. Phase21 split `P20-COV-001` into row-level
blockers. The remaining known work is assigned as follows:

| phase | source blocker | scope |
| --- | --- | --- |
| Phase22 | P20-COV-001 / C01-C03 | completed / closed_proven: task contract, request admission, behavior obligations |
| Phase23 | P20-COV-001 / C04-C06 | completed / closed_proven: artifact role, workspace scope, ownership |
| Phase24 | P20-COV-001 / C07-C10 | completed / closed_proven: artifact ledger, completion evidence, evidence binding, deliverable audit |
| Phase25 | P20-COV-001 / C11-C12 | completed / closed_proven: active-job arbitration and recovery dispatch |
| Phase26 | P20-COV-002 / C13-C20 | recovery task, setup/profile, semantic repair, repair brief, action envelope |
| Phase27 | P20-COV-003 / C21-C32 | target admission, verifier orchestration, repair lifecycle, completion, patch validation |
| Phase28 | P20-COV-004 / C33 | contract conflict job |
| Phase29 | P20-COV-005 / C34-C44 | language/profile/tool/workspace/runtime-support surface |
| Phase30 | P20-COV-006 / C49-C50 | quality and slash/plan UI priority decisions |
| Phase31 | P20-LEDGER-001 / P17-L001 | external timeout proof or explicit limitation |
| Phase32 | final closure | final coverage closure, broad sign-off, migration decision |

Detailed issue mapping is recorded in `current_issue_phase_map.md`.

## Phase22+ Execution Package

Phase22 and later must not start runtime changes from the summary table alone.
Each phase must first create a phase-local execution package under:

```text
workspace/mvp/logic/anvil/loadmap2/phase_<N>/
```

Required and conditional files:

| file | status | purpose |
| --- | --- | --- |
| `README.md` | always required | Phase scope, selected coverage rows, non-goals, design alignment, and exit gate. |
| `implementation_tasks.md` | always required | Checklist grouped by row-level blocker, including docs and eval tasks. |
| `concrete_work_plan.md` | always required | Ordered implementation plan with target modules, tests, focused eval, and rollback/split rules. |
| `source_alignment_matrix.md` | always required | One row per selected coverage ID mapping Anvil source files, adopted behavior, omitted behavior, CommandAgent target modules, and proof method. |
| `row_closure_matrix.md` | always required | One row per coverage responsibility with owner, target, proof, and disposition. |
| `blocking_ledger.md` | always required | Row-level blockers with incomplete contract, suspected module, proof command, and closure condition. |
| `reconciliation.md` | always required | Mapping from Phase20/Phase21 blocker to coverage row, implementation task, proof command, and broad sign-off. |
| `focused_worklist.md` | conditionally required | Required only when model-facing behavior changes, focused eval assertions change, or focused proof is part of the exit gate. |
| `implementation_report.md` | required at closure | Final phase result, proof commands, row dispositions, remaining blockers, and review result. |

Runtime code changes are not allowed until the phase has at least:

- `row_closure_matrix.md`
- `source_alignment_matrix.md`
- `blocking_ledger.md`
- `reconciliation.md`

Each phase must also record whether every assigned row is:

- `closed_proven`
- `excluded_with_rationale`
- `blocked_external` with owner/action/evidence, only where allowed
- `split_forward` to a narrower same-surface blocker with failed proof evidence

No phase may close from a summary paragraph, CI success, or broad sign-off
alone.

Before Phase32, `split_forward` may close a phase only when the split blocker
is narrower than the original row, stays on the same responsibility surface,
names a later phase, and includes failed proof evidence. At Phase32,
`split_forward` means migration is not complete unless the roadmap is extended
with a new distinct responsibility class and Phase32 records
`migration_not_complete`.

## Historical Phase 0: Rebaseline And Freeze Completion Table

Objective:

Turn the current coverage document into the authoritative migration ledger.

Deliverables:

- Update `docs/eval/legacy-control-stack-coverage-20260621.md`.
- Add final columns:
  - `target_final_state`
  - `current_stage`
  - `required_stage`
  - `owner_module`
  - `blocking_dependency`
  - `focused_eval_case`
  - `completion_evidence`
- Split rows that hide multiple runtime responsibilities.
- Reclassify all rows currently marked `Missing`:
  - adopt into this roadmap, or
  - exclude with explicit design rationale.

Acceptance:

- No row uses `Partial` without a concrete stage, owner, and completion test.
- The final migration surface is explicit before more runtime code is changed.

## Historical Phase 1: Task Contract And Behavior Obligation Authority

Workstreams:

- A

Adopted Anvil responsibilities:

- Task contract core
- Task contract inference and admission
- Objective and behavior contract projection
- Artifact role taxonomy
- Deliverable obligation audit foundations

Implementation goals:

- Add a common `TaskContract` shape for slash-command plans, ultra plans, and
  repair tasks.
- Derive required behavior obligations from user request, profile, and plan.
- Connect behavior obligations to plan lint and profile verification.
- Make artifact roles a shared SSOT used by plan lint, profile, verifier,
  target admission, and eval.

Acceptance:

- Plan generation cannot omit required artifacts or profile-critical behavior
  without a deterministic plan-lint failure.
- A route integration request, dependency/setup request, docs literal request,
  data schema request, and test-artifact request each produce distinct
  obligations.
- Focused eval proves every obligation type.

## Historical Phase 2: Failure Observation And Terminal Classification

Workstreams:

- B

Adopted Anvil responsibilities:

- Failure packet / safe-stop payload
- Terminal state classification
- Provider, plan, profile, tool, setup, verifier, and eval success failures

Implementation goals:

- Add `FailureObservation` as the common payload for every failure source.
- Normalize failure classification before repair:
  - provider transport
  - plan parse
  - plan lint
  - step policy
  - tool protocol
  - setup artifact
  - dependency setup
  - profile verification
  - verifier command
  - eval success contract
  - completion evidence
  - evidence binding
- Remove raw `rc:1` from top-level reports unless paired with a diagnostic
  code and failure signature.

Acceptance:

- Every failure in smoke, focused, and large eval has terminal state,
  contract layer, violated contract, diagnostic code, and source of truth.
- Reports no longer require manual stderr inspection to identify the owning
  layer.

## Historical Phase 3: Artifact Ledger, Workspace Scope, And Ownership

Workstreams:

- C

Adopted Anvil responsibilities:

- Task workspace scope
- Artifact ownership
- Artifact ledger
- Workspace candidates and walk
- Post-tool reconciliation
- Scaffold delta recording
- Verifier observation recording

Implementation goals:

- Add a scope-aware workspace walk that ignores dependency/cache/generated
  paths.
- Record:
  - observed files
  - read files
  - written/edited files
  - scaffold-created files
  - verifier-mentioned files
  - setup manifests
  - generated/cache/dependency files
- Classify artifacts as:
  - owned
  - candidate-only
  - out-of-scope
  - verifier-owned
  - scaffold-owned
  - dependency/cache/generated
- Feed ledger facts into target admission, repair briefs, setup/profile
  contracts, and eval reports.

Acceptance:

- A repair cannot target dependency/cache/generated/out-of-scope files unless a
  specific setup/verifier contract owns that path.
- Eval reports show the artifact ledger state for failed and repaired cases.
- Route, manifest, source, test, docs, and data targets are distinguished by
  role and ownership.

## Historical Phase 4: Completion Evidence And Evidence Binding Authority

Workstreams:

- D

Adopted Anvil responsibilities:

- Completion evidence
- Evidence binding
- Deliverable lifecycle
- Deliverable freshness
- Evidence runner binding

Implementation goals:

- Make completion evidence authoritative for step/phase completion.
- Add pass-side producers:
  - repo edit evidence
  - docs section evidence
  - data schema evidence
  - report completeness evidence
  - manifest identity evidence
  - route binding evidence
  - import binding evidence
  - executable handle evidence
  - test script binding evidence
- Add fail-side producers for verifier/profile/setup/tool failures.
- Distinguish:
  - missing deliverable
  - missing evidence
  - unbound evidence
  - failed evidence
  - stale evidence

Acceptance:

- A file existing is not enough to pass if required evidence is missing or
  unbound.
- Pass-side focused cases report `evidence_binding_status=bound` and
  `completion_evidence_status=passed`.
- Missing evidence and missing artifact are different terminal states.

## Historical Phase 5: Active Job Arbiter And Dispatch Gate

Workstreams:

- E

Adopted Anvil responsibilities:

- Active job arbiter
- Recovery owner / dispatch gate
- Loop control action
- Repair job dispatch
- Artifact recovery flow

Implementation goals:

- Select exactly one recovery owner before repair prompt construction.
- Implement deterministic owner candidates:
  - setup bootstrap
  - manifest repair
  - scaffold materialization
  - route integration repair
  - source implementation repair
  - test artifact completion
  - test alignment repair
  - documentation repair
  - evidence binding repair
  - verifier contract correction
  - tool protocol correction
  - contract conflict
  - explicit stop
- Add dispatch gate so competing owners cannot act in the same repair turn.
- Add tie-safe explicit stop when ownership is ambiguous.

Acceptance:

- A single failure cannot simultaneously behave as setup, source, and profile
  repair.
- Focused cases prove correct active job for setup, manifest, route,
  source, test, docs, tool protocol, evidence binding, and contract conflict.

## Historical Phase 6: Setup, Manifest, Scaffold, Profile, And Dev-server Jobs

Workstreams:

- F

Adopted Anvil responsibilities:

- Setup bootstrap
- Setup artifact validation
- Project probe/profile/scaffold profile
- Profile failure to recovery job
- Scaffold pipeline
- Bash/setup command classification

Implementation goals:

- Treat setup as an explicit active job with lifecycle:
  - setup artifact validation
  - setup readiness
  - setup command authority
  - setup execution result
  - verifier rerun result
- Validate Node, Rust, and Python manifests.
- Add profile-output schema shared by Next.js, Python, Rust, docs, and data.
- Connect profile failures to specific recovery jobs:
  - manifest repair
  - setup bootstrap
  - scaffold materialization
  - route integration repair
  - source implementation repair
  - dev-server smoke
  - explicit stop
- Add dev-server job state:
  - requested port
  - port preflight
  - server command
  - endpoint smoke result
  - port conflict classification

Acceptance:

- `EADDRINUSE` is classified as port conflict, not app/build failure.
- Dependency setup, manifest repair, route integration, and endpoint smoke are
  separate active jobs.
- Next.js, Python, and Rust profile failures map to common recovery jobs rather
  than profile-specific hidden workflows.

## Historical Phase 7: Target Admission, Prioritization, And Focused Edit Recovery

Workstreams:

- G

Adopted Anvil responsibilities:

- Repair target decision/admission
- Repair target prioritization
- Focused edit recovery
- Recovery targets
- Verifier repair targeting

Implementation goals:

- Admit targets only when ownership, scope, role, and source-of-truth match the
  selected active job.
- Prioritize targets by:
  - failure kind
  - source of truth
  - artifact role
  - edited/read history
  - scaffold/verifier ownership
  - exhausted target and role history
- Add focused edit recovery as target-admission evidence:
  - use previously read/editable target when safe
  - require current excerpt before patch-like edit
  - reject stale edit targets

Acceptance:

- Wrong-target repair is rejected before model execution.
- Route failures target route/integration artifacts, not arbitrary source.
- Test-owned failures do not default to source repair unless spec authority
  says implementation is wrong.

## Historical Phase 8: Semantic Failure Report And Verifier Diagnostics

Workstreams:

- H
- L

Adopted Anvil responsibilities:

- Semantic failure report
- Semantic repair planning
- Verifier diagnostic assessment
- Verifier diagnostic payload
- Verifier weak reason and weak target filters
- Verifier command policy

Implementation goals:

- Convert verifier/profile/eval failures into semantic clusters:
  - failure kind
  - observed/expected pairs
  - affected cases
  - source of truth
  - contract conflict
  - preferred repair role
  - candidate targets
  - admitted cluster targets
  - confidence
- Add diagnostic parsers for common failure shapes:
  - import/module missing
  - assertion mismatch
  - route not integrated
  - package/dependency missing
  - compile/type error
  - command not found
  - port in use
  - generated test weakness
  - self-referential verifier
- Reject weak or self-referential verifier commands.

Acceptance:

- Large FastAPI/Rust verifier failures are no longer reported only as `rc:1`.
- Repair can choose implementation vs test vs setup vs docs target based on
  semantic failure and source-of-truth authority.

## Historical Phase 9: Semantic Repair Plan, Repair Brief, And Action Envelope

Workstreams:

- H
- I

Adopted Anvil responsibilities:

- Semantic repair plan
- Repair brief
- Repair action space
- Repair plan admission
- Repair authority
- Tool policy and effective policy

Implementation goals:

- Build one repair plan per selected failure cluster:
  - selected cluster
  - selected target
  - repair role
  - repair hypothesis
  - expected improvement
  - must-preserve constraints
  - allowed change kind
  - disallowed actions
  - allowed tool category
  - rerun authority
- Validate the repair plan before invoking the minimal loop.
- Project owner/action-aware tool policy into the repair task.

Acceptance:

- Repair prompts contain concrete target, action, forbidden changes, and
  success check.
- A repair action that conflicts with target role or source-of-truth is
  rejected before the model turn.
- Tool policy differs correctly for setup, manifest, source, docs, test,
  evidence binding, and tool protocol correction jobs.

## Historical Phase 10: Repair State, Attempt Ledger, And No-progress Recovery

Workstreams:

- J

Adopted Anvil responsibilities:

- Repair job state machine
- Repair attempt ledger
- No-progress recovery
- Repair progress
- Verifier repair pass flow
- Safe stop payload

Implementation goals:

- Persist repair job state during a step/phase:
  - active job
  - selected cluster
  - current target
  - current target role
  - before/after signatures
  - verifier command
  - changed files
  - attempt outcome
  - exhausted targets
  - exhausted roles
  - exhausted clusters
- Detect no-progress, duplicate, noop, malformed, worsened, improved-still
  failing, passed, and explicit-stop outcomes.
- Add bounded strategy switching:
  - switch target
  - switch role
  - route to evidence binding
  - route to contract conflict
  - scaffold rebuild
  - explicit stop

Acceptance:

- Repeated failed repair cannot keep editing the same target/role without
  strategy change or explicit stop.
- No-progress focused case proves target/role exhaustion and strategy switch.
- Explicit-stop focused case reports safe-stop payload and attempt ledger.

## Historical Phase 11: Tool Failure Recovery And Protocol Correction

Workstreams:

- K

Adopted Anvil responsibilities:

- Tool failure recovery
- Protocol failure handling
- Reply retry note
- Tool protocol correction

Implementation goals:

- Make tool protocol failure a first-class recovery owner/action.
- Distinguish:
  - malformed tool call
  - missing required field
  - invalid path
  - stale edit target
  - prose-only response where tool was required
  - provider transport parse failure
- Allow one bounded correction when deterministic and safe.
- If a repo-changing task still has no changed files after protocol failure,
  escalate to the appropriate deliverable or artifact-completion recovery job.
- Stop explicitly when protocol correction is exhausted.

Acceptance:

- Missing `Write.path` and stale `Edit.old` failures produce structured
  protocol evidence, one correction if allowed, then deliverable recovery or
  explicit stop.
- Tool protocol failures are not misreported as source implementation failures.

## Historical Phase 12: Patch Validation And Bounded Mechanical Repair

Workstreams:

- I
- J
- M

Adopted Anvil responsibilities:

- Repair patch validation
- Repair patch executor boundary
- Test weakening filter
- Deterministic fallback plan
- Mechanical compile/import/dependency repair adapters

Implementation goals:

- Add patch validation around model-produced edits:
  - unsafe
  - malformed
  - noop
  - duplicate
  - test weakening
  - out-of-scope
  - worsened verifier
- Add bounded mechanical repair adapters only after action/target contracts are
  authoritative:
  - Rust compile/import/dependency hints
  - Python import/test/assertion hints
  - Node/Next manifest/type/route hints
- Keep deterministic fallback visible and bounded; no hidden mutation loop.

Acceptance:

- Test weakening is rejected.
- Worsened patch can be reported and, where safe, rolled back only with
  verifier evidence.
- Mechanical adapters are invoked only from admitted repair actions.

## Historical Phase 13: Profile And Language Adapter Parity

Workstreams:

- F
- M

Adopted Anvil responsibilities:

- Project probe/profile/scaffold profile
- Data/docs/research/ops evidence, where relevant to CommandAgent profiles
- Language-specific repair evidence adapters

Implementation goals:

- Generalize profile output:
  - project kind
  - expected manifests
  - entrypoints
  - route/integration contracts
  - setup commands
  - verifier commands
  - completion evidence requirements
  - profile failure mappings
- Bring Next.js, Python/FastAPI, Rust, docs, and data profiles to the same
  structural contract.
- Add profile-specific adapters only behind common contracts.

Acceptance:

- Next.js, Python, and Rust large cases fail or pass through the same common
  contract fields.
- No profile uses a hidden workflow engine.
- Profile-specific logic is limited to facts, adapters, and verifier hints.

## Historical Phase 14: Runtime Job Reporting And Eval Lifecycle Funnel

Workstreams:

- B
- N

Adopted Anvil responsibilities:

- Job report / progress events
- Lifecycle funnel
- Completion/evidence recheck
- Report projection

Implementation goals:

- Add job-level report schema:
  - lifecycle stage
  - active owner
  - selected action
  - target admission status
  - repair action plan status
  - attempt outcome
  - evidence runner status
  - verifier rerun result
  - explicit stop reason
- Update `summary.tsv`, `recheck_summary.tsv`, `meta.json`, and report output.
- Ensure dry-run reports cannot be confused with runtime success.

Acceptance:

- Eval output alone shows where the workflow stopped.
- Recheck reports distinguish existing success, runtime success, dry-run
  placeholder success, and evidence-only success.

## Historical Phase 15: Focused Control-recovery Matrix Completion

Workstreams:

- all

Objective:

Prove every adopted control path independently before broad eval is used as a
quality signal.

Required focused cases:

- task contract admission
- behavior obligation projection
- plan parser block scalar `>-`
- tool protocol missing required field
- stale edit target
- missing artifact completion
- missing evidence
- evidence binding failure
- route integration repair
- Next.js dependency setup
- Next.js manifest repair
- Next.js dev-server port conflict
- Next.js endpoint smoke
- Python import binding
- Python missing test artifact
- FastAPI assertion mismatch
- Rust Cargo verifier binding
- Rust compile diagnostic repair target
- docs literal mismatch
- data schema completion
- generated test weakening rejection
- no-progress target switch
- contract conflict explicit stop
- setup manifest invalid
- out-of-scope target rejection
- focused edit recovery

Acceptance:

- All focused cases pass with real local LLM or deterministic fixture where the
  case is specifically about malformed model output.
- Every case records terminal state, active job, recovery owner, target/action,
  attempt outcome, evidence binding, completion evidence, and final result.

## Historical Phase 16: Broad Local LLM Migration Sign-off

Workstreams:

- N

Objective:

Demonstrate that the migrated control stack works on broad tasks without
hidden retries, provider-specific behavior, or verifier weakening.

Required eval:

- `eval/cases/smoke`
- `eval/cases/focused/control-recovery`
- `eval/cases/large`
- local LLM primary run
- optional Gemini/OpenAI supplemental runs, recorded separately
- normal and `--recheck` reports for every root

Acceptance:

- Focused control-recovery: all adopted focused assertions pass.
- Smoke: no unowned failure.
- Large: every failure is layer-owned and actionable; no raw `rc:1`.
- Large Next.js, Python/FastAPI, and Rust failures either converge or stop with
  specific active job, target/action, and evidence.
- No failure falls back to generic `source_implementation_repair` when a more
  specific owner is available.

## Historical Phase 17: Recovery Rebaseline And Blocking Ledger

Objective:

Recover from the Phase 16 broad sign-off failure by turning every sign-off
blocker into an owned blocking ledger row before more migration work is called
complete.

Phase 17 is not a declaration phase. It exists because Phase 16 proved that the
roadmap had an execution-process gap: phase implementation was treated as
complete even though the migration sign-off remained red. From Phase 17 onward,
a phase cannot close unless every blocking row assigned to that phase reaches
its exit gate.

Deliverables:

- Add a recovery ledger derived from the Phase 16 report:
  - focused assertion mismatches
  - focused raw diagnostic rows
  - large timeout rows
  - large generic source fallback rows
  - missing owner/action/target/evidence rows
- Add a sign-off reconciliation table that maps every checker finding to:
  - blocking ledger row
  - coverage responsibility
  - downstream phase
  - proof command
- For every blocker, record:
  - owning layer
  - failed contract
  - current behavior
  - expected behavior
  - responsible phase
  - proof command / eval case
  - pass condition
- Update the roadmap so sign-off failure becomes a hard gate, not a follow-up.
- Add Phase 17 planning docs under
  `workspace/mvp/logic/anvil/loadmap2/phase_17/`.

Acceptance:

- The Phase 16 sign-off failure can be explained entirely from the recovery
  ledger.
- The current sign-off finding count reconciles exactly with
  `phase_17/signoff_reconciliation.md`.
- No reconciliation row lacks a coverage responsibility, downstream phase, or
  proof command.
- No blocker is described only as "rerun eval" or "model quality" unless the
  row already has owner/action/target/evidence and an explicit environment or
  provider limitation.
- The next runtime phase has a finite blocking set and a proof command for
  each item.
- "Phase complete" is redefined as "implementation complete plus assigned
  blockers pass their proof gate".

## Historical Phase 18: Focused Sign-off Recovery

Objective:

Clear the focused control-recovery blockers exposed by Phase 16.

Scope:

- `focused-docs-literal-mismatch`
- `focused-nextjs-dependency-setup`
- `focused-nextjs-endpoint-smoke`
- `focused-nextjs-route-integration`
- any focused row with raw `rc:*`, unknown contract, wrong owner, or failed
  expected assertion

Implementation goals:

- Repair the responsible contract layer for each focused blocker:
  - plan lint / plan parser when the step plan is invalid
  - setup/profile contract when setup should complete
  - recovery task contract when explicit stop is chosen instead of admitted
    repair
  - eval assertion fixture when the expected row is stale or wrong
- Rerun only the affected focused cases first.
- Rerun the full focused matrix only after targeted cases pass.

Acceptance:

- Focused control-recovery has zero failed expected assertions.
- No focused row reports raw `rc:*` without diagnostic classification.
- Recheck output remains consistent with the original focused proof.

## Historical Phase 19: Large Ownership And Evidence Recovery

Objective:

Clear the large broad-signoff ownership blockers without treating timeout or
model throughput as implementation success.

Scope:

- `provider_transport:eval_timeout` rows
- large rows missing evidence binding or completion evidence
- large rows missing active job, owner, action, or target where applicable
- profile failures mapped to generic source repair instead of manifest, setup,
  route, verifier, evidence, or explicit stop

Implementation goals:

- Make timeout rows first-class provider/eval boundary evidence with explicit
  owner and not-applicable completion semantics.
- Ensure large failed rows have:
  - terminal state
  - contract layer
  - active job
  - recovery owner
  - repair action or explicit stop
  - target path/role or a clear not-applicable reason
  - evidence binding status
  - completion evidence status
  - attempt outcome
- Rerun large with a practical release timeout or approved faster local coding
  model only after evidence fields are complete.

Acceptance:

- Large broad eval has no unowned failure and no generic source fallback when a
  more specific owner is available.
- Timeout rows are blockers or provider/eval limitations, not ambiguous
  migration gaps.
- The sign-off checker no longer flags missing owner/action/target/evidence
  for large rows.

## Historical Phase 20: Final Coverage Closure And Migration Complete Declaration

Historical note:

This was the original final-closure phase. Phase20 did not satisfy the
completion definition; it produced the `migration_not_complete` decision and
the continuation ledger that led to Phase21+. Read this section as the
historical closure target that is now superseded by Phase32.

Objective:

Close the migration ledger and declare migration complete only if the evidence
supports it.

Deliverables:

- Update `docs/eval/legacy-control-stack-coverage-20260621.md`:
  - all adopted rows become `Implemented`
  - all excluded rows include rationale
  - no `Partial`
  - no unowned `Missing`
- Add final report:
  - `docs/eval/anvil-migration-complete.md`
- Update:
  - `docs/philosophy.md`
  - `docs/architecture.md`
  - `docs/ultra-plan-run.md`
  - `docs/evaluation.md`
  - `docs/known-limitations.md`
  - `docs/adr/0002-contract-recovery.md`
- Record final local LLM and supplemental eval roots.

Acceptance:

- The final report can answer:
  - what was migrated
  - what was intentionally excluded
  - what eval proves each control path
  - which broad limitations remain and why they are not Anvil migration gaps
- The phrase "Anvil migration complete" is not used unless every adopted row is
  `Implemented` and eval-proven.
- Phase 16 broad sign-off checker passes on the final evidence roots.

## Final Migration Checklist

Migration is complete only when every item is checked:

- [ ] Coverage table has no adopted `Partial` rows.
- [ ] Coverage table has no adopted `Missing` rows.
- [ ] Every adopted row is at stage 5 or has an approved exception.
- [ ] Every excluded row has design rationale.
- [ ] Focused control-recovery matrix passes.
- [ ] Broad local LLM eval has no unowned terminal state.
- [ ] No raw `rc:1` appears without diagnostic classification.
- [ ] No profile failure is disconnected from recovery job selection.
- [ ] No evidence/completion success is claimed without bound evidence.
- [ ] No repair prompt is built without selected owner, target, action, tool
  policy, and rerun authority.
- [ ] No repeated no-progress repair continues without strategy switch or
  explicit stop.
- [ ] Docs and ADRs reflect the final architecture.

## Review Notes

Review findings applied in this roadmap:

- The earlier Phase1-Phase8 plan was measurement-heavy and could be mistaken
  for a completion plan. This version defines completion first.
- `Partial` is no longer acceptable as an end state. It must become
  `Implemented`, or the responsibility must be explicitly excluded.
- Runtime-effective behavior and eval proof are required for every adopted
  responsibility.
- The roadmap intentionally includes previously deferred rows such as focused
  edit recovery, no-progress recovery, contract conflict, patch validation, and
  language adapters because current eval results show those gaps are relevant.
- The plan keeps CommandAgent's design boundary: one minimal loop, explicit
  contracts, bounded repair, no hidden provider-specific behavior.
