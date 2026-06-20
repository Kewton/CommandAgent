# Logic 003: Recovery Policy Contract Evaluation

Date: 2026-06-20

## Summary

Implemented the first Recovery Policy Contract slice in the step runner. The
change turns deterministic failure evidence into visible active jobs and repair
actions before the Recovery Task Contract is rendered. It does not add a new
execution engine, hidden continuation, provider-specific policy, or retry
budget.

The focused Gemini E2E initially exposed a preceding plan-lint issue: the
Next.js route-integration obligation rejected a component creation step even
though a later step in the same plan edited `app/page.tsx` to render that
component. That was corrected as a plan-contract rule. A second clean E2E run
completed all six ultra phases and all build verification steps.

## Implementation

- Added `src/agent/step_runner/recovery_policy.rs`.
- Added stable `active_job` labels such as `manifest_repair`,
  `route_integration_repair`, `integration_artifact_creation`,
  `source_implementation_repair`, and `explicit_stop`.
- Added stable `repair_action` labels such as `add_manifest_dependency`,
  `repair_tailwind_contract`, `repair_tsconfig_alias`,
  `connect_artifact_to_selected_route`, and
  `create_missing_integration_artifact`.
- Added `repair_action` to `ContractEvidence` and `RecoveryTaskContract`
  rendering.
- Converted Next.js profile failure propagation to use the policy layer for
  route integration, missing integration artifacts, manifest dependency/script
  drift, Tailwind contract drift, and tsconfig alias drift.
- Added verifier evidence labels for setup bootstrap, manifest repair, and
  source verifier repair.
- Added plan-lint support for a later route integration step in the same
  step plan when it touches the selected route and names the artifact by path
  or file stem.

## Verification

Local checks:

```text
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Result:

- `cargo fmt --check`: pass
- `cargo test`: pass, 463 tests
- `cargo clippy --all-targets -- -D warnings`: pass
- `cargo build --release`: pass

Focused E2E:

- Provider/model: Gemini, `gemini-3.1-flash-lite`
- Planner model: `gemini-3.5-flash`
- Command: `/ultra-plan-run --profile nextjs ...`
- First run root: `/private/tmp/commandagent-logic003-e2e`
- First result: stopped at `plan_lint.profile_obligations` because later route
  integration was not recognized.
- Second run root: `/private/tmp/commandagent-logic003-e2e-2`
- Second result: success, six phases completed with build verification `ok`.

See `workspace/mvp/uat/test0620_008.md` for the run notes.

## Remaining Limits

- The Recovery Policy Contract currently covers the observed deterministic
  profile/verifier/plan-lint classes; it is not a general semantic repair
  planner.
- The successful E2E verifies build and route contract progress, not visual or
  gameplay quality in a browser.
- Next.js route-integration plan lint now recognizes a later route-editing
  step in the same plan, but broader cross-phase artifact graph planning should
  wait for another observed failure.
