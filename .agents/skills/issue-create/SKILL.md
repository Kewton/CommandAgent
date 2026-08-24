---
name: issue-create
description: Draft a high-quality GitHub issue for CommandAgent and create it only when publication is explicitly authorized. Use when the user asks to propose, draft, or create a new issue.
---

# CommandAgent Issue Create

Turn a request into an evidence-backed issue. Separate drafting from the external action of creating the issue.

## Workflow

1. Identify whether the request is a feature, bug, refactor, documentation, performance, or maintenance issue.
2. Inspect the smallest relevant source, tests, docs, and existing issue context. Do not invent paths or current behavior.
3. Search open issues for likely duplicates when GitHub access is available.
4. Draft the issue body with:
   - summary and motivation
   - current and expected behavior for bugs
   - scoped requirements and explicit non-goals
   - likely affected modules and compatibility constraints
   - testable acceptance criteria
   - verification expectations
   - risks, dependencies, and related issues
5. Check that the issue is independently actionable and does not prescribe unsupported implementation details.
6. Present the draft. Run `gh issue create` only if the user explicitly asked to create/publish the issue.

Use repository `Kewton/CommandAgent`. Query existing labels before applying them; do not assume that a legacy label still exists.

## Quality Gates

- Every acceptance criterion is observable or verifiable.
- File and module references exist or are clearly marked as hypotheses.
- The scope is small enough for one coherent pull request; otherwise recommend `$issue-split`.
- Rust changes mention proportional checks such as `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and focused/full tests.
- Planner or minimal-loop changes acknowledge `docs/dev-guardrails.md` and its growth tripwires.

Report the final title, body summary, labels, duplicate-search result, and created URL when publication occurs.
