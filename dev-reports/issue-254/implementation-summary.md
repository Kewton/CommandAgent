# Issue #254 Implementation Summary

## Outcome

Implemented the first read-only MCP tool extension path without changing the
default built-in registry or adding a mutable/process-launching extension
surface.

## Changes

- Added `src/tools/extension.rs` with:
  - a closed argument vocabulary for strings, integers, booleans, and existing
    workspace paths;
  - generated function schemas with `additionalProperties: false`;
  - declaration and duplicate validation;
  - read-class `--allow` enforcement, declared network `--offline`
    enforcement, and existing workspace/hidden-path confinement;
  - normalized workspace-relative arguments at the client boundary;
  - a 64 KiB result bound; and
  - scrubbed `extension_tool_call`, `extension_tool_rejected`, and
    `extension_tool_result` events.
- Added `src/tools/mcp.rs` with the injected read-only `McpClient` contract and
  an MCP-to-extension adapter. This slice does not spawn an MCP server or add a
  mutable MCP method.
- Added minimal `ToolRegistry` registration/spec/dispatch wiring and a
  recoverable classification for extension preflight rejections. Existing
  default tool specs and built-in dispatch remain byte-for-byte unchanged.
- Registered the extension workspace-path helper in
  `tests/protection_coverage_audit.rs`.
- Added focused tests for successful Plan-mode dispatch, closed schemas,
  absolute-path normalization, explicit allow rejection, offline rejection,
  traversal and hidden-path rejection, result bounds, event records, and name
  collisions.
- Added `tests/corpus/apps/issue254-read-only-mcp` to pin the additive event
  vocabulary.

## Scope kept unchanged

- `docs/dev/extension-catalog.md` was not edited; row #256 owns it.
- No runner chokepoint, CLI/config surface, extension-root supply path,
  `.anvil` runtime state, existing event schema, or historical evidence was
  changed.
