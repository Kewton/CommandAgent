# Phase32 Blocking Ledger

Date: 2026-06-23 JST

Status: completed / reviewed

## Planned Blockers

| blocker id | row | owner layer | incomplete contract | target | proof command | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P32-FC-001 | FC-01 | Coverage authority | Final coverage counts must prove no adopted `Partial` or `Missing`. | `docs/eval/legacy-control-stack-coverage-20260621.md` | Coverage audit and final report review. | Counts recorded as Implemented/Excluded only for final states. | closed_proven |
| P32-FC-002 | FC-02 | Recovery plan / ledgers | Phase22-Phase31 ledgers must contain no open blocker. | Phase22-Phase31 phase directories. | Phase-local report audit. | Every phase row is closed, excluded, or explicitly accepted. | closed_proven |
| P32-FC-003 | FC-03 | Architecture / coverage decision | Excluded rows must have rationale. | Coverage table, Phase30 report, final report. | Exclusion rationale audit. | No excluded row hides adopted behavior. | closed_proven |
| P32-FC-004 | FC-04 | Eval/sign-off | Final broad sign-off must exit zero. | Eval roots and `scripts/eval_signoff.py`. | `python3 scripts/eval_signoff.py --require-recheck ...` | Sign-off status is `pass`. | closed_proven |
| P32-FC-005 | FC-05 | Eval docs / final report | Final migration state must be written and bounded. | `docs/eval/anvil-migration-complete.md`. | Report review plus `git diff --check`. | Report states the final decision and evidence. | closed_proven |
| P32-FC-006 | KI-011 | Current issue map / recovery plan | Final closure issue remains open. | loadmap2 README, recovery plan, current issue map. | Roadmap consistency review. | KI-011 is closed because P32-FC-001 through P32-FC-005 closed. | closed_proven |

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
