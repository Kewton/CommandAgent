# ADR 0001: Minimal-only Runtime

Date: 2026-06-16

## Status

Accepted.

## Context

The migration targets a new CommandAgent repository with a small codebase and a
single active runtime. The previous repository contains historical control
systems that are not part of the MVP: legacy engine selection, sidecar routing,
case memory, advisory layers, anti-pattern corpora, and larger repair jobs.

The migration goal is not feature parity with that system. It is a smaller
runtime whose behavior can be attributed, tested, and compared by evaluation
scripts.

## Decision

CommandAgent will ship one execution engine: the minimal loop. There is no
engine flag and no legacy compatibility layer.

Planning, verification, repair, and profile behavior live outside the minimal
loop in the step runner. Provider modules stay transport-only. Sidecar behavior
is deferred until a measured failure shows that deterministic evidence and
bounded repair are insufficient.

## Rationale

A second engine would keep old behavior reachable, but it would also keep old
complexity alive. It would make evaluations ambiguous because a result could be
caused by engine selection rather than the current minimal design.

Sidecar routing is deferred for the same reason. It can be valuable, but it adds
another model, another prompt surface, and another source of nondeterminism. The
MVP first exhausts deterministic evidence: verifier output, source excerpts,
expected artifact checks, and explicit repair packets.

## Consequences

- The CLI is simpler.
- Evaluation focuses on the minimal loop and step runner.
- Old mechanisms are not copied unless re-admitted from current evidence.
- The provider/profile/step-runner boundaries are easier to enforce.
- Users who need historical behavior must use the historical repository.

## Revisit Criteria

This ADR can be revisited only with a failure report showing that the minimal
loop plus step runner cannot solve a class of tasks without a specific removed
mechanism. The proposed reintroduction must include a bounded trigger, an off
switch if it changes runtime behavior, and an eval plan.
