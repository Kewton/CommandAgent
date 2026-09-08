# AC4 follow-up design

The post-CI UAT at `3952f99ff3b2afde40b8d4e88380141044815836` correctly found
that the generic positive control did not cover promotion under the original
Next.js contract. Read all five parent evidence files under the integration
worktree's `workspace/management/runs/20260909-r0-wave1-uat-evidence/448`.
Their fresh six builds, Python nine tests, Rust five tests and clean-source
integrity agree with the earlier results; they do not close AC4.

## Added coverage

- Add a separate test module and separate bounded observation fixture. Keep
  every byte under `tests/corpus/apps/issue448-nextjs-r0/` unchanged for #449.
- Materialize the existing repaired source only in disposable workspaces. Bind
  the byte-identical original Next.js completion contract and original goal to
  the actual `RunnerRecoveryDriver::finish` path.
- Keep `npm run build`, required build-before-readiness, the Next.js browser
  observer, original port 60302, all three capabilities, all eight evidence
  requirements and the implementation obligation. Real static source checks
  and observer/acceptance dispatch must run without result overrides at the
  Recovery gate itself.
- Use existing test-only interaction response seams and an explicitly scripted
  HTTP child. A workspace-local test npm shim replays measured compiler output
  only after comparing every source/config/lock input against the corresponding
  frozen variant. Also support actual locked Next.js builds where feasible.
  These observations are simulated browser responses, never live browser or
  model acceptance. Do not prepopulate final success evidence or bypass the
  production observer's consumption/serialization of interaction responses.
- Positive control must reach ordinary promotion with contract hashes unchanged.
  Negative controls cover a real observed compile failure, failed interaction,
  unavailable interaction evidence and failed HTTP readiness. All rejections
  must preserve the full control source hash. Expand only if another original
  requirement needs separate coverage.
- Record contract bytes/hash, observer identity, required observation outcomes,
  build invocations, source hashes and final decision. Preserve earlier records
  and update the current verification report only after required checks pass.

## Coordination and verification

The only shared wiring change is a `#[cfg(test)]` module registration in
`src/planner/auto_recovery.rs`; this overlap is reported to the parent in the
worker progress/final response. No production behavior, guardrail baseline,
existing source fixture, original evidence, PR or remote branch is changed.

Run the added focused tests, prior issue tests, relevant Python integrity check,
corpus/protection/generality audits, formatting and Clippy. Run the full Rust
suite before committing. If the unchanged contract cannot pass without dishonest
evidence or weakened checks, record the exact remaining AC4 boundary as blocked.

## Refinement from the unchanged-contract check

The API-only repaired fixture still has the original engine scaffold and was
correctly rejected with `non_implementation_obligation_only:scaffold` after all
registered observations passed. Retain it as an additional negative. The
positive treatment combines the existing UI-only overlay and the existing
export/Promise repairs in a temporary workspace. Measure this new combination
with the original locked real build before permitting a fast measured replay.
None of the original corpus files or their hashes change.
