# Issue #479 CI runtime follow-up

The first PR acceptance run (34765112448, candidate 3c814d91) failed two saved
TypeScript module checks: `issue479_saved_modules_strengthen_loadability_and_explicit_export_boundary`
and `issue479_refined_final_acceptance_executes_target_failures_without_business_credit`.
The import executed, but its runtime namespace differed from the explicit
`["default", "module.exports"]` expectation. The worker's actual Node v24.1.0
controls passed. The failing workflow did not record its effective Node version,
so a runtime-version difference remains a hypothesis until CI verifies the fix.

The main CI and Next.js-domain workflows already selected Node v24.1.0, but
acceptance invokes the same full suite without selecting Node; the release
source-test job likewise runs cargo test without declaring this prerequisite.
Select Node v24.1.0 in both jobs and print its version before running tests.
The release publishing trigger, permissions, dependencies and build jobs remain
unchanged. No release is invoked by this change.

No Rust source, classifier, expected namespace, fixture or acceptance predicate
is changed. The strict positive and refusal controls remain the same. Codex2
review agreed that consistent test prerequisites are appropriate and rejected
weakening the expectation. Effective old CI Node is unobserved; the follow-up
CI result remains a parent gate and is not predeclared passed here.

Local validation parses all workflow YAML and checks that the acceptance/release
Node setup precedes the complete test command, uses 24.1.0, and records its version.
Git diff inspection confirms only the two workflows and this design/report change.
Previously passed Rust checks are reused because production and tests are byte-identical.
