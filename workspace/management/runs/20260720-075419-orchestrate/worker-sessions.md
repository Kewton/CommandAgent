# Worker Sessions

## Issue #19

- Branch: `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #20

- Branch: `feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #21

- Branch: `feature/issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #22

- Branch: `feature/issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #23

- Branch: `feature/issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #24

- Branch: `feature/issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #25

- Branch: `feature/issue-25-setup-add-commandagent-doctor-built-in-environme`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #26

- Branch: `feature/issue-26-setup-release-distribution-tagged-binary-release`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-26-setup-release-distribution-tagged-binary-release`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #27

- Branch: `feature/issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #28

- Branch: `feature/issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui Codex issue worker task for Issue #19

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

- Title: [docs] Overhaul README (EN) and add README.ja.md: quickstart, install, features, demo, badges, license
- Objective: 現在の `README.md`(173行・英語)は唯一のユーザー入口だが、簡素な2行の説明(`README.md:3-4`)の直後にBuild節(`README.md:11`)へ飛び、最初の実行例は6フラグ+プレースホルダモデルIDのコマンド(`README.md:22`)という構成で、初見者に厳しい。「だれもが使いたくなる」品質のREADMEへ全面改訂し、日本語版 `README.ja.md` を併設する。

## Acceptance Criteria

- **1行の価値提案**(何であるか+なぜ使うか)と簡潔なリード文
- **バッジ**: CIステータス(`.github/workflows/ci.yml` のワークフロー名 `CI` を参照)、ライセンス(MIT)。存在しないもの(crates.io等)のバッジは貼らない
- **機能一覧**: minimal loop / step plan(plan-run)/ ultra plan run / プロファイル(generic・nextjs・python_cli・data)/ マルチプロバイダ(Ollama・Gemini・OpenAI)/ 検証・修復ループ / TUI(フッター・スピナー・割り込み)を箇条書きで
- **デモ**: `--ux-demo` を録画したGIFまたはSVGアニメを `docs/assets/` に置き先頭近くに埋め込む。再現手順(録画コマンド)をアセットと同じ場所にメモとして残す(例: charmbracelet/vhs のtapeファイル or asciinema。ツール選定は実装者に委ねるが再現可能にすること)
- **Quickstart(最小経路)**: Ollamaのみで動く最短手順(Ollamaインストール→モデルpull→`cargo install --path .`→1コマンド実行)。モデルIDは「**手元に実在するモデルに置き換える**」旨を明記し、例は `<your-model>` プレースホルダ表記にする
- **Install**: 前提条件表(Rust 1.88+ / 任意: Ollama、Gemini・OpenAI APIキー、Node.js+npm(インタラクションプローブ用)、Python 3(eval用))と導入手順。`scripts/setup.sh`(別Issue)が入り次第そのリンクを張る前提の構成にする
- **Usage**: 代表的なCLI実行例とREPLの主要スラッシュコマンド抜粋(全リファレンスは docs/guide/ へのリンク。別Issue)
- **Configuration**: preset の存在と設定ファイルの場所のみ簡潔に示し、詳細はガイドへのリンク
- **License 節**(MIT)
- 現READMEの UAT・copy-validation・symlink運用などの**開発者向け内容は README から削除し docs/dev/ へ移す**(docs再編Issueと調整。両Issueを同一PRにしてもよい)
- `README.ja.md` を新設し、英語版と**同一構成・同一情報**の日本語訳とする
- 両ファイルの冒頭に相互リンク(`English | 日本語`)を置く
- 以後の更新で両方を同時に更新する旨をCONTRIBUTING(別Issue)に記載するため、本Issueでは両ファイル冒頭コメントに「対訳ファイルあり・同時更新のこと」の注記を入れる
- 記載するフラグ・コマンド・パスはすべて現行コード(`src/cli.rs`, `src/tui/slash.rs:408-429`, `src/config.rs:663-668`)と突き合わせて正確にする
- 「Anvil does not auto-create them」相当の情報(設定ファイルは自動生成されない)は**正確なので維持**する(`src/config.rs:520-558` に書き込み経路なし)
- `.anvil/` パス表記は現状の実パスのまま書く(リネームは Issue #16/#17 のスコープ。先走らない)
- 製品名表記は Issue #15(ユーザー可視ブランディング)の完了状態に合わせる。**#15完了後の着手を推奨**(未完了なら本Issueで名前だけ先行変更しない)

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- README.md
- README.ja.md
- .github/workflows/ci.yml
- src/tui/ux_demo.rs
- scripts/setup.sh
- src/cli.rs
- src/providers/gemini.rs
- src/providers/openai.rs
- docs/assets
- docs/guide
- docs/dev
- src/tui/slash.rs
- src/config.rs
- github/workflows/ci.yml
- src/minimal_loop/evidence.rs

## References

- なし

## Required Predecessors

- None

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui
- Worktree: ../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
