# Issue 65 Design: final tracking audit

## Decision and scope

Issue #65 is the tracking source for the GUI UX inventory. It adds no product
behavior. This final audit is performed only after Issues #63, #64, and #66
through #80 have passed their worker verification, PR CI, orchestrator UAT, and
merge gates.

The latest `develop` is integrated before this report is finalized. Relative to
that integrated base, this PR changes only the three Issue #65 reports. It does
not rewrite historical evidence under `workspace/management/runs/` or the live
`.anvil/` namespace.

The authoritative child-to-inventory mapping remains the table in the Issue #65
body. Its 17 child Issues cover every item A-1 through A-5, B-6 through B-17,
C-18 through C-26, and D-27 through D-31.

## Final audit model

Final acceptance requires four independent gates for every child:

1. the committed child verification on `develop` has `Status: passed`;
2. the child PR's latest GitHub checks have no failure or pending result;
3. the orchestrator UAT report records every child scenario as passed;
4. the child PR merge commit is an ancestor of the integrated `develop` head.

Worker-local smoke or test evidence is not substituted for PR CI or UAT. The
correspondence table records the live PR number/head and merge commit so the
final judgment remains auditable.

## Verification plan

- Read the 17 committed `dev-reports/issue-<n>/verification.md` files from the
  integrated `develop` tree and require the exact passed status marker.
- Query PRs #82, #83, and #85 through #99, excluding this audit PR #84; require
  `MERGED` and only successful or intentionally skipped checks.
- Require all 17 reported merge commits to be ancestors of `develop`.
- Inspect the externally retained orchestrator evidence for run
  `20260816-105037-orchestrate` read-only; require its UAT report to contain 17
  child headings, 86 passed scenarios, and an overall passed gate. Do not copy
  or rewrite that untracked historical evidence into this branch.
- Check the A-1...D-31 mapping for gaps and unknown IDs. B-6 is the intentional
  overlap: Issue #77 owns its focused style/a11y part and Issue #78 owns the
  overall one-screen/one-state redesign.
- Require the diff against integrated `develop` to contain only these three
  Issue #65 Markdown files and to pass whitespace checks.

Because the final audit diff is documentation-only and changes no shared
behavior or contract, no additional Rust, GUI, corpus, release, or runtime test
is required for PR #84 itself. The integrated child code has already passed its
own CI, UAT, and merge gates.
