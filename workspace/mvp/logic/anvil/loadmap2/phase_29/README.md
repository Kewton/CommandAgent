# Loadmap2 Phase29 Plan

Date: 2026-06-23 JST

Status: completed / closed_proven

Implementation proof root:

```text
eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335
```

Broad sign-off:

```text
python3 scripts/eval_signoff.py --require-recheck ... --root supplemental=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 ...
status: pass
```

## Scope

Phase29 closes `P20-COV-005` / KI-008:

| row | responsibility |
| --- | --- |
| C34 | Language-specific mechanical repair families and live proof. |
| C35 | Owner/action-aware tool policy and effective policy across setup, evidence, and repair jobs. |
| C36 | Tool failure recovery for provider parse, stale edit, prose-only, and schema branches. |
| C37 | Bash/setup command classification with evidence binding and setup command authority. |
| C38 | Workspace candidates/walk with scope-aware ignored-dir single source of truth. |
| C39 | Job report/progress events with active owner, action plan status, and attempt outcomes. |
| C40 | Scaffold pipeline as setup/artifact contract, not hidden workflow engine. |
| C41 | Data/docs/research/ops evidence via generic completion/binding producers. |
| C42 | Answer-only and work-mode gating without broadening normal coding repair. |
| C43 | Interruption, lifecycle, and turn state only where explicit recovery contracts require it. |
| C44 | Provider/model request plumbing kept transport-only and policy-free. |

Phase29 is intentionally broad but not a permission to add another control
stack. It closes runtime-support parity by tightening existing CommandAgent
contracts and adding row-specific proof. If a row cannot be closed without a
new unplanned subsystem, it must be split forward with failed proof evidence
or excluded with design rationale.

Implementation result: all C34-C44 rows are closed at CommandAgent's accepted
bounded contract boundary. Phase29 adds deterministic support projections and
report fields; it does not add an Anvil-style actor loop, hidden continuation,
implicit setup execution, provider-owned policy, or profile workflow engine.

## Problem Statement

After Phase28, the core recovery path can select owner/action/target,
diagnostic, conflict, and patch outcomes. Remaining parity gaps are around the
supporting surfaces that make those contracts reliable across languages,
tools, workspace discovery, reports, scaffold/data/docs tasks, lifecycle, and
provider request plumbing.

The recurring risk is that these rows are easy to treat as miscellaneous
helpers. Phase29 must not do that. Each row needs a clear owner layer,
admissible behavior, and proof that the behavior is visible to recovery/eval
without creating hidden continuation or provider/model policy.

## Non-goals

- Do not import Anvil's actor loop, advisory memory, or hidden continuation
  behavior.
- Do not add provider/model-specific behavioral policy while working on C44.
- Do not let C40 scaffold support become a hidden workflow engine.
- Do not broaden answer-only gating into ordinary coding repair suppression.
- Do not implicitly run dependency/network setup from C37.
- Do not mark a row `Implemented` from docs, CI, or broad sign-off alone.
- Do not close C49/C50 quality or slash/plan UI decisions in Phase29.

## Design Alignment

Phase29 follows the same bounded contract shape as Phase22 through Phase28:

```text
deterministic evidence
  -> row-owned contract/support data
  -> existing active-job / recovery-task / verifier / eval surfaces
  -> bounded repair action, explicit safe stop, or row-level split/exclusion
```

The minimal loop remains the only executor. Profiles remain fact providers.
Providers remain transport. Tool policy remains a projection of the selected
owner/action, not a planner. Eval remains evidence and reporting, not runtime
tuning.

## Architecture Shape

Phase29 should add or complete small shared boundaries only when they are
consumed by multiple rows:

| boundary candidate | rows | intent |
| --- | --- | --- |
| `MechanicalRepairAdapter` completion | C34 | language family hints and admitted mechanical proposals without direct mutation |
| effective tool policy projection | C35, C36, C37 | owner/action/job to allowed tool category and disallowed action evidence |
| tool failure recovery facts | C36 | parse/schema/prose/stale-edit failures as bounded correction jobs |
| setup command authority | C37 | command classification and setup evidence binding without implicit install |
| workspace candidate discovery | C38 | scope-aware path discovery and ignored-dir single source of truth |
| job event report schema | C39 | active owner, action status, attempt outcome, lifecycle transition |
| scaffold contract facts | C40 | scaffold as setup/artifact obligations, not an engine |
| non-coding evidence producers | C41 | generic docs/data/research/ops completion and binding facts |
| answer/work mode gate | C42 | deterministic answer-only vs work-mode admission |
| lifecycle state projection | C43 | explicit interruption/turn state only where recovery needs it |
| provider request boundary audit | C44 | transport-only request construction and prompt boundary tests |

