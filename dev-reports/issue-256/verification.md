# Issue 256 verification

- Status: `passed`

## Checks

- `rg -n '^## (Extend a task family|Add an intent)$' docs/dev/extension-catalog.md`: `passed`
- `cargo test --test doc_drift`: `passed`
- `git diff --cached --check`: `passed`
