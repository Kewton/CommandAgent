# Issue #498 design

Parent: `981239c91f1ee97cb76c08620202a9f0d03056f8`. The merged #496
commits (`6085f3a7`, `574aa49e`) already retain complete profile instructions,
capture immutable sources, and reject over-capacity proposals. No predecessor
merge is needed. The complete GitHub Issue and approved decision were read.

Add a conservative bounded-repair proof in a recovery_step_plan_binding leaf.
Only the first capacity rejection with unambiguous, boundary-captured sources
may prove infeasibility. Match the frozen model/host duties to the actual
preservation view and uncut original owner, retaining checks, owner order,
registered contract and package/marker evidence. Missing or inconsistent
acquisition evidence returns unknown. Distinguish capacity from provenance
failures structurally, rather than matching error text.

For nonempty Implement/pass duties owning the same normalized multi-path scope,
general preservation requires each matching owner to contain both complete
instructions. Compute their shortest common superstring length in Unicode
scalars (containment, otherwise maximum suffix/prefix overlap in either
direction). A lower bound above 2500 proves impossibility only when no registered
split applies. Share the existing package-only split eligibility predicate; do
not introduce a split or infer feasibility from a small lower bound.

Admission keeps its existing stage order and event fields/classification. Add
explicit terminal reason, retry permission and proof evidence. A terminal proof
takes precedence over legacy `proposal_repairable`, reports `stopped` with two
attempts remaining, and returns a distinct error without a repairable/exhausted
cause chain. No runner changes or fallback execution are needed.

Keep the saved #496 fixture/hash evidence intact, update its terminal expectation,
and move downstream retry tests to reachable repairable inputs or direct
preservation tests. Cover Unicode bounds, normalized scopes, bypass attempts,
provenance/acquisition negatives, first-attempt-only capture, event compatibility,
single-request replay, bounded Ready and legal retry/split controls. Ordering
remains a separate downstream check; no ordering conclusion follows from stop.

Run focused #498/#496 and binding tests, corpus regression, fmt, clippy and all
tests. Perform the requested isolated replay under a fresh `/Volumes/SSD_NX/tmp`
directory and record its report under a new `workspace/tmp` directory. Do not
modify generated apps, historical evidence, live `.anvil`, guardrail baselines,
or external lifecycle state. Commit the implementation and required reports.
