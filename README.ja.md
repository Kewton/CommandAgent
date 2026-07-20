<!-- 対訳ファイル: README.md と README.ja.md は必ず同時に更新してください。 -->

[English](README.md) | [日本語](README.ja.md)

# CommandAgent

[![CI](https://github.com/Kewton/CommandAgent/actions/workflows/ci.yml/badge.svg)](https://github.com/Kewton/CommandAgent/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/license/mit)

**最小ループまたは構造化されたプランで、ゴールを検証済みのコードへ変える
ローカルファーストなコーディングエージェントです。**

CommandAgent は信頼できるローカルワークスペース内で動作し、モデル
プロバイダーとして Ollama、Gemini、OpenAI を利用しながら、実装を検証に
結び付けます。1つのプロンプトから始めることも、再利用できる YAML プランを
生成することも、UltraPlan で大きなゴールをフェーズに分割して途中の失敗を
修復することもできます。

## デモ

<p align="center">
  <img src="docs/assets/ux-demo.svg" alt="CommandAgent のターミナルデモアニメーション" width="900">
</p>

このデモは完全にオフラインです。`commandagent --ux-demo` で実行できます。
再現方法は[録画メモ](docs/assets/ux-demo.md)を参照してください。

## 機能

- **Minimal loop** — 直接的な反復コーディングループで、調査、編集、ツール実行、
  検証を進めます。
- **Step plan** — `--plan-steps`、`--plan-run`、`--run-plan` で YAML の
  ステッププランを作成または実行します。
- **UltraPlan run** — `--ultra-plan`、`--ultra-plan-run`、
  `--run-ultra-plan` で大きなゴールをフェーズに分割します。
- **タスクプロファイル** — `generic`、`nextjs`、`python-cli`、`data` の
  ガイダンスと検証契約を利用できます。
- **マルチプロバイダー** — Ollama でローカル実行するか、Gemini と OpenAI に
  接続できます。
- **検証と修復** — 申告された結果を確認してエビデンスを収集し、失敗を有界な
  修復ループへ戻します。
- **対話型 TUI** — 固定ステータスフッター、アクティビティスピナー、ストリーミング
  出力、入力キュー、Esc/Ctrl-C による割り込みを利用できます。

## Quickstart

これはローカルだけで動かす最短経路です。リモートのモデルプロバイダーには
何も送信しません。

1. [Ollama をインストール](https://ollama.com/download)し、起動していることを
   確認します。
2. 手元のマシンに合うモデルを pull します。

   ```bash
   ollama pull "<your-model>"
   ```

3. CommandAgent のソースディレクトリでバイナリをインストールします。

   ```bash
   cargo install --path .
   ```

4. 信頼できるプロジェクトへ移動し、1つのプロンプトを実行します。

   ```bash
   cd /path/to/your/project
   commandagent --provider ollama --model "<your-model>" \
     --prompt "Inspect this project and suggest one useful improvement."
   ```

`<your-model>` はプレースホルダーであり、実際のモデル ID ではありません。
手元の `ollama list` に実在するモデルへ必ず置き換えてください。

## インストール

### 前提条件

| 要件 | 用途 |
| --- | --- |
| Rust 1.88 以降 | CommandAgent のビルドとインストール |
| Ollama（任意） | ローカルモデルの実行 |
| `GEMINI_API_KEY` または `OPENAI_API_KEY`（任意） | Gemini または OpenAI の実行 |
| Node.js と npm（任意） | インタラクションプローブの導入と実行 |
| Python 3（任意） | 評価ツールと Python 向けチェック |

### ソースからの導入

```bash
git clone https://github.com/Kewton/CommandAgent.git
cd CommandAgent
cargo install --path .
commandagent --help
```

### リリース済みバイナリ

Rust ツールチェーンなしで macOS または Linux x86_64 に導入できます。

```bash
curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | sh
# より安全な方法: ダウンロードして確認後 `sh install.sh` を実行
```

SHA-256 検証後 `~/.local/bin` に配置します（`--version`、`--prefix` で指定可能）。
リモートスクリプトのパイプ実行にはリスクがあります。`scripts/setup.sh` はバイナリ
取得ではなく、ソースビルドと開発環境の準備を行います。crates.io メタデータを整備し
`cargo publish --dry-run` を確認しますが、公開は行いません。公開前にパッケージ名、
同梱ファイル、yank 不可の方針を確認してください。将来 `Kewton/homebrew-tap` に
formula を追加する案がありますが、外部リポジトリは作成しません。

前提条件の確認、インストール、任意のプロバイダー／プローブ設定を対話形式で
進めるには、`./scripts/setup.sh` を実行します。非対話の安全な既定値には
`--yes`、何も変更せず前提条件だけ確認するには `--check-only` を使います。

リモートプロバイダーを使う場合は、対応するキーをプロセス環境またはアクティブな
ワークスペース直下の `.env` に設定します。CommandAgent はログ内の値を
秘匿します。

## 使い方

ローカルモデルで対話型 REPL を開始します。

```bash
commandagent --provider ollama --model "<your-model>"
```

REPL に入らず、minimal loop を1回実行します。

```bash
commandagent --provider ollama --model "<your-model>" \
  --prompt "Add a focused test for the parser edge case."
```

ステッププランを生成して実行します。

```bash
commandagent --provider ollama --model "<your-model>" \
  --plan-run --profile python-cli "Build a small JSON formatting CLI."
```

フェーズ分割された UltraPlan を生成して実行します。

```bash
commandagent --provider ollama --model "<your-model>" \
  --ultra-plan-run --profile nextjs "Build a small task board."
```

API キーを設定した後はリモートプロバイダーも利用できます。

```bash
export GEMINI_API_KEY="<your-api-key>"
commandagent --provider gemini --model "<gemini-model>" \
  --prompt "Review the current diff."

export OPENAI_API_KEY="<your-api-key>"
commandagent --provider openai --model "<openai-model>" \
  --prompt "Review the current diff."
```

REPL では、まず次のスラッシュコマンドを利用できます。

| コマンド | 用途 |
| --- | --- |
| `/help` | 利用できる全スラッシュコマンドを表示 |
| `/status` | 有効な設定と準備状況を表示 |
| `/plan-run <goal>` | ステッププランを生成して実行 |
| `/ultra-plan-run <goal>` | UltraPlan を生成して実行 |
| `/runs` | 最近の実行と復旧可否を一覧表示 |
| `/resume [run-id\|yaml-path]` | 復旧用 UltraPlan から再開 |
| `/exit` または `/quit` | TUI を終了 |

CLI と REPL の全リファレンスは[ユーザーガイド](docs/guide/README.md)を参照して
ください。コントリビュータ向け契約、検証手順、歴史記録は
[ドキュメント一覧](docs/README.md)から参照できます。実行ファイルが常に正本です。
インストール済みバージョンの内容は
`commandagent --help` と `/help` で確認できます。

## 設定

名前付き preset は、次のいずれかの正規ファイルに保存できます。

- アクティブなワークスペースの `.commandagent/config.toml`
- ユーザーのホームディレクトリの `~/.commandagent/config.toml`

対応する `.anvil/config.toml` は旧形式のフォールバックとして引き続き利用できます。
CommandAgent はこれらのファイルを読み込みますが、ファイルや preset を**自動生成
しません**。実行、プラン、修復のライブアーティファクトは、引き続き既存の
`.anvil/` パスを使います。

preset は `commandagent --preset <name>` で選択します。対応フィールドと優先順位は
[設定ガイド](docs/guide/README.md#configuration)を参照してください。

## 開発とセキュリティ

リポジトリメンテナー向けの UAT、リリースビルド、symlink、ライブプロバイダー、
copy-validation の手順は
[docs/dev/repository-validation.md](docs/dev/repository-validation.md)に移しました。
Codex harness の詳細は [docs/codex-harness.md](docs/codex-harness.md)にあります。

CommandAgent は信頼できるワークスペースとゴールでの利用を想定しています。
`--yes` を使う場合や、未確認のプロジェクトコードを実行する場合は、事前に
[SECURITY.md](SECURITY.md)を読んでください。

## ライセンス

CommandAgent は [MIT License](LICENSE) でライセンスされています。
