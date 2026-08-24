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
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #21

- Branch: `feature/issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #22

- Branch: `feature/issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #23

- Branch: `feature/issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #24

- Branch: `feature/issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #25

- Branch: `feature/issue-25-setup-add-commandagent-doctor-built-in-environme`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #26

- Branch: `feature/issue-26-setup-release-distribution-tagged-binary-release`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-26-setup-release-distribution-tagged-binary-release`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #27

- Branch: `feature/issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-27-setup-shell-completions-clap-complete-and-man-pa`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 6 failed worker verification (#26: worktree contains uncommitted changes after worker completion)

## Issue #28

- Branch: `feature/issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 6 failed worker verification (#26: worktree contains uncommitted changes after worker completion)

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i Codex issue worker task for Issue #21

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

- Title: [docs] Reorganize docs/ into guide vs dev with an index; translate data-profile-contract; add benchmarks README
- Objective: `docs/` 配下12ファイルはほぼ全て開発内部向け・歴史記録(mechanism-ledger、dev-guardrails、generality宣言、perfノート、UAT、移行記録)だが、ユーザー向け文書と区別するインデックスがなく、初見者が `docs/` を開くと内部台帳に迷い込む。また `docs/data-profile-contract.md` は日本語のみで英語話者が読めず、`benchmarks/` はREADMEのない孤立ディレクトリになっている。ディレクトリを再編し、案内を整備する。

## Acceptance Criteria

- `docs/dev/` を新設し、内部向け・歴史記録を移動する: `dev-guardrails.md` `mechanism-ledger.md` `generality.md` `perf-notes.md` `integration-notes.md` `uat-corpus.md` `uat/` `migration/` `profile-manifest.md` `data-profile-contract.md`
- 移動は `git mv` で行い、**内容は一切書き換えない**(歴史記録の改変禁止。とくに `mechanism-ledger.md` と `migration/`)
- `model-probe.md` はユーザー寄りとして `docs/guide/` 側(ユーザーガイドIssueのディレクトリ)へ移すか、ガイドからリンクする(ガイドIssueと調整。同時実装可)
- 移動に伴うパス参照の追従: `rg -n 'docs/' src tests .github *.md docs` で全参照を洗い出して更新する(確認済みの参照: `src/minimal_loop/repair_pressure.rs:231` のコメント。ほかにMarkdown間の相互リンクが `README.md` `SECURITY.md` などにある)
- `docs/README.md` を新設: 全ドキュメントの一覧表(ファイル / 1行説明 / **言語(EN・JA・混在)** / **対象読者(エンドユーザー・コントリビュータ・歴史記録)**)。「歴史記録は現状のコードと一致しないことがある」旨の注意書きを含める
- ルート `README.md` から `docs/README.md` へのリンクを追加(README改訂Issueと調整)
- `docs/dev/data-profile-contract.md`(日本語のみ・凍結v0契約)の**英訳を別ファイル**(例: `data-profile-contract.en.md`)として追加し、相互リンクする。原文は正本として無改変(「翻訳は参考、正本は日本語」の注記を英訳側に入れる)
- `mechanism-ledger.md` / `integration-notes.md` の混在は**翻訳しない**(内部台帳としてJA混在を許容する旨をインデックスに明記)
- `benchmarks/README.md` を新設: `minimal-loop-expanded.yaml` が何のフィクスチャで、`scripts/bench.sh` からどう使われるか(`bench.sh` の引数 `--model` `--runs` `--max-iterations` `--recheck-root` を含む)を簡潔に記載

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- docs/data-profile-contract.md
- dev-guardrails.md
- mechanism-ledger.md
- generality.md
- perf-notes.md
- integration-notes.md
- uat-corpus.md
- uat/scenarios.md
- profile-manifest.md
- data-profile-contract.md
- model-probe.md
- docs/README.md
- minimal-loop-expanded.yaml
- eval/README.md
- docs/integration-notes.md
- README.md
- SECURITY.md
- docs/dev/data-profile-contract.md
- data-profile-contract.en.md
- benchmarks/README.md
- scripts/bench.sh
- bench.sh
- tests/generality_guardrails.rs
- src/minimal_loop/repair_pressure.rs
- docs/dev
- docs/guide
- src/minimal_loop/loop_run.rs

## References

- なし

## Required Predecessors

- Issue #19: branch `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`, worktree `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Issue #20: branch `feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`, worktree `../CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i
- Worktree: ../CommandAgent-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-25-setup-add-commandagent-doctor-built-in-environme Codex issue worker task for Issue #25

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

- Title: [setup] Add commandagent --doctor: built-in environment diagnosis
- Objective: 環境診断の部品(preflight・プローブ可用性・model-probe・`/status` readinessカード)は既に揃っているが、束ねる入口がない。**`commandagent --doctor`(および `/doctor`)** として環境診断コマンドをバイナリに内蔵し、「動かない」時の一次切り分けを1コマンドにする。`scripts/setup.sh`(別Issue)より堅牢でクロスプラットフォームな中期解。

## Acceptance Criteria

- CLIフラグ `--doctor`(`src/cli.rs` に追加、既存Action系フラグの流儀に従う)とREPLの `/doctor`(`render_help` にも追加)の両方から実行できる
- **設定解決**: 有効な model / provider / planner_model / planner_provider / profile と、その出所(CLI / preset / config / default。既存の `field_sources` を利用)
- **設定ファイル**: 探索した各パス(`.anvil/config.toml` 等、`src/config.rs:663-668`)の存在・パース可否。presetが指定されているのに**不完全で解決不能**な場合はどのキーが欠けているかを列挙(`preset_complete`, `src/config.rs:560-571` を流用)
- **プロバイダ疎通**:
- Ollama(providerまたはplanner_providerがollamaのとき): 設定ホストへ `/api/tags` を短タイムアウトで照会し、到達性と**設定中のモデルがtagsに存在するか**を確認
- Gemini / OpenAI: **キーの存在のみ**確認(env → `.env` の解決順で。値は `redact` でマスクし絶対に表示しない)。実APIへのリクエストは行わない(課金・レート考慮)
- **インタラクションプローブ**: `playwright_availability` の結果と、unavailable時は既存remediation文言
- **状態ディレクトリ**: state_dir の書き込み可否(実際に一時ファイルを作って消す)
- **端末**: TTYか、色が有効か(`NO_COLOR`)、端末幅、フッター無効化条件に該当していないか
- **ワークスペース**: workspace_root の書き込み可否、`.env` の有無(有ればどのキーが定義済みかを**キー名のみ**表示)
- 人間可読の整列されたチェックリスト(✓/!/✗)。1項目1行+失敗時のみ対処行
- いずれかが fail なら終了コード非0、warnのみなら0
- `--doctor --json` で機械可読JSON(キー名は新規設計でよいが、一度出したら安定させる前提で命名する)
- 破壊的操作・自動修復はしない(診断のみ。修復はsetup.shや既存remediationへ誘導)
- 実行時間は正常系で数秒以内(ネットワーク照会は全て短タイムアウト)

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- scripts/setup.sh
- docs/model-probe.md
- src/cli.rs
- .anvil/config.toml
- src/doctor.rs
- src/lib.rs
- docs/dev-guardrails.md
- runner.rs
- loop_run.rs
- docs/mechanism-ledger.md
- setup.sh
- src/preflight.rs
- src/minimal_loop/interaction_probe.rs
- src/tui/banner.rs
- src/tui/slash.rs
- src/config.rs
- anvil/config.toml
- README.md

## References

- なし

## Required Predecessors

- Issue #19: branch `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`, worktree `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Issue #20: branch `feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`, worktree `../CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-25-setup-add-commandagent-doctor-built-in-environme
- Worktree: ../CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl Codex issue worker task for Issue #22

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

- Title: [docs] Add doc-drift guard tests: keep CLI flags, slash commands, config keys and EN/JA structure in sync
- Objective: READMEのフラグ記載が37個中約13個しかない等、ドキュメントとコードの乖離(doc drift)が既に起きている。今後リファレンス(docs/guide/)を整備しても、同期を保つ仕組みがなければ再び陳腐化する。**ドキュメントとコードの同期を機械検証するテスト(doc-drift guard)**を追加する。

## Acceptance Criteria

- clapの `Command` イントロスペクション(`Cli::command().get_arguments()`)で全フラグ名を列挙し、`docs/guide/en/cli-reference.md` に**全フラグが出現する**ことを検証する(方向は「コードにあるものがドキュメントに漏れなく載っている」。ドキュメント側の追加説明は自由)
- 逆方向も検証: ドキュメントの表に載っているフラグ名が実在しないならfail(タイポ・削除済みフラグの検知)。表のパースは「行頭 `| \`--flag\`` 形式」など**単純な規約**を決めてガイド側もそれに従う
- `render_help` の出力からコマンド名(`/xxx`)を抽出し、`docs/guide/en/slash-commands.md` に全て出現することを検証(逆方向も同様)
- 可能なら `render_help` とディスパッチ(`handle_command`)の一致もこの機会にテスト化する(helpに載っているのに処理がない/その逆の検知)
- presetキー10個とトップレベルキーの一覧をテスト内の定数ではなく**コードから導出**できない場合は、`src/config.rs` 側に「サポートするキーの一覧」を返す関数(またはconst)を追加してそれを正とし、ドキュメント出現を検証する
- `docs/guide/en/` と `docs/guide/ja/` の**ファイル集合が一致**することを検証
- 各対訳ペアで**h2/h3見出しの数が一致**することを検証(内容の翻訳品質は対象外。構造の欠落だけ検知)
- 失敗時のメッセージは「どのフラグ/コマンド/キーがどちら側に欠けているか」を列挙し、修正先ファイルパスを示す
- CI(`.github/workflows/ci.yml` の `cargo test --all-targets`)で自動実行される(統合テストとして置けば追加設定不要のはず。要確認)

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/cli.rs
- tests/doc_drift.rs
- docs/guide/en/cli-reference.md
- docs/guide/en/slash-commands.md
- src/config.rs
- .github/workflows/ci.yml
- docs/guide/README.md
- docs/guide
- src/tui/slash.rs
- docs/guide/en
- docs/guide/ja
- github/workflows/ci.yml
- docs/dev-guardrails.md

## References

- なし

## Required Predecessors

- Issue #19: branch `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`, worktree `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Issue #20: branch `feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`, worktree `../CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Issue #21: branch `feature/issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`, worktree `../CommandAgent-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Issue #23: branch `feature/issue-23-repo-add-license-mit-contributing-md-changelog-m`, worktree `../CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Issue #25: branch `feature/issue-25-setup-add-commandagent-doctor-built-in-environme`, worktree `../CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl
- Worktree: ../CommandAgent-issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-24-setup-add-scripts-setup-sh-prerequisites-check-b Codex issue worker task for Issue #24

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

- Title: [setup] Add scripts/setup.sh: prerequisites check, build/install, .env, probe setup, smoke test
- Objective: 現在の導入手順は `cargo build` + 手動symlink(README記載)のみで、前提条件(Rust 1.88+ / Ollama / APIキー / Node+Playwright / Python)が散在しており、初回セットアップの失敗ポイントが多い。**冪等なセットアップスクリプト `scripts/setup.sh`** を新設し、「クローン→1コマンド→動く」を実現する。

## Acceptance Criteria

- 引数なし = 対話モード(各ステップで確認)。`--yes` = 非対話(安全な既定で全実行)。`--check-only` = 前提チェックのみ実行して結果表示(何も変更しない)
- **冪等**: 2回目以降の実行で壊れない・重複作業しない(導入済みならスキップと表示)
- **前提チェック**: `cargo`/`rustc`(バージョンが `Cargo.toml` の `rust-version` 以上か。値はCargo.tomlからgrepで取得しハードコードしない)、`git`。任意依存として `node`/`npm`(プローブ用)、`ollama` または既定ホストへの疎通(`curl http://localhost:11434/api/tags`)、`python3`(eval用)。必須欠落は導入方法のURL付きで案内して終了、任意欠落は警告して続行
- **ビルド/インストール**: `cargo install --path . --locked` を提案・実行(拒否時は `cargo build --release` +PATH追加案内にフォールバック)。完了後 `commandagent --version` で確認
- **APIキー設定(任意)**: Gemini/OpenAIを使うか尋ね、使うなら `.env` に `GEMINI_API_KEY=` / `OPENAI_API_KEY=` を書き込む。**入力はエコーしない**(`read -s`)。**既存の `.env` は上書きせず**、欠けているキーの追記のみ。作成時はパーミッション600。`--yes` モードではキー入力はスキップし追記もしない(案内のみ)
- **Ollamaモデル(任意)**: `ollama` 検出時、`ollama list` を表示し、モデルが無ければ pull を提案(モデル名はユーザー入力。既定値を押し付けない)
- **インタラクションプローブ(任意)**: `node`/`npm` 検出時、`commandagent --setup-interaction-probe` の実行を提案・実行
- **スモーク確認**: `commandagent --version` の出力を表示し、Ollama疎通が取れていれば `--model-probe` の実行を提案(時間がかかる旨を表示)
- **サマリー**: 各ステップの結果(ok / skipped / warn)を最後に一覧表示し、次の一歩(READMEのQuickstartへのリンク)を出す
- `set -euo pipefail`。bash 3.2互換(macOS標準)で動くこと(配列の使い方等に注意)
- macOS / Linux 両対応(OS分岐は最小限に)
- 失敗時は必ず「何が失敗し、手動でどうすればよいか」を1行で出す
- シークレット(キーの値)を画面・ログに一切出さない
- `shellcheck scripts/setup.sh` が警告ゼロ。CI(`.github/workflows/ci.yml`)にshellcheckステップを追加(`scripts/*.sh` 対象)
- README(改訂Issueと調整)のInstall節から `./scripts/setup.sh` を案内する1行を追加

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- scripts/setup.sh
- bench.sh
- eval-run.py
- Cargo.toml
- .github/workflows/ci.yml
- scripts/*.sh
- ./scripts/setup.sh
- install.sh
- src/cli.rs
- src/config.rs
- src/minimal_loop/interaction_probe.rs
- github/workflows/ci.yml
- README.md

## References

- なし

## Required Predecessors

- Issue #19: branch `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`, worktree `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Issue #20: branch `feature/issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`, worktree `../CommandAgent-issue-20-docs-add-bilingual-user-guide-docs-guide-en-ja-c`
- Issue #21: branch `feature/issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`, worktree `../CommandAgent-issue-21-docs-reorganize-docs-into-guide-vs-dev-with-an-i`
- Issue #22: branch `feature/issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`, worktree `../CommandAgent-issue-22-docs-add-doc-drift-guard-tests-keep-cli-flags-sl`
- Issue #23: branch `feature/issue-23-repo-add-license-mit-contributing-md-changelog-m`, worktree `../CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Issue #25: branch `feature/issue-25-setup-add-commandagent-doctor-built-in-environme`, worktree `../CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-24-setup-add-scripts-setup-sh-prerequisites-check-b
- Worktree: ../CommandAgent-issue-24-setup-add-scripts-setup-sh-prerequisites-check-b
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-26-setup-release-distribution-tagged-binary-release Codex issue worker task for Issue #26

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

- Title: [setup] Release & distribution: tagged binary releases, install.sh, crates.io/Homebrew (decisions included)
- Objective: 配布手段が「ソースからの `cargo build`」しかない。リリースワークフロー無し(CIはテストのみ)、リリースバイナリ無し、crates.io未公開、Homebrew無し。`cargo install` すら README に書かれていない。バイナリ配布を整備し、Rustツールチェーン無しでも導入できるようにする。

## Acceptance Criteria

- `.github/workflows/release.yml` を新設: `v*` タグのpushで発火し、以下のターゲットのリリースバイナリをビルドして GitHub Release に添付する
- `aarch64-apple-darwin` / `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`(可能なら `-musl` も。glibc依存を減らせる)
- 各アーティファクトは `commandagent-<version>-<target>.tar.gz` 形式+ `sha256` チェックサムファイルを添付
- ビルドは `--release --locked`。`cargo test` をリリースビルド前に実行(壊れたタグを出さない)
- リリースノートはタグメッセージ+自動生成(GitHubの release notes 自動生成で可。CHANGELOG連携はCHANGELOG Issue完了後に検討)
- 実装手段は素のGitHub Actionsで可。`cargo-dist` を採用する場合は生成物をコミットし、選定理由をPRに記載
- `scripts/install.sh` を新設: 最新(または指定バージョン)のGitHub Releaseから、実行環境のOS/アーキテクチャに合うバイナリを取得し、チェックサム検証のうえ `~/.local/bin`(または `--prefix` 指定先)へ配置する
- `curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | sh` で動く想定の作りにする(ただしREADMEにはパイプ実行のリスク注記と「ダウンロードして確認してから実行」の選択肢も書く)
- `shellcheck` 警告ゼロ。PATH未設定時は案内を出す
- ソースビルド版 `scripts/setup.sh`(別Issue)との違い(バイナリ取得 vs ソースビルド+環境セットアップ)をREADMEで明確にする
- 検討事項をコメントで提示して判断を仰ぐ: パッケージ名 `commandagent` の空き状況、公開に必要なメタデータ追加(`repository`/`readme`/`keywords`)、`workspace/` や `tests/corpus/` など巨大ディレクトリの `exclude` 設定、公開後のyank不可性
- 承認された場合のみ: メタデータ整備 + `cargo publish --dry-run` 通過 + 公開手順のドキュメント化
- リリースバイナリ運用が安定した後の選択肢として、`Kewton/homebrew-tap` リポジトリ+formulaの構成案を提示して判断を仰ぐ(本体リポジトリ側の変更はほぼ不要)
- Apply approved decision: Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.

## Approved Decision

Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- ci.yml
- Cargo.toml
- build.rs
- .github/workflows/release.yml
- scripts/install.sh
- scripts/setup.sh
- install.sh
- tests/corpus
- tests/tui_pty.rs
- github/workflows/release.yml
- README.md

## References

- なし

## Required Predecessors

- Issue #19: branch `feature/issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`, worktree `../CommandAgent-issue-19-docs-overhaul-readme-en-and-add-readme-ja-md-qui`
- Issue #23: branch `feature/issue-23-repo-add-license-mit-contributing-md-changelog-m`, worktree `../CommandAgent-issue-23-repo-add-license-mit-contributing-md-changelog-m`
- Issue #24: branch `feature/issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`, worktree `../CommandAgent-issue-24-setup-add-scripts-setup-sh-prerequisites-check-b`
- Issue #25: branch `feature/issue-25-setup-add-commandagent-doctor-built-in-environme`, worktree `../CommandAgent-issue-25-setup-add-commandagent-doctor-built-in-environme`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-26-setup-release-distribution-tagged-binary-release
- Worktree: ../CommandAgent-issue-26-setup-release-distribution-tagged-binary-release
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
