# ADR 0004: Evidence Envelope And Payload Variants

Date: 2026-06-20

## Status

Accepted.

## Context

`ContractEvidence` had become a broad compatibility structure carrying facts
from planning, tool protocol, step policy, verification, profile, setup, and
recovery attempts. That shape is useful as a read adapter but makes producer
responsibility and consumer interpretation hard to verify.

## Decision

CommandAgent adds `EvidenceEnvelope` with typed payload variants:

- planning
- provider transport
- tool protocol
- step policy
- verification
- profile
- setup
- recovery attempt
- unsupported

The existing `ContractEvidence` remains readable and can be projected into
the new envelope. New consumers should prefer the envelope and payload variant
when they need failure category or producer ownership.

Recovery instructions remain outside evidence. A recovery attempt payload
describes an observed attempt result; it does not authorize the next repair.

## Non-Decisions

This ADR does not create a shared recovery manager or unbounded cross-layer
automation. Producer migration remains incremental.

## Consequences

Eval and repair packets can distinguish failure classes without string
guessing. The compatibility adapter allows staged migration, but new evidence
producers should avoid adding more optional fields to the old catch-all shape.
