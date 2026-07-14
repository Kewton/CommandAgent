---
name: bug-fix
description: Reproduce, diagnose, fix, and verify a CommandAgent defect with a regression test. Use when the user asks to implement a fix for an issue number, failing test, crash, or described incorrect behavior.
---

# CommandAgent Bug Fix

Fix the proved cause with the smallest safe behavior change.

## Workflow

1. Read the issue or defect report, reproduction, logs, environment, and expected behavior.
2. Reproduce the failure or establish an equivalent deterministic signal. If evidence is insufficient, use the `$cause-analysis` workflow before editing.
3. Trace the execution path and distinguish the direct trigger from the root condition. Inspect similar call sites for the same risk.
4. Add the narrowest regression test and run it to confirm a meaningful failure (**Red**).
5. Implement the smallest coherent fix (**Green**). Do not bundle unrelated cleanup.
6. Refactor only when it reduces risk or makes the fix maintainable; keep the regression test green.
7. Run focused checks, then broader verification proportional to the touched contracts.

For Rust changes, normally include:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Follow `docs/dev-guardrails.md`. Preserve public CLI/config/event behavior outside the reported defect. Do not weaken tests or raise growth baselines to admit a fix.

## Completion

Report reproduction evidence, direct and root cause, regression test, fix, verification results, and residual risk. Write issue reports when an issue number is available. Commit only when requested.
