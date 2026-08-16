# Issue 65 Design: tracking audit snapshot

## Decision and scope

Issue #65 is a tracking issue. It adds no product behavior and does not merge
the child branches. This worker records only the current audit snapshot under
`dev-reports/issue-65/`; it does not modify historical evidence under
`workspace/management/runs/` or the live `.anvil/` namespace.

The authoritative child-to-inventory mapping is the table in the Issue #65
body. Its 17 child Issues cover every item A-1 through A-5, B-6 through B-17,
C-18 through C-26, and D-27 through D-31.

## Audit model

The audit keeps two judgments separate:

1. **Worker snapshot verification** may pass when every child branch has a
   committed `dev-reports/issue-<n>/verification.md` with `Status: passed`, the
   recorded head SHA matches the inspected commit, the mapping is complete,
   and this change is confined to new Issue #65 reports.
2. **Issue #65 final acceptance** remains pending until every child Issue has
   passing PR CI, passing orchestrator UAT, and a completed merge into
   `develop`. A child worker's local smoke or test result is not substituted for
   any of those external gates.

The correspondence table will therefore show four independent columns for
each child: committed worker verification, PR CI, UAT, and `develop` merge.
The initial snapshot records the first as passed and the other three as pending,
as directed by the owner. It explicitly states that Issue #65 must remain open.

## Verification plan

- Re-read all 17 verification reports from their committed branch heads and
  require the exact passed status marker.
- Verify that none of the 17 head commits is an ancestor of `develop` or this
  Issue #65 branch at the snapshot time.
- Check the A-1...D-31 mapping for gaps and unknown item IDs. B-6 is the one
  intentional overlap: Issue #77 owns its focused style/a11y part and Issue
  #78 owns the overall one-screen/one-state redesign.
- Check Markdown whitespace and require the working diff to contain only the
  three new `dev-reports/issue-65/` files.

Because this is documentation-only and changes no shared behavior or contract,
Rust, GUI, corpus, release, and runtime tests are not required for this worker
snapshot.
