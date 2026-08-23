# Issue #352 implementation summary

## Implemented

- Added a per-session execution workspace at
  `<execution-root>/sessions/<session-id>/`. The confirmed initial delegate
  creates and validates this directory, then supplies it as both process cwd and
  CLI `--cwd`.
- Kept GUI-owned confirmations, events, summaries, directives, session index,
  and lease recovery under the existing canonical
  `<execution-root>/.commandagent/runs/<session-id>/` path, with legacy
  `.anvil/runs` reads unchanged.
- Revalidated session workspace parents and leaves as canonical real
  directories, rejected symlinked `sessions/` roots, and limited spawn-failure
  rollback to the empty UUID directory created by the confirmed dispatch.
- Routed Gate 3/4 continuation plan creation and CLI execution back into the
  original session workspace.
- Added a caller-scoped deterministic-route inventory exclusion and applied it
  only to GUI Gate 1's reserved `sessions/` subtree. Existing non-GUI routing
  retains its previous inventory behavior.
- Updated the provider-free GUI smoke harness to read delegated arguments from
  the canonical `.commandagent/runs` write namespace.
- Documented the execution-workspace/run-record split in `gui-setup.md` and
  `gui-history.md`.

## Tests

- Added a two-session GUI integration regression that records the delegated cwd
  and `--cwd`, creates source plus plans/evidence/repairs, verifies every output
  is under the first session directory, and proves the second Gate 1 ignores a
  route-conflicting first-session artifact.
- Added a symlink-root rejection regression and a deterministic-route unit test.
- Extended the existing end-to-end delegation/directive test to prove a Gate
  3/4 continuation plan remains in the same session workspace.
- Existing GUI integration coverage continued to pass for session indexing,
  lease recovery, artifacts/events APIs, event-byte compatibility, and terminal
  Gate 3/4 projection.

## Compatibility

No API request/response shape, event name/schema, session ID, lease projection,
central run-record layout, or legacy `.anvil/` read contract changed.