If a shared boundary starts to become a workflow engine, split it back into row
contracts before implementation.

## Row Strategy

| row | plan disposition target | implementation stance |
| --- | --- | --- |
| C34 | `closed_proven` or split by language family | Complete adapters only for deterministic Rust/Python/TypeScript/Next.js families that have diagnostic evidence and focused proof. |
| C35 | `closed_proven` | Broaden tool policy projection from selected owner/action to setup, evidence binding, repair, tool-protocol, and verifier-contract actions. |
| C36 | `closed_proven` or split live-provider E2E | Keep bounded tool-correction packets; add stale edit/prose/schema/provider-parse coverage without retry expansion. |
| C37 | `closed_proven` | Classify Bash/setup commands and setup authority; preserve explicit setup policy. |
| C38 | `closed_proven` | Make workspace candidate discovery use one ignored-dir/scope source shared by ownership, target admission, and recovery. |
| C39 | `closed_proven` | Add job/progress report fields to eval/runtime evidence; no spinner/UI-only parity requirement. |
| C40 | `closed_proven` or split profile-specific scaffold | Treat scaffold as setup/artifact obligations and completion evidence. |
| C41 | `closed_proven` or split non-coding profile families | Add generic docs/data/research/ops evidence producers before profile-specific expansion. |
| C42 | `closed_proven` | Deterministic answer-only/work-mode gating with no broad normal-coding suppression. |
| C43 | `closed_proven` or excluded-with-rationale for actor-loop-only details | Add lifecycle state only where explicit recovery contracts need it. |
| C44 | `closed_proven` | Audit and test provider request plumbing so behavior policy stays outside transports. |

Final row dispositions:

| row | disposition | proof |
| --- | --- | --- |
| C34 | `closed_proven` | `language_repair_adapter_status=projected`; focused case `phase29-language-repair-adapter`; `cargo test runtime_support --lib`. |
| C35 | `closed_proven` | `effective_tool_policy=file_mutation_repair`; focused case `phase29-effective-tool-policy`; `cargo test recovery_orchestration --lib`. |
| C36 | `closed_proven` | `tool_failure_recovery_status=bounded_correction`; focused case `phase29-tool-failure-recovery`; `python3 tests/test_eval_report.py`. |
| C37 | `closed_proven` | deterministic shell command classification; focused case `phase29-setup-command-classification`; `cargo test command_classification --lib`; `cargo test setup_lifecycle --lib`. |
| C38 | `closed_proven` | workspace candidate and ignored-dir policy fields; focused case `phase29-workspace-candidate-policy`; `cargo test workspace_snapshot --lib`. |
| C39 | `closed_proven` | `job_report_status` and `job_report_owner_action`; focused case `phase29-job-report`; report recheck. |
| C40 | `closed_proven` | `scaffold_contract_status=artifact_obligation`; focused case `phase29-scaffold-contract`; report recheck. |
| C41 | `closed_proven` | `noncoding_evidence_status=generic_producer`; focused case `phase29-noncoding-evidence`; report recheck. |
| C42 | `closed_proven` | `answer_work_mode_status=deterministic_gate`; focused case `phase29-answer-work-mode`; report recheck. |
| C43 | `closed_proven` | `lifecycle_projection_status=selected`; focused case `phase29-lifecycle-projection`; report recheck. |
| C44 | `closed_proven` | `provider_boundary_status=transport_only`; focused case `phase29-provider-boundary`; report recheck. |

## Cross-phase Boundaries

