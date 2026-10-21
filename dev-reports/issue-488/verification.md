# Issue #488 verification

- Status: `passed`

All required checks below completed successfully after the corrections recorded
in the intermediate-failures section. The full suite ran outside the sandbox
because its existing local HTTP tests require listeners.
Environment: macOS/aarch64, Node `v24.1.0`, rustc `1.94.0`, cargo `1.94.0`.
Tests use real local Node processes and communication-success mock providers.
No live model API or browser experiment was used.

## Checks

- `ISSUE488_NODE_EVIDENCE=dev-reports/issue-488/node-observations.json ISSUE488_PLANNER_EVIDENCE=dev-reports/issue-488 cargo test issue488 --lib`: `passed`
- `cargo test recovery_step_plan_binding --lib`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test protection_coverage_audit --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

Focused: 12 tests, including 24 real Node/direct/shell/Verify cases, 24 actual
final-executor cases, 24 registration batches and four mock series. Related
binding tests: 53; corpus: 7; generality guardrails: 10; protection audit: 2.
Each command exited 0. The full suite completed unit, integration and doc tests:
2976 passed / 0 failed / 42 ignored across top-level targets (nested subprocess
results are not double-counted). Its unit result was 2624 passed / 0 failed /
19 ignored. Existing ignored tests
retain their defaults; no ignored/live/PTY test is claimed as executed.
The pre-production baseline reproduction passed one test, as recorded in
design.md; its baseline-only expectation was subsequently updated to the
corrected behavior rather than left as a failing regression.

Final raw local logs (not committed): `/tmp/issue488-focused-final.log`,
`/tmp/issue488-related-final.log`, `/tmp/issue488-corpus-final.log`,
`/tmp/issue488-audits-final.log`, `/tmp/issue488-clippy-final.log`,
`/tmp/issue488-cargo-test-complete.log`. Corpus fixture hashes and all four
command SHA256 values were also checked against source-manifest.json and the
saved Issue command values. Captured JSON was regenerated with the corrected
argv observation names and original→replacement/trusted-contract records.

## Intermediate failures and corrections

- Initial test compilation used the wrong deferred-requirement argument type;
  subsequent test additions had missing imports/wrong CompileError fields. These
  were test compilation errors, corrected without changing product contracts.
- Initial policy controls incorrectly assumed curl/rm/nonliteral Node spellings
  were rejected by this baseline, and that a compound containing inline JS was
  safely splittable. Inspected existing normalization; now test a known rejected
  parent-traversal command, preserve nonliteral behavior and use a supported
  simple compound. No allowance was added to satisfy the test.
- An unknown obligation is rejected by contract validation before formation;
  the test now distinguishes that rejection from the new gate predicate.
- Initial setup-fallback test took deterministic scaffold generation before any
  mock request. Corrected the invocation to reach the actual fallback after three
  mock replies. Added actual last-valid return and formation-Retry-clearing cases
  following parent review; direct finish tests remain separately identified.
- A shell success wrapper was erased by existing normalization. Added raw proposal
  validation for the new finite rewrite, consistent with #484, then its refusal
  test passed.
- First corpus run: 6 passed / 1 failed because its lightweight parser interpreted
  a quoted fixture key literally. Changed the new expectation key to existing
  corpus syntax; no detector/acceptance expectation was weakened.
- First clippy run reported two needless test borrows/clones; corrected them.
- Parent review corrected an overstrong argv observation field. Saved evidence
  now states classifier_parser_argv and shell_and_direct_outcomes_match, with no
  claim of observing shell-process argv.
- The sandbox full-suite attempt failed local HTTP listeners with `Operation not
  permitted`; five tests then remained waiting. Terminated only that invocation
  and its confirmed child test process. This attempt is not counted as passed.
  Raw log: `/tmp/issue488-cargo-test.log`.
- The first outside-sandbox full suite passed all unit tests (2624 passed, 19
  ignored), then failed `nextjs_boundary_erosion_tripwire_keeps_dispatch_sites_audited`
  (generality guardrails: 9 passed, 1 failed; cargo exit 101). The new registration
  helper had two direct Next.js literal comparisons, and the new final-acceptance
  test file lacked the existing scanner's local cfg(test) module convention.
  Reused `profile::is_nextjs_profile` for both predicates and wrapped the test file
  in `#[cfg(test)] mod tests`, following existing final-acceptance tests. No
  guardrail expectation, baseline or scanner was changed. Focused guardrails now
  pass 10/10. Raw failed log: `/tmp/issue488-cargo-test-unsandboxed.log`.
- The next outside-sandbox full suite reached `protection_coverage_audit` and
  failed its normalization-boundary rule on a new test's direct
  `validate_verify_command` call (1 passed, 1 failed; cargo exit 101). That rule
  scans both production and test files without a cfg(test) exclusion. The control
  now calls `normalize_verify_command` (the same implementation used by
  `validate_verify_command`) and asserts the exact retained spelling. No policy,
  audit exception or test expectation was relaxed. Raw failed log:
  `/tmp/issue488-cargo-test-final.log`.


## Acceptance criteria trace

Test names below omit their module prefix. All new tests are selected by
`cargo test issue488 --lib`; related existing tests are selected by
`cargo test recovery_step_plan_binding --lib` and the full suite.

