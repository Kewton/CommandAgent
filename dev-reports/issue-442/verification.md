# Issue #442 verification

- Status: `passed`

Follow-up to immutable PR #444 head `8920d519e458d43ee79c9942236d57a0ea10a5b0`.
All required local checks passed. `bash scripts/ci.sh` exited 0 outside the
sandbox with `RUSTFLAGS=-D warnings`; the existing ignored tests are unchanged.
The final test-only boundary clarification was rechecked with the focused suite
and formatting check after full CI passed; production code was unchanged.
The report was set to blocked before implementation. The review snapshots in
`20260908-0054-issue442-review` were read only; all 24 match the reviewed commit.
No push, PR update, remote CI, UAT, or campaign was performed in this follow-up.

## Review reproduction before the fix

`cargo test --lib planner::profiles::nextjs::response_shape::tests::review_ -- --nocapture`
exited 101 on the unchanged production analyzer: 3 failed, 1 passed. The optional
cursor and matching `entries` controls reproduced their false positives. The
exact bare `json => json.title` example passed because `=>` was mistaken for
reassignment and suppressed analysis of the entire binding. Parenthesized
`(json) => json.title` reproduced the scope false positive. New controls also
require a later unguarded missing key to be detected after either arrow form.
The review's own JSON explicitly records its examples as not executed; the
results above are this worker's actual executions, not changes to that evidence.

## Checks

- `cargo test --lib planner::profiles::nextjs::response_shape`: `passed` (14 tests, including S3 candidate, E3 fallback, error/status controls and all review regressions)
- `bash scripts/ci.sh`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --all-targets`: `passed` (includes the business and Space/Breakout/Quiz knowledge matrix)
- `cargo test --test corpus_regression`: `passed` (7 tests)
- `cargo test --test generality_guardrails`: `passed` (10 tests; baselines unchanged)
- `cargo test --test conformance`: `passed` (18 passed, 1 existing ignored helper)
- `RUSTFLAGS='-D warnings' cargo test --doc`: `passed` (2 tests)
- `python3 scripts/validate_codex_skills.py --tracked-only`: `passed` (29 skills)
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/codex_orchestrate.py scripts/validate_codex_skills.py tests/test_codex_orchestrate.py workspace/management/scripts`: `passed`
- `python3 -m pytest tests/test_codex_orchestrate.py -q`: `passed` (71 tests)
- `python3 -m unittest discover -s workspace/management/scripts -p 'test_*.py'`: `passed` (179 tests)
- `python3 tests/eval/test_acceptance_contract.py`: `passed`
- `python3 tests/eval/test_completion_contract_snapshots.py`: `passed`
- `python3 tests/eval/test_false_positive_regression.py`: `passed`
- `shellcheck scripts/*.sh`: `passed`
- `/tmp/issue442-ci-venv/bin/python -m pytest tests/test_nextjs_domain_oracle.py -q`: `passed` (11 tests; Python 3.12.3, pinned pytest 8.4.2, Node 24.1.0, locked local TypeScript 5.9.3)
- `/tmp/issue442-ci-venv/bin/ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/nextjs_domain_oracle.py tests/test_nextjs_domain_oracle.py tests/test_codex_orchestrate.py`: `passed` (Ruff 0.16.0)
- `git diff --check`: `passed`

The dedicated workflow YAML was parsed and its pinned Node version and mandatory
pytest step checked locally. It has no path filter, skip-on-missing-dependency,
conditional job, or continue-on-error step. Branch-protection settings are not
modified or claimed verified.

## Original handler replay

The original implementation additionally compared all 43 frozen source files
with the original sessions; all SHA-256 values matched. The follow-up leaves
those fixtures and the historical response-shape corpus byte-identical to
`8920d519`; the HTTP replay rechecks their provenance hashes. The replay uses Node
24.1.0 and TypeScript 5.9.3 with unmodified source, real HTTP and real filesystem
operations in temporary directories. It is not a Next.js build/start/UI run.

| Original | Expected oracle failures reproduced |
| --- | --- |
| S1 | corrupt/directory staff reads; lost concurrent staff create; orphan shift after parent deletion |
| E1 | corrupt departments/expenses; empty data reseed; lost concurrent department create; negative amount; budget counted across months |
| I2 | corrupt/directory products/orders; lost concurrent product create; duplicate-SKU aggregate exceeds stock |
| S3 | corrupt staff/shifts; directory shifts read; empty data reseed; break equals shift duration |

The same 51 cases pass on the correct control. Original 0444 writes return 5xx
and preserve all related bytes/modes (8 file cases); these are positive controls.
S3's synchronous concurrent-create case retains both records in this one-process
replay. Rejected-operation cases require unchanged stored bytes, not status alone.
The race scheduler exposes an unsafe async read-modify-write interleaving; it is
not evidence about every scheduler or multiple Next.js processes.

## Space / Breakout / Quiz matrix

All three rows pass `space_breakout_quiz_scenario_matrix_preserves_hooks_build_and_goal`:
the goal is included, primary/state/input-coupled/restart obligations remain, and
the final phase remains build verification only. The full existing corpus (including Space/Breakout/Quiz), guardrails and
conformance passed; no baseline or threshold was changed.

## Corrections during verification

The initial HTTP test could not bind loopback inside the sandbox (`listen EPERM`);
the same test passed outside it. Initial adapter checks caught S1's mixed bare/
wrapped bodies and I2's implicit type imports. The adapter was corrected against
the unchanged source, and TypeScript compiler erasure replaced native Node type
stripping. The final focused rerun used the local npm lockfile's TypeScript,
installed with `npm ci --ignore-scripts --include=dev --prefix tests/nextjs_domain`,
and passed 11 tests. S3 directory-staff is a passing 500/preservation case and is recorded
as such. These were test-adapter corrections, not repairs to frozen artifacts.

Initial Rust checks caught the obsolete game-wording assertion and the duplicate
manifest/knowledge prompt mismatch. Text expectations and declarative manifest
values were synchronized; no gate was weakened. Candidate-review controls were
added for error branches, optional guards, inherited members and nested scopes.

The full suite also exposed repository-search contamination in the existing
`test_dependency_batches_enforce_configured_max_parallel`: the already-tracked
evaluation script contains "Independent", so all five synthetic inputs gained
a shared implementation path. Only that unit test's enrichment was isolated;
its batch/order assertions and production orchestration code are unchanged.
Per shared-test coordination, the final file was copied from the specified #440
worktree and `cmp` confirms byte identity with its `aaf5c87f4b789fe22a2cac90617bbebf14a1d673`
ancillary patch. No other #440 changes were copied.

## Deferred effect measurement

No generation campaign, B-1 experiment, build/start improvement, UI completion or
reduction in data loss is claimed. The orchestrator owns the #439/#440 merged
binary campaign, subsequent one-factor experiments, pushes/PRs, remote CI/UAT
and lifecycle changes. Its fixed metrics and loss targets are recorded in
`tests/corpus/apps/nextjs-domain-oracles/cases.json` and the adjacent README.
