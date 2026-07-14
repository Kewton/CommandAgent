---
name: multi-stage-design-review
description: Run independent staged reviews over a CommandAgent design policy. Use for cross-cutting, security-sensitive, compatibility-sensitive, or otherwise high-risk designs that need more than one review lens.
---

# CommandAgent Multi-stage Design Review

Use sequential, independent passes with separate evidence. Do not spawn reviewers or dispatch CommandMate unless the user explicitly requests multi-agent execution.

## Setup

Read the issue, design policy, relevant code/tests, and `docs/dev-guardrails.md`. Store a new review run under `dev-reports/issue-<number>/design-review/<timestamp>/`.

## Stages

1. **Design principles**: responsibility boundaries, cohesion, coupling, simplicity, duplication, and unnecessary abstraction.
2. **Repository consistency**: fit with current planner/minimal-loop/tool/provider/profile patterns and existing contracts.
3. **Impact and compatibility**: public CLI/config behavior, event consumers, corpus/golden data, migrations, rollback, and downstream effects.
4. **Security and resilience**: untrusted input, secret/path handling, process bounds, failure modes, recovery, and observability.
5. **Verification adequacy**: acceptance-to-test mapping, focused tests, integration scenarios, and guardrail checks.

After every stage, record findings without editing the design. Then create a consolidated disposition proposal, deduplicating overlapping findings and highlighting conflicts between stages.

## Output

Write one report per stage and `summary.md` with severity, evidence, required design changes, residual risks, and `APPROVED`, `CONDITIONAL`, or `REJECTED`. Use `$apply-review` to update the design after the user accepts the findings.
