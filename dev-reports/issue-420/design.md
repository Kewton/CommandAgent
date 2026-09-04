# Issue #420 Design

## Problem

The Next.js setup path writes a deterministic `src/app/page.tsx` fallback. The
runtime evidence classifier currently treats that non-empty, route-bound TSX
file as an implementation artifact. Consequently an `implement` step can read
the fallback, satisfy the completion contract on the next iteration, emit
`step_short_circuited`, and report no changed path.

## Design

- Expose a narrow Next.js profile predicate that recognizes the exact
  deterministic page templates written by scaffold completion. Normalize only
  surrounding whitespace and line endings so the authored template remains
  recognizable across platforms.
- Consult that predicate from the existing scaffold-role classifier before the
  generic source-file implementation rule. A task-specific rewrite therefore
  stops matching the template and can satisfy the `implementation` obligation.
- Preserve the existing short-circuit policy and event schema. Setup, inspect,
  verify, and non-scaffold implement flows retain their current behavior.
- Add a focused runtime-loop regression covering Read followed by Write and a
  corpus expectation for the captured fallback page. Keep the existing API
  route plan behavior covered by the full suite.

## Scope and risks

The change is intentionally content-specific rather than a broad heuristic.
That avoids classifying legitimate pages as scaffold merely because they retain
instrumentation or visual elements from the fallback. The tradeoff is that a
semantically unchanged but materially reformatted template is no longer
recognized; this is acceptable because it has left the exact engine-owned
scaffold state and normal completion evidence applies.
