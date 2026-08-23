# Issue #352 design: isolate GUI Trial workspaces by session

## Current behavior

The GUI server keeps each session's confirmation, event stream, summary, and
directive state in `.commandagent/runs/<session-id>`, but delegates the product
CLI with the configured execution root itself as both the process working
directory and `--cwd`. Generated source and workspace-owned
`.commandagent/plans`, `evidence`, and `repairs` therefore share one directory
across every Trial. Gate 1 also inventories that shared root, so prior outputs
can become routing evidence for a later proposal.

## Design

- Extend the GUI-owned `SessionPaths` value with the execution workspace
  `<execution-root>/sessions/<session-id>`. Keep the existing run root at
  `<execution-root>/.commandagent/runs/<session-id>` so session indexing, lease
  recovery, confirmation lookup, events, summaries, artifact APIs, and event
  schemas retain their current contracts.
- Create the session workspace only inside the already-confirmed dispatch path.
  Require `sessions/` to be a real directory, create the UUID directory with
  create-new semantics, and revalidate it as a real canonical directory before
  every delegated initial or continuation command. If the initial process
  cannot spawn, remove only the newly created session workspace while the
  existing unstarted-run rollback removes the central run record.
- Run the delegated CLI with the session workspace as both process cwd and
  `--cwd`. Continue to set `--state-dir` and `COMMANDAGENT_EVAL_EVENTS` to the
  central run root so GUI lifecycle projections do not move. Prepare Gate 3/4
  continuation plans inside the same session workspace.
- Add an opt-in top-level inventory exclusion to deterministic routing. Existing
  callers retain today's behavior; GUI Gate 1 excludes only the reserved
  `sessions` subtree. The Gate 1 identity continues to freeze the configured
  execution-root boundary, while prior session products no longer contribute
  route or family observations.
- Add a focused two-session integration regression. Its first delegated CLI
  writes source and `.commandagent` products in its cwd; the test verifies
  their session-scoped placement and then proves a second Gate 1 proposal is
  unaffected by a first-session artifact that would otherwise create an
  ambiguous route.
- Document the split between session workspaces and central GUI run records in
  `gui-setup.md` and `gui-history.md`.

## Compatibility and verification

No event name/schema, API request/response shape, session ID, lease, or live
`.anvil/` compatibility contract changes. Focused verification will cover the
new two-session behavior plus the existing delegation, recovery, session-index,
artifact/event, and directive continuation integration tests. Because Rust
routing and GUI server behavior are shared/CI-sensitive, run formatting,
Clippy, and the full Rust suite, followed by GUI lint, typecheck, and the
provider-free smoke command.
