# Issue #496 design

## Baseline and scope

The worktree starts at `6085f3a72928e8fbd35740959cc2184216cd9ced`, an existing partial
implementation of Issue #496 on top of Issue #494 merge
`26814ae56b9c8f31e8ea078b3917e75aff4d4e00`. Inspect and complete that work; do not
reimplement it or change the four-output ownership rules. The full Issue #496
body was read from GitHub. No required predecessor is outstanding.

## Design before implementation

Keep the public sanitizer's 2,500 Unicode scalar limit and model-only truncation.
Admission supplies explicit nonempty host provenance to its internal sanitizer
entry, tracks the owner uniquely across canonicalization, and rejects missing or
ambiguous correspondence. Assess protected instructions after all sanitizer notes
without truncation, before any candidate becomes executable.

On the first capacity or formation rejection, freeze the pre-augmentation model
and independent host scopes, the post-transformation untruncated plan, registered
contract, marker obligations, and package-script source evidence as one retained
baseline. Subsequent proposals, workspace changes, and fallback must not recapture
or bypass that baseline. Keep existing event fields, retry budget, lint, check
order, and ownership rules. Driver changes remain minimal wiring.

Add focused regressions for the saved 529/2,237-character parsed-stage fixture,
Unicode boundaries including readiness notes, provenance variants and movement,
missing duties and outputs, immutable capture, mixed failures and fallback,
Recovery/configured contracts, and deterministic templates. Extend corpus
metadata to state the precise reconstructed provenance and expected failures.

## Verification

Run focused Issue #496 tests and related formation/profile/sanitizer tests first.
Then run `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test`. Record actual
results, implementation details, and the exact baseline in the required reports
before an Issue-scoped commit. No push, PR, or external lifecycle mutation is in
scope.

## Refinement after focused regression

Retain both acquisition evidence and the existing transformed preservation view
at the same first-retry boundary. Replacing that view with every pre-sanitizer
step would reject the established Issue #484 Inspect canonicalization positive.
The two snapshots therefore serve distinct purposes: acquisition evidence is
serialized unchanged, while the preservation view accounts for already-supported
host transformations. Profile-target model and host duties remain separate and
verbatim, including a removed target. No later proposal participates in either
snapshot. Marker acquisition failure is retained explicitly rather than retried
against mutable workspace state.

For ambiguous model IDs, capture the entire model plan directly before mutation;
do not reconstruct that acquisition snapshot by owner ID. The later transformed
view and protection resolver still reject ambiguous correspondence explicitly.
