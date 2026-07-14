---
name: pm-auto-design2dev
description: Take a CommandAgent issue from design policy through reviewed implementation and verification. Use when the issue definition is ready but design, planning, and development all remain.
---

# CommandAgent Design To Development

Run gated stages in the current agent. Do not create PRs or use other agents unless the user explicitly includes those actions.

## Stages

1. **Design policy**: create or refresh `dev-reports/issue-<number>/design-policy.md` using the `$design-policy` workflow.
2. **Design review**: use `$multi-stage-design-review` for high-risk/cross-cutting work or `$architecture-review` for a focused review.
3. **Review disposition**: apply accepted findings with `$apply-review`; stop if the design remains rejected or blocking decisions are unresolved.
4. **Work plan**: produce an acceptance-mapped, file-aware plan with `$work-plan`.
5. **Development**: execute `$pm-auto-dev` stages: TDD, focused review, acceptance verification, docs, and broad checks.
6. **Final gate**: confirm all issue criteria, design decisions, and verification evidence agree.

Keep stages proportional. A small, low-risk issue may use a focused architecture review instead of a multi-stage review, but record that choice. Respect `docs/dev-guardrails.md` and preserve event schemas unless explicitly in scope.

## Completion

Summarize every stage, generated report, code/test change, quality check, residual risk, and pull-request readiness. Do not publish or merge without separate authorization.
