---
name: acceptance-test
description: Map a CommandAgent issue's acceptance criteria to executable tests and evidence, then run the appropriate verification. Use when the user asks to validate whether an implementation satisfies an issue or to add missing acceptance coverage.
---

# CommandAgent Acceptance Test

Treat every acceptance criterion as a claim that needs observable evidence.

## Workflow

1. Read the issue, implementation reports, changed files, and existing tests.
2. Build a matrix with criterion, test/evidence, command, expected result, and status.
3. Reuse existing tests when they directly prove a criterion.
4. Add focused automated tests only when coverage is missing and the request authorizes implementation changes.
5. Define manual UAT for CLI/TTY, GUI, release, timing, or real-device behavior that automation cannot prove.
6. Run focused checks first, then the broader suite required by shared-surface risk.
7. Record failures honestly; do not mark a criterion passed from indirect or stale evidence.

Common Rust gates are:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Also run relevant corpus, conformance, scenario, or snapshot tests when those contracts are affected.

## Output

Write `dev-reports/issue-<number>/acceptance-test.md` when an issue-scoped artifact is appropriate. Include the criterion matrix, commands/results, manual checks, evidence gaps, and final `PASS`, `FAIL`, or `BLOCKED` decision. Use `$codex-uat` for a full UAT report.
