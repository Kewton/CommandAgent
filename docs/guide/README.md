# CommandAgent User Guide / ユーザーガイド

[English](#english) | [日本語](#日本語)

This is the entry point for the bilingual end-user guide. English and Japanese
pages have matching structures and cover the same behavior.

英語版と日本語版から成るエンドユーザーガイドの入口です。両言語のページは同じ構成で、
同じ動作を説明します。

Recommended route / 推奨順路:

- English: [Getting started](../user/getting-started-cli.md) →
  [Detailed tutorial](en/tutorial.md) → [CLI reference](en/cli-reference.md)
- 日本語: [CLI 入門](../user/getting-started-cli.md) →
  [詳細チュートリアル](ja/tutorial.md) → [CLI リファレンス](ja/cli-reference.md)

## English

- [Tutorial](en/tutorial.md) — a 20-minute walkthrough with real screens:
  doctor, the first REPL request through Gate 1–4, and one GUI Trial
- [CLI getting started](../user/getting-started-cli.md) — install, provider,
  configuration, doctor, first loop, and exact-pack A/B
- [GUI getting started](../user/getting-started-gui.md) — readiness, sample
  Trial, Gate 1, and separate status, history, and result pages
- [Extensions](en/extensions.md) — four layers, boundaries, registration, and
  the [supply lifecycle](../user/gui-extensions.md)
- [CLI reference](en/cli-reference.md) — all 65 public flags, defaults, and
  conflicts
- [Plan YAML editing](en/plan-yaml.md) — commented templates, offline
  validation, next commands, and recovery diffs
- [Slash commands](en/slash-commands.md) — all 24 accepted command names,
  inline flags, file expansion, and profile inference
- [Configuration](en/configuration.md) — precedence, presets, paths, legacy
  files, and environment variables
- [Providers](en/providers.md) — Ollama, LM Studio, OpenAI, and Gemini setup
- [Troubleshooting](en/troubleshooting.md) — common startup, provider, and TUI
  problems
- [Model behavior probe](en/model-probe.md) — the bounded role-specific
  provider/model measurement workflow

Start with the [project README](../../README.md) for installation and a short
walkthrough. Read the [security model](../../SECURITY.md) before enabling
`--yes` or working in a new workspace.

## 日本語

- [チュートリアル](ja/tutorial.md) — 実際の画面で追う 20 分のウォークスルー:
  doctor、最初の REPL 依頼から Gate 1〜4、GUI Trial 1 本
- [CLI 入門](../user/getting-started-cli.md) — 導入、provider、設定、doctor、
  最初の 1 周、exact pack A/B
- [GUI 入門](../user/getting-started-gui.md) — 前提確認、サンプル Trial、Gate 1、
  分離された実行状況・履歴・結果ページ
- [拡張](ja/extensions.md) — 4 レイヤー、境界、登録導線、
  [供給ライフサイクル](../user/gui-extensions.md)
- [CLI リファレンス](ja/cli-reference.md) — 公開されている全 65 フラグ、既定値、排他関係
- [Plan YAML の編集](ja/plan-yaml.md) — コメント付き template、offline 検証、次コマンド、
  recovery 差分
- [スラッシュコマンド](ja/slash-commands.md) — 受け付ける全 24 コマンド名、インラインフラグ、
  ファイル展開、プロファイル推論
- [設定](ja/configuration.md) — 優先順位、preset、探索パス、旧形式ファイル、環境変数
- [プロバイダ](ja/providers.md) — Ollama、LM Studio、OpenAI、Gemini のセットアップ
- [トラブルシューティング](ja/troubleshooting.md) — 起動時、プロバイダ、TUI の一般的な問題
- [モデル動作プローブ](ja/model-probe.md) — 役割別のプロバイダ／モデルを限定的に測定する手順

インストールと短いチュートリアルは[プロジェクト README](../../README.ja.md)から始めてください。
`--yes` を有効にする前や新しいワークスペースで使う前に、
[セキュリティモデル](../../SECURITY.md)を確認してください。
