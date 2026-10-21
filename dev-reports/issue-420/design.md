# Issue #420 Reopened Design

## 2026-09-07 dependent fix

Read the full Issue JSON (including the latest reopen comment), the supplied
reference re-audit, AGENTS.md, and development guardrails before implementation.
Inspected predecessor #435 commit `0384999cb3a74c2603f035cadb3d0b0faddae9bc`
and its passing verification report, then fast-forwarded this branch to it.
Its command-registration and repair-classification changes remain intact.

The #422 classifier correctly rejects the engine page as implementation evidence,
but another API route can satisfy the run-wide obligation. The step's explicit
expected paths therefore need an independent placeholder check, including when
the run contract is only observed or has no implementation obligation.

- Capture hashes of explicit implement-step paths before execution. Block
  completion while any of those paths still matches an engine-owned page. Apply
  this at iteration short-circuit and ordinary final/post-tool completion so an
  unrelated Write, a textual final response, or observe mode cannot bypass it.
- Keep the existing contract verification and runner step verification. Allow a
  non-placeholder existing artifact to short-circuit only after its contract is
  verified; do not force an artificial write. Preserve setup/inspect/verify and
  runner pre-satisfied configuration behavior, plus existing Recovery and
  synthesized-precheck mutation requirements.
- Preserve actual expected-path changes, including Bash changes, using content
  hashes as well as the existing tool-path tracking. Add step ID, Write/Edit
  observation, placeholder paths, before/after hashes, and satisfaction basis to
  iteration short-circuit events without removing or renaming existing fields.
- Put behavior and focused tests in leaf modules; keep chokepoint wiring small
  and do not change growth baselines, source/promotion gates, or runtime namespaces.
- Project all 19 historical events from the nine runs into source-only corpus
  fixtures, with original event lines and file hashes: 8 skipped, 2 failed,
  3 completed with changes, 6 completed without changes, and 11 missing step IDs.
  Preserve historical outcomes separately from replay expectations. Fix the
  #422-only false positive in fixture expectations, and exercise Read then Write,
  unrelated Write, Bash mutation, existing satisfaction, and API-before-UI plans.

Required checks: focused Rust regressions, corpus and growth/conformance checks,
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test`. No Python harness change or new live campaign is planned. Exact
template recognition does not prove arbitrary business semantics; normal source,
step, acceptance, and release verification remain responsible for those checks.
Historical files and generated application originals are read-only references.
