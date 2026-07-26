# Ingest catalog binding plan (E-4a)

Status: **draft planning only (2026-07-26)**. The contract under review is
[`docs/ingest-profile-contract.md`](../ingest-profile-contract.md). No runtime
component, manifest binding, projection, admission, or fetch probe is
implemented by this plan. Admission remains `off`.

E-4a covers stage 1 only: acquired local HTML/text snapshots are transformed
into declared records. Network acquisition and freshness evidence belong to
the stage-2 fetch probe and remain **QUEUED**.

## Catalog disposition

| Evidence | Disposition | Catalog plan |
|---|---|---|
| N1 `ingest_probe` | ✅ reuse | Reuse the isolated, bounded pipeline-probe execution boundary, normalized child environment, capped stdout/stderr capture, timeout outcome, and artifact observation. |
| N2 `source_binding` | 🟡 new comparator | Reuse the E2/I2 evidence shape—bound claim, source observation, verdict, and `nearest_miss`—but add a comparator that binds every output field to a real fragment in the same frozen candidate block. |
| N3 `accounting` | 🟡 new candidate/accounting component | Reuse the E1 reconciliation equation and reason-bucket representation, but add deterministic candidate selection, pre-execution candidate-set freezing, candidate identity, and shrink/replacement rejection. |
| N4 `format_schema` | ✅ reuse | Reuse the data E4 schema assertion boundary after binding the goal-declared field names, requiredness, types, and top-level shape before execution. |
| N5 `rerun` | ✅ reuse | Reuse the data E3 rerun equality leaf over the frozen snapshots, declared format, selector, and generated outputs. |
| E-0 measurement equipment | ✅ reuse | Reuse bench preflight, automatic run classification, acceptance-sheet generation, calibration collection, scrub, campaign reporting, and band aggregation shape. |
| Manifest v1 and admission | ✅ reuse | Reuse closed-schema validation, phase/check resolution, draft admission cap, and the `off`-until-reviewed lifecycle. |

The green rows mean that their established mechanism and evidence semantics are
reused; they do not make the profile adapter free. The yellow rows are the new
validation surface. N2 must reject fabricated events and altered values without
claiming source completeness. N3 must close only the mechanically enumerable
candidate set and must not turn its selector into a completeness claim.

## Pre-declared estimate

E-4a applies the E-3 settlement formula before implementation:

```text
ingest stage 1 =
  shared reuse surface 0 new production Rust lines
  + N2/N3 comparators and the profile plumbing: 500–1,000 production Rust lines
  + one to two calibration rounds from measured source text
  + two to three measurement campaigns
```

The 500–1,000-line band includes the two comparators and the five recurring
plumbing obligations identified by E-3:

1. typed N1–N5 evidence schemas;
2. pre-execution freezing of snapshot identity, declared format, candidate
   selector, and candidate set;
3. catalog dispatch from the five checks to their execution components;
4. honest N1–N5 assurance projection, including the draft admission cap;
5. manifest and production-final-acceptance adapters.

The estimate excludes tests, fixtures, documentation, the stage-2 fetch probe,
and network/freshness semantics. Those costs must be quoted separately if the
review authorizes stage 2. Implementation must report production and test Rust
lines independently and must not lower this wager after observing the result.

## Scaffold-driven acceptance obligations

Before admission review, the implementation plan must contain both checklist
items exercised for the first time after E-3:

- a completion-assurance projection mapping with measured ingest fixtures;
- a production acceptance-path test proving that final acceptance actually
  starts every N1–N5 component, rather than only proving that each component
  exists in conformance.

The first calibration round must use verbatim measured municipal-event
snapshots and outputs; synthetic fixtures cannot settle comparator precision.
Candidate-selector misses may improve the declared selector only through an
explicit contract or fixture review and may never be hidden by shrinking the
frozen candidate set.