| AC | Evidence and scope |
| --- | --- |
| 1. Baseline/version/gap | design.md records the pre-production dynamic test: O/V1/SWALLOW already Retry in Admission, direct StepPlan registration still succeeded, final static gate failed; V2 passed, but O/V1/SWALLOW→V2 rewrites were refused. `issue488_real_formation_registration_and_final_boundary` locks the corrected behavior. #479/#484 commits are already ancestors; no predecessor branch was assumed or imported. |
| 2. Exact commands/fixtures | `tests/corpus/apps/issue488-d4-inline/{commands,contract,matrix,source-manifest}.json`, normal + four omission fixtures + missing file. Per-command hashes match the Issue and immutable historical command input. `issue488_real_node_matrix_preserves_classification_and_failure_observations` consumes 24 cases. `node-observations.json` saves exit, stdout, stderr hash/failure excerpt and classifier-parser argv supplied to direct Node. It asserts shell/direct outcomes match; shell argv itself is not observed. README describes #479/#484 corpus differences. |
| 3. Narrow scope + broad prior refusal | `issue488_authority_and_requirement_controls_preserve_existing_broad_formation`: configured, unowned, already registered, Config profile, contract profile, fix/investigate intent, empty requirements, unknown/normalized obligations, capability-only controls. Authority comes from Config + current-run provenance, never proposal goal/name. #466 nonexecuting-step/compound test covers candidate filtering; raw policy/profile filtering precedes the new guard. The 24-case matrix repeats Admission/registration with every source omission, showing file-independent classification. |
| 4. V2 and independent gates | `issue488_final_executor_keeps_weak_and_actual_command_failure_separate` runs actual final acceptance for all 24 cases using an explicitly generic profile to isolate command/evidence, and separately asserts the Next.js gate rejects the reduced non-app fixture. `issue488_other_requirements_compile_and_business_failure_remain_independent` executes an independent Node assertion over synthetic saved/reloaded data and passes the negative result to the existing behavior gate. No live app success, browser or build is claimed. Baseline has no separate source_verifier_admission module; parent WIP is not imported. |
| 5. Requirement-preserving rewrite | `issue488_mock_rewrite_accepts_full_requirements_and_records_mapping` admits O/V1/SWALLOW→V2 while preserving full model/host duties and all four ordered predicates. `issue488_mock_exhaustion_and_missing_duties_never_register_or_execute` consumes 15 refusal cases (condition, path, stdout, message, output, both owners, result, owner/check order, other check, added success exit, unknown regex, shell masking, repeated weak). Existing replacement event records original/normalized command, step/index, target/predicates/stdout and validated replacement. Saved mappings and trusted-contract snapshot/hash are in planner-success.json. |
| 6. Four mock series + real returns | planner-success.json, planner-refusals.json, planner-fallback.json and planner-last-valid.json capture actual last user request bodies, complete request hashes, returned plan/error, proposal count and boundary events. All replies are communication-success mocks. Success takes 2 proposals; repeated weak/omission takes 3, leaving a fourth reply unused; the execution client is untouched and contract bytes unchanged. `issue488_mock_fallback_rechecks_retained_scope_and_weak_candidate` reaches actual setup fallback; its direct finish checks are separately labelled. `issue488_driver_last_valid_return_and_formation_retry_clearing_are_observed` reaches quality→lint/schema→last-valid return and separately observes formation Retry clearing last_valid. Recovery budget flags remain false; no retry cap changes. |
| 7. Atomic registration | `issue488_each_registration_entry_is_atomic_in_both_orders_and_accepts_good_batch`: command, StepPlan/producer and empty handoff entry × O/V1/SWALLOW/all-good × both orders = 24 batches. 18 mixed batches reject with byte-identical contract/commands, unchanged producer obligations, no success event; 6 all-good batches succeed. |
| 8. Existing exclusions/authority | The authority test plus `issue488_policy_normalization_generated_hooks_and_closed_contracts_stay_separate` cover configured/unowned/registered/closed, other profile/intent, nonliteral classification, policy rejection, supported simple compound vs unsupported inline compound and generated hooks. Baseline nonliteral Node policy is not silently tightened or relaxed. #466/#479/#484 related tests cover existing broader restrictions and closed producer authority. |
| 9. Mixed failures/identity | `issue488_other_requirements_compile_and_business_failure_remain_independent` retains missing required evidence together with weak evidence, then adds a labelled synthetic compiler observation without erasing either; synthetic business failure remains a separate behavior-gate failure. `issue488_contract_identity_is_checked_before_fixed_evidence_diagnosis` changes only frozen contract whitespace and verifies the independent contract-changed rejection for O and V2. No C5-specific routing is introduced or claimed. |
| 10. Verification | Final Checks section records focused, related, corpus, fmt, clippy and full cargo test exit results. No release-sensitive change: no release build/version check required. No Python harness change. |

## Boundary/source notes

Trusted run profile/intent use `Config::profile` and `resolved_run_intent`;
contract ownership is `begin_run` + `record_generated_contract`/`owns_run_contract`.
Configured-path detection and registered command membership prevent new authority
for closed checks. `admitted_final_success_commands` retains normal normalization,
command/path validation and artifact/profile filtering. The new helper feeds the
same `verify_command_kind` and `refresh_runtime_acceptance_report` used by final
acceptance, with an empty workspace to establish command-local classification.
Unknown obligation roles still fail contract validation; normalized roles and
empty requirement sets follow the existing final gate.

The fully saved Issue body, C5 report/repair-spec/freeze/command input, independent
review and cause-report-D4 were read from the parent read-only workspace. No
historical artifacts or parent WIP were changed. Historical mixed C5 routing
observations are context, not measurements of this baseline. Curated evidence
contains mock bodies and test observations only; raw full-suite logs stay in /tmp.
