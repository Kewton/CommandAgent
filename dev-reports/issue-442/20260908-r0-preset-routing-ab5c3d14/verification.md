# Issue #442 corrective verification — 2026-09-08

- Status: `passed`

All required local checks passed. The full CI retry exited 0 outside the
sandbox with `RUSTFLAGS=-D warnings`; documentation tests also passed. This new
record covers the corrective iteration on `ab5c3d1461ede2a294a281dc56a5dc82ca97494e`;
previous #442 reports and external historical evidence remain immutable.

## Base and scope

- `git status --short`: `passed` (empty before changes)
- `git diff --quiet`: `passed`
- `git diff --cached --quiet`: `passed`
- `git fetch origin develop`: `passed`
- `git merge --ff-only origin/develop`: `passed` (75c9a784 -> ab5c3d14, no conflict)
- `git rev-parse HEAD origin/develop`: `passed` (both ab5c3d1461ede2a294a281dc56a5dc82ca97494e after fast-forward)

Read original Issue/approved decision, phase-base/scope audit, R0 diagnosis,
source-comparison and selector-replay artifacts, and the later independent
review preparation. No main worktree mutation/build or GitHub operation.

## Reproduction and corrected failures

Before product edits, the new R0 fixture was frozen and checked against exact
old/new knowledge git objects. `cargo test --lib issue442_preset_routing_tests`
then exited 101: 3 failed, 1 passed. Actual full production rendering and real
registered selection returned nextjs-port-scripts for R0 core, with the real
package instruction and eight required artifacts. The R0 hash test passed.

Two later reviewer concerns were reproduced before their corrections:

- `cargo test --lib plural_implementation_intent_is_not_port_only`: exit 101;
  all four plural cases selected port scripts incorrectly in both layouts.
- `cargo test --lib mixed_implementation_intent_is_not_setup_or_build_only`:
  exit 101; all four mixed tasks selected setup/build incorrectly in both layouts.

The final selector keeps the complete legacy conservative implementation veto
and runs it before accepting setup/build ids. All these negatives now pass as
part of the seven-test focused group, alongside strict positive keyword checks.
The 19 original keywords plus suffix controls and scoreboard/player controlling
are exercised through the actual renderer/selector.

An early new assertion assumed every full-context template would use port 60302;
the existing global-default/port-extraction behavior is outside this scope.
Full-context controls assert selection and real configure/verify steps; the
isolated explicit port-only input asserts 60302 and package-only paths. No port
parser/guidance behavior was changed or claimed repaired.

Growth checks rejected initial module wiring in the central test index and
then the driver-test file. Wiring now lives in the existing UltraPlan flow test
module with the tests in a new leaf. Baselines and assertions are unchanged.

The first full `bash scripts/ci.sh` failed the existing manifest/knowledge
consistency test: port markers differed. The manifest's mirrored selector
keywords were synchronized, and the focused consistency check passed. The test
was not weakened. The failed log is preserved at `/tmp/issue442-routing-ci.log`:
SHA-256 `1aac776ba78a5d8ec3dccce9fcd9f75e2fd5c509ca1b78e9094954f5d7f74af6`.
The full retry uses `/tmp/issue442-routing-ci-final.log`.

## Required checks

- `cargo test --lib issue442_preset_routing_tests`: `passed` (7 tests)
- `cargo test --test nextjs_domain_knowledge --test generality_guardrails`: `passed` (2 knowledge + 10 guardrail tests)
- `cargo test --lib planner::profiles::nextjs::response_shape`: `passed` (14 tests)
- `cargo test --lib planner::profile_manifest::tests::embedded_manifest_keeps_existing_nextjs_knowledge_values`: `passed` (1 test)
- `bash scripts/ci.sh`: `passed` (retry; exit 0, failed first log preserved)
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --all-targets`: `passed` (all prior #442 tests retained; existing ignored tests unchanged)
- `cargo test --test corpus_regression`: `passed` (7 tests)
- `cargo test --test generality_guardrails`: `passed` (10 tests; no baseline changes)
- `cargo test --test conformance`: `passed` (18 passed, 1 existing ignored helper)
- `python3 scripts/validate_codex_skills.py --tracked-only`: `passed` (29 skills)
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/codex_orchestrate.py scripts/validate_codex_skills.py tests/test_codex_orchestrate.py workspace/management/scripts`: `passed`
- `python3 -m pytest tests/test_codex_orchestrate.py -q`: `passed` (71 tests)
- `python3 -m unittest discover -s workspace/management/scripts -p 'test_*.py'`: `passed` (179 tests)
- `python3 tests/eval/test_acceptance_contract.py`: `passed`
- `python3 tests/eval/test_completion_contract_snapshots.py`: `passed`
- `python3 tests/eval/test_false_positive_regression.py`: `passed`
- `shellcheck scripts/*.sh`: `passed`
- `RUSTFLAGS='-D warnings' cargo test --doc`: `passed` (2 tests)
- `/tmp/issue442-ci-venv/bin/python -m pytest tests/test_nextjs_domain_oracle.py -q`: `passed` (11 tests; Python 3.12.3, pytest 8.4.2, Node 24.1.0, local locked TypeScript 5.9.3)
- `/tmp/issue442-ci-venv/bin/ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/nextjs_domain_oracle.py tests/test_nextjs_domain_oracle.py tests/test_codex_orchestrate.py`: `passed` (Ruff 0.16.0)
- `git diff --check`: `passed`

## Coverage and integrity

The new tests run 80 real preset phase/layout/goal combinations, 23 custom
controls in both layouts, four plural and four mixed-task negatives in both
layouts, and legacy keyword/inflection controls. Both old/new R0 tasks run via
the full renderer. The current core task equals the frozen #442 task exactly.
No copied selector or stub template builder is used.

All seven diagnosis source hashes and the frozen R0 hash were checked again;
all match. All non-selector knowledge sections match the corrective base.
The retained HTTP/disk suite checks original S1/E1/I2/S3 failures against the
same 51 cases and correct controls, verifies the 43 historical source hashes,
and checks restoration. Existing response-shape controls include S3/E3 and the
previous optional/error/unknown/scope corrections.

The original six UAT scenarios remain required. Their retained local evidence
covers shape behavior, business/game guidance, original/control domain oracles,
unchanged campaign metrics, profile/event restrictions, and fixture-first/leaf/
verification scope. Prior 75c9a784 UAT approval is not transferred to this commit.

## Limits

None is planner fallback, not app completion. The existing line-based phase
field projection is not a general multiline/custom parser. The legacy veto is
conservative and may miss optimization opportunities. Full-context default-port
selection, broad template artifact ownership and the 3011 guidance conflict are
unchanged. No release binary or live generation was run here. Root owns binary
pinning, push/PR/CI/UAT/merge and lifecycle. R0 failed for separate reasons; the
nine business runs and B-1 comparison did not run. No live recovery or effect is
claimed, and the diagnostic's intermediate cutoff is not a new final result.