| adjacent phase | boundary |
| --- | --- |
| Phase22-24 | Task, artifact, ledger, completion, and evidence contracts are inputs. Phase29 must reuse them rather than creating alternate producers. |
| Phase25-28 | Active-job, recovery task, target/verifier/patch/conflict contracts are consumers. Phase29 support data must project into these existing contracts. |
| Phase30 | Quality and slash/plan UI helpers remain unresolved priority decisions. Phase29 may expose data used later but must not decide C49/C50. |
| Phase31 | Timeout/external proof remains separate. Phase29 cannot hide timeouts by adding continuation. |
| Phase32 | Final closure is still separate. Phase29 may only close C34-C44. |

## Horizontal Expansion

Phase29 must avoid Next.js-only support work. Required horizontal checks:

- Rust, Python, TypeScript/Next.js mechanical repair evidence for C34.
- Setup/tool command classification across Node, Cargo, and Python for C37.
- Workspace candidate discovery across greenfield, existing single-project,
  nested project, ignored dependency/cache/build output, and raw data paths for
  C38.
- Docs/data/research/ops evidence through generic producers for C41.
- Provider request boundary tests across Ollama, Gemini, and OpenAI for C44
  where local tests can do so without network calls.

## Documentation Updates

Runtime changes in Phase29 must update the smallest applicable docs:

- `docs/architecture.md` for new or completed support boundaries.
- `docs/adr/0002-contract-recovery.md` for tool policy, setup command,
  lifecycle, and provider-boundary implications.
- `docs/evaluation.md` and `eval/README.md` for new eval/report fields or
  focused fixture expectations.
- `docs/profiles.md` if profile/scaffold/non-coding evidence contracts change.
- `docs/providers.md` if provider request plumbing or native/fallback request
  shape is clarified.
- `docs/eval/legacy-control-stack-coverage-20260621.md` only after row proof.
- `workspace/mvp/logic/anvil/loadmap2/*` roadmap files only after proof or
  explicit split/exclusion.

## Required Proof

Minimum local proof before any C34-C44 row can be marked `closed_proven`:

- row-specific unit tests for the changed module;
- `python3 tests/test_eval_report.py` when eval/report fields change;
- focused deterministic fixtures for model-facing or recovery-facing behavior;
- focused recheck for the Phase29 fixture root;
- broad sign-off with the Phase29 root added as supplemental evidence;
- `cargo fmt --check`;
- `cargo test`;
- `cargo build --release`.

Executed proof:

- `cargo test command_classification --lib`
- `cargo test runtime_support --lib`
- `cargo test setup_lifecycle --lib`
- `cargo test workspace_snapshot --lib`
- `cargo test recovery_orchestration --lib`
- `python3 tests/test_eval_report.py`
- `scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/runtime-support --out eval/runs/loadmap2-phase29-runtime-support-fixtures --runs 1 --proof-mode deterministic_fixture`
- `python3 scripts/eval_report.py eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 --cases-dir eval/cases/focused/control-recovery/runtime-support --recheck`
- `python3 scripts/eval_signoff.py --require-recheck ... --root supplemental=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 ...`

Final repository checks are recorded in `implementation_report.md`.

Rows that do not change model-facing behavior may close with targeted unit
tests and broad sign-off only if `row_closure_matrix.md` explains why no
focused fixture is required.

## Exit Gate

Phase29 can close only when each C34-C44 row is one of:

- `closed_proven` with row-specific proof;
- `excluded_with_rationale` with design boundary and proof that the exclusion
  does not leave an accepted migration gap;
- `blocked_external` only for allowed provider/model-throughput/network or
  environment proof limits after owner/action/evidence exist;
- `split_forward` to a narrower same-surface blocker with failed proof,
  owner, downstream phase, and closure condition.

The phase cannot close by saying the rows are miscellaneous support. KI-008
can close only after the row matrix and coverage table are reconciled.

## Plan Review

Review findings applied:

- Split the broad Phase29 surface into eleven row-owned closure paths instead
  of one runtime-support bucket.
- Added explicit non-goals to prevent scaffold, provider plumbing, lifecycle,
  or tool policy from becoming hidden orchestration.
- Required horizontal checks for language, setup, workspace, non-coding, and
  provider surfaces.
- Required focused proof only for model-facing/recovery-facing behavior, while
  allowing pure reporting/boundary audits to justify unit-test-only proof.
- Kept C49/C50 and Phase31/32 decisions out of Phase29 scope.
