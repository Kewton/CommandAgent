# Worker Sessions

## Issue #19

- Branch: `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #20

- Branch: `feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #21

- Branch: `feature/issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #22

- Branch: `feature/issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #23

- Branch: `feature/issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #24

- Branch: `feature/issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #25

- Branch: `feature/issue-25-setup-add-commandagent-doctor-built-in-environme`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #26

- Branch: `feature/issue-26-setup-release-distribution-tagged-binary-release`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-26-setup-release-distribution-tagged-binary-release`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #27

- Branch: `feature/issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #28

- Branch: `feature/issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c Codex issue worker task for Issue #20

If `$codex-issue-worker` is available in this worktree, follow that skill.
If it is not available, treat this message as the full worker instruction.

## Required Workflow

1. Read the Issue summary, acceptance criteria, approved decision, suspected files, and references.
2. Write a short design note before editing.
3. Implement the smallest coherent change that satisfies the Issue.
4. Add or update focused tests where appropriate.
5. Run focused verification, and broader checks if shared contracts are touched.
6. Write `dev-reports/issue-<number>/design.md`, `implementation-summary.md`, and `verification.md`.
7. In `verification.md`, record the exact line "- Status: `passed`" only when every required check passed, followed by one "- `<command>`: `passed`" entry per check. Use `blocked` when any required check fails or cannot run.
8. Commit the work with a clear Issue-scoped commit message.
9. Report blockers only if implementation cannot safely proceed.

## Issue Summary

- Title: [docs] Add bilingual user guide (docs/guide/ en+ja): CLI, slash commands, configuration, providers, troubleshooting
- Objective: エンドユーザー向けのリファレンスが存在しない。CLIフラグは**37個中約13個**、スラッシュコマンドは**15個中1個**しかREADMEに記載がなく、設定ファイルのスキーマ・プロバイダ設定・トラブルシューティングもまとまった文書がない。`docs/guide/` 配下に英日対訳のユーザーガイドを新設する。

## Acceptance Criteria

- `cli-reference.md` — **全37フラグ**の表(フラグ / 引数 / 既定値 / 説明 / 関連項目)。既定値・conflicts(例: `--footer` と `--no-footer` は排他, `src/cli.rs:115-128`)は `src/cli.rs` から正確に転記
- `slash-commands.md` — **全15コマンド**(`render_help` の内容が正)+ インラインフラグ + `$(cat <path>)` 展開 + プロファイル自動推論(`src/tui/slash.rs:192-199`)の説明
- `configuration.md` — 設定解決の全体像(CLIフラグ > preset > トップレベルキー > 既定値の優先順位を実コードで確認して記載)、presetの全10キーと**完全性の罠**、トップレベルキー、探索パスと順序、レガシー `.anvil/config`、環境変数(`NO_COLOR`、`ANVIL_NO_FOOTER`/`ANVIL_NO_SPINNER`/`ANVIL_NO_MARKDOWN`/`ANVIL_NO_INTERRUPT` 等のユーザー向けのもの)
- `providers.md` — プロバイダ別セットアップ表(必要なキー・取得先URL・設定方法 env/.env・Ollamaのホスト設定)。**キーの値を画面に出さない**注意書きと `.env` の権限推奨(600)を含める
- `troubleshooting.md` — 最低限: 「`GEMINI_API_KEY is not set`」(`src/config.rs:1117` のエラー文言そのまま見出しに)/「port N is busy」preflight(`src/preflight.rs:95` 周辺の挙動と選択肢)/「interaction probe unavailable」(`src/minimal_loop/interaction_probe.rs:20` の remediation)/ フッター描画乱れ(`--footer off`)/ モデルIDが実在しない場合の失敗の見え方 / Ollama未起動
- `docs/guide/README.md` — ガイドの目次(en/ja両方への入口)。`docs/model-probe.md` へのリンクを含める
- en/ja は**同一構成・同一情報**。見出し構造(h2/h3の数と順序)を一致させる(後続のdoc-driftガードIssueで機械検証する前提)
- 各ファイル冒頭に対訳ファイルへの相互リンク
- 全記載をコードと突き合わせる。とくに既定値(`num_predict`、`max_iterations`、`chat_timeout_secs`、`chat_retries`、`context_budget`)は `src/cli.rs` / `src/config.rs` の実値を確認して記載
- `.anvil/` パス・`ANVIL_*` 変数名は現状のまま記載(リネームは Issue #16/#17。ガイド側は完了後に追従)

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/cli.rs
- docs/model-probe.md
- cli-reference.md
- slash-commands.md
- configuration.md
- providers.md
- troubleshooting.md
- docs/guide/README.md
- src/config.rs
- docs/guide
- src/tui/slash.rs
- src/providers/gemini.rs
- src/providers/openai.rs
- docs/guide/en
- docs/guide/ja
- src/preflight.rs
- src/minimal_loop/interaction_probe.rs
- src/planner/side_effect_paths.rs

## References

- なし

## Required Predecessors

- Issue #19: branch `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`, worktree `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c
- Worktree: ../CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-23-repo-add-license-mit-contributing-md-changelog-m Codex issue worker task for Issue #23

