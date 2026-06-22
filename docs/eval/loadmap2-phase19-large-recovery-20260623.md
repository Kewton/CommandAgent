# Loadmap2 Phase19 Large Recovery

Date: 2026-06-23 JST

## Scope

Phase19 closed the large-eval ownership/evidence blockers tracked as
P17-L001 through P17-L004. This did not make the large tasks pass; it made
their failures attributable from eval artifacts.

## Implementation

- Added deterministic eval-report projection for provider/eval timeout rows:
  `active_job=provider_transport_blocker`,
  `recovery_owner=provider_transport`,
  `repair_action=stop_for_provider_timeout`,
  `attempt_outcome=blocked_external`, and field-sensitive
  `not_applicable` target/evidence.
- Added deterministic projection for
  `profile_verification:nextjs_dependency_version_conflict`:
  `active_job=manifest_repair`, `recovery_owner=manifest`,
  `target_path=package.json`, and `target_role=setup_manifest`.
- Updated broad sign-off so `not_applicable` is accepted only for fields and
  terminal states where owner/action/attempt outcome prove the field is truly
  not applicable.

## Proof Roots

- smoke: `eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759`
- focused: `eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638`
- focused fixture:
  `eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659`
- large recheck:
  `eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149`

## Commands

```bash
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149 \
  --cases-dir eval/cases/large \
  --recheck
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

## Result

The broad sign-off result is `status: pass`.

Large recheck rows remain failed, but now have owner/action/evidence:

- Five timeout rows are provider/eval boundary blockers with
  `attempt_outcome=blocked_external`.
- `large-nextjs-app-modify` is a manifest repair targeting `package.json`
  instead of generic source repair.

## Ledger Status

- P17-L001: closed as valid `blocked_external` timeout ownership.
- P17-L002: closed proven by manifest target projection and sign-off pass.
- P17-L003: closed proven by meaningful evidence fields and sign-off pass.
- P17-L004: closed proven by target projection or valid target
  not-applicability.

## Phase20 Handoff

Phase20 should decide migration-complete status. It should not reopen Phase19
unless a new large sign-off row has missing owner/action/evidence/target fields
after recheck projection.
