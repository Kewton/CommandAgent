# Phase 17 Sign-off Reconciliation

## Purpose

This file reconciles the current Phase 16 broad sign-off output with the Phase
17 blocking ledger. It exists to prevent the same process failure from
recurring: a phase cannot proceed from a narrative summary when individual
sign-off findings have not been accounted for.

Current sign-off command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase16-focused-local-llm/20260622T173940 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Current output:

```text
status: fail
focused findings: 5
large findings: 14
total sign-off findings: 19
```

## Reconciliation Table

| finding_id | family | case_id | signoff_code | detail summary | ledger_row | coverage_responsibility | downstream_phase | proof_command | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S001 | focused | `focused-docs-literal-mismatch` | `focused_assertion_failed` | Expected source repair fields, observed explicit stop / step policy failure | P17-F001 | Active job arbiter; recovery task contract; step policy failure handling | Phase18 | targeted focused rerun, then full focused sign-off | mapped |
| S002 | focused | `focused-nextjs-dependency-setup` | `focused_assertion_failed` | Expected completed setup, observed explicit stop with passed selected action | P17-F002 | Setup bootstrap; setup artifact validation; recovery task contract | Phase18 | targeted focused rerun, then full focused sign-off | mapped |
| S003 | focused | `focused-nextjs-endpoint-smoke` | `focused_assertion_failed` | Expected ok/runtime success, observed plan lint failure and source repair | P17-F003 | Failure observation; plan lint; eval success contract; dev-server smoke | Phase18 | targeted focused rerun after diagnostic extraction, then full focused sign-off | mapped |
| S004 | focused | `focused-nextjs-route-integration` | `focused_assertion_failed` | Expected ok/runtime success, observed manifest repair and plan lint failure | P17-F004 | Profile route contract; plan lint obligation projection; active job arbitration | Phase18 | targeted focused rerun, then full focused sign-off | mapped |
| S005 | focused | `focused-nextjs-endpoint-smoke` | `raw_undiagnostic_rc` | `reason=rc:1 diagnostic_code=rc_1` | P17-F003 | Failure observation; eval lifecycle funnel | Phase18 | targeted focused rerun after `plan_lint.invalid_expected_path` extraction | mapped |
| S006 | large | `large-fastapi-app-modify` | `missing_evidence_binding` | Missing evidence binding for failed large row | P17-L003 | Completion evidence; evidence binding; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S007 | large | `large-fastapi-app-modify` | `missing_completion_evidence` | Missing completion evidence for failed large row | P17-L003 | Completion evidence; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S008 | large | `large-fastapi-app-new` | `missing_evidence_binding` | Missing evidence binding for failed large row | P17-L003 | Completion evidence; evidence binding; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S009 | large | `large-fastapi-app-new` | `missing_completion_evidence` | Missing completion evidence for failed large row | P17-L003 | Completion evidence; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S010 | large | `large-nextjs-app-modify` | `generic_source_fallback` | `profile_contract_failed` was represented as source repair fallback | P17-L002 | Profile failure mapping; active job arbitration; manifest repair | Phase19 | large Next.js modify rerun or profile fixture plus sign-off checker | mapped |
| S011 | large | `large-nextjs-app-modify` | `missing_evidence_binding` | Missing evidence binding for failed large row | P17-L003 | Completion evidence; evidence binding; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S012 | large | `large-nextjs-app-modify` | `missing_completion_evidence` | Missing completion evidence for failed large row | P17-L003 | Completion evidence; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S013 | large | `large-nextjs-app-modify` | `missing_target` | Missing target for target-applicable failed large row | P17-L004 | Target admission; target role projection; eval report projection | Phase19 | target-admission fixture or large rerun plus sign-off checker | mapped |
| S014 | large | `large-nextjs-app-new` | `missing_evidence_binding` | Missing evidence binding for failed large row | P17-L003 | Completion evidence; evidence binding; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S015 | large | `large-nextjs-app-new` | `missing_completion_evidence` | Missing completion evidence for failed large row | P17-L003 | Completion evidence; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S016 | large | `large-rust-app-modify` | `missing_evidence_binding` | Missing evidence binding for failed large row | P17-L003 | Completion evidence; evidence binding; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S017 | large | `large-rust-app-modify` | `missing_completion_evidence` | Missing completion evidence for failed large row | P17-L003 | Completion evidence; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S018 | large | `large-rust-app-new` | `missing_evidence_binding` | Missing evidence binding for failed large row | P17-L003 | Completion evidence; evidence binding; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |
| S019 | large | `large-rust-app-new` | `missing_completion_evidence` | Missing completion evidence for failed large row | P17-L003 | Completion evidence; eval report projection | Phase19 | report fixture or large rerun plus sign-off checker | mapped |

## Context Blocker Not Emitted As A Sign-off Finding

| context_id | source | detail | ledger_row | reason |
| --- | --- | --- | --- | --- |
| C001 | Phase16 report | Five large rows ended as `provider_transport:eval_timeout` in the time-boxed large root. | P17-L001 | The sign-off checker reports missing evidence fields, but the root cause context is timeout/provider boundary behavior. This must remain tracked so timeout does not get mistaken for model-quality or migration success. |

## Reconciliation Summary

| Check | Result |
| --- | --- |
| Sign-off findings represented | 19/19 |
| Findings without ledger row | 0 |
| Findings without downstream phase | 0 |
| Findings without proof command | 0 |
| Findings requiring split before runtime work | 0 currently identified |
| Coverage responsibilities missing from table | Must be verified against `docs/eval/legacy-control-stack-coverage-20260621.md` during Phase17 execution |

## Phase Gate

Phase17 may proceed to implementation planning for Phase18/19 only after the
coverage-table verification row above is resolved. If the coverage table lacks
an explicit responsibility for any reconciliation row, that is a Phase17
coverage gap, not a Phase18/19 runtime task.
