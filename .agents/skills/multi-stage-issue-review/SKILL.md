---
name: multi-stage-issue-review
description: Review a CommandAgent issue in independent specification, evidence, and impact passes before implementation. Use for high-risk, ambiguous, cross-cutting, or release-sensitive issue definitions.
---

# CommandAgent Multi-stage Issue Review

Run distinct review passes and keep their findings traceable. Do not simulate independent agents or dispatch CommandMate unless the user explicitly requests that coordination.

## Setup

Read the issue and save review artifacts under `dev-reports/issue-<number>/issue-review/`. Never overwrite an existing review run; add a timestamped subdirectory when needed.

## Stages

1. **Hypothesis validation**: list factual claims and suspected causes, then confirm, refute, or mark each unknown using code and test evidence.
2. **Specification review**: evaluate objective, scope, non-goals, terminology, reproduction, and testability of acceptance criteria.
3. **Impact review**: inspect affected modules, public behavior, profile/corpus data, event compatibility, migration risk, and test coverage.
4. **Revision pass**: produce a proposed issue body that addresses accepted findings without inventing requirements.
5. **Fresh consistency pass**: reread only the revised body plus key evidence and look for contradictions, hidden dependencies, or overreach.
6. **Readiness decision**: classify as `READY`, `CONDITIONAL`, or `BLOCKED` with concrete reasons.

For planner and minimal-loop changes, check `docs/dev-guardrails.md`. For event-producing changes, explicitly assess schema and consumer compatibility.

## Outputs

Record the original issue snapshot, hypothesis table, stage findings, proposed revision, and final readiness summary. Update the GitHub issue only with explicit authorization; otherwise leave the revision as a local proposal.
