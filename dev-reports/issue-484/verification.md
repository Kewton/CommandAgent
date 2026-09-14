# Issue #484 verification

- Status: `passed`

All required worker checks passed on the final implementation. The full Cargo
suite completed outside the sandbox with exit 0, including integration and
doc-tests; its library suite passed 2,609 tests with the existing 19 ignored.

## Checks

- `cargo test issue484 --lib`: `passed`
- `cargo test issue478 --lib`: `passed`
- `cargo test issue479 --lib`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test template_owned_prompt_feedback_rejects_mixed_final_acceptance_scope --lib`: `passed`
- `cargo test child_that_responds_500_reports_http_failure --lib`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence and scope

Baseline/fetched develop: `b7b65f8fd75793f2075491889c66372b02a06d14`.
Node: `v24.1.0`. Final focused #484 suite: 13 passing tests. #478: 7;
#479/#480-related regressions: 21; corpus: 7; guardrails: 10.
The new corpus expectation deliberately rejects implementation-artifact credit
for a passing package-only comparison.

The saved runner replay rejects the weak original on attempt one and admits the
complete literal comparison and ordered owner split on attempt two. Real Node
checks preserve the matching stdout exactly and fail nonzero with no success
stdout for missing script/object, mismatch, absent file and invalid JSON. An
absent package file retains the existing product `DependencyMissing` report
classification; raw Node and the product both fail. No failure is accepted as
success.

The design and owner-split follow-ups are covered by unsafe-character and
per-key conflict controls, stdout/exit controls, and the table-driven source,
writer, alias, ID and ordering refusals. The code-review follow-up is covered by
the Ready-then-lint-retry acquisition test and explicit duplicate-strengthen
refusal. Retained snapshots match the proposal first retained, then remain fixed.

`formation-evidence.json` contains selected fields from actual product events,
not the raw event stream. `initial_rejection.event` and `planner_attempt` locate
the acquisition event; `validated.package_script_formation.sources.source_file`
identifies its stream. Each Source JSON pointer resolves relative to `validated`
into the embedded immutable snapshot; tests recompute its step/instruction hashes.
`source-manifest.json` separately pins the historical file SHA-256 and pointer.
Missing source evidence is never replenished from artifacts or later proposals.
Standalone ordered-split lint is not final acceptance: the positive test passes
real runner Admission against saved sources and then lint.

## Earlier attempts

The initial sandbox full run could not bind local HTTP mocks (`Operation not
permitted`); its waiting test process was stopped and the same suite rerun
outside the sandbox. An early outside run exposed a template-prompt regression,
which was narrowed to preserve the old behavior. Its HTTP-500 mock also failed
once under concurrent execution and passed the focused rerun. Integration guards
then identified growth/profile-literal violations; the existing setup predicate
was extracted to a leaf and profile access uses the existing boundary APIs and
identifiers. No guard baseline, timeout, attempt budget or evidence gate was
raised. The final full run passed with no failures; existing ignored tests were not
reclassified or enabled.

## Parent-only stages

PR, exact-HEAD CI/UAT, integration and integrated release version/hash: **not run
by this worker; pending parent execution**. They are not included in the worker
passed verdict. No push, PR, Issue mutation or live model reevaluation occurred.
