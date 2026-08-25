# Issue 395 implementation summary

## Outcome

Added a non-authoritative Phase 2 evaluator for `create` VerificationSpec
proposals. It produces claim-level shadow coverage from reviewed gold bindings
and separate execution evidence without calling a provider, executing proposed
commands, or changing any existing verification or release authority.

## Implementation

- Added `verification_spec::create_shadow` with:
  - reviewed required/optional gold claims and one-or-more exact typed bindings;
  - exact strategy, expected-polarity, input, observation, and minimum-strength
    comparison;
  - separate `OracleExecutionEvidence`, so provider-declared lifecycle, result,
    and observed strength cannot manufacture execution or full coverage;
  - one report row per gold claim with strategy, strength, executed, outcome,
    evidence path, oracle IDs, and an explicit unverified reason;
  - fail-closed aggregation requiring at least one required claim and every
    required claim to have sufficiently strong passing execution evidence;
  - stable rejection reasons for missing claims/bindings, provider failure,
    policy rejection, missing/duplicate evidence, unsafe paths,
    under-strength execution, and unsupported negative-condition strategies.
- Reused the existing declarative argv validator through a crate-private
  wrapper. This preserves the established shell, inline-code, install/setup,
  dev-server, mutation, and workspace-confinement policy.
- Added golden Next.js and Python CLI proposals and coverage snapshots for:
  - UI copy and computed style;
  - button interaction;
  - explicit port and route;
  - known CLI aggregation values; and
  - two distinct input rows.
- Added negative conformance coverage for build-only substitution, missing
  matrix rows, install/dev-server/shell candidates, unsafe evidence paths,
  provider failure, unsupported no-network observation, and model-declared
  pass without external execution evidence.
- Added the Issue 395 corpus contract and documented the shadow report and
  trust boundary in `docs/dev/create-shadow-coverage-v0.md`.

## Compatibility

The Phase 1 VerificationSpec schema and prompt are unchanged. No runner or
minimal-loop chokepoint changed. Existing profile verifiers, browser release
gate, admission cap, CompletionContract, adjudication, terminal projection,
event schemas, and `.anvil/` runtime state remain authoritative and unchanged.
