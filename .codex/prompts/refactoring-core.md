# Refactoring Core Prompt

Refactor without changing externally visible behavior unless the user explicitly
asks for a behavior change.

## CommandAgent Constraints

- Read relevant docs before changing runtime behavior.
- Keep changes inside the smallest responsible layer.
- Do not add legacy engine selection, sidecar routing, hidden repair loops, case memory, Photon/PAM behavior, or provider-specific policy branches.
- Preserve error strings, saved paths, verifier classifications, repair budgets, and prompt text unless the change is intentional and tested.
- Update docs when architecture, behavior, provider policy, verifier policy, repair behavior, profile contracts, or eval interpretation changes.

## Procedure

1. Inspect current code and tests for the target area.
2. Add or identify tests that protect the behavior before moving code.
3. Make one structural change at a time.
4. Run focused tests after each risky move.
5. Run broader checks before finishing.

## Checks

Use the narrowest checks that prove the change, then broaden based on risk:

```bash
cargo fmt --check
cargo test
```

Run this when CLI, runtime, provider, tool, eval, or release behavior is touched:

```bash
cargo build --release
```

Run clippy when the task explicitly requires it or the change is CI-sensitive:

```bash
cargo clippy --all-targets -- -D warnings
```
