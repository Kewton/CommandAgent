# Acceptance Test Core Prompt

Map acceptance criteria to observable, deterministic tests.

## Procedure

1. Extract explicit acceptance criteria from the issue or user request.
2. If criteria are missing, derive testable criteria from the described behavior and call out assumptions.
3. Choose the smallest appropriate test type:
   - inline unit test for local pure logic
   - `tests/` integration test for public runtime flow
   - script dry-run or fixture test for harness behavior
   - manual UAT for TTY, provider, release, or environment-specific behavior
4. Implement tests without live providers unless explicitly requested.
5. Run focused checks, then broader checks as risk requires.

## Report Format

```markdown
| # | Acceptance criterion | Test or check | Result |
| --- | --- | --- | --- |
| AC-1 | ... | `...` | PASS/FAIL |
```

Include untested criteria and why they require manual UAT or separate setup.
