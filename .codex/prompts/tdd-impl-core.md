# TDD Implementation Core Prompt

Use Red-Green-Refactor when the task is implementation work and tests can be
written before or alongside the change.

## Red

1. Clarify the behavior or contract.
2. Add a focused failing test.
3. Run the narrowest test command that proves the failure.

## Green

1. Implement the smallest change that satisfies the test.
2. Run the focused test again.
3. Keep behavior inside the responsible CommandAgent layer.

## Refactor

1. Improve structure while tests remain green.
2. Avoid broad rewrites or unrelated cleanup.
3. Update docs when behavior or boundaries changed.

## Test Placement

- Use inline unit tests near private logic.
- Use `tests/` integration tests for public runtime flows, CLI smoke paths, or cross-module contracts.
- Keep live providers out of normal tests.
- Use temp workspaces and mock chat clients for deterministic coverage.

## Final Checks

```bash
cargo fmt --check
cargo test
```

Add `cargo build --release` for CLI, runtime, provider, tool, eval, release, or harness execution changes.
