# Issue #448 verification

- Status: `passed`

## Checks

- `python3 scripts/issue448_nextjs_r0.py --output dev-reports/issue-448/build-results-initial.json`: `passed`
- `python3 -m pytest -q tests/test_issue448_nextjs_r0.py`: `passed`
- `python3 -m ruff check scripts/issue448_nextjs_r0.py tests/test_issue448_nextjs_r0.py`: `passed`
- `cargo test --lib issue448`: `passed`
- `cargo test --lib issue448 -- --nocapture`: `passed`
- `cargo test --test corpus_regression --test generality_guardrails`: `passed`
- `cargo test --test protection_coverage_audit --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --edition 2024 --check src/planner/auto_recovery/issue448_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Results

Verified on 2026-09-09 on the assigned feature worktree, based on
`5572467ee5028ae43f8497f875e1ef16030cf470`.

- Independent builds: all six expected outcomes matched using the original
  package lock. Original, UI-only, export-only, Promise-only and
  projects-and-export exited 1 with the expected compiler diagnostics. Repaired
  exited 0 after strict type checking and full production build, retaining all
  three API routes. The repaired status helper passed all 13 input cases.
- Node `v24.1.0`, npm `11.3.0`, Next.js `14.2.35`, TypeScript `5.9.3`, React and
  React DOM `18.3.1`. Installation used
  `npm ci --include=dev --no-audit --no-fund`; every variant used `npm run build`.
  The successful matrix output was moved from the temporary report filename to
  `tests/corpus/apps/issue448-nextjs-r0/evidence/build-results.json`, the
  authoritative committed copy. It binds every result to exact source hashes.
- Python: 9 passed. Focused Rust: 5 passed, no ignored tests; six diagnostic
  variants and eleven boundary decisions exercised. `recovery-results.json`
  records ten rejections with unchanged control hashes and one promotion whose
  resulting source hash equals the treatment hash.
- Corpus: 7 passed. Generality guardrails: 10 passed. Protection audit: 2 passed.
  Final full `cargo test`: 2,797 passed, 0 failed, 38 pre-existing default-ignored
  tests across 76 result groups. Full verification ran outside the sandbox for
  the existing local socket/subprocess integration tests. No new test is ignored.
- Original source integrity was rechecked read-only: all 14 source/config/lock
  hashes and all three referenced source-evidence hashes still match provenance.
  Fixture builds also assert that compiler input bytes and the corpus remain
  unchanged throughout execution.

## Resolved verification setup findings

The initial sandbox dependency install failed with npm's `Exit handler never
called!`; the outside-sandbox retry revealed that the environment omitted
devDependencies. The harness now explicitly includes them and the full matrix
passed. Early Rust test setup/expectation errors were corrected before the
final passes (including replacing a read-only treatment contract for the
hostile-candidate test).

Full-suite audits also caught a direct test parser call and an included test
file lacking a locally explicit test boundary. The final tests use the existing
bounded build verifier, which retains complete scripted output before parsing,
and an explicit `#[cfg(test)]` module. Both audits and the full suite were rerun
successfully. No audit, baseline, compiler check, requirement or promotion gate
was weakened or allowlisted to obtain these passes.

## Scope of the result

The compile runs are real; the fast Recovery observations and additional
verifier results are scripted. The positive generic boundary control proves
the gate conjunction, not complete R0 business/browser acceptance. A separate
test preserves the original Next.js contract bytes, hash and browser observer
after repair. Build success alone does not authorize promotion.

Runtime lock deadlock and data loss were **not observed**. Type repair does not
prove mutual exclusion or persistence correctness. No live model/campaign,
browser evaluation, product repair or release-sensitive behavior changed, so
release build/version verification was not required.
