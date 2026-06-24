# Loadmap2 Phase39 Plan

Date: 2026-06-24 JST

Status: completed / reviewed

## Scope

Phase39 closes the Phase32 recovery sequence by producing the final migration
decision from current evidence.

Phase39 consumes the outputs of Phase33 through Phase38. It does not add
runtime behavior, retry logic, provider/model policy, or new recovery
mechanisms.

| source | Phase39 responsibility |
| --- | --- |
| `phase_32/followup_phase_split.md` Phase39 | Retry final closure after Phase33-Phase38 and report completion or explicit non-completion. |
| `phase_32/recovery_task_ledger.md` exit gate item 6 | Write the final decision without relying on superseded historical roots. |
| `phase_37/proof_gap_ledger.md` P37-H002 | Consume row proof and root admission without claiming task success for failed large rows. |
| `phase_38/root_admission_report.md` | Use admitted current roots with 91/91 current case coverage as final sign-off input. |

## Current Evidence To Consume

Current final-current roots:

```text
smoke:   eval/runs/current-all-local-llm/smoke/20260623T203030
focused: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
large:   eval/runs/current-all-local-llm/large/20260623T204816
```

Current closure inputs:

| phase | closure signal |
| --- | --- |
| Phase33 | Focused recheck projection no longer drops deterministic fixture fields. |
| Phase34 | Raw diagnostics and unknown-contract findings are classified or owned. |
| Phase35 | Focused assertions pass current recheck: `passed_recheck=82`. |
| Phase36 | Six current large rows are `closed_owned_failure` with owner/action/target/evidence. |
| Phase37 | C01-C54 are represented; 91 current cases are mapped or supplemental; proof gaps are 0. |
| Phase38 | Root admission passes with 3 smoke, 82 focused, 6 large, 0 optional small. |

## Decision Model

Phase39 may produce exactly one of these final states:

| decision | allowed when |
| --- | --- |
| `migration_complete` | All adopted rows are implemented and eval-proven, no unowned sign-off finding remains, all ledger rows are `closed_proven`, and exclusions are treated as outside the adopted migration target. |
| `migration_complete_with_explicit_exclusions` | Adopted rows are implemented and eval-proven, excluded rows have design rationale, current sign-off passes, and the report makes the excluded surface explicit. |
| `migration_not_complete` | Any adopted row is partial/missing, any sign-off finding is unmapped, any blocker lacks owner/action/evidence, current root admission fails, or current sign-off fails. |

The expected candidate for Phase39 is
`migration_complete_with_explicit_exclusions`, because the accepted CommandAgent
surface is implemented while C46-C54 remain intentionally excluded legacy
surfaces. The implementation step must still recompute the decision from
evidence and must not hard-code the expected outcome.

## Architecture Approach

Phase39 should use a small final-decision evidence chain:

```text
coverage table
  -> Phase32 recovery ledger
  -> Phase33-Phase38 implementation reports
  -> current root admission and current sign-off
  -> final decision report
  -> roadmap/status document updates
```

If an automation helper is useful, it should be report-only and eval-only. It
must read existing documents and sign-off output; it must not rerun models,
mutate run workspaces, run setup, or reinterpret row outcomes.

## Layer Boundaries

| layer | Phase39 stance |
| --- | --- |
| Provider transport | No changes. |
| Minimal loop | No changes. |
| Step runner / recovery orchestration | No changes unless a report-only field is missing, which should become a new blocker rather than hidden repair. |
| Eval/sign-off | Re-run current sign-off only; do not weaken gates. |
| Eval/report | May be used to inspect existing summaries; no reclassification unless a deterministic report bug is found and assigned. |
| Docs/eval | Primary output layer for final decision report. |
| Roadmap docs | Must be updated so Phase32/39 state is not stale. |

## Required Outputs

Phase39 implementation should produce or update:

- `workspace/mvp/logic/anvil/loadmap2/phase_39/final_closure_report.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_39/decision_evidence_matrix.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_39/implementation_report.md`
- `docs/eval/loadmap2-final-migration-decision-20260624.md`, or an updated
  current final decision report with a clear supersession note
- `docs/migration-progress.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md` Phase32 recovery
  appendix
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_32/recovery_task_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_32/followup_phase_split.md`

## Horizontal Rollout

Phase39 should define final closure as a reusable evidence pattern:

- final reports must name current proof roots, not just historical roots;
- broad sign-off must include root admission output;
- row-level proof must be linked to coverage rows;
- excluded surfaces must remain explicit and not hidden behind a green sign-off;
- large task failures may be migration-safe only when they are owned,
  actionable, target/evidence-bound, and not claimed as task success.

This pattern should be documented for later migration or release sign-off work.

## Documentation Updates

Docs must remove or supersede stale statements that still say:

- current broad sign-off exits non-zero;
- Phase32 remains recovery-open for current root admission;
- KI-011 remains open due to focused/large blockers already closed by
  Phase33-Phase38;
- historical roots alone are final proof.

Docs must retain the warning that broad sign-off is necessary but not
sufficient. The final decision must cite row proof, root admission, and
excluded-surface rationale.

## Stability And Complexity Controls

Phase39 remains stable by:

- using existing run roots and current sign-off output;
- treating final reporting as docs/eval work, not runtime behavior;
- accepting only deterministic evidence;
- failing to `migration_not_complete` if any required proof is absent;
- not adding retries, provider branches, implicit setup, or verifier weakening;
- keeping the final decision separate from large task success.

Complexity is controlled by a single decision matrix instead of adding a new
final-closure engine.

## Exit Gate

Phase39 is complete only when:

- current root admission passes with 91/91 current cases;
- current broad sign-off exits zero on the admitted current roots;
- Phase33-Phase38 reports are cited as closure evidence;
- C01-C45 adopted rows remain implemented/proven;
- C46-C54 excluded rows remain explicit and justified;
- no adopted row depends only on historical roots that omit current cases;
- large rows are described as owned failed task rows, not task success;
- final decision report states exactly one decision;
- roadmap/status docs no longer say the current sign-off is failing;
- any discovered missing proof creates a named follow-up blocker and the final
  decision remains `migration_not_complete`;
- no hidden retry, runtime orchestration, provider/model branch, implicit
  setup, or verifier weakening is introduced.

## Plan Review Result

Review findings incorporated:

- Kept Phase39 as final evidence/reporting rather than runtime implementation.
- Required recomputing the decision from evidence instead of hard-coding the
  expected completion outcome.
- Added stale-document cleanup because Phase32/roadmap files still mention
  current sign-off failure.
- Required a decision evidence matrix so green sign-off cannot be used alone.
- Preserved the distinction between Anvil migration completion and large task
  application success.
- Added failure handling: missing proof must produce `migration_not_complete`
  or a named blocker, not an optimistic completion claim.

## Implementation Result

Phase39 produced:

- `decision_evidence_matrix.md`
- `final_closure_report.md`
- `implementation_report.md`
- `docs/eval/loadmap2-final-migration-decision-20260624.md`

Final decision:

```text
migration_complete_with_explicit_exclusions
```

The decision is based on admitted current roots with `91/91` case coverage,
current broad sign-off pass, focused assertion closure, large owned-failure
closure, row proof reconciliation, and explicit C46-C54 exclusions.
