# Issue #448 verification — post-CI AC4 follow-up

- Status: `passed`

## Checks

- `python3 scripts/issue448_nextjs_contract.py --output tests/corpus/apps/issue448-nextjs-promotion/measured-contract-results.json`: `passed`
- `python3 scripts/issue448_nextjs_contract.py --output /tmp/issue448-ac4-final-real.json`: `passed`
- `python3 -m pytest -q tests/test_issue448_nextjs_r0.py tests/test_issue448_nextjs_contract.py`: `passed`
- `python3 -m ruff check scripts/issue448_nextjs_contract.py tests/test_issue448_nextjs_contract.py`: `passed`
- `cargo test --lib issue448 -- --nocapture`: `passed`
- `cargo test --test corpus_regression --test protection_coverage_audit --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --edition 2024 --check src/planner/auto_recovery/issue448_contract_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --exit-code 3952f99ff3b2afde40b8d4e88380141044815836 -- tests/corpus/apps/issue448-nextjs-r0`: `passed`
- `git diff --check`: `passed`

## Scope and evidence

Verified on 2026-09-09 in the assigned Issue #448 worktree, based on
`3952f99ff3b2afde40b8d4e88380141044815836`. Parent evidence under the integration
worktree's `workspace/management/runs/20260909-r0-wave1-uat-evidence/448` was
read-only; all five evidence-file hashes are recorded in the new corpus's
[provenance](../../tests/corpus/apps/issue448-nextjs-promotion/provenance.json).
The prior design, implementation summary, R0 source corpus, diagnostics and
Recovery records remain unchanged. The previous verification report remains in
the base commit; this report records the requested AC4 follow-up.

- [Follow-up design](ac4/design.md) was written before implementation.
- [Follow-up implementation](ac4/implementation-summary.md) describes the new
  matrix and the boundary of its scripted observations.
- [Reproduction instructions](../../tests/corpus/apps/issue448-nextjs-promotion/reproduction.md)
  distinguish real builds from the fast replay. Use a new output filename;
  the harness refuses to overwrite evidence.
- [Measured observations](../../tests/corpus/apps/issue448-nextjs-promotion/measured-contract-results.json)
  preserve all original contract requirements, 14 input hashes per case,
  actual build invocations, Next.js observer identity, readiness/interaction
  results, static requirement bindings and final promotion decisions.
- [Final real rerun](ac4/real-rerun-results.json) and
  [fast replay](ac4/replay-results.json) record matching requirements, source
  hashes, gate decisions and static acceptance results. The final rerun keeps
  a bounded summary of the temporary output; raw process logs are not committed.

## Results

Each real matrix executes twelve actual `npm run build` invocations: the
required build before Next.js readiness and the unchanged registered verifier
for each of six cases. Ten succeed, and the UI-only candidate's two builds fail
as expected. Every successful build performs strict type checking, generates
all six pages and retains `/api/projects`, `/api/tasks` and `/api/tasks/[id]`.
The positive source combines only the existing UI, export and Promise overlays.
All input bytes are checked before and after observation.

The byte-identical original completion contract SHA-256 is
`eb2b04647a28ab10508c21e9c1cd026bea293a1c7f95dda261f4bd5529d8c9ab`.
It keeps the Next.js profile, original business goal and port 60302, all eight
required paths, original command, three capabilities, eight evidence requirements
and implementation obligation. Actual `RunnerRecoveryDriver::finish` promotes
the positive treatment. All eight static evidence tiers are `strong`; no
capability, evidence or obligation is missing. The five negatives retain the
whole control hash: UI-only build failure, failed interaction, missing
interaction evidence, HTTP 500, and the API-only repaired scaffold. The scaffold
passes both builds and scripted observations but still fails the original
implementation obligation. Build success alone does not establish promotion.

Node `v24.1.0`, npm `11.3.0`, Next.js `14.2.35`, TypeScript `5.9.3`, React and
React DOM `18.3.1`. Both real runs install the unchanged original lock with
`npm ci --include=dev --no-audit --no-fund` in disposable storage. Existing
corpus hashes remain stable for #449, including all source/config/lock bytes.

Python: 18 passed (original nine plus nine new tests). Focused Rust: six passed
(original five plus the six-case original-contract matrix), none ignored.
Corpus: seven passed; protection audit: two; generality guardrails: ten.
Formatting and Clippy passed. Full `cargo test`: 2,788 top-level tests passed,
zero failed, 38 pre-existing default-ignored tests across 66 top-level result
groups. This count excludes nested mock-child result lines; the library group
has 2,455 passed and 16 ignored. No new test is ignored. Required loopback and
subprocess checks ran outside the sandbox.

## Resolved setup findings and limits

The sandbox npm install reported `Exit handler never called!`; the permitted
outside-sandbox retry succeeded. Initial test-record assertions used the wrong
readiness JSON level and attempted to serialize a non-serializable report type;
these were corrected before the successful full matrix and final rerun.
The API-only repaired scaffold's original-contract rejection was preserved as a
negative, not bypassed. No audit, baseline, requirement or gate was weakened.

HTTP/interaction responses are explicitly scripted in both modes, through the
existing test input seams and the same production Next.js observer path. The
HTTP child is actually spawned/reaped; the interaction record explicitly has
`probe.child_spawned: false` and fixture Playwright provenance. No browser or
model is run, and these records do not claim real browser/model/business
acceptance. The positive is a conditional regression test of ordinary promotion
when all original required observations pass.

Runtime lock deadlock and data loss were **not observed**. Type repair does not
prove mutual exclusion or persistence correctness. Production behavior, original
history and live runtime state are unchanged. The sole shared wiring overlap is
five test-registration lines in `src/planner/auto_recovery.rs`, reported to the
parent. No audit baseline, PR or remote branch was changed. Changed-head CI/UAT
belongs to the parent; no push or PR edit was performed. Release build/version
verification is not required for this test-only change.
