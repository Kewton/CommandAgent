# ADR 0001: Minimal-only Runtime

Date: 2026-06-16

## Status

Accepted.

## Context

The migration targets a new CommandAgent repository with a small codebase and a
single active runtime. The previous repository contains historical control
systems that are not part of the MVP.

## Decision

CommandAgent will ship one execution engine: the minimal loop. There is no
engine flag and no legacy compatibility layer.

## Consequences

- The CLI is simpler.
- Evaluation focuses on the minimal loop and step runner.
- Old mechanisms are not copied unless re-admitted from current evidence.
- Users who need historical behavior must use the historical repository.
