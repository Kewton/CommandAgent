# ADR 0003: Versioned Job/Event Protocol

Date: 2026-06-20

## Status

Accepted.

## Context

CommandAgent runtime events were originally passive observations for TUI and
tests. CommandMate integration and future external planner surfaces need a
stable event boundary that can be persisted, replayed, and interpreted without
parsing terminal text.

## Decision

CommandAgent adds a versioned external event envelope around existing
`RuntimeEvent` values. The envelope includes schema version, event id,
sequence, timestamp, run id, job id, optional phase/step/attempt ids, event
type, source, and payload.

The internal TUI event remains the primary runtime event. External JSONL is
produced by an adapter and can be enabled for one-shot runs with
`COMMANDAGENT_EVENT_JSONL`.

A replay projector can derive job state from the event sequence. Unknown event
types and unknown fields are compatibility cases, not panics.

## Non-Decisions

This ADR does not add queueing, scheduling, approval UI, dashboard behavior,
parallel execution, or hidden retry. CommandMate may own those concerns
outside CommandAgent.

## Consequences

Eval and CommandMate can observe stable job progress without depending on TUI
strings. The protocol adds schema maintenance burden, so protocol changes must
include compatibility tests and docs.
