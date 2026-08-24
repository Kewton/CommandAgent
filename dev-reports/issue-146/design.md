# Issue #146 design: final tracker reconciliation

## Objective

Create an auditable, repository-local completion record for tracking Issue
#146. Reconcile its direct children #147-#154 against current GitHub state and
the merged W1-W6 evidence already present on `develop`.

## Approach

- Capture the current Issue state, close reason, close time, and canonical URL
  for every direct child with read-only `gh issue view` queries.
- Map each child to the Wave slice that delivered it, including combined rows
  where #149, #151, or residual GUI work was recorded under another Issue's
  report directory.
- Bind the reconciliation to the exact audited `origin/develop` commit and
  prove each cited implementation/evidence commit is its ancestor.
- Summarize W1-W6 from the closed Wave epics and their merge, CI, acceptance,
  smoke, and honest-failure records. State stale tracker metadata explicitly
  instead of treating it as implementation work.

## Scope and constraints

Only `dev-reports/issue-146/` will change. Production code, tests, repository
documentation, historical evidence under `workspace/management/runs/` and
`docs/migration/`, the live `.anvil/` namespace, and GitHub Issue lifecycle or
body state remain unchanged. No new runtime verification is needed because
this Issue changes no runtime contract; verification will check the report
contract, exact scope, current GitHub states, commit ancestry, and whitespace.

## Completion rule

The tracker is implementation-complete when all direct children are
`CLOSED` with reason `COMPLETED`, their merged evidence is reachable from the
audited `origin/develop`, and the cumulative W6 record shows final current-tree
CI, acceptance, GUI smoke, polling, plan-run honest-failure, and README GIF
verification. Open or stale parent tracker metadata is reported as a
reconciliation finding and is not mutated by this task.
