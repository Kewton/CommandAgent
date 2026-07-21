# Dependency Plan

## Parallel Batches

- Batch 1: #19
- Batch 2: #20, #23
- Batch 3: #21, #25
- Batch 4: #22
- Batch 5: #24
- Batch 6: #26
- Batch 7: #27
- Batch 8: #28
- Batch 9: #29

## Merge Order

#19, #20, #23, #21, #25, #22, #24, #26, #27, #28, #29

## Issue Plans

### Issue #19

- Classification: `weak-conflict`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Worktree: `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Suspected files: `README.md, README.ja.md, .github/workflows/ci.yml, src/tui/ux_demo.rs, scripts/setup.sh, src/cli.rs, src/providers/gemini.rs, src/providers/openai.rs, docs/assets, docs/guide, docs/dev, src/tui/slash.rs, src/config.rs, github/workflows/ci.yml, src/minimal_loop/evidence.rs`
- References: `none`

### Issue #20

- Classification: `weak-conflict`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Worktree: `../CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Suspected files: `src/cli.rs, docs/model-probe.md, cli-reference.md, slash-commands.md, configuration.md, providers.md, troubleshooting.md, docs/guide/README.md, src/config.rs, docs/guide, src/tui/slash.rs, src/providers/gemini.rs, src/providers/openai.rs, docs/guide/en, docs/guide/ja, src/preflight.rs, src/minimal_loop/interaction_probe.rs, src/planner/side_effect_paths.rs`
- References: `none`

### Issue #21

- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #19, #20
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Worktree: `../CommandAgent-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Suspected files: `docs/data-profile-contract.md, dev-guardrails.md, mechanism-ledger.md, generality.md, perf-notes.md, integration-notes.md, uat-corpus.md, uat/scenarios.md, profile-manifest.md, data-profile-contract.md, model-probe.md, docs/README.md, minimal-loop-expanded.yaml, eval/README.md, docs/integration-notes.md, README.md, SECURITY.md, docs/dev/data-profile-contract.md, data-profile-contract.en.md, benchmarks/README.md, scripts/bench.sh, bench.sh, tests/generality_guardrails.rs, src/minimal_loop/repair_pressure.rs, docs/dev, docs/guide, src/minimal_loop/loop_run.rs`
- References: `none`

### Issue #22

- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #20, #21
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Worktree: `../CommandAgent-issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Suspected files: `src/cli.rs, tests/doc_drift.rs, docs/guide/en/cli-reference.md, docs/guide/en/slash-commands.md, src/config.rs, .github/workflows/ci.yml, docs/guide/README.md, docs/guide, src/tui/slash.rs, docs/guide/en, docs/guide/ja, github/workflows/ci.yml, docs/dev-guardrails.md`
- References: `none`

### Issue #23

- Classification: `weak-conflict`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Worktree: `../CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Suspected files: `.github/workflows/ci.yml, docs/dev-guardrails.md, docs/mechanism-ledger.md, tests/corpus_regression.rs, docs/dev/dev-guardrails.md, docs/guide/en, github/workflows/ci.yml, Cargo.lock`
- References: `none`

### Issue #24

- Classification: `weak-conflict`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Worktree: `../CommandAgent-issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Suspected files: `scripts/setup.sh, bench.sh, eval-run.py, Cargo.toml, .github/workflows/ci.yml, scripts/*.sh, ./scripts/setup.sh, install.sh, src/cli.rs, src/config.rs, src/minimal_loop/interaction_probe.rs, github/workflows/ci.yml, README.md`
- References: `none`

### Issue #25

- Classification: `weak-conflict`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-25-setup-add-commandagent-doctor-built-in-environme`
- Worktree: `../CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme`
- Suspected files: `scripts/setup.sh, docs/model-probe.md, src/cli.rs, .anvil/config.toml, src/doctor.rs, src/lib.rs, docs/dev-guardrails.md, runner.rs, loop_run.rs, docs/mechanism-ledger.md, setup.sh, src/preflight.rs, src/minimal_loop/interaction_probe.rs, src/tui/banner.rs, src/tui/slash.rs, src/config.rs, anvil/config.toml, README.md`
- References: `none`

### Issue #26

- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #19, #23, #24
- Dependency source: `explicit`
- Approved decision: Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.
- Branch: `feature/issue-26-setup-release-distribution-tagged-binary-release`
- Worktree: `../CommandAgent-issue-26-setup-release-distribution-tagged-binary-release`
- Suspected files: `ci.yml, Cargo.toml, build.rs, .github/workflows/release.yml, scripts/install.sh, scripts/setup.sh, install.sh, tests/corpus, tests/tui_pty.rs, github/workflows/release.yml, README.md`
- References: `none`

### Issue #27

- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #24
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Worktree: `../CommandAgent-issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Suspected files: `Cargo.toml, src/cli.rs, scripts/setup.sh, src/config.rs, docs/guide, README.md, docs/codex-harness.md, docs/generality.md`
- References: `none`

### Issue #28

- Classification: `weak-conflict`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Worktree: `../CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Suspected files: `.github/workflows/ci.yml, tests/live_provider.rs, scripts/bench.sh, scripts/eval-run.py, .devcontainer/devcontainer.json, Cargo.toml, tests/eval/test_, github/workflows/ci.yml, devcontainer/devcontainer.json, src/planner/profiles/data/manifest.toml`
- References: `none`

### Issue #29

- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #19, #20, #21, #22, #23, #24, #25, #26, #27, #28
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-29-tracking-documentation-modernization-en-ja-setup`
- Worktree: `../CommandAgent-issue-29-tracking-documentation-modernization-en-ja-setup`
- Suspected files: `README.md, data-profile-contract.md, Cargo.toml, docs/dev-guardrails.md, docs/mechanism-ledger.md, scripts/setup.sh, docs/guide, scripts/eval_lib/acceptance_contract.py`
- References: `none`

## Blocked Items

None at dry-run planning time.
