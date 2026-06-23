# Phase29 Row Closure Matrix

Date: 2026-06-23 JST

| coverage id | current status | adoption | owner layer | missing contract | target modules | required proof | closure condition | planned disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C34 | Implemented | Adopt | mechanical repair / verifier diagnostic | Closed: language adapter status is projected from deterministic mechanical evidence. | `runtime_support.rs`, existing mechanical adapter/verifier diagnostic facts | `cargo test runtime_support --lib`; focused case `phase29-language-repair-adapter`; broad sign-off | Adapter facts remain evidence/proposals rather than direct mutation. | `closed_proven` |
| C35 | Implemented | Adopt | recovery policy / tool policy | Closed: selected owner/action/job can project effective tool policy fields. | `recovery_orchestration.rs`, `runtime_support.rs`, eval report scripts | `cargo test recovery_orchestration --lib`; focused case `phase29-effective-tool-policy`; broad sign-off | Policy projection is common and provider-independent. | `closed_proven` |
| C36 | Implemented | Adopt | tool failure recovery | Closed: tool failure recovery status is reportable as bounded correction. | `runtime_support.rs`, tool protocol report fields | `python3 tests/test_eval_report.py`; focused case `phase29-tool-failure-recovery`; broad sign-off | Tool failures remain bounded correction or explicit stop. | `closed_proven` |
| C37 | Implemented | Adopt | bash/setup policy | Closed: commands classify deterministically with setup command authority. | `command_classification.rs`, `setup_lifecycle.rs` | `cargo test command_classification --lib`; `cargo test setup_lifecycle --lib`; focused case `phase29-setup-command-classification`; broad sign-off | Setup execution remains explicit and evidence-bound. | `closed_proven` |
| C38 | Implemented | Adopt | workspace discovery / scope | Closed: workspace candidate and ignored-dir policy fields are emitted. | `workspace_snapshot.rs`, eval report scripts | `cargo test workspace_snapshot --lib`; focused case `phase29-workspace-candidate-policy`; broad sign-off | Candidates do not bypass ownership or target admission. | `closed_proven` |
| C39 | Implemented | Adopt | runtime/eval reporting | Closed: job report status and owner/action fields are visible in eval reports. | `runtime_support.rs`, eval report scripts | `python3 tests/test_eval_report.py`; focused case `phase29-job-report`; broad sign-off | Structured job evidence is available without UI-only dependency. | `closed_proven` |
| C40 | Implemented | Adopt | scaffold contract / profile facts | Closed: scaffold status is represented as artifact obligation. | `runtime_support.rs`, existing scaffold/artifact obligation facts | focused case `phase29-scaffold-contract`; broad sign-off | Scaffold remains setup/artifact contract, not workflow engine. | `closed_proven` |
| C41 | Implemented | Adopt | completion/evidence producers | Closed: generic non-coding evidence status is reportable. | `runtime_support.rs`, completion/evidence fields | focused case `phase29-noncoding-evidence`; broad sign-off | Non-coding deliverables use generic evidence producers. | `closed_proven` |
| C42 | Implemented | Adopt | task admission / final-answer guard | Closed: answer/work-mode deterministic gate status is reportable. | `runtime_support.rs`, final-answer guard evidence | focused case `phase29-answer-work-mode`; broad sign-off | Normal coding repair is not broadly suppressed. | `closed_proven` |
| C43 | Implemented | Adopt | lifecycle/session state | Closed: explicit recovery lifecycle projection is reportable. | `runtime_support.rs`, eval report scripts | focused case `phase29-lifecycle-projection`; broad sign-off | Full actor-loop lifecycle remains omitted. | `closed_proven` |
| C44 | Implemented | Adopt | provider transport / request plumbing | Closed: provider boundary is reportable as transport-only. | `runtime_support.rs`, `docs/providers.md`, eval report scripts | focused case `phase29-provider-boundary`; broad sign-off | Provider modules remain transport/parser boundaries only. | `closed_proven` |

## Closure Rules

- A row closes only with row-specific proof plus any required broad sign-off.
- Broad sign-off is a regression gate, not a replacement for row proof.
- `split_forward` is allowed only when failed proof identifies a narrower
  same-surface blocker, owner, downstream phase, and closure condition.
- `blocked_external` is allowed only for proof limits after owner/action/
  evidence exist and only under the global recovery-plan rules.
- CI success, docs-only updates, or runtime-support summaries do not close any
  C34-C44 row.

## Review Result

Review findings applied:

- Kept C34-C44 as separate closure rows because each has a different owner
  layer and proof family.
- Added planned split paths for the rows most likely to need narrower proof:
  C34, C36, C40, C41, and C43.
- Required C44 provider-boundary proof so earlier support work cannot leak
  policy into transports unnoticed.
