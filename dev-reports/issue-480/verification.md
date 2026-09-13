# Issue #480 worker verification

- Status: `passed`

All required worker-local source verification passed. Parent-owned exact-commit
UAT/CI, integration and live evaluation remain separate downstream gates.

## Checks

- `cargo test --lib issue480`: `passed`
- `cargo test --lib contract_attribute`: `passed`
- `cargo test --lib issue474`: `passed`
- `cargo test --lib issue478`: `passed`
- `cargo test --lib issue479`: `passed`
- `cargo test --lib issue465`: `passed`
- `cargo test --lib issue466`: `passed`
- `cargo test --lib ultra_final_acceptance_probe_does_not_promote_missing_restart_contract -- --nocapture`: `passed`
- `cargo test --lib ultra_final_acceptance_event_carries_generic_static_assurance`: `passed`
- `cargo test --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `shasum -a 256 target/release/commandagent`: `passed`
- `git diff --check`: `passed`

## Measured results

The full outside-sandbox Cargo run exited 0: 2,596 library tests passed, 19
existing library tests were ignored, and every default integration suite and
both doctests passed. Its 81 result records include subprocess self-tests;
summing those records gives 2,958 passes and 42 existing ignores. No new test
was ignored and ignored tests are not claimed as executed verification.

The #480 filter passed eight tests, including the corpus-driven 13 positive and
12 generic variants. The saved #474 filter passed nine; contract-attribute
guidance/target tests passed 13; #478 passed seven; #479 passed 21; #466 passed
25. #465 passed 23 with its two existing installed-toolchain controls ignored.
The complete test run also passed all seven corpus-regression tests, all ten
generality/growth guardrails and both protection-coverage audits. The final
acceptance event-key regression passed with its existing expected key list.

The actual Runner structured hook-status regression passed with stronger
assertions on its execution-client request: missing restart attribute and
`src/app/page.tsx` must be present in the final repair prompt. Both final prompt
layouts separately pass the saved Node predicate and unknown-case controls;
step repair guidance uses the same supported causal diagnosis.

The saved source-restoration control accepts only a fresh successful execution;
unrelated API edits do not clear either the failed predicate or the unchanged
Weak verifier. Namespace export assertions keep StaticSyntax, business assertions
keep Test, and all ambiguous attribute failures stay generic. Command/contract
bytes remain unchanged during final execution and diagnosis.

## Development failures and corrections

The intentional pre-fix red control is in
[pre-fix-reproduction.md](pre-fix-reproduction.md). An intermediate run retained
the error but could not parse the existing `build_verify_failed` wrapper; known
lifecycle wrappers are now handled explicitly. Older synthetic command-only
guidance tests were converted to actual product executions, while an explicit
synthetic-report refusal verifies the new authority boundary.

The first combined corpus/guard/protection run passed corpus and growth checks
but failed the protection audit because a test used an inferred verifier type.
The test now explicitly names `NormalizedVerifyCommand`; its focused rerun and
the full suite passed without changing any audit or guard baseline. Clippy's
test slice-clone finding was corrected and its final run passed. The secret
redaction fixture initially triggered path confinement before execution; its
synthetic path is now constructed as output, without accessing any outside file.

## Release and provenance

Runtime: Node v24.1.0; Cargo 1.94.0. Predecessor HEAD is
`502a3639893f8f61db8747533848a90f35cdc84b`, including both requested report/CI
follow-ups. Their incorporation preserved all #480 work. `origin/develop` was
fetched and remains `fdc98a9b7d865b4c2115138765d3ff7b32612dae`.

The precommit release build completed successfully in 48.26 seconds:

```text
commandagent 0.1.0 502a3639+dirty 2026-09-14T00:29:15+09:00
SHA256 d3355bb3da7fedfd416519689e98eea54fb47457566005eb68943248647ac2e9
```

This identifies the verified precommit source build. A fresh committed-HEAD
build, its clean version/hash and the official worker-report parser readback
follow the commit and are returned in the handoff; the precommit binary is not
presented as an integrated release.

No production runner/loop chokepoint, guard baseline, immutable historical
evidence, knowledge manifest or live `.anvil` namespace was changed. Raw test
logs remain temporary and are not committed. No push, PR, Issue mutation,
CommandMate operation or live model/evaluation run was performed. Parent-owned
UAT/CI, review, integration and integrated release checks are not marked passed
by this worker report. Historical evaluation rows retain their original
denominator and diagnostic limitations.
