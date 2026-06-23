# Phase23 Focused Worklist

Date: 2026-06-23 JST

Focused proof is required because Phase23 changes or validates model-facing
planning/recovery evidence and eval report assertions for role, scope, and
ownership.

## Required Focused Cases

Use an existing focused case only if it explicitly proves the C04-C06 fields.
Otherwise add the Phase23 fixture below.

| case | path | coverage rows | expected assertion focus |
| --- | --- | --- | --- |
| `focused-artifact-role-scope-ownership` | `eval/cases/focused/control-recovery/planning/artifact-role-scope-ownership.yaml` | C04, C05, C06 | artifact role projection status, workspace scope kind/root, ownership decision/source/reason, generated/cache/build target rejection. |

## Optional Additional Cases

| candidate | purpose |
| --- | --- |
| Next.js generated/cache target rejection | Prove `.next`, `node_modules`, generated declarations, and build outputs do not become owned implementation targets. |
| ambiguous parent scope | Prove multiple project roots do not silently expand ownership. |
| docs/data ownership | Prove non-code deliverables keep docs/data roles and are not forced into source repair. |
| repeated rejected target | Prove repeated-target exclusion where deterministic attempt facts are already available. |

## Recheck Rules

- Do not update focused expected fields until unit tests prove the new fields.
- Do not mark focused failures as an external limitation unless owner, action,
  evidence, and the external proof limit are already explicit.
- If a focused case fails because a role/scope/ownership field is absent, map
  the failure back to the C04-C06 blocker that owns the field.
- If a focused case fails because a later phase owns the missing behavior,
  record a same-surface split-forward blocker with failed proof evidence.
- If no new focused fixture is added, record which existing focused root and
  assertions prove C04-C06.

## Executed Focused Execution

The Phase23 fixture was added and executed:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/planning \
  --out eval/runs/loadmap2-phase23-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Then recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023 \
  --cases-dir eval/cases/focused/control-recovery/planning \
  --recheck
```

Result:

```text
focused fixture root: eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023
focused assertions: passed_recheck
broad sign-off: pass
```

## Review Result

Review findings applied:

- Focused proof is mandatory for closure; only the fixture creation is
  conditional.
- Focused assertions are limited to C04-C06 role/scope/ownership fields.
- Recheck rules prevent updating expected assertions ahead of implementation
  proof.
