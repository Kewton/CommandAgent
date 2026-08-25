# Issue 394 implementation summary

## Outcome

Added frozen `VerificationSpec v0` and prompt v0 contracts as opt-in,
non-authoritative shadow data. No planner runner, minimal loop, existing event,
CompletionContract, adjudication, assurance, terminal projection, or `.anvil/`
schema was changed.

## Implementation

- Added `src/verification_spec.rs` and exported it from `src/lib.rs`.
  - strict serde parsing and v0/prompt version checks
  - fixed raw size, goal, claim/oracle count, binding count, ID, text, path,
    fixture, argv, and timeout limits
  - workspace-relative path and local HTTP-route checks; structured argv and
    read-only HTTP method checks
  - caller-authoritative goal reconstruction plus original/provider/raw
    response SHA-256 provenance
  - create goal ranges and post-hoc fix F1/F2/F3 / investigation I1/I2
    references with stage, polarity, lineage, binding ID, and epoch
  - closed oracle strategy/input/observation, lifecycle, result, strength,
    timeout, and semantic-equivalence lineage types
  - sorted/deduplicated validation errors for duplicate IDs, missing
    references, unmatched reverse bindings, orphan oracles, and unbound claims
  - isolated shadow-failure classification and fixed-name artifact persistence
    below a caller-selected directory
- Added frozen JSON Schema and prompt snapshots under `eval/goal_verify/v0/`
  and documented the versioning, authority, safety, lineage, and limit contract
  in `docs/dev/verification-spec-v0.md`.
- Added create/fix/investigate provider goldens plus an expected-rejection
  unknown golden. Unknown remains outside the three-value runtime intent and
  is never coerced to create.
- Added Rust conformance tests, Draft 2020-12 schema tests, an exact
  shadow-failure artifact snapshot, and an Issue corpus contract fixture.
- Saved only Issue #394-owned schema, golden, corpus contract, and shadow
  artifact copies under the designated local verification directory
  `/Users/maenokota/share/work/localwork/commandagent_trial/issue/394`.

## Compatibility

The implementation is a new leaf module with no runtime wiring. Existing
completion event bytes (including the frozen 81-key set) and CompletionContract
snapshots replay unchanged. A malformed response, empty claims, timeout,
provider outage, or policy rejection is represented only as a shadow failure;
the authoritative result is cloned unchanged into the side observation.
