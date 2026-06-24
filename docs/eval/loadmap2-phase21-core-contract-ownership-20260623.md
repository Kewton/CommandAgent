# Loadmap2 Phase21 Core Contract And Ownership

Date: 2026-06-23 JST

## Scope And Inputs

Phase21 selected the Phase20 continuation blocker `P20-COV-001`, covering
coverage rows C01-C12:

- task contract core;
- task contract inference and admission;
- objective and behavior contract projection;
- artifact role taxonomy;
- task workspace scope;
- artifact ownership;
- artifact ledger;
- completion evidence;
- evidence binding;
- deliverable obligation audit;
- active job arbiter;
- recovery owner / dispatch gate.

Inputs:

- Phase20 continuation ledger
- Phase20 coverage closure
- Phase20 final migration decision report
- legacy-control coverage table
- Phase21 workspace row closure matrix
- Phase21 workspace blocking ledger
- Phase21 workspace reconciliation map

Baseline:

| field | value |
| --- | --- |
| commit | `a3c5eb3` |
| branch | `develop` |
| selected blocker | `P20-COV-001` |

## Decision

Phase21 is complete as an admission/reconciliation phase.

It does not declare migration completion and does not mark C01-C12 as
`Implemented`. The existing implementation has partial boundaries for these
responsibilities, but Phase20 still identifies missing lifecycle, producer,
binding, scope, and E2E proof. Phase21 therefore splits the grouped blocker
into row-level blockers with owners, proof commands, downstream phases, and
closure conditions.

## Row Disposition

| disposition | count |
| --- | ---: |
| `closed_proven` | 0 |
| `excluded_with_rationale` | 0 |
| `split_forward` | 12 |
| `open` | 0 |

Rows C01-C12 are assigned to downstream phases:

| downstream phase | rows | responsibility |
| --- | --- | --- |
| Phase22 | C01-C03 | task contract, request admission, behavior obligations |
| Phase23 | C04-C06 | artifact role, workspace scope, ownership |
| Phase24 | C07-C10 | artifact ledger, completion evidence, evidence binding, deliverable audit |
| Phase25 | C11-C12 | active-job arbitration and dispatch lifecycle |

## Broad Sign-off

The established broad sign-off command remains:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Phase21 reruns this command for regression safety. A pass keeps the broad
sign-off state healthy, but it is not used as row-level implementation proof.

## Current Migration State

The migration state remains:

```text
migration_not_complete
```

Reason:

- C01-C12 are no longer vague in the Phase21 ledger, but they remain
  `split_forward`;
- coverage table row statuses are not promoted without implementation proof;
- downstream phases must close each P21-Cxx blocker with row-specific tests or
  focused eval.

## Verification

Local verification for this report:

| command | result |
| --- | --- |
| `python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py` | passed |
| `python3 tests/test_eval_report.py` | passed, 40 tests |
| `python3 tests/test_eval_signoff.py` | passed, 10 tests |
| `cargo fmt --check` | passed |
| `cargo test` | passed, 683 library tests plus integration/doc-test suites |
| `bash scripts/eval_smoke.sh` | passed, including release build and offline smoke |
| broad sign-off command | passed, `status: pass` |
| `scripts/check_branding.sh` | passed |
| `git diff --check` | passed |

## Review Result

The reviewed outcome avoids the earlier process failure: it does not count
phase execution as migration parity. Every selected row now has an owned
follow-up with proof criteria, and no row is closed by prose-only rationale.
