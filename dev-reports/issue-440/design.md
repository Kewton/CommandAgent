# Issue #440 design

- Base: `36f73eeb`; no required predecessors. #439 is a parallel lane and must
  be combined by the orchestrator before campaign revalidation.
- Read the complete supplied issue JSON (no comments), approved action-items
  A-2, parallel development plan, repository instructions, and dev guardrails.
  Campaign evidence and those references remain read-only.
- Add a per-session leaf guard for replies whose observed completion token
  count reaches the configured positive `num_predict` and whose normalized
  tool-call list is empty. Missing usage is not guessed. Read-only calls,
  short replies, and failed writes do not erase the streak; successful Write
  or Edit resets both count and accumulated provider duration.
- Use the original repair approach's default limit of two. The original tests
  and parallel plan also request a third-response replay: an internal session
  limit permits testing three against the I1 projection as well. Neither the
  step wall-clock cap nor completion/acceptance rules change.
- On reaching the limit, stop with `max_length_no_tool_call_repeated`, count,
  total matching provider duration, scope, and recovery paths. Use existing
  recovery prompt/plan writers so a typed Recovery candidate is registered.
  Failure to save recovery remains an explicit failure.
- Add `tool_calls_in_response` and `write_or_edit_succeeded` to existing
  `provider_turn_duration` events, plus the configured output limit and its
  observed reach status. A scoped event-file buffer retains the provider event
  and following events until tool execution outcomes are known, preserving
  their existing order and schema version. Live UI projection stays immediate;
  the buffer flushes on all returns/continues/errors and before post-tool
  verification. Normalized text calls count just like native calls.
- Keep runner changes to wiring, with guard/handoff and scoped telemetry in
  leaf modules. Add issue-specific sanitized I1 event projections and a long
  Write fixture under `tests/corpus/apps/issue440-max-length`.
- Verify default and third-response termination, duration accounting, recovery
  prompt/YAML/candidate, long native/text calls, Write/Edit resets, failed
  mutations, unknown/short usage, telemetry ordering and legacy time profiles.
  Run focused Rust tests, then the complete `scripts/ci.sh` check set and
  doctests separately (the full all-targets suite covers the other `cargo test`
  targets). No live provider or campaign is needed.


## Boundary and implementation refinement

Orchestrator clarifications confirmed the existing `finish_reason` is synthesized
and I1 contains successful short Read batches between capped replies. Default
two therefore stops after source event 607, without requesting 609; internal
limit three stops after 629, without requesting 631. Neither resets on Reads.

The event tail uses the already locked `tempfile` crate (moved from development
to runtime dependencies), retaining at most 64 KiB in memory before anonymous
spill. Both ordinary and failsafe writers share the buffer, and scopes also
flush explicitly before completion verification. Nested classifier events
retain independent success fields. Reports and tests distinguish fixture-time
replay from actual fast mock-provider execution; no campaign rerun is claimed.
