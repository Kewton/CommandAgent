# Phase25 Row Closure Matrix

Date: 2026-06-23 JST

| coverage id | current status | adoption | owner layer | missing contract | target modules | required proof | closure condition | disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C11 | Implemented | Adopt | active-job arbitration / recovery orchestration | Active-job lifecycle and arbitration need row-level proof for candidate, selected, no-owner, ambiguous tie, same-owner merge, conflict-stop, and attempt-progress transition inputs. | `active_job.rs`, `recovery_orchestration.rs`, `recovery_contract.rs`, `evidence.rs`, eval scripts | `cargo test active_job`, `cargo test recovery_orchestration`, focused root `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`, broad sign-off | Active-job candidates and selected/stop decisions are deterministic, evidence-visible, source-labelled, and cannot silently fall back to generic source repair. | closed_proven |
| C12 | Implemented | Adopt | recovery owner/action dispatch gate / recovery task input | Recovery owner/action gate must connect candidate producers to a single dispatch decision before repair prompt rendering. | `recovery_orchestration.rs`, `recovery_policy.rs`, `recovery_task.rs`, `repair_brief.rs`, `repair_action_plan.rs`, `runtime/repair_loop.rs`, eval scripts | `cargo test recovery_orchestration`, `cargo test recovery_task`, `python3 tests/test_eval_report.py`, focused root `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`, broad sign-off | Profile/setup/verifier/evidence/tool candidates select exactly one bounded action or explicit stop, and repair prompts consume that decision without recomputing owner/action from prose. | closed_proven |

## Closure Rules

- `closed_proven` requires row-specific unit or fixture proof plus focused
  proof where listed.
- `split_forward` is allowed only for a narrower same-surface blocker with
  failed proof evidence, owner, downstream phase, and closure condition.
- Broad sign-off is regression evidence, not row proof.
- C11-C12 cannot be closed by docs alone, CI alone, field existence alone, or
  post-hoc eval derivation from reason text.

## Review Result

Review findings applied:

- Kept C11 and C12 as separate closure rows.
- Required proof for both arbitration decisions and prompt-input consumption.
- Added stop-state proof so owner/action ambiguity is not hidden behind source
  repair fallback.

## Implementation Result

- C11 and C12 are `closed_proven`.
- Focused fixture root: `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`.
- Focused assertions: `passed: 10`; recheck assertions: `passed_recheck: 10`.
- Broad sign-off: `status: pass`.
- No C11-C12 split-forward blocker remains.
