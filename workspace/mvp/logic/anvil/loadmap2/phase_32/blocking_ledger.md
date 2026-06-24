# Phase32 Blocking Ledger

Date: 2026-06-23 JST

Status: superseded by Phase39 / reviewed

Phase39 closes the remaining final-reporting blocker with
`migration_complete_with_explicit_exclusions`. This ledger is retained as
historical recovery context; current closure evidence is in
`../phase_39/decision_evidence_matrix.md` and
`../phase_39/final_closure_report.md`.

## Planned Blockers

| blocker id | row | owner layer | incomplete contract | target | proof command | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P32-FC-001 | FC-01 | Coverage authority | Final coverage counts must prove no adopted `Partial` or `Missing`. | `docs/eval/legacy-control-stack-coverage-20260621.md` | Coverage audit and final report review. | Counts recorded as Implemented/Excluded only for final states. | closed_proven |
| P32-FC-002 | FC-02 | Recovery plan / ledgers | Phase22-Phase31 ledgers must contain no open blocker. | Phase22-Phase31 phase directories. | Phase-local report audit. | Every phase row is closed, excluded, or explicitly accepted. | closed_proven |
| P32-FC-003 | FC-03 | Architecture / coverage decision | Excluded rows must have rationale. | Coverage table, Phase30 report, final report. | Exclusion rationale audit. | No excluded row hides adopted behavior. | closed_proven |
| P32-FC-004 | FC-04 | Eval/sign-off | Final broad sign-off must exit zero. | Eval roots and `scripts/eval_signoff.py`. | `python3 scripts/eval_signoff.py --require-recheck ...` | Sign-off status is `pass`. | closed_proven_phase35 |
| P32-FC-005 | FC-05 | Eval docs / final report | Final migration state must be written and bounded. | `docs/eval/loadmap2-final-migration-decision-20260624.md`. | Report review plus `git diff --check`. | Report states the current final decision and evidence. | closed_proven_phase39 |
| P32-FC-006 | KI-011 | Current issue map / recovery plan | Final closure issue must close against current roots. | loadmap2 README, recovery plan, current issue map, Phase39 reports. | Roadmap consistency review. | KI-011 is closed because P32-FC-001 through P32-FC-005 closed under current roots. | closed_proven_phase39 |

## Recovery Blockers

| blocker id | row | owner layer | incomplete contract | target | proof command | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P32-R001 | FC-04 | Eval/sign-off | Previous accepted proof roots did not cover the current eval case set. | `phase_32/current_eval_manifest.md` | current eval manifest diff | Current signoff roots cover every current YAML case. | open |
| P32-R002 | FC-04 | Eval/report | Current broad signoff fails on fresh roots. | current eval roots | `python3 scripts/eval_signoff.py --require-recheck --root smoke=... --root focused=... --root large=...` | Command exits zero on current roots. | closed_proven_phase35 |
| P32-R003 | FC-05 | Eval/report | Fixture recheck projection dropped `fixture_fields`. | `scripts/eval_report.py` | `python3 tests/test_eval_report.py`; focused recheck | Fixture values survive `--recheck`. | closed_proven |
| P32-R004 | C07-C44 | Eval/report / focused matrix | 35 focused assertions still fail after fixture recheck repair. | focused current root | `python3 scripts/eval_report.py eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 --cases-dir eval/cases/focused/control-recovery --recheck` | Focused assertions pass or each failure has row-level accepted disposition. | closed_proven_phase35 |
| P32-R005 | C07-C44 / large | Failure observation | Raw `rc:1` or `rc_1` diagnostics remain in current recheck/signoff output. | focused and large current roots | current broad signoff | No unowned raw diagnostic remains. | closed_proven_phase35 |
| P32-R006 | large proof | Runtime/eval proof | Six current large real LLM rows fail; one row lacks target/candidate evidence. | `eval/runs/current-all-local-llm/large/20260623T204816` | large report and recheck | Large failures are owned/actionable/target-bound or explicitly accepted. | closed_proven_phase36 |
| P32-R007 | FC-02 / FC-04 | Row proof mapping | C01-C54 row closure is not tied to current eval case coverage. | coverage table and eval cases | row-to-case reconciliation | Every adopted row has current proof root or explicit rationale. | closed_proven_phase37 |

## If A Blocker Fails

| failure | required response |
| --- | --- |
| Coverage audit finds adopted `Partial` or `Missing`. | Keep KI-011 open, record the row, and declare `migration_not_complete`. |
| Phase-local ledger has an open row. | Reopen the owning phase or create a narrower phase with owner/proof. |
| Sign-off fails. | Convert every finding into a ledger row before any roadmap extension. |
| Final report cannot justify an exclusion. | Convert the excluded row into a new adopted blocker or update the exclusion rationale with authority. |
| New distinct responsibility class is found. | Add a post-Phase32 roadmap extension and declare `migration_not_complete`. |

## Review Notes

- The ledger deliberately treats final reporting as a blocker with proof,
  because earlier phases failed when completion was left to narrative summary.
- No blocker authorizes runtime retries, provider-specific behavior, or hidden
  continuation.
