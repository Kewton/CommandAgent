# Issue #155 design: final tracker reconciliation

## Objective

Create an auditable, repository-local completion record for tracking Issue
#155. Reconcile its direct children #156-#172 against current GitHub state and
the merged W1-W6 evidence already present on `develop`.

## Approach

- Capture the current Issue state, close reason, close time, and canonical URL
  for every direct child with read-only `gh issue view` queries.
- Map each child to the Wave slice that delivered it, including combined rows
  where #163/#170, #167, or residual GUI work is recorded under another
  Issue's report directory.
- Bind the reconciliation to the exact audited remote `develop` commit and
  prove each cited merge commit is its ancestor.
- Summarize W1-W6 from the closed Wave epics and their merge, CI, acceptance,
  smoke, polling, and honest-failure records.
- Report stale parent-tracker metadata explicitly instead of treating it as
  implementation work.

## Scope and constraints

Only `dev-reports/issue-155/` will change. Production code, tests, repository
documentation, historical evidence under `workspace/management/runs/` and
`docs/migration/`, the live `.anvil/` namespace, and GitHub Issue lifecycle
or body state remain unchanged.

No runtime verification is required because this Issue changes no runtime
contract. Verification will check the report contract, exact file scope,
current GitHub states, remote/local revision identity, commit ancestry,
committed child verification evidence, and whitespace.

## Completion rule

The tracker is implementation-complete when all 17 direct children are
`CLOSED` with reason `COMPLETED`, their merged delivery evidence is
reachable from the audited `develop`, and the cumulative W6 record shows
final current-tree CI, acceptance, release build, two-base-path GUI smoke,
ten-minute polling, honest standalone plan-run behavior, and README GIF
verification.

An open or stale parent tracker is a reconciliation finding, not authority to
mutate GitHub state. This task records that finding and leaves it unchanged.
