# Issue #233 design: final tracker reconciliation

## Objective

Create an auditable, repository-local completion record for tracking Issue
#233. Reconcile its ten direct children, #234-#243, against current GitHub state
and the merged W1-W6 evidence reachable from the audited `develop` commit.

## Approach

- Capture each direct child's current state, close reason, close time, and
  canonical URL with read-only GitHub queries.
- Map every child to its delivery Wave and merged pull request, retaining the
  intentional combined W2 delivery and report evidence for #234 and #235.
- Bind every cited delivery merge and W1-W6 completion commit to the exact
  audited `develop` SHA through Git ancestry checks.
- Summarize the closed Wave epics' exact-SHA CI, acceptance, smoke, and
  honest-failure evidence, and identify W6 as having no direct #233 child.
- Record stale tracker or roadmap metadata as an audit finding without
  modifying GitHub state.

## Scope and constraints

Only `dev-reports/issue-233/` will change. Production code, tests, repository
documentation, historical evidence under `workspace/management/runs/` and
`docs/migration/`, the live `.anvil/` namespace, and GitHub Issue lifecycle or
body state remain unchanged.

The required predecessor commits for Issues #146, #155, #173, #203, and #224
will be inspected in their dedicated branches. Their report-only changes will
not be merged or copied into this branch because they do not alter the #233
child ledger or supply missing runtime behavior.

## Completion rule

The tracker is implementation-complete when all direct children are `CLOSED`
with reason `COMPLETED`, every cited child-delivery merge and Wave completion
commit is reachable from the audited `develop`, and the cumulative W6 record
demonstrates current-tree CI, acceptance, release build, full GUI smoke,
ten-minute polling, plan-run honest-failure handling, and README GIF
verification. Open or stale tracker metadata is a reconciliation finding, not
authorization for a lifecycle or body mutation.
