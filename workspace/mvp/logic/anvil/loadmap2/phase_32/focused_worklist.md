# Phase32 Focused Worklist

Date: 2026-06-23 JST

Status: recovery_open / Phase33 projection closed

## Policy

Phase32 recovery does not add hidden model-facing behavior. It must, however,
track the focused cases that are present in the current eval roots and were not
fully represented by the historical Phase32 sign-off roots.

Focused work is required because current recheck shows:

- 82 focused control-recovery cases;
- 9 focused successes;
- 35 focused assertion failures after fixture recheck projection was fixed;
- 4 focused assertion failures after Phase33 recheck projection repair;
- many raw `rc:1` / unknown diagnostic rows that still need ownership and
  row-level disposition.

## Historical Focused Evidence

| purpose | root |
| --- | --- |
| focused control-recovery sign-off | `eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638` |
| runtime-support focused fixture | `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335` |

## Current Focused Evidence

| purpose | root |
| --- | --- |
| current focused control-recovery | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` |

## Current Focused Recovery Groups

| group | representative cases | required action |
| --- | --- | --- |
| explicit-stop projection mismatch | `focused-artifact-role-scope-ownership`, `focused-out-of-scope-target-rejection`, `phase27-target-priority-tie-stop`, `phase27-verifier-command-policy`, `phase27-verifier-orchestration-safe-stop`, `phase28-ambiguous-authority-safe-stop`, `phase28-phase27-no-progress-handoff` | Closed by Phase33. Recheck now preserves deterministic explicit-stop terminal and owner/action fields. |
| evidence/completion status mismatch | `focused-completion-evidence-producers`, `focused-deliverable-obligation-freshness`, `focused-evidence-binding-failure`, `focused-evidence-binding-producers`, `focused-missing-evidence`, `phase27-artifact-completion-job` | Closed by Phase33. Recheck now preserves completion/evidence binding states instead of collapsing them to `failed`. |
| missing-deliverable vs safe-stop mismatch | `focused-contract-conflict-explicit-stop`, `focused-generated-test-weakening-rejection`, `focused-missing-artifact-completion`, `focused-no-progress-target-switch`, `focused-tool-protocol-missing-write-path` | Closed by Phase33 where deterministic fixture/meta evidence already expressed the safe-stop expectation. |
| attempt/lifecycle mismatch | `focused-docs-literal-mismatch`, `focused-phase26-safe-stop-evidence-binding`, `phase27-attempt-ledger-outcomes`, `phase27-repair-lifecycle-rerun` | Closed by Phase33 where explicit fixture/meta fields carried attempt outcome and lifecycle semantics. |
| verifier-specific terminal mismatch | `focused-python-fastapi-assertion-mismatch`, `focused-stale-edit-target`, `phase27-focused-edit-stale-rejection`, `phase27-no-progress-deferral`, `phase27-patch-validation-rollback` | Closed by Phase33 where explicit fixture/meta fields carried specialized verifier/setup/step-policy terminal state. |
| remaining setup/profile/dev-server/readiness mismatch | `focused-nextjs-dependency-setup`, `focused-nextjs-endpoint-smoke`, `focused-nextjs-route-integration` | Phase35. These rows require setup/profile/dev-server/readiness connection rather than eval/report projection. |
| remaining dispatch-action semantic mismatch | `focused-dispatch-manifest-repair` | Phase34/35. The remaining assertion expects `add_missing_manifest_dependency`, while current report observes `resolve_manifest_conflict`; owner/action semantics must be reconciled without weakening the assertion. |

## Conditional Additions

If a focused case is added during Phase32, record:

| field | requirement |
| --- | --- |
| case id | Stable case path under `eval/cases/focused/...`. |
| expected assertion | Exact assertion field and expected value. |
| coverage row | C-row or FC-row that requires the case. |
| owner layer | Planning, execution, recovery task, profile, setup, verifier, eval/report, or provider transport. |
| proof root | Fresh eval root and recheck command. |
| closure condition | Assertion passes and broad sign-off remains green. |

## Review Notes

- Keeping this file explicit prevents hidden focused-case work during final
  closure.
- The default is no hidden runtime change; focused discrepancies must either
  become deterministic report fixes or be assigned to an explicit follow-up
  phase with owner, target, and proof command.
