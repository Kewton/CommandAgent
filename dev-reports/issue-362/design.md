# Issue #362 design: keep generated completion contracts inside isolated workspaces

## Problem

GUI Trial now runs each confirmed session in
`<execution-root>/sessions/<session-id>/`, while it intentionally keeps the
session event stream and other GUI run records in
`<execution-root>/.commandagent/runs/<session-id>/`. The planner currently puts
an automatically generated completion contract beside `eval_events_path`.
That makes the GUI contract a sibling of the execution workspace. When a
`core-implementation` step loads the contract, the existing safety boundary
correctly rejects the file because it is outside the workspace (and outside an
allowed system temp root in a normal installation).

## Design

- Add a small planner leaf module that chooses the generated completion-contract
  directory. Preserve the existing event-adjacent location only when the event
  directory canonically resolves inside the CLI execution workspace.
- When events are outside that workspace, generate the contract under the
  workspace-owned `.commandagent/` directory. This applies equally to
  `plan-run` contracts used by implementation steps and `ultra-plan-run`
  contracts used by initial and Gate 3/4 continuation runs.
- Keep explicit completion-contract loading and its workspace-or-temp
  normalization unchanged. In particular, an explicitly supplied contract
  outside both allowed boundaries remains rejected.
- Do not move GUI confirmations, events, state, summaries, session indexes, or
  lease records. No API or event schema changes.

## Tests and verification

- Adapt the focused plan-run regression to use the GUI split between a session
  execution workspace and a central event directory, and assert that the
  generated `plan-run` contract remains loadable from the session workspace.
- Adapt the ultra final-acceptance regression to the same split and assert the
  `ultra-plan-run` contract location used by initial and continuation runs.
- Add leaf tests for preserving an event-adjacent path within a workspace and
  rejecting an event directory that canonically escapes through a symlink.
- Run the focused runner tests and GUI delegation/lifecycle regression first,
  then formatting, Clippy, the full Rust suite, and the provider-free GUI smoke.
