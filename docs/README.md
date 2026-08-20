# Documentation Index

CommandAgent separates end-user guidance in [`guide/`](guide/README.md) from
contributor material in [`dev/`](dev/) and repository-level contracts kept at
stable paths. Start with the root [English](../README.md) or
[Japanese](../README.ja.md) README if you are new to the project.

> Historical records describe the repository at the time they were written.
> They may not match the current code. In particular, recorded paths and
> implementation details are evidence, not current instructions.

Language values are **EN**, **JA**, or **Mixed**. Audience values distinguish
**End users**, **Contributors**, and **Historical records**. The mixed-language
content in `mechanism-ledger.md` and `integration-notes.md` is intentionally
left untranslated because these files are internal ledgers.

The immutable migration evidence is explicitly excluded from reorganization:
both files remain under [`migration/`](migration/) at their original tracked
paths.

## End-user guides

| File | Description | Language | Audience |
| --- | --- | --- | --- |
| [`guide/README.md`](guide/README.md) | Bilingual entry point for the user guide. | Mixed | End users |
| [`guide/model-probe.md`](guide/model-probe.md) | Bounded provider/model behavior measurement workflow. | EN | End users |
| [`guide/en/tutorial.md`](guide/en/tutorial.md) | Real-screen walkthrough: doctor, first REPL request through Gate 1–4, one GUI Trial. | EN | New users |
| [`guide/en/cli-reference.md`](guide/en/cli-reference.md) | CLI flags, defaults, and conflicts. | EN | End users |
| [`guide/en/configuration.md`](guide/en/configuration.md) | Configuration files, presets, and precedence. | EN | End users |
| [`guide/en/providers.md`](guide/en/providers.md) | Ollama, LM Studio, OpenAI, and Gemini setup. | EN | End users |
| [`guide/en/slash-commands.md`](guide/en/slash-commands.md) | Interactive slash-command reference. | EN | End users |
| [`guide/en/troubleshooting.md`](guide/en/troubleshooting.md) | Startup, provider, and TUI troubleshooting. | EN | End users |
| [`guide/ja/tutorial.md`](guide/ja/tutorial.md) | 実際の画面で追うウォークスルー: doctor、最初の REPL 依頼から Gate 1〜4、GUI Trial 1 本。 | JA | New users |
| [`guide/ja/cli-reference.md`](guide/ja/cli-reference.md) | CLI フラグ、既定値、排他関係。 | JA | End users |
| [`guide/ja/configuration.md`](guide/ja/configuration.md) | 設定ファイル、preset、優先順位。 | JA | End users |
| [`guide/ja/providers.md`](guide/ja/providers.md) | Ollama、LM Studio、OpenAI、Gemini の設定。 | JA | End users |
| [`guide/ja/slash-commands.md`](guide/ja/slash-commands.md) | 対話型スラッシュコマンドのリファレンス。 | JA | End users |
| [`guide/ja/troubleshooting.md`](guide/ja/troubleshooting.md) | 起動、プロバイダ、TUI のトラブルシューティング。 | JA | End users |
| [`user/getting-started-cli.md`](user/getting-started-cli.md) | Install-to-first-loop CLI path, pack A/B, and bilingual glossary. | Mixed | New users |
| [`user/getting-started-gui.md`](user/getting-started-gui.md) | First GUI landing, readiness, sample Trial, Gate 1, and result reading. | Mixed | New users |
| [`user/gui.md`](user/gui.md) | Stable compatibility index for the former monolithic GUI guide. | EN | End users |
| [`user/gui-trial.md`](user/gui-trial.md) | Gate 1–4 Trial flow, reconnect, and read-only lease recovery. | Mixed | End users |
| [`user/gui-history.md`](user/gui-history.md) | Trial session source, pack columns, refresh, and A/B reading. | Mixed | End users |
| [`user/gui-extensions.md`](user/gui-extensions.md) | Extension catalog, bounded lifecycle, naming, and review preparation. | Mixed | Extenders |
| [`user/gui-setup.md`](user/gui-setup.md) | Setup/preflight, base paths, root separation, and proxy routing. | Mixed | Operators |
| [`user/gui-operations.md`](user/gui-operations.md) | Tokens, Origins, APIs, backups, recovery, and GUI smoke. | Mixed | Operators |
| [`user/gui-help-map.md`](user/gui-help-map.md) | One-to-one map from stable in-app help copy to guide sections. | Mixed | End users |
| [`user/headless.md`](user/headless.md) | Machine-readable terminal summaries and exit semantics. | EN | End users |
| [`user/first-loop.md`](user/first-loop.md) | Japanese four-gate and exact-pack A/B walkthrough. | JA | End users |

