# Issue #424 Design

## Problem

The Next.js build-oracle predicate accepts any command containing `next build`.
That makes package-script inspection such as `node -p "... 'next build' ..."` look
like a production build. A successful inspection can therefore save a
last-known-good compile snapshot even though no build ran. After bounded compile
repair exhausts, rollback restores that snapshot and checks only the static
profile contract, allowing a still-unbuildable workspace to complete.

Build-verifier commands also have no execution-duration field in their lifecycle
event, so actual build time is absent from `time_profile.builds_ms`.

## Predecessor constraints

- Issue #420 is already present in the branch ancestry. Its exact engine-owned
  Next.js scaffold classification and implementation-obligation behavior must
  remain unchanged.
- Issue #425 is complete on its sibling worktree. It binds a failed candidate's
  typed verification commands into generated Recovery contracts, so this change
  must leave a failed rollback as an ordinary `plan_step_failed` outcome with the
  original verification evidence available to Recovery.

## Design

- Recognize only executable Next.js build command forms (`npm run build`,
  `pnpm build`, `yarn build`, and direct `next build` forms, including a
  workspace `cd ... &&` prefix). Package-script inspection remains a verifier
  lifecycle but cannot authorize a compile snapshot.
- Require snapshot promotion to come from a passed lifecycle whose registered
  command is a production build, rather than from any passed build-verifier
  lifecycle.
- After restoring compile-error paths, run both the existing static profile
  verification and the production build command recorded in the failed report.
  Emit the existing `compile_rollback_failed` event and return no rollback when
  either check fails or no production build command is available. Only an
  observed passing rebuild may emit `compile_rollback_applied`.
- Add backward-compatible fields to rollback and plan-step events. In particular,
  `plan_step_completed` records `rollback_applied: true` only for a verified
  rollback; all other terminal step events record `false`.
- Measure each actual build-verifier execution, expose the summed duration as an
  additive `build_duration_ms` lifecycle field, and aggregate that field into
  `time_profile.builds_ms` independently from dependency setup time.

## Tests and corpus

- Cover accepted build commands and rejection of the captured `node -p`
  inspection command.
- Cover snapshot eligibility for a passed inspection lifecycle versus a passed
  production build lifecycle.
- Cover a broken stored snapshot: rollback re-runs the registered build, emits
  `compile_rollback_failed`, returns no successful rollback, and contributes
  positive build time to the time profile.
- Preserve the existing successful rollback regression and assert its additive
  event fields.
- Add an Issue #424 corpus fixture whose ordered terminal sequence is
  `compile_rollback_failed` followed by `plan_step_failed`, never a synthetic
  `completed_after_rollback` record.

## Compatibility and risk

No event is renamed or removed and schema version 1 remains valid. New fields are
additive. The build re-check uses the same normalized, bounded verifier path and
does not grant dependency-setup authority, so rollback cannot install packages
or weaken a failed verification gate.
