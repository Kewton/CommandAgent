# Issue #430 verification

- Status: `passed`

## Checks

- `cargo test --lib planner::profiles::nextjs::api_contract::tests`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Results

- Verified on 2026-09-05 from parent
  `0296d779eed87f244899199a5687f8261b449e3f`.
- The focused API-contract suite passed all 7 tests. It covers Japanese,
  box-drawing, emoji, and ASCII before/at/after the cutoff and all interior
  UTF-8 byte positions, both checked and unchecked mutations, empty/short
  input, long ASCII, response identity, and existing response-check forms.
- The byte-bound matrix accepts response tokens ending at 4,095 and 4,096
  bytes and rejects tokens ending at 4,097 bytes, for both `response.ok` and
  inline `).ok`, with ASCII and multibyte padding.
- All 7 corpus regression tests passed. The new fixture asserts that the
  cutoff lies inside `─` and that both final and invariant profile checks
  return exactly `api_contract_failure: POST /api/shifts used by src/app/page.tsx
  does not check Response.ok before accepting the mutation`.
- The full `cargo test` run completed with exit 0 outside the sandbox,
  including 2,334 passing library tests, integration tests, guardrail tests,
  and 2 passing documentation tests. Existing ignored tests retain their
  existing status; this change adds no ignored tests or skip conditions.
- Formatting and Clippy completed with exit 0. No production changes were
  made after those checks.

## Before/after evidence and environment

- Before the production fix,
  `cargo test --lib planner::profiles::nextjs::api_contract::tests::response_window_handles_characters_around_every_utf8_cutoff`
  failed with the expected panic at `api_contract.rs:97`: byte 4178 lies
  inside `日` at bytes 4176..4179.
- Before the production fix,
  `cargo test --test corpus_regression issue430_utf8_response_window_reports_honest_api_failure`
  failed with the expected panic at the same slice: byte 4202 lies inside
  `─` at bytes 4201..4204. Both regressions pass in the final suites above.
- The initial sandboxed full-suite attempt encountered `PermissionDenied`
  / `Operation not permitted` while tests bound local mock-server sockets.
  That run was interrupted (exit 130), then the unchanged full command was
  rerun outside the sandbox and passed. No tests were weakened to work around
  the sandbox restriction.
- Initial test-construction and formatting errors were corrected before the
  final passing checks; no failed or pending required check remains.
- This source-only corpus fixture does not assert an npm build result and
  does not modify or revalidate the original S1 generated application or
  historical evidence. No live provider probe or external lifecycle action
  was performed.

## CI follow-up: interaction-probe harness timeout (2026-09-06)

- PR #434's first CI run (`CI` run 34006429509 and `acceptance` run
  34006429504) failed one pre-existing library test:
  `minimal_loop::interaction_probe::tests::draft_only_storage_does_not_satisfy_committed_persistence`.
  The failing observation reported `failure_kind:
  "probe_infrastructure_failed:probe_timeout"`, `duration_ms: 12045`, and the
  last stage `persistence_reload`, so the probe was killed by the harness
  budget instead of returning the `persistence_not_evaluated:*` verdict the
  test asserts. Neither the Issue #430 change set nor this branch touched
  `interaction_probe.rs` or `assets/interaction_probe.js`.
- Root cause: the fake-Playwright harness `run_fake_probe_scenario` passed a
  hard-coded 12s budget, while the production callers pass 60s
  (`browser_probe.rs`) and 120s (`planner/runner/acceptance.rs`). The
  `draft_only_persistence` scenario is a negative case: the typed token is
  never echoed and the reload never shows a committed mutation, so the probe
  script legitimately exhausts three full 3s polling windows
  (`TOKEN_ECHO_SETTLE_MS`, `RELOAD_RENDER_SETTLE_MS` for the persistence
  marker, and `RELOAD_RENDER_SETTLE_MS` again for the post-reload echo) plus
  the 800ms input-marker wait and the recovery-transition attempt. Running the
  probe script standalone against the same fake module on an idle machine
  measured `observing` at 6ms, `persistence_reload` at 3,934ms, and process
  exit at 11,836-12,008ms wall clock. The harness deadline starts after the
  child spawn and is polled every 50ms, leaving well under 200ms of headroom,
  so GitHub-runner scheduling jitter under the parallel test load crosses it.
- Flakiness evidence: the same test passed on `develop` (run 33899471627) and
  on the sibling `CI` run for the issue-428 branch (34006422653) but failed
  on that branch's `acceptance` run (34006422596) at the same time as the
  PR #434 failures. All other fake-probe scenarios measured between 289ms and
  6,561ms standalone; `draft_only_persistence` is the only one at the limit.
- Fix: `run_fake_probe_scenario` now uses a documented test constant
  `FAKE_PROBE_SCENARIO_BUDGET` of 60s, matching the smallest production
  budget. No persistence, token-echo, or failure-kind assertion changed, no
  test was ignored or retried, and no probe-script timing constant changed.
  The scenario still takes ~12s of real polling; the budget simply no longer
  races it.
- Rechecks on this branch after the fix:
  `cargo test --lib draft_only_storage_does_not_satisfy_committed_persistence`
  passed (finished in 11.94s);
  `cargo test --lib minimal_loop::interaction_probe::tests` passed all 36
  tests under a CPU-load generator; `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings` (with `RUSTFLAGS=-D warnings`)
  passed. Full-suite results are recorded below.
- Full-suite rerun after the fix (2026-09-06, outside the sandbox, with
  `RUSTFLAGS=-D warnings`): `cargo test --all-targets` exit 0 with 2,334
  passing library tests and 16 ignored (unchanged), followed by
  `cargo test --test corpus_regression`, `cargo test --test generality_guardrails`,
  and `cargo test --test conformance`, each exit 0. Every `test result:` line
  reported 0 failed. `git diff --check` reported no whitespace errors.
- A local reproduction of the timeout under a `yes`-based CPU load
  generator did not trip the old 12s budget on this 28-core machine; the
  root-cause evidence is the CI observation payload plus the standalone
  stage timings above, not a local failure.