## Contributor documents and contracts

| File | Description | Language | Audience |
| --- | --- | --- | --- |
| [`README.md`](README.md) | This documentation map. | EN | Contributors |
| [`codex-harness.md`](codex-harness.md) | Repository-local Codex workflow and command map. | EN | Contributors |
| [`fix-intent-contract.md`](fix-intent-contract.md) | Frozen v0 fix-intent evidence and assurance contract. | JA | Contributors |
| [`investigation-intent-contract.md`](investigation-intent-contract.md) | Frozen v0 investigation-intent contract. | JA | Contributors |
| [`intent-skeleton.md`](intent-skeleton.md) | Intent adjudication design and compatibility boundaries. | JA | Contributors |
| [`dev/dev-guardrails.md`](dev/dev-guardrails.md) | Source growth budgets and engineering guardrails. | EN | Contributors |
| [`dev/generality.md`](dev/generality.md) | Generality and earned-assurance policy. | Mixed | Contributors |
| [`dev/profile-manifest.md`](dev/profile-manifest.md) | Profile manifest schema and lifecycle. | EN | Contributors |
| [`dev/extension-catalog.md`](dev/extension-catalog.md) | Registered source/check, profile descriptor, guard, locator, and supply workflows. | EN | Contributors |
| [`dev/repository-validation.md`](dev/repository-validation.md) | Maintainer build, UAT, and copy-validation procedures. | EN | Contributors |
| [`dev/data-profile-contract.md`](dev/data-profile-contract.md) | Canonical frozen v0 data-profile contract; see the [English counterpart](dev/data-profile-contract.en.md). | JA | Contributors |
| [`dev/data-profile-contract.en.md`](dev/data-profile-contract.en.md) | Reference translation; the [Japanese counterpart](dev/data-profile-contract.md) remains authoritative. | EN | Contributors |

## Historical records

| File | Description | Language | Audience |
| --- | --- | --- | --- |
| [`dev/mechanism-ledger.md`](dev/mechanism-ledger.md) | Chronological implementation and decision ledger. | Mixed | Historical records |
| [`dev/integration-notes.md`](dev/integration-notes.md) | Integration findings and bounded regression notes. | Mixed | Historical records |
| [`dev/perf-notes.md`](dev/perf-notes.md) | Recorded performance investigations and results. | EN | Historical records |
| [`dev/uat-corpus.md`](dev/uat-corpus.md) | UAT corpus definitions and observed outcomes. | EN | Historical records |
| [`dev/uat/scenarios.md`](dev/uat/scenarios.md) | Historical UAT scenario prompts and procedures. | Mixed | Historical records |
| [`migration/migration-report.md`](migration/migration-report.md) | Repository migration record, retained at its immutable original path. | EN | Historical records |
| [`migration/anvil-commit-map.txt`](migration/anvil-commit-map.txt) | Frozen old-to-new commit mapping, retained at its immutable original path. | EN | Historical records |

## Supporting assets

| File | Description | Language | Audience |
| --- | --- | --- | --- |
| [`assets/ux-demo.md`](assets/ux-demo.md) | Instructions for reproducing the README terminal demo. | EN | Contributors |
| [`assets/ux-demo.tape`](assets/ux-demo.tape) | VHS source for the terminal demo. | EN | Contributors |
| [`assets/ux-demo.svg`](assets/ux-demo.svg) | Rendered terminal-demo artwork used by the READMEs. | EN | End users |
