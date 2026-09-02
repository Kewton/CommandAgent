# Create shadow coverage v0

`commandagent.verification_spec.create_shadow_coverage.v0` is the Phase 2
claim-coverage report for `create` VerificationSpec proposals. It is additive,
opt-in, and non-authoritative. It does not replace `StepPlan.verify`, a profile
verifier, the browser release gate, profile admission, final adjudication, or
terminal projection.

## Inputs and trust boundary

The evaluator receives three separate inputs:

1. reviewed gold claims with exact typed expected bindings and minimum evidence
   strength;
2. a Phase 1 `ShadowGeneration`; and
3. caller-supplied execution evidence with an oracle ID, observed strength,
   outcome, and workspace-relative evidence path.

The provider proposal is only a candidate. Its `lifecycle`, `result`, and
`observed_strength` fields do not prove execution. The evaluator never runs a
command and reports `unverified` until separate execution evidence is present.

## Matching and coverage

Claims match by stable ID. Every expected binding then requires exact equality
of expected polarity, typed input, and expected observation plus an accepted
strategy. A claim may require multiple bindings, so both rows of a CLI input matrix or both UI
copy and computed-style observations must be present. Build success cannot
match an interaction, DOM, known-output, HTTP port/path, or negative-condition
binding.

Every report row contains `strategy`, `strength`, `executed`, `outcome`,
`evidence_path`, matched `oracle_ids`, and `unverified_reason`. Missing claims,
missing bindings, provider failure, rejected policy, missing/duplicate
execution evidence, unsafe evidence paths, and under-strength observations are
explicitly unverified. Unsupported negative observations such as a network-log
oracle are also explicitly unverified; the evaluator does not substitute a
weak file-absence check.

`all_required_passed` is true only when at least one required gold claim exists
and every required row has external execution evidence, meets minimum strength,
passes, and has no unverified reason.

## Verify policy

Command-like candidates reuse the existing registered declarative argv policy.
Structured argv does not grant execution authority. Shell interpreters, inline
code, install/setup commands, dev-server commands, filesystem mutation,
workspace escapes, and anything else rejected by the shared verify policy stay
rejected. HTTP and DOM bindings observe a runtime managed elsewhere; they do
not start one.
