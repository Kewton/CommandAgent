# Issue #224 design: final tracker reconciliation

## Objective

Create an auditable, repository-local completion record for tracking Issue
#224. Reconcile its eight direct children, #225-#232, against current GitHub
state and the merged W1-W6 evidence reachable from the audited `develop`
commit.

## Approach

- Capture each direct child's current state, close reason, close time, and
  canonical URL with read-only GitHub queries.
- Map every child to its delivery Wave and merged pull request, retaining the
  intentional combined rows for #225 with #217/#219, #231 with #151, and
  #229/#232 with #255.
- Bind every cited delivery merge and W1-W6 completion commit to the exact
  audited `develop` SHA through Git ancestry checks.
- Summarize the closed Wave epics' exact-SHA CI, acceptance, smoke, and
  honest-failure evidence, and identify Waves with no direct #224 child.
- Record stale parent-roadmap metadata as an audit finding without modifying
  GitHub state.

## Scope and constraints

Only `dev-reports/issue-224/` will change. Production code, tests, repository
documentation, historical evidence under `workspace/management/runs/` and
`docs/migration/`, the live `.anvil/` namespace, and GitHub Issue lifecycle or
body state remain unchanged.

The required #203 predecessor commit `3204bdc0a76a21b1b569da39007c7a64523dfe34`
was inspected in its dedicated branch. It adds only the three
`dev-reports/issue-203/` reconciliation files, reports successful verification,
and is intentionally not merged or copied into this branch.

## Completion rule

The tracker is implementation-complete when all direct children are `CLOSED`
with reason `COMPLETED`, every cited child-delivery merge and Wave completion
commit is reachable from the audited `develop`, and the cumulative W6 record
demonstrates current-tree CI, acceptance, release build, full GUI smoke,
ten-minute polling, plan-run honest-failure handling, and README GIF
verification. Open or stale tracker metadata is a reconciliation finding, not
authorization for a lifecycle or body mutation.
