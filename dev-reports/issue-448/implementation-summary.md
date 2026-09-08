# Issue #448 implementation summary

Added a fixed R0 corpus and focused compile/Recovery regression checks. The
only change to an existing Rust file is five lines registering a Unix-only test
module under `#[cfg(test)]`; product execution, repair, verification, event
schemas and promotion policy are unchanged.

## Fixed evidence and repairs

`tests/corpus/apps/issue448-nextjs-r0/` contains 14 original source/config/lock
files checked against R0's saved SHA-256 index, the exact UI-only treatment
page, original completion contract, compiler excerpts, selected Recovery
events, provenance, a complete fixture hash manifest and reproduction guide.
The source campaign and integration worktree remain read-only.

Fixture overlays append a typed `isValidTaskStatus(unknown)` predicate and fix
the release callback return types in projects and tasks. Fixing only the two
initially observed locations exposes `tasks/route.ts:167:7` (`void` is not
callable), so a separate case preserves that failure. No API, original type
definition, build command, strict setting or dependency lock is removed or
weakened. Existing `any` annotations in the original tasks route are retained;
the repair adds none.

## Regression coverage

- `scripts/issue448_nextjs_r0.py` installs the original lock in disposable
  storage, builds six fresh variants with the same `npm run build`, verifies
  expected diagnostic presence/absence and unchanged input hashes, and checks
  all API routes plus 13 runtime values against the repaired status helper.
- `tests/test_issue448_nextjs_r0.py` checks fixture integrity, exact variant
  changes, preservation of APIs/configuration/verification, refusal to overwrite
  an existing workspace, and measured results bound to their source hashes.
- `src/planner/auto_recovery/issue448_tests.rs` tests the real import scanner,
  bounded build verifier, compile parser/scope, contract binding and Recovery transaction gate with
  measured diagnostic replay. Five tests cover six diagnostic variants and
  eleven promotion decisions: ten rejections retain control hashes, and the
  positive boundary control promotes exactly the treatment source.
- The original completion contract is also rebound byte-for-byte with its
  original SHA-256 and browser observer still required after fixture repair.
  API deletion and replacement of the host-owned verifier contract are rejected.

Actual compiler results are in
`tests/corpus/apps/issue448-nextjs-r0/evidence/build-results.json`.
`recovery-results.json` records actual gate decisions with explicitly scripted
observations; command timing/stdout suffixes are omitted from that summary.

## Interpretation

Only the fully repaired fixture passes the strict independent build. The
positive promotion control uses a generic gate-test contract with all original
required paths plus API paths, original goal, replayed build and an additional
scripted verifier. It demonstrates the normal gate conjunction, not successful
R0 business/browser acceptance. A successful build with a failed extra verifier,
missing supported acceptance evidence, removed API or changed verification
authority is still rejected.

Runtime lock deadlock and data loss were **not observed**. Neither type repair
nor these tests establish mutual exclusion, persistence correctness, live-model
repair effectiveness or the full original business requirements. No live
campaign, browser evaluation, product repair, release, PR or Issue mutation was
performed.
