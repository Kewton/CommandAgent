# CommandAgent Issue Worker Template

Use this template when constructing or interpreting an issue-worker dispatch.

## Objective

Implement the assigned issue according to its body, acceptance criteria, and orchestration notes.

## Checklist

1. Read the issue summary, acceptance criteria, orchestration notes, and relevant files.
2. Write a short design note to `dev-reports/issue-<number>/design.md` before implementation edits.
3. Make the smallest coherent implementation.
4. Add or update focused tests where needed.
5. Run focused verification first, then broader verification if shared behavior changed.
6. Write `implementation-summary.md` and `verification.md` in the same issue report directory. Use the exact overall line `- Status: passed` and one `<command>: passed` line per check, with each status formatted as inline code in the report. Use `blocked` honestly for failed or unavailable checks.
7. Commit only when authorized, using a clear issue-scoped message.
8. Report changed files, tests and results, readiness, and blockers.

Prefer focused `cargo test` checks for Rust changes. Add Clippy, formatting, CLI snapshot checks, or Python harness tests according to the touched surface and risk.
