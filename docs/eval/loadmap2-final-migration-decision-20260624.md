# Anvil Migration Decision - 2026-06-24

## Decision

```text
migration_complete_with_explicit_exclusions
```

This report supersedes
`docs/eval/loadmap2-final-migration-decision-20260623.md`.

The accepted Anvil control/recovery responsibility surface has been migrated
into CommandAgent's explicit contract, evidence, bounded recovery, profile, and
eval-report layers. The remaining legacy advisory, UI-helper, engine-selection,
hidden-loop, provider-policy, and implicit setup surfaces are intentionally
excluded.

This is not a byte-for-byte Anvil engine port and not a claim that every large
application-generation task now succeeds. It is a closure decision for the
adopted control/recovery responsibilities recorded in
`docs/eval/legacy-control-stack-coverage-20260621.md`.

## Current Proof Roots

| family | root | admitted cases |
| --- | --- | ---: |
| smoke | `eval/runs/current-all-local-llm/smoke/20260623T203030` | 3 |
| focused | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` | 82 |
| large | `eval/runs/current-all-local-llm/large/20260623T204816` | 6 |

Final-current sign-off command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Observed result:

```text
root_admission_status: pass
root_admission_reason: current_roots_admitted
family_case_counts: focused=82, large=6, small=0, smoke=3
current_case_coverage: 91/91
status: pass
```

## Coverage Result

| final coverage state | count |
| --- | ---: |
| Implemented | 45 |
| Partial | 0 |
| Missing | 0 |
| Excluded | 9 |

| adoption decision | count |
| --- | ---: |
| Adopt | 45 |
| Partial | 0 |
| Missing | 0 |
| Excluded | 9 |

There are no adopted `Partial` rows and no adopted `Missing` rows.

## Implemented Surface

| rows | closed by | responsibility |
| --- | --- | --- |
| C01-C03 | Phase22 | Task contract, request admission, and behavior obligations. |
| C04-C06 | Phase23 | Artifact role, workspace scope, and ownership. |
| C07-C10 | Phase24 | Artifact ledger, completion evidence, evidence binding, and deliverable audit. |
| C11-C12 | Phase25 | Active-job arbitration and recovery dispatch. |
| C13-C20 | Phase26 | Recovery task packets, setup/profile mapping, semantic repair, repair brief, and action envelope. |
| C21-C32 | Phase27 | Target admission, target prioritization, repair lifecycle, verifier orchestration, completion job, focused edit, no-progress, and patch validation. |
| C33 | Phase28 | Contract conflict job and authority decision. |
| C34-C44 | Phase29 | Language/profile/tool/workspace/runtime-support projections. |
| C45 | Coverage table | Provider transport parser remains thin transport and policy-free. |

## Explicit Exclusions

| rows | exclusion rationale |
| --- | --- |
| C46-C48 | Working memory/reminders, case records, anti-pattern corpora, and PAM/Photon advisory systems would reintroduce advisory sidecars and remain outside MVP scope. |
| C49 | Anvil semantic quality confirmation and secondary model feedback classification are excluded; CommandAgent uses deterministic eval/report categories and visible recovery evidence. |
| C50 | Anvil slash/plan UI helper compatibility is excluded; CommandAgent keeps native CLI/REPL slash command behavior and docs. |
| C51 | Legacy engine selector is excluded; CommandAgent has one execution engine. |
| C52 | Hidden or unbounded repair loop is excluded; repair remains bounded and user-visible. |
| C53 | Provider/model-specific behavioral policy is excluded; shared behavior stays outside provider transports. |
| C54 | Model-issued dependency installation is excluded; setup remains explicit and evidence-bound, not implicitly model-driven. |

## Current Recovery Closure

| recovery item | current state |
| --- | --- |
| Current root admission | pass: 3 smoke, 82 focused, 6 large, `91/91` case coverage |
| Focused assertions | pass: 82 current focused rows recheck as `passed_recheck` |
| Large rows | migration-safe owned failures: 6 `closed_owned_failure`, no external limitation used |
| Row proof | C01-C54 represented, 91 current cases mapped or supplemental, open proof gaps 0 |
| Historical roots | superseded for final closure; retained as regression evidence only |

## Large Task Caveat

The large eval rows are not successful generated applications. They remain
failed user tasks. They are accepted for migration sign-off only because the
current control/recovery surface now records owner, action, target/evidence,
and disposition for each failure instead of leaving an unowned or ambiguous
control-stack gap.

## Final Statement

The Anvil migration surface that CommandAgent intentionally adopts is complete
under the current 91-case proof bundle, with explicit exclusions for the legacy
surfaces listed above.
