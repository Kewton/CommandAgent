# Issue #430 design

## Evidence and scope

- Read `AGENTS.md`, `docs/dev/dev-guardrails.md`, the issue-worker skill, and
  the current [Issue #430](https://github.com/Kewton/CommandAgent/issues/430)
  body and comments through `gh issue view ... --json` on 2026-09-05. There
  were no comments. The worktree starts at `0296d779eed87f244899199a5687f8261b449e3f`;
  no required predecessors are listed.
- The reported S1 panic cuts byte 8257 inside `─` at bytes 8256..8259.
  `mutation_response_checked` currently clamps the suffix end to the string
  length but can still slice inside a UTF-8 character.
- The approved scope is the API-contract leaf module, focused tests/corpus,
  and these issue reports. Historical evidence, unrelated generated app
  defects, event schemas, runtime state, and guardrail baselines stay untouched.

## Change

Use the existing `crate::util::floor_char_boundary` for the saturated
`matched.end() + 4_096` endpoint. Regex match endpoints are character
boundaries, so rounding the suffix end down cannot cross its start. The
inspection window remains at most 4,096 bytes; short input still uses its
complete suffix. Keep all existing response-assignment, returned-fetch, and
inline `.ok` checks intact.

## Verification plan

- Add focused regressions for Japanese, box drawing, emoji, and ASCII at and
  around the cutoff, including every interior byte of multibyte characters,
  with and without a response check.
- Check response tokens ending immediately before, exactly at, and after the
  byte limit, plus short input and existing response-recognition forms.
- Add a synthetic S1-style Next.js corpus source with Japanese UI and a box
  drawing comment crossing the cutoff. Verify final and invariant profile
  checks report an unchecked mutation as an honest API-contract failure.
  This is a source-only fixture, not a claim that the original S1 app builds.
- Run regressions against the original code to demonstrate the panic, then
  apply the fix and run focused unit/corpus checks, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
- Record final results and commit only the task-owned files; no external
  state changes or live probes are part of this worker task.
