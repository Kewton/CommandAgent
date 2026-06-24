# Phase 3 Active Job Arbiter

Date: 2026-06-21

## Scope

Phase 3 adds a deterministic dispatch gate inside Recovery Orchestration. The
gate converts failure evidence into active-job candidates, selects exactly one
owner/action pair, projects the loop-control action, or stops explicitly when
top-priority candidates conflict.

## Contract Fields

The orchestration evidence now carries:

- `loop_control_action`
- `dispatch_status`
- `dispatch_reason`
- `candidate_jobs`
- `tie_break_reason`

These fields are attribution data for bounded repair prompts and eval reports.
They do not add a retry loop, execute tools, or dispatch to another runtime.

## Local Verification

Executed locally on the implementation commit before push:

- `cargo test`

The test suite covers tool-protocol dispatch, read-only explicit stop,
same-priority conflict stop, missing-artifact ownership before source fallback,
profile repair packet preservation, and eval report dispatch sections.

## Residual Risk

Dispatch still depends on deterministic evidence producers. If a verifier or
profile guard emits only broad prose without target or failure code, the gate
will correctly stop or select a conservative fallback, but the repair task may
remain less specific than a future producer could make it.
