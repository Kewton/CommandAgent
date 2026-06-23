# Phase29 Blocking Ledger

Date: 2026-06-23 JST

| blocker id | coverage id | status | owner layer | source root | problem | required action | proof gate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P29-C34-001 | C34 | closed_proven | mechanical repair | coverage C34 | Language-specific adapter output needed row-level proof. | Added `language_repair_adapter_status` projection. | `cargo test runtime_support --lib`; focused case `phase29-language-repair-adapter`; broad sign-off |
| P29-C35-001 | C35 | closed_proven | tool policy | coverage C35 | Effective tool policy needed reportable owner/action/job projection. | Added `effective_tool_policy` and `effective_tool_policy_status`. | `cargo test recovery_orchestration --lib`; focused case `phase29-effective-tool-policy`; broad sign-off |
| P29-C36-001 | C36 | closed_proven | tool failure recovery | coverage C36 | Tool failure recovery needed bounded correction visibility. | Added `tool_failure_recovery_status`. | `python3 tests/test_eval_report.py`; focused case `phase29-tool-failure-recovery`; broad sign-off |
| P29-C37-001 | C37 | closed_proven | bash/setup policy | coverage C37 | Bash/setup commands needed deterministic classification and authority. | Added `command_classification.rs` and setup lifecycle projection. | `cargo test command_classification --lib`; `cargo test setup_lifecycle --lib`; focused case `phase29-setup-command-classification`; broad sign-off |
| P29-C38-001 | C38 | closed_proven | workspace discovery | coverage C38 | Workspace candidates and ignored-dir rules needed shared report fields. | Added candidate status, ignored-dir policy, and ignored reasons. | `cargo test workspace_snapshot --lib`; focused case `phase29-workspace-candidate-policy`; broad sign-off |
| P29-C39-001 | C39 | closed_proven | runtime/eval reporting | coverage C39 | Job owner/action status needed structured report projection. | Added `job_report_status` and `job_report_owner_action`. | `python3 tests/test_eval_report.py`; focused case `phase29-job-report`; broad sign-off |
| P29-C40-001 | C40 | closed_proven | scaffold contract | coverage C40 | Scaffold support needed artifact-obligation proof. | Added `scaffold_contract_status=artifact_obligation`. | focused case `phase29-scaffold-contract`; broad sign-off |
| P29-C41-001 | C41 | closed_proven | non-coding evidence | coverage C41 | Non-coding deliverables needed generic evidence status. | Added `noncoding_evidence_status=generic_producer`. | focused case `phase29-noncoding-evidence`; broad sign-off |
| P29-C42-001 | C42 | closed_proven | answer/work-mode gate | coverage C42 | Answer-only/work-mode gates needed deterministic report proof. | Added `answer_work_mode_status=deterministic_gate`. | focused case `phase29-answer-work-mode`; broad sign-off |
| P29-C43-001 | C43 | closed_proven | lifecycle/session state | coverage C43 | Recovery lifecycle needed explicit projection. | Added `lifecycle_projection_status`. | focused case `phase29-lifecycle-projection`; broad sign-off |
| P29-C44-001 | C44 | closed_proven | provider request boundary | coverage C44 | Provider boundary needed transport-only proof. | Added `provider_boundary_status=transport_only` and docs update. | focused case `phase29-provider-boundary`; broad sign-off |
| P29-KI008-001 | KI-008 | closed_proven | roadmap reconciliation | issue map KI-008 | KI-008 needed coverage/roadmap reconciliation after row proof. | Reconciled coverage, issue map, recovery plan, loadmap README, and implementation report. | row closure matrix plus broad sign-off |

## Blocking Rules

- A blocker can move to `closed_proven` only after its proof gate passes.
- A blocker can move to `split_forward` only when failed proof narrows the
  blocker to a same-surface downstream row with owner and closure condition.
- A blocker can move to `excluded_with_rationale` only when the behavior is
  actor-loop/advisory/provider-policy/UI-only and exclusion does not leave an
  accepted migration gap.
- C34-C44 blockers cannot be closed by broad sign-off alone.

## Review Result

Review findings applied:

- Added one row-owned blocker per C34-C44 responsibility plus a KI-008
  reconciliation blocker.
- Made C36, C40, C41, and C43 split/exclusion-sensitive instead of forcing
  broad implementation without proof.
- Required roadmap reconciliation only after row proof.
