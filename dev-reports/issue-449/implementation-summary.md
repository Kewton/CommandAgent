# Issue #449 implementation summary

Fixed two reproduced losses of supplied repair context. Compact compile and
regeneration prompts now retain the original goal, relevant source paths,
registered verification commands, remaining profile failures and files already
changed. A read-only exhaustion handoff now retains the exhausted objective as
quoted failure context while preserving the completion contract's original goal
and verification authority.

## Changes and evidence

- `src/planner/repair/compile_context.rs` is a 27-line leaf renderer, called by
  the existing compact and regeneration prompt builders in `repair.rs`.
- `src/minimal_loop/stagnation_escalation.rs` records the objective as a JSON
  string inside failure evidence. Embedded headings cannot become top-level
  handoff sections; existing normalization and secret scrubbing remain active.
- `stagnation_escalation/issue449_tests.rs` consumes #448's frozen source,
  contract, overlays and measured compiler excerpts via the real bounded build
  verifier/parser. Four tests cover both diagnostic cases, both prompt layouts,
  compact/regeneration context, UI-change carryover, read-only exhaustion,
  saved prompt/YAML fidelity, contract authority, actual Write/Edit execution,
  no-op Edit rejection and root boundaries.
- `tests/corpus/apps/issue449-repair-context/` describes the two replay cases
  without modifying the #448 source or hash manifest.
- Two old assertions now check the authoritative original goal instead of
  demanding its absence from compact context or excluding all step text from
  failure evidence. The existing runner compact recovery test still requires
  successful repair, the same appended/compact sequence and success events.
- [investigation.md](investigation.md) traces diagnostics, target selection,
  permission observations, calls/results, changed files and verification.
  [r0-trace.json](r0-trace.json) contains derived event rows and saved-artifact
  hashes. Missing historical observations stay `unknown`.

Before the production fix, `cargo test --lib issue449` had two focused failures:
missing original goal in compact context and missing `isValidTaskStatus` in the
saved handoff. Actual target Write/Edit and root/no-op tests already passed.
The fixed matrix covers both the export and Promise cases. Compile outputs are
measured #448 evidence replayed by a script; no new model or independent Next.js
build is represented by these fast tests.

## Scope and limitations

No targeting priority, tool allowlist, stop budget, verifier, acceptance,
promotion gate or event schema changed. Runner production chokepoints and
growth baselines are unchanged. Historical evidence, #448's frozen fixture,
integration worktree, generated app, live runtime, README, CHANGELOG and UX
assets were not edited. No PR, push, merge, release or Issue mutation was made.

The tests explicitly supply scanner-derived known targets; they do not claim
automatic recovery of the earlier initial-phase API diagnostic loss. The
read-only diagnostic loss happens after the final R0 Reads. The compact retry
is separately reproduced; it was not observed after that terminal exhaustion.
Neither fix establishes why the historical model chose Reads, nor an improved
real-model success rate. Evaluate actual effect in a separate new campaign/B-1.

#448's additional original-contract promotion coverage is being reviewed by the
parent. These changes stay pinned to `3952f99f` and do not touch its Recovery
test files. Passing local development checks does not declare dependency or
merge readiness, or original business/browser acceptance.
