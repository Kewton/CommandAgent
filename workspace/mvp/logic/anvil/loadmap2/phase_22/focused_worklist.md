# Phase22 Focused Worklist

Date: 2026-06-23 JST

Focused proof is required because Phase22 changes planner-facing task contract
and behavior obligation fields.

## Required Focused Cases

| case | path | coverage rows | expected assertion focus |
| --- | --- | --- | --- |
| `focused-task-contract-admission` | `eval/cases/focused/control-recovery/planning/task-contract-admission.yaml` | C01, C02 | task contract kind, admission status, lifecycle/status fields, behavior status, artifact role projection status. |
| `focused-behavior-obligation-projection` | `eval/cases/focused/control-recovery/planning/behavior-obligation-projection.yaml` | C01, C03 | behavior obligation codes, owner/status/path fields, Next.js manifest/route/build/dev-port obligations. |

## Optional Additional Cases

Add focused cases only if unit tests cannot prove row behavior:

| candidate | purpose |
| --- | --- |
| docs literal projection | prove docs behavior obligation and required literal reporting. |
| data schema projection | prove data/schema behavior obligation if profile support already exists. |
| ambiguous request admission | prove conflict/partial admission if deterministic fixture support exists. |

## Recheck Rules

- Do not update focused expected fields until unit tests prove the new fields.
- Do not mark focused failures as model quality.
- If a focused case fails because a new field is absent, map the failure back
  to the C01-C03 blocker that owns the field.
- If a focused case fails because a later phase owns the missing behavior,
  record a same-surface split-forward blocker with failed proof evidence.

## Execution Result

Focused fixture root:

```text
eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658
```

| case | result | assertion |
| --- | --- | --- |
| `focused-task-contract-admission` | pass | `passed_recheck` |
| `focused-behavior-obligation-projection` | pass | `passed_recheck` |

## Review Result

Review findings applied:

- Focused proof is limited to C01-C03 planner-facing fields.
- Optional cases are listed but not required unless implementation needs them.
- Recheck rules prevent updating expected assertions ahead of implementation
  proof.
