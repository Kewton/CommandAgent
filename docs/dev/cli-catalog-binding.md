# CLI catalog binding measurement (E-3b)

The E-3a forecast expected two new components at approximately 180 production
Rust lines total:

| Evidence | Existing component forecast | E-3b result |
|---|---|---|
| C1 normal/error execution | pipeline probe execution boundary | reused bounded process, environment allowlist, process group, and capped stream capture |
| C2 help binding | new comparator, ~90 Rust LOC | new execution-observed comparator |
| C3 output claims | claims-binding comparator | reused the bound normal observation; output examples are frozen with the usage case |
| C4 rerun consistency | E3 rerun consistency | reused the shared equality leaf on the repeated normal observation |

The implemented manifest has four catalog bindings but dispatches only two
execution components: `cli_probe` supplies C1, C3, and C4 observations;
`help_binding` supplies C2. The Manifest v1 metadata remains `draft`, and the
E-1 management scaffold remains `admission = "off"`.

## Rust line measurement

The primary metric counts added production Rust lines per commit. Lines under
`#[cfg(test)]` and integration-test Rust are reported separately. Counts are
the per-commit additions, so a refactor that replaces an earlier line remains
visible instead of reducing the wager after the fact.

| Commit | Production additions | Test additions | Total additions | Cumulative production | Cumulative total |
|---|---:|---:|---:|---:|---:|
| 1 — argv probe (C1/C4) | 257 | 71 | 328 | 257 | 328 |
| 2 — help binding (C2) | 217 | 56 | 273 | 474 | 601 |
| 3 — catalog, manifest, C3/runtime, conformance | 462 | 177 | 639 | 936 | 1,240 |

The production result is **936 lines**, 756 above the 180-line forecast
(5.20×). The kit assembled successfully, but it did not meet the forecasted
growth wager. The largest omitted costs in E-3a were typed evidence schemas,
pre-execution case freezing, four-to-two catalog dispatch, honest assurance
classification, and the manifest adapter.

## Reused mechanisms

- `bounded_process`: environment allowlist, process-group termination, and
  timeout outcomes;
- `verifier_env::normalized_command_at_root`: interpreter resolution at the
  workspace boundary;
- pipeline probe `StreamCapture`: bounded stdout/stderr draining and truncation
  metadata;
- data E3 shared rerun equality leaf;
- Manifest v1 parsing, closed schema validation, phase scoping, catalog
  resolution, and the draft admission cap;
- E2-style claim records: bound claim, observed value, verdict, and
  `nearest_miss` calibration receptacle.

No guardrail baseline was raised, and admission remained off for E-3b.

## E-3c admission update

On 2026-07-25 the profile was promoted to `admitted` from the measured
`uat-test0724-cli-001-v3` local arm: machine-attributed classes 0/6, honest
terminals 6/6, and full 0/6 (0%) with the price tag retained. The conformance
six negatives and executable positive fixture remained green. This update
changes the admission projection only; it does not rewrite the E-3b line
measurement or its draft-era observations above.
