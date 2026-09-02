# Issue 398 implementation summary

## Implemented

- Added `verification_spec::critic` as an additive shadow-only leaf module.
  Its closed `CriticJudgment` serde schema is parsed independently from the
  deterministic `validate_critic` runtime contract.
- Added typed freeze/bind/execute lineage checkpoints. Every checkpoint carries
  the artifact SHA-256, epoch, run ID, model, prompt version, schema version,
  and request ID. Validation recomputes contract hashes, requires monotonic
  epochs, and binds execute to the exact bound artifact.
- Added typed oracle contract comparison. Explicitly reasoned, policy-safe
  workspace path/argv concretization is allowed; claim substitution, input or
  expected-observation change, polarity change, and minimum-strength weakening
  fail closed. Strengthening remains allowed.
- Added closed counterfactual states. Generated evidence must bind the frozen
  hash, use a safe evidence path, execute, and discriminate. Absent or
  unavailable counterfactuals require a reason and become `unverified`.
- Added token, latency, and retry accounting against a versioned caller-supplied
  Phase 0 resource envelope. Missing/invalid or exceeded budgets become
  `unverified` rather than favorable evidence.
- Preserved authority structurally: reports always mark shadow-only, forbid
  candidate execution, and cannot change the cloned authoritative value.
  Provider failure and critic rejection have no call path to planner execution,
  adjudication, events, or `.anvil`.
- Added eight focused tests covering schema/runtime responsibility separation,
  semantic-equivalent concretization, strength ordering, a mutation sweep,
  counterfactual absence/unavailability, provider/resource failure, schema
  negatives, and an adversarial false-full count of zero.
- Added the Issue 398 corpus contract and documented the critic/counterfactual/
  lineage boundary in the VerificationSpec v0 developer contract.

## Compatibility

No existing `VerificationSpec v0` field, provider prompt/schema, event,
completion contract, evidence artifact, assurance evaluator, or runtime state
namespace changed. `src/planner/verify.rs` and runner/minimal-loop chokepoints
were not modified.
