# ADR 0002: Contract Recovery Without Legacy Mechanisms

Date: 2026-06-17

## Status

Accepted.

## Context

The CommandAgent migration intentionally did not copy the historical Anvil
legacy engine, case memory, sidecar routing, advisory layers, or larger repair
jobs. That choice remains correct.

The first large-task eval root exposed a different problem: several contracts
that are required by `/ultra-plan-run` were thinned too aggressively during the
migration. Modify tasks could be planned as new work, required artifacts from
eval cases were checked only after the run instead of being passed into the
runtime contract, plan lint did not know about existing workspace files, and
invalid generated plans were not always preserved for triage.

These are not legacy mechanisms. They are deterministic contracts and
observability surfaces.

## Decision

CommandAgent restores the following contracts:

- common plan intent: `new`, `modify`, `investigate`, `document`, `data`, or
  `unknown`
- common step kind: `inspect`, `create`, `edit`, `setup`, `verify`, `repair`,
  or `report`
- expected result: `pass`, `fail`, or `unavailable`
- required final artifacts, passed from CLI/eval into ultra and step plans
- workspace-aware plan lint limited to shallow path existence checks
- invalid generated plan preservation under `.commandagent/plans`

These fields are used for prompt contracts, linting, reporting, and triage.
They do not introduce a second executor, an unbounded repair loop, or
profile-specific agents.

## Non-Decisions

This ADR does not reintroduce:

- the legacy engine
- case memory or anti-pattern corpora
- sidecar semantic summarization
- multi-stage automatic repair
- provider/model-specific prompt branches
- framework-specific hidden rules that could live in the common DSL

## Rationale

The recovery follows the minimal-first principle: remove ambiguity before adding
capability. A typed plan contract keeps setup, verification, reporting, and
artifact expectations explicit. This prevents the model from inferring hidden
state and makes large-run failures easier to classify.

The most important boundary is that `intent` and `kind` are not execution
strategy switches. They are schema fields for the planner, lint, verifier, and
reporter. If a future change uses them to dispatch separate execution engines,
that change requires a separate design decision.

## Consequences

- Older plan files remain readable through defaulted fields and are normalized
  when saved again.
- Eval expected artifacts are now visible to the runtime and saved plan files,
  not only to post-run checks.
- Plan lint can reject alternative paths and globs while accepting existing
  workspace files for verification-only contracts.
- Large eval results should become easier to triage even when success rate does
  not immediately improve.

## Revisit Criteria

Revisit this ADR only if these contracts become control mechanisms rather than
schema/reporting boundaries, or if profile rules begin accumulating
provider/model-specific fixes. In that case, remove or narrow the offending
contract before adding new repair behavior.
