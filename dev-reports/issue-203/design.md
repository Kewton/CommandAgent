# Issue #203 design: final tracker reconciliation

## Objective

Create an auditable, repository-local completion record for tracking Issue
#203. Reconcile its twenty direct children, #204-#223, against current GitHub
state and the merged W1-W6 evidence already present on `develop`.

## Approach

- Capture the current Issue state, close reason, close time, and canonical URL
  for every direct child with read-only `gh issue view` queries.
- Map each child to the Wave slice that delivered it, preserving combined rows
  where several Issues intentionally share one report directory.
- Bind the reconciliation to the exact audited remote `develop` commit and
  prove each cited implementation/evidence commit is its ancestor.
- Summarize W1-W6 from the closed Wave epics and their merge, CI, acceptance,
  smoke, and honest-failure records. Report stale tracker metadata explicitly
  instead of treating it as implementation work.
- Inspect the committed #146 and #173 predecessor reconciliations, but do not
  merge their report-only commits or treat them as part of the #203 evidence.

## Scope and constraints

Only `dev-reports/issue-203/` will change. Production code, tests, repository
documentation, historical evidence under `workspace/management/runs/` and
`docs/migration/`, the live `.anvil/` namespace, and GitHub Issue lifecycle or
body state remain unchanged. No runtime verification is needed because this
Issue changes no runtime contract; verification will check the report
contract, exact scope, current GitHub states, commit ancestry, and whitespace.

## Completion rule

The tracker is implementation-complete when all direct children are `CLOSED`
with reason `COMPLETED`, their merged evidence is reachable from the audited
remote `develop`, and the cumulative W6 record demonstrates final current-tree
CI, acceptance, smoke, polling, plan-run honest failure, and documentation
artifact verification. Open or stale parent tracker metadata is a
reconciliation finding and is not mutated by this task.
