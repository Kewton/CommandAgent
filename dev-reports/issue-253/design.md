# Issue #253 design

## Goal

Extend the closed workflow-circle configuration to v0.2 so a registered
external draft profile can be selected for a node and a node can pin a planner
provider/model pair. Preserve v0 and v0.1 behavior and bytes, and prevent every
circle containing a draft node from being projected as `circle_full`.

## Predecessor compatibility

- Issue #240 adds role-specific model-probe evidence. The workflow declaration
  will use the same explicit provider/model pairing rule for the planner role;
  it will not alter probe schemas or classifier inheritance.
- Issues #227 and #221 change human/headless terminal summaries and SIGINT
  handling. Their commits are not present in this worktree. This change avoids
  `src/lib.rs` and the summary modules, so their later integration has no new
  workflow-specific conflict.

## Design

- Add `0.2` to the strict `WorkflowVersion` decoder. Existing numeric `0` and
  `0.1` forms and their validation paths remain unchanged.
- Add optional `planner_model` and `planner_provider` node fields. Accept them
  only as a non-empty pair in v0.2; reject a half-pair and reject either key in
  v0/v0.1. Executor `model`/`provider` retains its v0.1 behavior in v0.2.
- Continue requiring v0/v0.1 nodes to be admitted. In v0.2, require every
  profile to resolve to a registered compiled or extension descriptor, then
  allow both admitted and draft status. This prevents the draft allowance from
  turning misspelled profile IDs into executable profiles.
- Put the v0.2 circle-level admission decision in a new workflow leaf module.
  After `verify_origin` succeeds, a workflow containing any draft node is
  adjudicated `circle_failed` with the established reason
  `profile_not_admitted`; an all-admitted workflow keeps the exact existing
  `circle_full` projection. No new event name, evidence field, or intermediate
  circle verdict is introduced.
- Apply planner overrides only while constructing the child node `Config`, and
  set the corresponding provenance fields to `workflow_node`. Omitted values
  preserve the global planner configuration exactly. v0.2 node-created events
  add planner pins only when explicitly declared, so v0/v0.1 event bytes do not
  change.

## Closed condition vocabulary

Document a versioned extension procedure in the contract: add a typed enum
variant, a deterministic Rust evaluator, positive and negative schema fixtures,
an execution/corpus fixture, and contract/runbook updates in one change.
Unknown conditions, free-form expressions, scripts, and aliases remain rejected.
Issue #253 adds the procedure only; it does not add a new condition token.

## Tests and fixtures

- Schema tests cover v0.2 parsing, planner pair validation, v0/v0.1 rejection,
  existing v0.1 parsing, unknown profiles, and unknown conditions.
- Workflow leaf/config tests cover draft terminal capping and propagation versus
  inheritance of planner pins.
- A corpus case supplies one positive v0.2 YAML fixture and negative fixtures
  for a half planner pair, planner keys in v0.1, an unknown profile, and an
  unknown condition. The positive case uses the existing registered
  `static-site` draft manifest fixture.

## Verification

Run the focused workflow/schema and Issue #253 integration tests first, then the
corpus regression test. Because shared Rust configuration and workflow
contracts change, also run formatting, Clippy with warnings denied, and the full
Rust test suite.
