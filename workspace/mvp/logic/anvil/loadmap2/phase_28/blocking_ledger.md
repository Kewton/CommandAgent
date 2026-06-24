# Phase28 Blocking Ledger

Date: 2026-06-23 JST

| blocker id | coverage id | status | owner layer | source root | problem | required action | proof gate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P28-C33-001 | C33 | closed_proven | contract conflict object | coverage C33 / Phase27 C25 handoff | Conflict inputs are preserved but not normalized into a first-class conflict object with sides and evidence. | Added typed conflict boundary and render/eval fields. | `cargo test contract_conflict`; focused fixture root `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521` |
| P28-C33-002 | C33 | closed_proven | source-of-truth authority | coverage C33 | CommandAgent does not deterministically decide user/profile/docs/API/test/verifier authority for conflicts or separate the authoritative side from the repair target side. | Added authority categories, ambiguity/insufficient-evidence decisions, and repair-target-side projection. | `cargo test contract_conflict`; focused authority cases |
| P28-C33-003 | C33 | closed_proven | recovery orchestration | Phase25/26/27 handoff | Contract conflict can be detected or deferred but not routed to an authority-backed action or explicit safe stop. | Connected C33 decision to existing action envelope and recovery task evidence rendering. | `cargo test recovery_orchestration`; broad sign-off |
| P28-C33-004 | C33 | closed_proven | no-progress handoff | Phase27 C25 | Phase27 no-progress conflict branch is only deferred to Phase28. | Focused handoff fixture proves C33 consumes no-progress conflict evidence and safe-stops ambiguity. | focused no-progress handoff case |
| P28-C33-005 | C33 | closed_proven | eval/reporting | eval schema | Focused eval cannot assert C33 decisions directly without structured fields. | Added expected fields and report section for conflict status, sides, authority, action, safe-stop reason, missing evidence, and source of truth. | `python3 tests/test_eval_report.py`; focused C33 recheck |
| P28-C33-006 | C33 | closed_proven | documentation / coverage | docs/eval roadmap | Coverage and roadmap still show C33 as Missing/open. | Updated docs after focused proof and broad sign-off. | implementation report and broad sign-off |

## Blocking Rules

- A blocker remains open when proof fails with the same finding.
- A new finding must be mapped to C33 or a later row before Phase28 can close.
- Ambiguous authority is not an external limitation; it must produce an
  explicit safe stop.
- Model/provider quality cannot be used to close C33 without deterministic
  owner/action/evidence.

## Review Result

Review findings applied:

- Split conflict handling into object, authority, orchestration, handoff,
  eval, and docs blockers.
- Added repair-target-side projection to the authority blocker.
- Prevented docs/coverage closure before proof.
- Made ambiguous authority a required behavior, not a reason to defer.
