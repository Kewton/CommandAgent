# Test0619 001 Structured Repair Focus

Date: 2026-06-19

## Input Evidence

Follow-up planning under
`workspace/mvp/logic/test0619_001/eval/001` identified that runtime
`ContractEvidence` was present but still too shallow for focused bounded
repair. The remaining gaps were:

- verifier failure packets did not carry a stable failure signature
- build failures did not consistently expose candidate artifacts or repair
  target
- source excerpts were verifier prose, not structured repair input
- profile verification failures did not render through the common evidence
  payload
- repeated repair attempts were not visible as bounded evidence

The target behavior remains bounded repair, not retry-until-success.

## Change

Structured evidence now includes optional bounded fields for:

- failure signature
- failure kind and diagnostic code
- candidate artifacts
- repair target
- observed/expected pairs
- related source excerpt
- prior attempts and repair attempt ledger
- repair focus

Verifier failures populate these fields from deterministic verifier output.
When a source excerpt identifies a single file, that file becomes the repair
target. Next.js profile verification failures now render profile evidence in
the existing profile repair packet. Route integration failures use the selected
route as repair target and the unintegrated artifact as a candidate artifact.
Mixed `app/` and `src/app/` roots remain a profile contract failure without an
arbitrary repair target.

Repair prompts and repair packets render a `Repair focus` block derived from
the same evidence. The focus block is advisory input to the existing bounded
repair/correction path. It does not add retries, auto-resume an ultra phase, or
create a new repair engine.

## Validation

Local checks passed during implementation:

- `cargo fmt --check`
- `cargo test correction_evidence`
- `cargo test repair::tests`
- `cargo test repair_loop::tests`
- `cargo test profiles`
- `cargo test`
- `cargo build --release`
- `scripts/eval_smoke.sh`

Manual live UAT with Gemini was also run once:

- root: `/private/tmp/commandagent-uat-test0619-001-eval001`
- command: `target/release/commandagent --yes --context-budget 65536 --provider gemini --model gemini-3.1-flash-lite --planner-model gemini-3.5-flash "/ultra-plan-run --profile nextjs ..."`
- result: bounded failure at `verify-build`
- repair packet:
  `.commandagent/repairs/repair-verify-build-1781864246365.md`

The repair packet included structured verifier evidence and a `Repair focus`
block with `failure_signature`, `failure_kind`, `diagnostic_code`, and
candidate artifacts. The live UAT also exposed an over-broad candidate
extraction issue: Tailwind/PostCSS build errors could surface dependency
internals under `node_modules` as candidate artifacts/source excerpts. The
implementation was tightened after that run:

- verifier source excerpts now skip `node_modules/` and `.next/`
- runtime candidate extraction filters dependency/generated paths
- the Tailwind/PostCSS plugin diagnostic maps to `package.json` and
  `postcss.config.js` candidates instead
- focused tests cover the filter

The live run was not repeated after the filter change because the targeted
behavior is now covered by deterministic unit tests and the full local/smoke
checks above. A later UAT should confirm the same field set in a fresh live
repair packet.

## Expected Evaluation Reading

An eval should not be reported as fixed merely because evidence is clearer.
For each bounded failure, record:

- failure kind
- diagnostic code
- failure signature
- repair target
- candidate artifacts
- whether the next failure has the same or changed signature
- whether profile repair points at the selected route when route integration is
  the blocker

The desired improvement is better attribution and repair focus. App/game
quality remains a separate evaluation dimension.
