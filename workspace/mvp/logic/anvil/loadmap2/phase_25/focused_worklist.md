# Phase25 Focused Worklist

Date: 2026-06-23 JST

Focused proof is required because Phase25 changes or validates
owner/action/dispatch behavior before repair prompt rendering.

## Required Focused Cases

Use existing focused cases only if they explicitly prove the C11-C12 fields.
Otherwise add Phase25 fixtures below.

| case | path | coverage rows | expected assertion focus |
| --- | --- | --- | --- |
| `focused-dispatch-setup-bootstrap` | `eval/cases/focused/control-recovery/nextjs/dependency-setup.yaml` or new dispatch fixture | C11, C12 | `active_job=setup_bootstrap`, `recovery_owner=setup`, verifier-owned setup action, candidate source, tool policy, rerun authority. |
| `focused-dispatch-manifest-repair` | `eval/cases/focused/control-recovery/nextjs/manifest-repair.yaml` or new dispatch fixture | C11, C12 | `active_job=manifest_repair`, `recovery_owner=manifest`, selected action, target `package.json`, candidate list. |
| `focused-dispatch-route-integration` | `eval/cases/focused/control-recovery/nextjs/route-integration-repair.yaml` or new dispatch fixture | C11, C12 | route integration owner/action, selected route target, source layer, dispatch reason. |
| `focused-dispatch-source-diagnostic` | `eval/cases/focused/control-recovery/rust/compile-diagnostic-target.yaml`, `python/fastapi-assertion-mismatch.yaml`, or new dispatch fixture | C11, C12 | source owner/action, diagnostic source-of-truth, target hint, rerun authority. |
| `focused-dispatch-docs-literal` | `eval/cases/focused/control-recovery/docs/docs-literal-mismatch.yaml` or new dispatch fixture | C11, C12 | docs owner/action, docs target, disallowed source-repair fallback. |
| `focused-dispatch-evidence-binding` | `eval/cases/focused/control-recovery/completion/evidence-binding-failure.yaml` or new dispatch fixture | C11, C12 | evidence-binding owner/action, binding target, expected binding, completion/evidence handoff. |
| `focused-dispatch-verifier-contract` | `eval/cases/focused/control-recovery/recovery-policy/setup-manifest-invalid.yaml` or new dispatch fixture | C11, C12 | verifier/manifest contract owner/action, explicit selected dispatch before prompt rendering. |
| `focused-dispatch-tool-protocol` | `eval/cases/focused/control-recovery/tool-protocol/missing-write-path.yaml` or new dispatch fixture | C11, C12 | tool-protocol correction owner/action, allowed tools, exhausted correction fields. |
| `focused-dispatch-no-owner-stop` | new Phase25 fixture if no existing case proves it | C11 | `dispatch_status=no_owner`, `active_job=explicit_stop`, structured explicit stop reason. |
| `focused-dispatch-ambiguous-tie-stop` | new Phase25 fixture if no existing case proves it | C11, C12 | `dispatch_status=ambiguous_tie`, candidate list, tie-break reason, no bounded repair execution. |

## Existing Candidate Cases

| candidate | possible reuse | caveat |
| --- | --- | --- |
| `nextjs/dependency-setup.yaml` | setup dispatch owner/action | Must assert dispatch source and verifier-owned setup policy, not only dependency_missing. |
| `nextjs/manifest-repair.yaml` | manifest owner/action | Must assert target and prompt-input dispatch fields. |
| `nextjs/route-integration-repair.yaml` | route owner/action | Must assert selected route and disallowed generic source fallback. |
| `docs/docs-literal-mismatch.yaml` | docs owner/action | Must assert docs owner rather than source fallback. |
| `completion/evidence-binding-failure.yaml` | evidence binding owner/action | Must assert binding-specific dispatch fields. |
| `recovery-policy/contract-conflict-explicit-stop.yaml` | conflict-stop path | Proves only Phase25 handoff stop, not C33 resolution. |
| `tool-protocol/missing-write-path.yaml` | tool-protocol correction | Must assert correction action and allowed tools. |

## Recheck Rules

- Do not update focused expected fields until unit tests prove the new
  dispatch fields.
- Do not mark focused failures as external limitations unless owner, action,
  evidence, and the external proof limit are already explicit.
- If a focused case fails because dispatch fields are absent, map the failure
  back to the C11/C12 blocker that owns the field.
- If a focused case fails because Phase26/27/28 owns the missing behavior,
  record a same-surface split-forward blocker with failed proof evidence.
- If existing fixtures are reused, record exactly which fields and rows they
  prove.

## Planned Focused Execution

Suggested command if dispatch fixtures are added:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/dispatch \
  --out eval/runs/loadmap2-phase25-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Then recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase25-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/dispatch \
  --recheck
```

If proof spans additional focused directories, record every root and the
specific row it proves.

## Review Result

Review findings applied:

- Focused proof is mandatory for closure; only fixture creation is conditional.
- Focused assertions are limited to C11-C12 dispatch-visible fields.
- Existing fixtures may be reused only with explicit field-to-row mapping.
- Added no-owner and ambiguous-tie cases so dispatch cannot pass by only
  covering selected-success paths.

## Implementation Result

Executed focused fixture root:

```text
eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110
```

Commands:

```bash
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/dispatch --out eval/runs/loadmap2-phase25-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110 --cases-dir eval/cases/focused/control-recovery/dispatch
python3 scripts/eval_report.py eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110 --cases-dir eval/cases/focused/control-recovery/dispatch --recheck
```

Result:

```text
focused assertions: passed: 10
recheck assertions: passed_recheck: 10
```
