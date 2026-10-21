# Issue #440 implementation summary

## Result

The minimal loop now stops on the second measured output-limit response with
no normalized tool calls since its last successful Write/Edit. It emits
`max_length_no_tool_call_repeated` and does not request a third response. The
internal limit-three replay stops after the third matching response, before
requesting a fourth. Reads, short responses, and failed mutations retain the
sequence. Long native and normalized text Write/Edit calls remain allowed.
No completion rule, acceptance gate, provider budget, step cap, growth baseline,
or live runtime namespace was changed.

## Implementation

- `src/minimal_loop/loop_run/max_length_guard.rs` owns the count, accumulated
  matching provider time, threshold, explicit failure, and existing recovery
  prompt/plan writers. The plan writer registers a typed Recovery candidate.
  Runner changes are 16 production wiring lines and one test include; existing
  user-interruption checks retain priority over the new stop.
- `RunSessionOptions` has an internal optional threshold, defaulting to two.
  The model-probe initializer retains this default. The detector uses observed
  completion tokens against positive `config.num_predict`; synthesized
  `finish_reason = stop` and absent usage never stand in for token-cap evidence.
- Existing `provider_turn_duration` gains `tool_calls_in_response` and
  `write_or_edit_succeeded`; `tools` retains its offered-spec count. Loop turns
  also record `num_predict` and `output_limit_reached`. Write/Edit success is
  set after actual tool execution, including when later calls fail.
- A caller-thread/path scoped telemetry leaf delays event-file persistence
  until execution facts are known, retaining existing event order. Ordinary and
  failsafe writers use the same buffer. UI projection remains immediate.
  Normal no-tool replies flush before completion handling; tool batches flush
  before verification; all other returns, continues, and unwinds flush on Drop.
  Only the owning response is enriched, so nested classifier results retain
  their own facts. Sequential sessions, nested scopes, and separate thread/path
  sessions are covered. The tail spills to an anonymous temporary file above
  64 KiB; the already locked `tempfile` dependency moves to runtime dependencies.
- `time_profile` uses the additive returned-call count when raw tool events
  are absent, retaining old-stream classification and provider duration totals.
  Stop totals are not added again as a separate time category.
- The issue-specific corpus contains a sanitized projection of I1 events
  584–643 and a synthetic 16 KiB page payload. Historical files remain untouched.
  I1's `implement-main-ui` / `implement` / `inspect-current-state` scope is
  preserved. Default-two matching time is 262217 ms (584, 607); limit-three
  matching time is 385947 ms (584, 607, 629), below the existing 900000 ms cap.

## Validation scope and ancillary repair

Eleven focused Rust tests cover default/third-response boundaries, I1 Read
interleaving and duration, native/text Write, successful Edit, failed Edit,
missing/short usage, Recovery prompt/YAML/candidate, event schema/order/path
isolation, nested calls, buffer spill, unwind, and legacy time-profile totals.
The replay accounts for recorded durations without sleeping through them;
mock-provider loop tests independently enforce the request boundary.

Full CI exposed a pre-existing non-hermetic Python scheduler unit fixture:
searching the title `Independent` found unrelated shared repository files and
made its nominally independent inputs conflict. That one test now isolates
file enrichment. Its exact batch, merge-order, and maximum-parallel assertions
are unchanged; the separate enrichment tests remain active. No orchestration
production code or scheduling rule changed. The focused Python module and Ruff
are included in final verification.

## Handoff

Commit implementation, fixtures, documentation, and these reports only. The
orchestrator owns push, PR, CI/UAT, merge with #439, and lifecycle actions. No
live provider probe or historical-campaign rerun was performed. Results here
establish deterministic regression behavior, not campaign performance.