If `$codex-issue-worker` is available in this worktree, follow that skill.
If it is not available, treat this message as the full worker instruction.

## Required Workflow

1. Read the Issue summary, acceptance criteria, approved decision, suspected files, and references.
2. Write a short design note before editing.
3. Implement the smallest coherent change that satisfies the Issue.
4. Add or update focused tests where appropriate.
5. Run focused verification, and broader checks if shared contracts are touched.
6. Write `dev-reports/issue-<number>/design.md`, `implementation-summary.md`, and `verification.md`.
7. In `verification.md`, record the exact line "- Status: `passed`" only when every required check passed, followed by one "- `<command>`: `passed`" entry per check. Use `blocked` when any required check fails or cannot run.
8. Commit the work with a clear Issue-scoped commit message.
9. Report blockers only if implementation cannot safely proceed.

## Issue Summary

- Title: [repo] Add LICENSE (MIT), CONTRIBUTING.md, CHANGELOG.md
- Objective: OSSプロジェクトとしての基本ファイルが不足している。`Cargo.toml:7` は `license = "MIT"` を宣言しているが **LICENSEファイルが存在しない**(宣言と実体の不一致はライセンス上問題)。CONTRIBUTING.md / CHANGELOG.md も無い。「だれもが使いたくなる」リポジトリの土台として整備する。

## Acceptance Criteria

- MIT License の全文を `LICENSE` としてルートに追加する。copyright行は `Copyright (c) 2026 Kewton` とする(リポジトリオーナー名。変更したい場合は本Issueにコメントで指示)
- README(改訂Issueと調整)に License 節を追加しリンクする
- 以下を含む(英語で作成し、要点の日本語併記は任意):
- 開発環境: Rust 1.88+(`Cargo.toml:5`)、テスト実行 `cargo test --all-targets`、PTYテスト `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`、Python 3.10(evalゴールデンテスト、`.github/workflows/ci.yml` 参照)
- **ガードレールの案内**: `docs/dev-guardrails.md`(行数バジェット: baseline+2%でCI失敗。新しいサブシステムは新モジュールへ)、`docs/mechanism-ledger.md` の互換凍結方針(イベント名・JSONキー・スキーマ不変)への言及と、抵触する変更にはledger追記が必要なこと
- corpus回帰(`tests/corpus_regression.rs`)・conformance・generality guardrails がCIで走ること
- ドキュメント対訳(README.md ⇔ README.ja.md、docs/guide/en ⇔ ja)は**同時更新**すること(doc-driftガードで構造検証される)
- PRの期待事項: テスト付き・`RUSTFLAGS="-D warnings"` でwarningゼロ(CI設定と同じ)
- docs再編Issueが先行完了している場合はパスをそれに合わせる(`docs/dev/dev-guardrails.md` 等)
- [Keep a Changelog](https://keepachangelog.com/) 形式で新設。過去の全履歴の掘り起こしはせず、`## [Unreleased]` セクション+「本ファイル開始時点(0.1.0, 2026-07)より前の変更は git 履歴と docs/mechanism-ledger.md を参照」という注記で開始する
- 以後のPRでUnreleasedに追記する運用をCONTRIBUTINGに記載

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- .github/workflows/ci.yml
- docs/dev-guardrails.md
- docs/mechanism-ledger.md
- tests/corpus_regression.rs
- docs/dev/dev-guardrails.md
- docs/guide/en
- github/workflows/ci.yml
- Cargo.lock

## References

- なし

## Required Predecessors

- Issue #19: branch `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`, worktree `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-23-repo-add-license-mit-contributing-md-changelog-m
- Worktree: ../CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
