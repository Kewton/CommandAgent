# Issue #424 Implementation Summary

## Outcome

Compile snapshots and rollback completion now require observed production-build
evidence. A package-script inspection that merely contains the text `next build`
can no longer authorize a last-known-good snapshot, and a restored snapshot is
accepted only after the recorded production build command passes again.

## Implementation

- Replaced substring-based Next.js build detection with executable command-form
  recognition for supported npm, pnpm, Yarn, npx, and direct `next` invocations.
- Moved compile-snapshot eligibility into a leaf module and require both a
  passing lifecycle and a registered production build command.
- Added rollback build re-verification through the existing normalized,
  bounded build-verifier lifecycle with no dependency-install authority.
- Kept the existing `compile_rollback_applied` and `compile_rollback_failed`
  events and schema version, adding build command, outcome, and duration fields.
- Added `rollback_applied` to terminal plan-step events so
  `completed_after_rollback` is distinguishable from ordinary completion and a
  rejected rollback remains an honest `plan_step_failed` outcome.
- Measured build-verifier execution and aggregated its additive
  `build_duration_ms` lifecycle field into `time_profile.builds_ms`, separately
  from dependency setup time.

## Tests and corpus

- Added command-classification coverage for real build invocations and the
  captured false-positive `node -p` inspection shape.
- Added snapshot-promotion coverage proving a passing inspection cannot save a
  compile snapshot while a passing production build can.
- Added a broken-snapshot rollback regression that requires an actual rebuild,
  emits `compile_rollback_failed`, refuses successful rollback, and accounts
  positive build time.
- Extended the existing successful rollback regression to require build
  re-verification and `rollback_applied: true`.
- Added the `issue424-compile-rollback-build-reverification` corpus case with
  the required `compile_rollback_failed` -> `plan_step_failed` terminal order.

## Compatibility and predecessor audit

- Issue #420's scaffold classification remains unchanged; its focused contract
  test passes in the full suite.
- Issue #425's Recovery command-binding work was inspected but not merged. A
  failed rollback retains the failed verification report and ordinary failed
  step path that Recovery consumes.
- Event names, schema versions, historical evidence, and the live `.anvil/`
  namespace were not changed. All new event fields are additive.

## Verification environment

The final full suite ran outside the filesystem/network sandbox because it
contains loopback-server and child-process tests. The existing pyenv shims were
prepended to `PATH` so the Python reference test used the available PyYAML
runtime while preserving the normal Node and tool paths. No dependencies or
runtime services were installed, started, stopped, or modified.
