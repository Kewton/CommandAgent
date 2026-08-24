# Design: Issue #232 (combined with #255 and #229)

The authoritative combined design is recorded in `dev-reports/issue-255/design.md`.

For #232 specifically, `--runs` gains detail, event, filter, and JSON projections without changing stored event schemas. `--trace` records opt-in, scrubbed provider request/reply artifacts below the selected run directory through one shared provider-call integration point.
