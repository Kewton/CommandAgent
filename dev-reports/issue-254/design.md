# Issue #254 Design

## Decision

Add a programmatic, read-only MCP extension path. An embedding supplies an
`McpClient` implementation and registers one or more closed `ExtensionTool`
definitions with `ToolRegistry`. The first slice intentionally does not add a
stdio process launcher, mutable extension operation, configuration discovery,
or extension-root write path.

## Shape

- `src/tools/extension.rs` owns the closed argument vocabulary, schema
  projection, declaration validation, policy preflight, workspace-path
  normalization, bounded result handling, and extension call/result/rejection
  events.
- `src/tools/mcp.rs` adapts an injected `McpClient` to a read-only extension
  executor. The MCP request contains only the declared server ID, remote tool
  name, and validated arguments.
- `src/tools/registry.rs` gains only extension registration, specification
  projection, name lookup, and dispatch wiring. The default built-in registry
  remains unchanged.

## Safety contracts

- Extension arguments use a closed type set: string, integer, boolean, or an
  existing workspace path. Unknown arguments and type mismatches are rejected
  before client dispatch.
- Existing workspace-path arguments pass through the current normalization,
  symlink confinement, hidden-path, and `WorkspacePolicy` checks. The MCP
  client receives normalized workspace-relative paths.
- Read-only extensions are authorized as the existing read tool class, so an
  explicit `--allow` policy remains a hard ceiling.
- Each declaration states whether it requires network access. `--offline`
  rejects network-backed declarations before MCP client dispatch.
- Every attempted extension call emits `extension_tool_call`. Pre-dispatch
  failures emit `extension_tool_rejected`; dispatched outcomes emit
  `extension_tool_result` without persisting result bodies.
- Tool names cannot collide with built-ins or another extension, and result
  size is bounded before it can enter model context.

## Verification

- Focused module tests will cover registration/spec projection, successful
  dispatch, explicit allow rejection, offline rejection, workspace escape and
  hidden-path rejection, closed-schema rejection, duplicate names, result
  bounds, and event records.
- The protection audit will register the new workspace-path boundary.
- A corpus fixture will pin the new event vocabulary.
- Run focused tool/protection/corpus tests, then formatting, Clippy, and the
  full Rust test suite because the shared tool registry contract changes.

## Non-goals

- No edits to `docs/dev/extension-catalog.md` (owned by row #256).
- No mutable MCP tools, raw command templates, shell execution, runtime
  extension discovery, or `.anvil` state migration.
