# Design: Issue #229 (combined with #255 and #232)

The authoritative combined design is recorded in `dev-reports/issue-255/design.md`.

For #229 specifically, new workspace runtime writes target `.commandagent`, default session writes target the platform `commandagent` state directory, and read paths preserve `.anvil` and `anvilminimal` compatibility. Runs are discoverable across both namespaces with canonical entries taking precedence.
