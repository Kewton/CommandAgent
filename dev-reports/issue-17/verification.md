# Issue #17 Verification

- Status: `passed`

## Checks

- `rg -n 'Option A|data-anvil-\*|<anvil_tool_call>|anvil_app|\.anvil/|JSON keys、event names、schemas' docs/mechanism-ledger.md`: `passed`
- `git diff --cached --check 6cc03bdaec803b8af874eb60d8cc2c3066634b5c`: `passed`
- `git diff --cached --name-only 6cc03bdaec803b8af874eb60d8cc2c3066634b5c`: `passed`
