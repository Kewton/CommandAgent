# Issue #409 design

## Context

GUI Trial currently requires `deterministic_route_excluding_top_level` to
return one catalog-backed route. A Next.js create request without a known
family word instead returns the three measured create families, so Gate 1
rejects the proposal with 422. Selecting an unrelated or multiple family must
continue to reject, and non-GUI routing must retain its typed-unknown behavior.

Issue #408 is the required predecessor. This worktree was fast-forwarded to
its committed stop-control implementation (`8075295b`) before this design was
written. Its Gate 1 change only completes the `SessionSpec` test fixture and
does not change routing.

## Decision

- Add a boundary-shell leaf module that recognizes exactly an admitted
  Next.js `create` deterministic result with no observed family evidence.
- For that case only, derive a `RouteCandidate` with family `unknown` and an
  unmeasured `BandValue`. Keep the family and band literal catalogs unchanged.
- Wire the GUI Gate 1 adapter to use that candidate when normal deterministic
  selection is not unique. The shared router and ambiguity classifier remain
  unchanged, so CLI and other callers retain their existing behavior.
- Let the existing confirmation identity, persistence, validation, and hash
  code serialize the derived route without adding or changing schema fields.
  Existing catalog-backed identities follow the same construction path and
  therefore retain their hashes.
- Treat the explicit `未計測` band source as a price result with zero samples
  rather than as a file path. In the GUI, render missing values from that
  source as `未計測`; measured bands that merely lack a recorded cost continue
  to render `未記録`.

## Safety boundaries

- The fallback requires a create-only candidate set for the admitted profile
  and rejects any explicit, request, or material family observation.
- Known family requests keep their catalog route and measured band.
- Multiple-family requests, Python CLI ambiguity, and all other unknown or
  contradictory results remain errors.
- Delegation continues to consume the confirmed `profile` and `intent`, so
  the accepted fallback launches with `--profile nextjs --intent create`.
- No historical evidence, `.anvil` state, band summary, catalog entry, or
  `generality_guardrails` literal baseline is modified.

## Verification plan

- Add focused unit tests for fallback admission and rejection boundaries.
- Add a GUI server integration test covering the unmeasured card/price,
  confirmation persistence, exact delegated arguments, known-family behavior,
  multi-family rejection, Python CLI ambiguity, and other typed unknowns.
- Add a corpus fixture for the newly observable Gate 1 projection.
- Run focused Rust tests, GUI type checking, formatting, Clippy, the full Rust
  suite, and the repository CI script because shared boundary-shell behavior
  and GUI/server contracts are touched.
