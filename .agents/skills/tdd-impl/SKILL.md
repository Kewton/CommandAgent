---
name: tdd-impl
description: Implement a scoped CommandAgent behavior using a red-green-refactor cycle and proportional verification. Use when the user asks for TDD implementation of an issue, feature, regression fix, or focused behavior change.
---

# CommandAgent TDD Implementation

Implement the requested behavior; keep every cycle small and observable.

## Workflow

1. Read the issue, design/work plan, acceptance criteria, and relevant guardrails.
2. Inspect the smallest code/test surface and confirm the current behavior.
3. Write or update `dev-reports/issue-<number>/design.md` before production edits when an issue number is available.
4. **Red**: add the smallest test that fails for the intended reason. Run it and record the failure.
5. **Green**: implement the smallest coherent change that passes that test.
6. **Refactor**: improve naming/structure without changing behavior; rerun the focused test.
7. Repeat until all acceptance criteria are covered.
8. Run broader checks according to shared-surface risk.

For Rust, normally finish with:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Use focused commands first. Follow `docs/dev-guardrails.md`; do not grow protected chokepoints when a focused module is appropriate. Preserve event schemas and public behavior outside the requested scope.

## Completion

Write issue implementation and verification reports when applicable. Report red/green evidence, changed files, tests, residual risks, and blockers. Commit only when the user's task includes committing.
