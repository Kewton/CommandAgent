---
name: apply-review
description: Apply accepted CommandAgent architecture or design review findings to an issue design document. Use when review findings exist and the user wants the design updated before implementation.
---

# CommandAgent Apply Review

Update design artifacts while preserving a traceable disposition for every finding. Do not implement production code in this skill.

## Workflow

1. Read `dev-reports/issue-<number>/architecture-review.md`, the current design policy, issue, and cited code evidence.
2. Build a disposition table for each finding:
   - `accepted`: apply to the design
   - `deferred`: record reason, owner, and follow-up condition
   - `rejected`: record evidence-based rationale
   - `needs-decision`: stop on a genuinely blocking user choice
3. Apply accepted changes to `dev-reports/issue-<number>/design-policy.md`.
4. Update affected sections such as boundaries, data flow, compatibility, security, risks, alternatives, or verification.
5. Add a review-application history section with date, source review, and disposition summary.
6. Recheck internal consistency and ensure no accepted finding was lost.

Do not turn reviewer suggestions into requirements without checking issue scope. Do not silently broaden the implementation. Preserve event schemas and guardrail budgets unless the issue explicitly authorizes their change.

## Output

Write `dev-reports/issue-<number>/review-application.md` summarizing accepted, deferred, rejected, and unresolved findings. Report whether the design is ready for `$work-plan` or needs another `$architecture-review`.
