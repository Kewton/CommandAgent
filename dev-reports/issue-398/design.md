# Issue 398 design: critic, counterfactual, and lineage hardening

## Scope

Add a Phase 5 post-hoc critic for `VerificationSpec v0` shadow proposals. The
critic is diagnostic only: existing create/fix/investigate evidence evaluators
remain the verdict authorities, and critic rejection, provider failure, or an
unavailable counterfactual cannot change their result or execution authority.

## Design

- Add a leaf `verification_spec::critic` module. A closed serde
  `CriticJudgment` represents provider output (decision plus issue codes), while
  `validate_critic` owns all deterministic acceptance rules. Deserializing an
  LLM judgment therefore never grants coverage by itself.
- Freeze a typed oracle contract before concretization. It contains the claim,
  expected polarity, minimum strength, input, observation, and proposed
  argv/cwd/fixtures. Record freeze, bind, and execute checkpoints with run ID,
  monotonically increasing epoch, artifact hash, schema/prompt/model identity,
  and request ID. Recompute every hash from canonical JSON and reject broken
  checkpoint correlation.
- Permit concretization only when typed semantics are unchanged. Workspace
  path normalization and argv spelling changes may be declared equivalent;
  changed claim identity, expected observation, polarity, or reduced minimum
  strength are deterministic weakening errors regardless of the critic's
  decision.
- Model counterfactual evidence as a closed status. A generated counterfactual
  must bind the same frozen contract and record an executed, discriminating
  result. `absent` and `unavailable` require a non-empty reason and always
  produce an explicit `unverified` shadow outcome.
- Keep authority and resource safety explicit in the report:
  `shadow_only=true`, `authoritative_verdict_changed=false`, and
  `candidate_execution_authorized=false`. Validate critic token, latency, and
  retry observations against caller-supplied immutable Phase 0 budget values;
  missing or exceeded budgets are unverified, never silently favorable.
- Add focused unit, schema/property-equivalent mutation, negative conformance,
  and adversarial corpus tests. The adversarial matrix counts a false-full if
  any weakening, bad lineage, missing counterfactual, critic/provider failure,
  or budget breach becomes verified; its required count is zero.

## Compatibility

The change is additive. It does not alter `VerificationSpec v0`, provider
prompt/schema snapshots, existing events, completion contracts, authoritative
assurance logic, runtime authority, or `.anvil`. The critic report has its own
version and may be discarded or disabled without migration.

## Verification plan

Run the focused critic tests and VerificationSpec schema/conformance tests,
then corpus regression and guardrails. Because shared Rust library surface is
added, run formatting, Clippy with warnings denied, and the full Rust suite.
