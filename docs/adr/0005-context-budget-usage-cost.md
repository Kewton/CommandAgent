# ADR 0005: Context Budget, Usage, And Cost Records

Date: 2026-06-20

## Status

Accepted.

## Context

`context_budget` was historically a configuration value more than an enforced
runtime contract. Large tool results and repeated requests can make context
growth hard to understand. Provider usage and cost data also need a common
shape for eval and CommandMate reporting.

## Decision

CommandAgent adds explicit budget data, usage records, and deterministic
tool-result budgeting.

Provider token metadata is normalized into `ModelUsage` when available.
Missing provider usage is recorded as unavailable, not as failure. Cost
records remain separate from usage records and may be unavailable when pricing
is not configured.

Tool result output is bounded by a deterministic budget. When truncation
occurs, CommandAgent emits a truncation event and marks the tool observation.

Budget-exceeded behavior must be explicit: stop, deterministic compaction,
shrink tool result, replan request, or approval required. The runtime must not
silently increase budgets, retry until success, or change providers/models.

## Non-Decisions

This ADR does not add live pricing API calls, cost dashboards, or CommandMate
budget approval UI.

## Consequences

Eval can report whether a run failed because implementation was wrong or
because context/tool output was bounded. Future budget enforcement can expand
from tool result limits to model-request preflight and deterministic
compaction without changing the design boundary.
