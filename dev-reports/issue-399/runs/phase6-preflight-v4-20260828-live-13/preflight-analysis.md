# Issue 399 Phase 6 A12 live-13 final preflight analysis

## Decision

**GO for instrument readiness.** This decision means that the frozen comparison
instrument may proceed to a separately preregistered 360-pair experiment. It does
not establish that the candidate is effective or should be adopted.

The final `preflight-report-v4.json` has all 14 checks set to `true` and
`ready_for_full_experiment_design: true`. The only check that changed during the
final aggregation was `semantic_review_complete`, after the contract-authorized
Fable calibration review was validated.

## Frozen provenance

- Contract: `phase6-preflight-v4-20260828-live-13`
- Contract SHA-256: `06eea9a925627be27be7f86c2ebd74b86f5783b276d3317b77f79e3827055ea6`
- Instrument code SHA: `8958ed6fb2368c58cbfb7dda57814481de1d53f7`
- CommandAgent binary SHA-256: `7467e19dc3610ae5ef0c2774230a859925fd41ad2d6c66b7c55426e35d1474db`
- Exact-SHA CI evidence SHA-256: `1e0354a04b54b98022b98972ba1aaa2dfb706ad2dce1a08136d762cfdf74d4ab`
- Request namespace: `phase6-preflight-v4-20260828-live-13`
- Record-ledger head SHA-256: `d823f15a0086e0d5b042963870e9a060204f0be11e33fdc76fa32664ad6920b7`

## Machine gates

- Collected: 40/40 pairs and 80/80 proposals.
- Schema compliance: 80/80 lanes, including 80/80 before host repair.
- Same product snapshot: 80/80 lanes.
- Host repairs: 0.
- Regenerations: 0.
- Reference fallback: 0.
- Gold used for execution: 0.
- Shadow false-full: 0.
- Recovery Plan automatic executions: 0.
- Baseline product runs: 40. All 40 terminated as honest failures and remained in
  the denominator; no baseline failure was overridden or excluded.

## Source-blind semantic review

Blind preparation produced 153 claim-group items and 177 unique oracle references.
Duplicate oracle references were 0. The full-item SHA-256 is
`3d2017b84ca8f87f6a50a117efe937ea666a0e12d6490b10d878f0b917d0ccd4`;
the fixed ten-item calibration sample SHA-256 is
`5e500aef795e37597904f6483a6c57d144f07ea384f886836204d5b86fb62266`.

The user-authorized Fable review was valid with no validator errors. Its declared
identity, authorization, contract-authoring involvement, source-blind boundary,
forbidden-material boundary, and reviewer-output independence all matched the A12
contract. The review document SHA-256 reported by the validator is
`ff7b92f0f99780b6cc494e11dd1649823c98ad3ba9404bd38c6550185fc82c58`.
`human_review_complete: false` is the retained legacy human-only field;
`calibration_review_complete: true` is the applicable A12 gate.

## Model evidence decision

Both model reviews were structurally valid and came from distinct families
(`gemma4` and `gpt-5.6`). Their verdict agreement was 56/153 (0.3660) and Cohen's
kappa was 0.0765, below the frozen 0.4 threshold. On the ten-item calibration
sample, three-way verdict consensus was 3/10, below the frozen 0.7 threshold.
Accordingly, model reviews are not eligible as corroborating evidence in the
360-pair experiment. This secondary evidence decision does not block instrument
readiness under the frozen A12 contract.

## Information boundary and repository retention

Fable fixed the calibration document before reading model verdicts or the final
preflight report. The isolated packet excluded the secret mapping, model reviews,
execution results, canonicalized output, preflight report, raw records, and prior
reviewer output.

The committed evidence set intentionally excludes `raw/`, `blind-review-v4/secret/`,
scratchpad files, `workspace/temp/`, and runtime state. `campaign-summary.json`,
`record-ledger.jsonl`, and the blind manifest retain the hashes that bind the
curated reports to the locally retained raw run.

## Next authorized phase

Before the first response of the 360-pair experiment, freeze a new contract for
12 cells x at least 30 pairs, multiple task clusters per cell, the four wall-time
and token budgets, the paired-run scope, exact-SHA CI evidence, exclusions,
scoring, the 2,000-resample cluster bootstrap, and a new reviewer authorization
scope. The live-13 ten-item Fable authorization must not be reused for that run.
