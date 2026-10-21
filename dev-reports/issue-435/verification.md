# Issue #435 verification

- Status: `passed`

## Checks

- `cargo test --lib issue435`: `passed`
- `cargo test --lib recovery_authority`: `passed`
- `cargo test --lib recovery_contract_authority`: `passed`
- `cargo test --lib issue425`: `passed`
- `cargo test --lib minimal_loop::repair_target`: `passed`
- `cargo test --lib artifact_only_verify`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test profile_runtime_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

Final verification was performed on 2026-09-07. The full `cargo test` exited 0:
2,378 library tests passed, zero failed, and 16 were ignored by default; all
integration suites and both doc tests also passed. Default ignored tests were
not enabled. Python harness code was unchanged, so Ruff/pytest were not required.

## Acceptance evidence

The four Issue #435 tests cover production run-contract initialization,
admitted StepPlan registration, refresh and empty-contract Recovery binding.
R0's copied source and original contract reproduce
`weak_verification_evidence:artifact_only_verify:test -f 'package.json'`.
Re-registering the checks retains `npm run build`, removes the eight inspections,
preserves all required paths/capabilities/evidence/obligations, and passes runtime
acceptance against the recorded browser observations. The corpus detector also
passes the same source with the corrected command list.

Reintroducing each of `test -f`, `test -e` and `cat` still fails acceptance;
an engine-owned scaffold still fails with the filtered contract. Both profiles
retain substantive checks, including the test segment of a compound command.
Invalid compound checks cannot mutate the registry. Configured/data and legacy
registered-contract authority tests remain green.

Historical R0 diagnostics now select `required_evidence_missing`; S3/E3 initial
and E3 treatment diagnostics select `implementation`. Their reports remain
failed. Real package/framework diagnostics in the same report still select
configuration repair, and existing missing-entrypoint obligation behavior passes.

All four #425 focused tests pass. The two independent local measured sessions
each observe failed preflight, start Recovery once, pass the repaired syntax
check, retain failed persistence acceptance, and emit zero
`contract_command_bind_failed` events. Their fake/local drivers do not claim
full business success or promotion.

## Execution conditions and limitations

The first full run inside the sandbox encountered `Operation not permitted`
when local HTTP fixtures attempted to bind sockets; it was interrupted and
replaced by verification outside the sandbox. Development checks also caught
test-module guardrail attribution and profile-literal comparisons, which were
corrected using the repository's existing test/typed-profile conventions.
The final full run outside the sandbox passed. No guardrail baseline or test
failure expectation was weakened.

R0's four completed phases, build success and browser observations are historical
fixture inputs, not new measurements. Source hashes and event line provenance
are in the corpus `provenance.md`. No fresh Next.js production build, browser
campaign or live nine-run campaign was run. Existing historical evidence and
live runtime state were not edited. The change does not establish full domain
correctness of those apps or resolve the separate #420 short-circuit issue.
