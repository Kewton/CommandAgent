# Phase28 Focused Worklist

Date: 2026-06-23 JST

Status: completed / closed_proven

Focused proof is required because C33 changes recovery behavior and eval
fields. Existing focused cases may be reused only if they assert C33 fields
directly. Otherwise add new deterministic fixture cases under:

```text
eval/cases/focused/control-recovery/contract-conflict/
```

## Required New Cases

| case id | row | purpose | required assertions |
| --- | --- | --- | --- |
| `phase28-source-vs-generated-test` | C33 | Generated test disagrees with source/user behavior. | passed: generated test is not authoritative without binding; selected action repairs the test side; source is not repaired by default. |
| `phase28-source-vs-preexisting-test` | C33 | Pre-existing in-scope test disagrees with implementation. | passed: test authority is admitted and selected action repairs the implementation side. |
| `phase28-docs-api-vs-source` | C33 | Existing docs/API/schema contract disagrees with implementation. | passed: docs/API authority is admitted and selected action repairs implementation. |
| `phase28-weak-verifier-contract` | C33 | Verifier command conflicts with source/test but is weak or self-referential. | passed: verifier authority is limited and selected action is verifier contract correction. |
| `phase28-phase27-no-progress-handoff` | C33 | Phase27 no-progress branch reaches contract conflict. | passed: C25 handoff becomes C33 contract conflict safe stop without retry budget increase. |
| `phase28-ambiguous-authority-safe-stop` | C33 | Equal or insufficient authority between conflict sides. | passed: explicit stop with no source repair fallback and missing evidence rendered. |

## Expected Fields

Each focused case should assert the applicable subset:

- `expected_contract_conflict_status`
- `expected_contract_conflict_sides`
- `expected_contract_conflict_authority`
- `expected_contract_conflict_repair_target_side`
- `expected_contract_conflict_selected_action`
- `expected_contract_conflict_safe_stop_reason`
- `expected_contract_conflict_missing_evidence`
- `expected_contract_conflict_source_of_truth`
- `expected_active_job`
- `expected_recovery_owner`
- `expected_selected_action`
- `expected_loop_control_action`
- `expected_explicit_stop_reason`

## Recheck Command

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/contract-conflict \
  --out eval/runs/loadmap2-phase28-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture

python3 scripts/eval_report.py \
  <phase28-focused-root> \
  --cases-dir eval/cases/focused/control-recovery/contract-conflict \
  --recheck
```

## Acceptance Criteria

- Every C33 focused case passes recheck.
- Unknown/raw coverage defects are absent or mapped to a later phase with
  owner/proof.
- Ambiguous-authority safe stop is proven.
- Phase27 no-progress conflict handoff is proven.
- No case relies on model/provider-specific behavior.

## Proof Result

- Focused fixture root:
  `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`
- Recheck: `passed_recheck: 6`
- Normal report assertions: `passed: 6`
- Broad sign-off: pass with Phase28 root included as `supplemental`.

## Review Result

Review findings applied:

- Required both repairable and non-repairable conflict cases.
- Required generated-test authority limitations to prevent verifier/test
  weakening.
- Added repair-target-side assertion to prevent confusing authority with the
  artifact that should be edited.
- Required Phase27 handoff case so C33 is not proven in isolation only.
