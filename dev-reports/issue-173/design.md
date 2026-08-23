# Issue #173 design: final tracker reconciliation

## Objective

Create an auditable, repository-local completion record for tracking Issue
#173. Reconcile direct children #153, #172, and #174-#202 against current
GitHub state and the merged W1-W6 evidence already present on `develop`.

## Approach

- Capture the current state, close reason, close time, and canonical URL for
  every direct child with read-only `gh issue view` queries.
- Map each child to the Wave and combined delivery row that implemented it,
  including cases where multiple Issues intentionally share one report.
- Bind the reconciliation to the exact audited remote `develop` commit and
  prove that every cited delivery and Wave completion commit is its ancestor.
- Summarize W1-W6 from the closed Wave epics and their merge, exact-SHA CI,
  acceptance, smoke, polling, honest-failure, and final UAT records.
- Record stale parent-tracker metadata as a finding without editing GitHub.

## Scope and constraints

Only `dev-reports/issue-173/` will change. Production code, tests, repository
documentation, historical evidence under `workspace/management/runs/` and
`docs/migration/`, the live `.anvil/` namespace, and GitHub Issue lifecycle or
body state remain unchanged.

No runtime verification is required because this Issue changes no runtime
contract. Verification will check current GitHub states, remote/local revision
identity, merged pull requests and Actions results, commit ancestry, committed
child verification evidence, exact report-only scope, and whitespace.

## Completion rule

The tracker is implementation-complete when all 31 direct children are
`CLOSED` with reason `COMPLETED`, their merged delivery evidence is reachable
from the audited `develop`, and the cumulative W6 record proves final
current-tree CI, acceptance, release build, two-base-path GUI smoke, ten-minute
polling, honest standalone plan-run behavior, and README GIF verification.

An open or stale parent tracker is a reconciliation finding, not authority to
mutate GitHub state. This task records the finding and leaves it unchanged.
