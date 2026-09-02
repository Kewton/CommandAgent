# Issue 396 design: fix VerificationSpec shadow projection

## Scope

Integrate fix-intent `VerificationSpec v0` proposals as a post-hoc shadow over
the existing F1/F2/F3 evidence contract. Preserve
`adjudication::fix::evaluate_fix_evidence` as the only verdict authority and do
not change `fix_runtime`, `fix_reproducer`, event schemas, `.anvil/`, profile
regression binding, or assurance caps.

## Design

- Add a leaf `verification_spec::fix_shadow` module. Its inputs are a parsed
  shadow generation plus caller-supplied authoritative `FixEvidenceArtifact`
  records. Each artifact carries its repository-relative `evidence/fix-*.json`
  path and the already persisted `FixEvidenceObservation`.
- Reconstruct an authoritative `FixEvidenceBundle` from those observations and
  the caller-supplied run-start frozen regression IDs/lineages, then call the
  existing evaluator unchanged. Clone that result into a versioned shadow
  report and state structurally that the authoritative verdict was not changed.
- Project one row for every authoritative F1, F2, and frozen F3 observation.
  Match only a fix claim whose origin preserves the evidence artifact path,
  requirement ID, stage, expected polarity, lineage, and epoch, and whose
  `existing_fix_evidence` oracle points to the same artifact with the same
  polarity. Missing, duplicate, switched, stale, weakened, or substituted
  claims remain explicitly unverified and cannot alter the authoritative row.
- Extract model-proposed reproducer candidates only from bounded, validated
  command-like oracles attached to an F1 `before_fails` / expected-failure
  claim. Preserve structured argv/cwd/fixtures and mark every candidate
  `execution_authorized=false`. The module performs no command execution;
  callers may consider a candidate only in an isolated copy or after
  authoritative F1/F2/F3 evidence is final.
- Compute shadow coverage separately from the F1/F2/F3 adjudication. Complete
  claim coverage is diagnostic only. Conversely, missing shadow coverage does
  not demote an existing verdict. Generic/profile unavailable `partial` or
  `static` results are copied verbatim and can never be promoted.

## Tests and fixtures

- Add focused unit/integration coverage for a complete F1/F2/frozen-F3
  projection, candidate/authority separation, missing F1 (after-only),
  before/after substitution, stale epoch, shrunken/changed frozen regressions,
  initially passing baselines, duplicate claims, model-declared execution, and
  partial/static non-promotion.
- Add a fix shadow proposal and expected report golden plus an Issue corpus
  contract. Tighten the existing fix conformance negatives without changing
  create guidance or existing fixtures.
- Run focused fix-shadow and fix-intent tests first, then VerificationSpec
  fixture replay, corpus/guardrails, formatting, Clippy, profile-crossing fix
  tests, and the full Rust suite.

## Compatibility

This is additive and shadow-only. `VerificationSpec v0`, the provider prompt,
F1/F2/F3 schemas and semantics, the bounded one-rebuild reproducer flow,
profile regression execution, create verification, and terminal assurance
projection remain unchanged.
