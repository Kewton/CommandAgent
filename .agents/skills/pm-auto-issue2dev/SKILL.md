---
name: pm-auto-issue2dev
description: Take a CommandAgent issue from specification review through design, implementation, and acceptance verification. Use for an end-to-end local delivery flow when the initial issue may still be ambiguous or incomplete.
---

# CommandAgent Issue To Development

Use explicit readiness gates; do not silently implement an unverified issue assumption.

## Stages

1. **Issue review**: run the `$multi-stage-issue-review` workflow and validate hypotheses against current code.
2. **Issue readiness**: prepare the improved local specification. Update GitHub only when the user explicitly authorizes it.
3. **Design**: create the issue design policy and assess compatibility, guardrails, security, and alternatives.
4. **Design review/disposition**: review independently and apply accepted findings. Stop if the design is rejected or a blocking choice remains.
5. **Plan**: create an executable `$work-plan` mapped to acceptance criteria.
6. **Develop**: execute the `$pm-auto-dev` workflow through TDD, focused review, acceptance, docs, and broad verification.
7. **Final consistency**: compare the delivered behavior, revised specification, design decisions, and evidence.

Execute stages in the current agent unless the user explicitly requests subagents or CommandMate. Do not push, create pull requests, merge, or alter unrelated issue state as an implicit side effect.

## Completion

Report issue readiness, all generated artifacts, implementation and tests, criteria status, remaining risk, and readiness for publication.
