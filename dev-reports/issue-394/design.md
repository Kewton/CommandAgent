# Issue 394 design: VerificationSpec v0 shadow contract

## Scope

Introduce the Phase 1 `VerificationSpec v0` as an opt-in, non-authoritative
shadow artifact. Keep `StepPlan.verify`, `CompletionContract`, existing events,
adjudication, terminal projection, and `.anvil/` state byte-compatible.

## Design

- Add a leaf `verification_spec` module. It owns the versioned serde schema,
  provider-output parsing, deterministic validation, goal provenance, and the
  shadow-only result wrapper. No runner or minimal-loop chokepoint changes are
  needed.
- Parse provider output only after enforcing a raw-byte limit. The caller
  supplies the authoritative original goal and supported intent separately.
  The provider's `goal` is retained only as a SHA-256 provenance value; the
  accepted spec always contains the caller's original goal and its hash.
- Represent commands as argv arrays rather than shell strings. Validate argv
  element count/length/control characters and validate every declared path as
  workspace-relative. Bound total claims, oracles, bindings, IDs, statements,
  goals, and serialized input size.
- Bind claims to oracles by stable IDs. Validation reports sorted, deduplicated
  error codes for duplicate claim/oracle IDs, missing oracle references,
  claims without bindings, and orphan oracles. This makes malformed-provider
  behavior deterministic regardless of input ordering.
- Preserve shadow isolation structurally: `observe_shadow` returns the
  authoritative verdict unchanged together with success/failure diagnostics.
  The shadow result cannot manufacture or mutate an adjudication value.
- Check in a JSON Schema, provider prompt snapshot, and golden create/fix/
  investigate fixtures. The unknown-intent golden is an expected rejection,
  consistent with the Phase 0 decision that unknown is not a v0 intent and
  must not be coerced to create.

## Verification

Add focused unit/integration tests for parsing, limits, path/argv safety, goal
authority/provenance, deterministic reference errors, golden snapshots, and
shadow failure isolation. Add a corpus contract fixture and run existing
adjudication/CompletionContract compatibility replay, corpus/guardrails, then
the formatting, clippy, and full Rust suite required for shared Rust code.
