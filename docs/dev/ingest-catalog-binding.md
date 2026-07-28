# Ingest catalog binding plan (E-4a)

Status: **forecast sealed 2026-07-26; E-4b implementation measured
2026-07-27; admitted 2026-07-29**. The fixed contract is
[`docs/ingest-profile-contract.md`](../ingest-profile-contract.md). Admission
is `admitted`; the stage-2 fetch probe remains unimplemented and QUEUED.

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

## E-4b implementation measurement

The primary metric is added production Rust lines per commit. Lines under
`#[cfg(test)]` and integration-test Rust are reported separately. As in E-3,
the count is commit additions, so later replacement does not lower an earlier
wager.

| Commit | Component | Production | Test | Cumulative production |
|---|---|---:|---:|---:|
| 1 | N3 selector freeze and candidate accounting | 391 | 140 | 391 |
| 2 | N2 same-candidate source binding and declared normalization | 478 | 90 | 869 |
| 3 | catalog/manifest/runtime/N4/N1+N5 reuse/projection/production activation/conformance | 929 | 434 | 1,798 |

Commit 3 production breaks down as catalog+manifest 242, N1/N4/N5 runtime and
assurance 463, domain/final-acceptance and frozen-lineage wiring 150, and
completion projection 74 lines. Its 434 test lines break down as
leaf/projection unit tests 238, production activation 103, conformance 83,
source-binding lineage assertions 6, and guardrail coverage 4.

The result is **1,798 production Rust lines**, 798 above the 1,000-line upper
forecast (1.798× the upper bound; 3.596× the 500-line lower bound). Test Rust is
664 lines, for 2,462 total Rust additions. N2/N3 alone consumed 869 production
lines; typed runtime state, the declared-format adapter, catalog/manifest
binding, projection, admission dispatch, and production activation plumbing
added the remaining 929. The fourth-profile estimate therefore captured the
order of magnitude better than E-3's 180-line estimate, but still omitted most
profile-plumbing cost from its numeric band.

### Reused mechanisms

- N1 binds the existing isolated/bounded `pipeline_probe`, including normalized
  child environment, timeout, stdout/stderr capture, and artifact observation.
- N5 binds the existing data rerun adapter and shared
  `rerun_consistency::reproduced` equality leaf.
- N4 reuses the data E4 closed-key/type assertion boundary and evidence
  discipline; the ingest-specific declared-field adapter is new because the
  data E4 schema is fixed rather than goal-declared.
- Manifest v1 closed parsing/resolution, catalog typing, draft admission cap,
  completion projection boundary, and final acceptance arbitration are reused.
- N2 reuses the E2/I2 binding/observation/verdict/`nearest_miss` evidence shape.
- N3 reuses the E1 detected=accepted+reasoned-excluded equation and reason
  buckets.

### Scaffold checklist self-evaluation

- Contract fixed and bound: **complete**.
- Real profile/intent and N1–N5 manifest mapping: **complete**.
- Assurance projection plus runtime-shaped fixtures: **complete**.
- Production final-acceptance activation test: **complete**; it executes a real
  Python subprocess and requires freeze, N1, N2, N3, N4, N5, and summary
  evidence.
- Conformance: **complete**, six negative fixtures and one full positive.
- Archived real-run corpus: **not complete**; synthetic conformance cannot
  settle comparator calibration.
- Reviewer admission: **complete in E-4d**; local 0/6 and elevated 4/6
  full-equivalent price tags remain visible.

Pre-push acceptance on 2026-07-27: focused conformance negatives **6/6** and
full positive **1/1**; production final-acceptance activation **1/1**; privileged
`cargo test --all-targets` **1,814 passed, 0 failed, 30 ignored**.

## Calibration arc interim measurement

INGEST-8 records the calibration cost before settlement rather than preserving
the forecast by omission. The estimate above allowed **one to two calibration
rounds**. The measured arc instead required eight machine-floor corrections:

1. removal of model-authored verification checkpoints;
2. command-classification precision;
3. execution-progress semantics;
4. literal canonical-form guidance;
5. machine-owned plan source;
6. phase-to-artifact producer decomposition;
7. CSS selector-engine coverage;
8. frozen candidate-ID vocabulary distribution and deterministic resolution.

It also required one contract revision, from fixed v0 to fixed v0.1, to define
document-level shared context under the value-preservation, declaration, and
two-positioned-fragment evidence conditions. The eight floors and one contract
revision are materially beyond the forecast one-to-two-round calibration
allowance. E-4 settlement must therefore revise the fourth-profile estimate
using this measured cost; this document does not silently relabel the arc as
two rounds or treat the excess as free reuse.
