# Issue #238 Implementation Summary

## Result

Repeated unchanged file reads now return a compact response instead of
resending the full tool result. The response contains the workspace-relative
path, an unchanged marker, and at most the first 20 cached content lines (also
bounded to 4 KB). A changed file always takes the existing full `Read` path and
refreshes the cache afterward.

When the repeated request is identical to the immediately preceding `Read` and
the same resolved path was successfully written through this registry, the
compact result and additive `tool_read_unchanged` event mark it as a completion
candidate. Issue #207's existing post-write confirmation gate consumes that
sequence and completes the direct minimal loop on the same turn. The
model-independent regression completes in five turns with an eight-turn
budget. No production behavior was added to `src/minimal_loop/loop_run.rs`.

## Implementation

- Added `src/tools/repeated_read.rs`, which owns per-request cache keys,
  SHA-256 whole-file fingerprints, compact head rendering, consecutive-call
  state, and successful-write path state.
- Wired one synchronized cache into `ToolRegistry`; cloned registries share
  their session cache while new registries remain isolated.
- Kept directory reads and fingerprint failures on the existing full-read
  path. Failed `Write`/`Edit` calls never create completion candidates.
- Added the backward-compatible `tool_read_unchanged` eval event with repeat,
  consecutive-call, compact-bound, and completion-candidate fields.
- Updated the existing model-independent post-write completion regression to
  use identical consecutive reads and assert completion within eight turns.
- Added focused leaf and registry tests for compaction bounds, changed-file
  invalidation, failed-edit honesty, and completion-candidate selection.
- Added `tests/corpus/apps/issue238-repeated-read-completion`, whose golden
  records full read then compact read and a completed outcome on turn 3 of the
  eight-turn bound.

## Compatibility and guardrails

Existing tool specifications, tool method signatures, read errors, workspace
policy enforcement, and event schemas are unchanged. The new event is
additive. Completion-contract, required-path, plan-step, failed-command, and
failed-mutation gates remain owned by the existing minimal-loop logic.

The production chokepoints `src/minimal_loop/loop_run.rs` and
`src/planner/runner.rs` were not modified. The generality guardrail tests pass.
No pull request, merge, push, release, or external Issue mutation was
performed.
