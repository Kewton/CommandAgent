# F-C-1b localhost fixture acceptance

Status: **full 3/3 (2026-08-03)**

This is fixture acceptance, not the first live-site measurement. No public
origin was contacted or selected. `scripts/fetch_fixture_recorder.py` served
robots and content from an ephemeral `127.0.0.1` origin; its local verification
matched the committed recording. CI replays that SHA-pinned recording without
DNS or network access.

- contract SHA-256:
  `574a195ede4a6150e711ebc6f94582d6d64632ae94ba3c466b6f255c15464121`
- recording SHA-256:
  `5aefb98944677a1fc7b8e0966ac8571873e8a4a189cf1bb3a65cc446e33d4f39`
- content SHA-256:
  `f5db7e08f869612e9d4136fc0a511128288ffc45ef35efce35df4d47f0b8399b`
- aggregate run-file SHA-256 before this report:
  `73b539ec9f7dd201723d26ad35ba17880a01350ce43276c800695ef759d54d5c`

| run | fetched at UTC | status | bytes | robots | N1 | N2 | N3 | N4 | N5 | N6 | final |
|---|---|---:|---:|---|---|---|---|---|---|---|---|
| localhost-recorded-001 | 2026-08-03T02:26:04.882Z | 200 | 59 | 404 allow | pass | pass | pass | pass | pass | pass | full |
| localhost-recorded-002 | 2026-08-03T02:26:05.268Z | 200 | 59 | 404 allow | pass | pass | pass | pass | pass | pass | full |
| localhost-recorded-003 | 2026-08-03T02:26:05.540Z | 200 | 59 | 404 allow | pass | pass | pass | pass | pass | pass | full |

N6 observed ages were 44 ms, 17 ms, and 16 ms against the contract-owned
86,400-second maximum. Each fetch entry records the exact contract hash as its
authorization SHA, and each run preserves the fetch evidence, freshness
evidence, stage-1 N1--N5 evidence, stage-2 assurance, exact snapshot, cache
entry, deterministic pipeline, and outputs. The three run directories are
independent; no cache is shared between runs.

Production Rust is 2,043 lines using the fixed E-4 counting rule:
comparator/checkers 946 (forecast 650--1,050) and plumbing 1,097 (forecast
700--1,200). The combined result is within the frozen 1,350--2,250 band.
Tests, fixtures, and non-production harnesses add 1,103 lines, within the
separately frozen 1,100--1,900 band; materialized run evidence is excluded from
both implementation counts.
