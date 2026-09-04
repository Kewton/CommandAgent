# Issue #409 implementation summary

## Outcome

GUI Trial now admits an unclassified, admitted Next.js `create` request to
Gate 1 with family `unknown`. Its comparable-run band and price projection are
explicitly `未計測`, and confirmation delegates the unchanged typed route as
`--profile nextjs --intent create`.

## Implementation

- Added `boundary_shell::unmeasured_route` as a leaf policy module. It derives
  the fallback only when deterministic routing is ambiguous solely because no
  family was observed and every candidate is the admitted Next.js create
  route.
- Reused the Next.js contract checks while supplying a zero-sample,
  `未計測` band for that exact fallback. No family or band catalog literal was
  added, and the shared deterministic router was not changed.
- Wired GUI Gate 1 to use the fallback after normal unique selection, and
  taught confirmation presentation and price projection to accept only its
  exact identity. Existing confirmation schema, persistence, and hash code are
  unchanged.
- Rendered missing duration and cost as `未計測` only when the proposal price
  source is `未計測`; existing missing-price displays remain `未記録`.
- Documented the narrow GUI behavior and added a changelog entry.

## Tests and fixtures

- Added leaf-module unit coverage for admitted explicit/inferred create routes
  and rejection of family evidence, other profiles, typed unknowns, and
  missing intent evidence.
- Added a GUI server integration test covering the card, stable repeated hash,
  persisted confirmation, exact CLI arguments, known-family measurements, and
  unchanged ambiguity rejections.
- Added a corpus projection fixture for the new family, band, and price shape.
- Extended the browser smoke to require two visible `未計測 (0 件)` price rows.

## Scope

The worktree was first fast-forwarded to required predecessor Issue #408 at
`8075295b`. No historical evidence, `.anvil` state, task-family vocabulary,
guardrail baseline, orchestration code, or Issue lifecycle state was changed.
