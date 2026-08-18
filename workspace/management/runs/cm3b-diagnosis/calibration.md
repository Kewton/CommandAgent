# CM-3b calibration boundary

Status: implemented after the raw-evidence diagnosis in `report.md`.

## Changed

- `community_package_missing` was reclassified from model to machine/transmission.
- A Community-only UltraPlan lint now rejects an `app-zone` phase unless a preceding phase names `app.spec.yaml` and the zone phase names both `package.json` and `package-lock.json`.
- The existing Community-only StepPlan quality gate now rejects an `app-zone` step unless the plan has already declared both build-material paths. The existing promotion-before-zone gate remains in force.
- Both failures enter the existing bounded planner repair path. The lint feedback carries the literal canonical paths, so the requirement does not depend on planner prose dialect.

## Intentionally unchanged

- The base Community profile guidance bytes are unchanged. A/C-equivalent L2 planning therefore receives the same initial prompt as before; the new feedback is emitted only after an L3/zone plan violates the gate.
- `d_mochimono_003` and `d_vote_004` remain `community_spec_artifact_missing` with model attribution: their plans explicitly required `app.spec.yaml`, but execution stopped after failing to read the not-yet-created file.
- All three B-arm closed-vocabulary failures remain model-attributed. The complete schema-derived root vocabulary and a passing literal example were present in the delivered guidance, so no local-dialect injection adjustment is justified.
- S/Z/B verification, sealed fixtures, schemas, golden suites, provider requests, and non-Community profile behavior are unchanged.

## Regression guards

- A Community L3 StepPlan without the two build materials is retryable; a complete L3 plan advances.
- A Community L3 UltraPlan without a preceding spec phase and build materials is rejected; a complete plan advances.
- A canonical qwen27 L2 plan serializes to identical bytes before and after quality evaluation and receives no new retry.
- The same UltraPlan under `nextjs` receives none of the Community-only errors.
