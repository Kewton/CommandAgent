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
