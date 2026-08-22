# CLI リファレンス

[English](../en/cli-reference.md) | [ガイド目次](../README.md)

`commandagent [OPTIONS] [GOAL]...` は、アクションを選ばなければ対話型 TUI を起動します。
末尾のゴールは最後の引数列として集約されるため、複数の単語を引用符なしでも指定できます。
インストール済みバイナリのバージョンがこのチェックアウトと異なる場合は、
`commandagent --help` の表示を正としてください。

## 呼び出し方

直接コマンドを実行するにはアクション選択フラグを 1 つ使い、TUI を使う場合はすべて省略します。
アクション選択フラグは `--prompt`、`--plan-steps`、`--plan-run`、`--run-plan`、
`--ultra-plan`、`--ultra-plan-run`、`--run-ultra-plan`、
`--validate-plan`、`--setup-interaction-probe`、`--runs`、`--ux-demo`、`--model-probe`、`--doctor` です。
オフライン pack アクションの `--packs`、`--pack-verify`、`--pack-pin`、生成アクションの
`--completions` と `--generate-man`、設定アクションの `--init-config`、委譲された manifest
アクションの `--validate-manifest` と `--init-profile` も、help の
`Actions (use one)` グループに表示します。相互排他の action contract を組み合わせると拒否されます。

Clap が生成する `-h`/`--help` と `-V`/`--version` は、以下のアプリケーション固有の
59 フラグには含めません。非表示の `--completion-contract-json <PATH>` は内部連携用であり、
公開ユーザーフラグではありません。

## フラグ一覧

| フラグ | 引数 | 省略時の既定値 | 説明 | 関連項目 |
| --- | --- | --- | --- | --- |
| `--yes` | なし | オフ | 全ツールを許可し、再開確認を省略します。認識された Bash 書き込みは引き続き workspace 内に制限されます。使用中ポートの所有プロセスを自動終了することはありません。信頼できるワークスペースでのみ使ってください。 | [使用中ポート](troubleshooting.md#preflight-port-n-is-busy) |
| `--allow` | `<read\|write\|bash:verify>` | 従来の read 許可と変更ごとの承認 | 選択したツールクラスだけを許可します。read、write、bash:verify は反復指定またはカンマ区切りにできます。選択した変更は自動承認され、省略したクラスは拒否されます。 | [セキュリティモデル](../../../SECURITY.md) |
| `--preset` | `<PRESET>` | なし | 設定ファイルから組み立てた名前付き `[preset.<name>]` を選びます。 | [Preset](configuration.md#preset) |
| `--pack` | `<ID@VERSION>` | preset の `pack`、その後なし | exact version の pack を有効化します。preset と矛盾する pack は run 前に拒否します。 | [Pack 選択](configuration.md#pack-選択) |
| `--pack-hash` | `<SHA256>` | 検証済み `pack.sha256` | 選択 pack の exact-byte hash を固定します。`--pack` が必要です。 | [Pack 選択](configuration.md#pack-選択) |
| `--extension-root` | `<DIR>` | トップレベル `extension_root`、その後なし | ローカル pack と `profiles/<id>/manifest.toml` の draft profile を読み込みます。外部 profile は draft に強制され exact-byte hash で固定されます。 | [Pack 選択](configuration.md#pack-選択) |
| `--packs` | なし | オフ | compatible な承認済み pack と `--extension-root` 配下の conformant な pack を供給元付きで一覧表示します。`--profile` と `--intent` が必要です。 | [排他関係](#排他関係と組み合わせ) |
| `--pack-verify` | `<DIR>` | なし | 1 個の pack directory を strict conformance 検査し、`pack_conformance` と同じ JSON report を表示します。 | [排他関係](#排他関係と組み合わせ) |
| `--pack-pin` | `<DIR>` | なし | green conformance 後に `pack.sha256` を作成し、同一 pin は変更せず、古い pin は拒否します。 | [排他関係](#排他関係と組み合わせ) |
| `--context-budget` | `<CONTEXT_BUDGET>` 整数 | `65536` | 会話を圧縮する概算コンテキスト予算を設定します。 | [重要な解決後の既定値](#重要な解決後の既定値) |
| `--model` | `<MODEL>` | `qwen3.6:27b-coding-nvfp4` | 実行モデル ID を設定します。 | [プロバイダ](providers.md) |
| `--provider` | `<PROVIDER>`: `ollama`、`lm-studio`、`openai`、`gemini` | `ollama` | 実行プロバイダを選びます。 | [プロバイダ](providers.md) |
| `--api` | `<chat-completions\|responses>` | `chat-completions` | OpenAI互換API面を明示選択します。モデル名から暗黙選択しません。 | [preset](configuration.md#preset) |
| `--tool-protocol` | `<native\|text>` | プロバイダ能力の既定値 | native function tools または既存text/XML tool protocolを明示選択します。 | [preset](configuration.md#preset) |
| `--prompt-layout` | `<stable\|legacy>` | `legacy` | A/B 測定用のプロンプトセクション順序を選びます。 | [解決の優先順位](configuration.md#解決の優先順位) |
| `--plan-preset` | `<profile\|none>` | 通常は `none`。明示的な `data` の fix/investigate では `profile` | planner 層の UltraPlan preset 選択を上書きします。`data/fix` は F1–F3 ステップを合成でき、`nextjs/fix` は none 相当のままです。 | [解決の優先順位](configuration.md#解決の優先順位) |
| `--intent` | `<create\|fix\|investigate>` | ゴールから推論 | ゴールに基づく解決ではなく intent を固定します。 | [例](#例) |
| `--workflow` | `<PATH>` | なし | 宣言的な workflow-circle 定義を実行します。`--intent` とは排他です。 | [例](#例) |
| `--origin` | `<PATH>` | なし | `--workflow` に、既存の失敗した origin run のワークスペースを渡します。 | [例](#例) |
| `--planner-model` | `<PLANNER_MODEL>` | プロバイダが同じなら実行モデル | planner モデル ID を設定します。planner と実行のプロバイダが異なる場合は必須です。 | [プロバイダの役割](providers.md#プロバイダ対応表) |
| `--planner-provider` | `<PLANNER_PROVIDER>`: `ollama`、`lm-studio`、`openai`、`gemini` | 実行プロバイダ | planner プロバイダを選びます。 | [プロバイダの役割](providers.md#プロバイダ対応表) |
| `--prompt` | `<PROMPT>` | なし | TUI に入らず、minimal loop のプロンプトを 1 件実行します。 | [例](#例) |
| `--plan-steps` | なし | オフ | 末尾のゴールに対する step plan を生成して保存します。 | [排他関係と組み合わせ](#排他関係と組み合わせ) |
| `--plan-run` | なし | オフ | 末尾のゴールから step plan を生成して実行します。 | [排他関係と組み合わせ](#排他関係と組み合わせ) |
| `--run-plan` | `<RUN_PLAN>` パス | なし | 既存の step plan YAML ファイルを実行します。 | [排他関係と組み合わせ](#排他関係と組み合わせ) |
| `--ultra-plan` | なし | オフ | 末尾のゴールに対する UltraPlan を生成して保存します。 | [排他関係と組み合わせ](#排他関係と組み合わせ) |
| `--ultra-plan-run` | なし | オフ | 末尾のゴールから UltraPlan を生成して実行します。 | [排他関係と組み合わせ](#排他関係と組み合わせ) |
| `--run-ultra-plan` | `<RUN_ULTRA_PLAN>` パス | なし | 既存の UltraPlan YAML ファイルを実行します。 | [排他関係と組み合わせ](#排他関係と組み合わせ) |
| `--validate-plan` | `<PATH>` | なし | step-plan または UltraPlan YAML を実行せずに検証し、エラーには行番号と列番号を表示します。 | [Plan YAML の編集](plan-yaml.md) |
| `--setup-interaction-probe` | なし | オフ | 管理対象の Playwright interaction probe をインストールまたは検証します。 | [Probe 利用不可](troubleshooting.md#preflight-interaction-probe-unavailable) |
| `--runs` | なし | オフ | プロバイダクライアントを作らず、現在のワークスペースの最近の run を一覧表示します。 | [スラッシュ `/runs`](slash-commands.md#コマンド一覧) |
| `--ux-demo` | なし | オフ | オフラインのプレゼンテーション UX デモを実行します。 | [排他関係と組み合わせ](#排他関係と組み合わせ) |
| `--model-probe` | なし | オフ | 限定的なモデル動作プローブ一式を実行します。 | [モデルプローブ](model-probe.md) |
| `--doctor` | なし | オフ | ネットワーク要求を行わず、設定ファイル、プロバイダ readiness、interaction probe、ローカル環境を診断します。 | [スラッシュ `/doctor`](slash-commands.md#コマンド一覧) |
| `--json` | なし | オフ | `--doctor` の出力を安定した機械可読 JSON として表示します。`--doctor` が必要です。 | [スラッシュ `/doctor`](slash-commands.md#コマンド一覧) |
| `--completions` | `<SHELL>`: `bash`、`elvish`、`fish`、`powershell`、`zsh` | なし | 現在の Clap 定義から補完スクリプトを生成し、stdout に出力します。 | [シェル補完と man ページ](#シェル補完と-man-ページ) |
| `--generate-man` | なし | オフ | 現在の Clap 定義から `commandagent(1)` man ページを生成し、stdout に出力します。 | [シェル補完と man ページ](#シェル補完と-man-ページ) |
| `--init-config` | なし | オフ | 既存ファイルを上書きせず、雛形から `.commandagent/config.toml` を作成します。 | [設定雛形](#設定雛形) |
| `--validate-manifest` | `<PATH>` | なし | 外部 profile manifest を実行せずに検証します。 | [Manifest v2](../../dev/profile-manifest.md) |
| `--init-profile` | `<ID>` | なし | `--extension-root` 配下に draft profile manifest を初期化します。 | [Manifest v2](../../dev/profile-manifest.md) |
| `--profile` | `<PROFILE>` | 推論後に `generic` | 組み込み profile または外部 draft ID を明示します。外部 ID には `profiles/<id>/manifest.toml` を宣言する extension root が必要です。 | [プロファイル推論](slash-commands.md#プロファイル推論) |
| `--style` | `<STYLE>` | `default` | plan の表示／生成スタイルを渡します。 | [インラインフラグ](slash-commands.md#インラインフラグ) |
| `--resume` | `<RESUME>` | なし | 直接 `--prompt` 実行で、指定した保存済み minimal-loop セッションを読み込みます。 | [セッションオプション](#排他関係と組み合わせ) |
| `--offline` | なし | オフ | runtime の依存関係セットアップと、npm/pnpm/yarn/cargo install、curl、wget を含む Bash コマンドを禁止します。provider/API 要求とその他のネットワーク可能なコマンドには影響しません。 | [プロバイダ](providers.md) |
| `--quiet` | なし | オフ（`narration = "normal"`） | プレゼンテーションのナレーションを抑制します。 | [トップレベルキー](configuration.md#トップレベルキー) |
| `--summary-json` | なし | オフ | stdout 最終行へ機械可読な終端runサマリを1件追加します。省略時は既存stdout bytesを維持します。 | [Headless execution](../../user/headless.md) |
| `--ollama-host` | `<OLLAMA_HOST>` URL | `http://localhost:11434` | CommandAgent が使う Ollama サーバーのベース URL を設定します。 | [Ollama のホスト](providers.md#ollama-のホストとモデル) |
| `--think` | `[=<true\|false\|low\|medium\|high>]` | 省略 | Ollama を使うすべての役割で thinking を有効化します。単独指定は `true`、明示値には `--think=high` のように `=` が必要です。 | [Ollama thinking](providers.md#ollama-thinking) |
| `--lm-studio-host` | `<LM_STUDIO_HOST>` URL | `http://localhost:1234` | LM StudioのベースURLを設定します。末尾の任意の`/v1`は正規化します。 | [LM Studioのサーバー](providers.md#lm-studio-のサーバーとモデル) |
| `--num-predict` | `<NUM_PREDICT>` 整数 | `8192` | プロバイダへ要求する最大出力トークン数を設定します。 | [重要な解決後の既定値](#重要な解決後の既定値) |
| `--max-iterations` | `<MAX_ITERATIONS>` 整数 | `12` | minimal loop の反復予算を設定します。 | [重要な解決後の既定値](#重要な解決後の既定値) |
| `--chat-timeout-secs` | `<CHAT_TIMEOUT_SECS>` 整数 | いずれかの役割が Ollama または LM Studio なら `600`、それ以外は `180` | プロバイダ呼び出しの接続およびリクエスト全体のタイムアウトを設定します。 | [重要な解決後の既定値](#重要な解決後の既定値) |
| `--chat-retries` | `<CHAT_RETRIES>` 整数 | `1` | 最初のプロバイダ試行後の再試行回数を設定します。 | [プロバイダ失敗](troubleshooting.md#model-id-が存在しない) |
| `--stream` | `<on\|off>` | TUI ではオン、直接アクションではオフ | executor と repair の表示ストリーミングを制御します。planner の機械形式出力は表示しません。ストリーミングには stdin と stdout の両方が対話型 TTY であることも必要です。 | [トップレベルキー](configuration.md#トップレベルキー) |
| `--state-dir` | `<STATE_DIR>` パス | `$XDG_STATE_HOME/anvilminimal`、なければ `~/.local/state/anvilminimal` | 保存セッションと REPL 履歴の格納先を上書きします。 | [パス](configuration.md#設定ファイルの探索パス) |
| `--cwd` | `<CWD>` パス | 現在のディレクトリ | 設定探索と実行の前に、アクティブなワークスペースを設定して正規化します。 | [パス](configuration.md#設定ファイルの探索パス) |
| `--fresh-session` | なし | オフ | 直接 `--prompt` 実行で `--resume` を無視し、新しいセッションを作ります。 | [セッションオプション](#排他関係と組み合わせ) |
| `--footer` | `<on\|off>` | `on` | 固定 TUI フッターを制御します。off でもスクロールバックの breadcrumb は残ります。 | [フッターの問題](troubleshooting.md#フッター描画の問題) |
| `--no-footer` | なし | オフ | 固定 TUI フッターを無効にします。効果は `--footer off` と同じです。 | [フッターの問題](troubleshooting.md#フッター描画の問題) |

## 既定値と優先順位

Clap の既定値として宣言された値だけが、設定解決より前に固定されます。model、provider、
context budget、timeout、profile、footer、stream などは `Config::from_cli` で実効既定値が
決まります。フィールドごとの正確な層は[設定](configuration.md)を参照してください。

### 重要な解決後の既定値

| 設定 | 実効既定値 |
| --- | --- |
| `num_predict` | `8192` |
| `max_iterations` | `12` |
| `chat_timeout_secs` | いずれかのプロバイダ役割が Ollama または LM Studio なら `600` 秒、両方がリモートなら `180` 秒 |
| `chat_retries` | 最初の試行後に `1` 回再試行 |
| `context_budget` | `65536` |

### 排他関係と組み合わせ

- `--footer` と `--no-footer` は Clap レベルで排他であり、同時に使えません。
- `--allow` は `read`、`write`、`bash:verify` を反復またはカンマ区切りで受け付けます。
  指定した場合、省略したツールクラスは拒否されます。`--yes` は後方互換の全ツール許可です。
- アクション選択フラグは 1 つだけ使用できます。構文解析後に検査され、違反すると
  `only one action selector can be used at a time` で失敗します。
- `--packs`、`--pack-verify`、`--pack-pin` は Clap レベルの直接アクションです。
  相互、run アクション、`--pack`、`--pack-hash` と排他です。一覧では
  `--extension-root` を使えますが、verify と pin は対象 directory を直接取ります。
- `--plan-steps`、`--plan-run`、`--ultra-plan`、`--ultra-plan-run` には末尾のゴールが必要です。
- `--validate-plan` は offline かつ read-only の action で、実行 action および生成物 action の
  すべてと排他です。step plan、UltraPlan、recovery UltraPlan YAML を受け付けます。詳細は
  [Plan YAML の編集](plan-yaml.md)を参照してください。
- `--planner-provider` が実行プロバイダと異なる場合、明示または preset の
  `planner_model` が必要です。なければ起動に失敗します。
- `--think` には、解決後の provider 役割の少なくとも一方で Ollama が必要です。
  両方が別プロバイダの場合、フラグを無視せず起動に失敗します。
- 直接 minimal-loop prompt では `--fresh-session` が `--resume` より優先されます。
  これらのセッションスイッチはスラッシュコマンドによる plan 再開には使われません。
- `--init-profile` には既存の `--extension-root` が必要です。短い draft v2 manifest を作成し、
  既存ファイルは上書きしません。
- `--validate-manifest` は profile を登録・実行せず、v1/v2 profile manifest または v1 overlay を
  検証します。失敗時はファイル、行、列、理由を 1 回だけ表示します。

## 設定雛形

`--init-config` はアクティブな workspace（現在の directory、または `--cwd`）に
`.commandagent/config.toml` を作成します。雛形の `local` preset には provider、model、planner、
classifier、budget、profile、表示設定が明示されています。`--preset local` で選ぶ前に値を確認してください。
既存の config は変更しません。

```bash
commandagent --init-config
commandagent --preset local
```

## シェル補完と man ページ

どちらの機能も現在の Clap コマンド定義から生成するため、新しいフラグは自動的に反映されます。
補完登録は各補完要求をインストール済み binary へ委譲します。`--model` と
`--planner-model` では、既定の Ollama `/api/tags` と LM Studio `/v1/models` を短時間だけ問い合わせ、
model ID を統合します。どちらかが到達不能でも警告は出しません。生成物の出力先は stdout だけです。
ユーザー所有の導入先へリダイレクトし、CommandAgent の更新後には再生成してください。

`scripts/setup.sh` は検出した Bash、Zsh、Fish 用の補完を導入するか確認します。
手動で導入する場合は、利用するシェルに対応するコマンドを実行します。

Bash の標準的なユーザー別 `bash-completion` ディレクトリ:

```bash
completion_dir="${BASH_COMPLETION_USER_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion}/completions"
mkdir -p "$completion_dir"
commandagent --completions bash > "$completion_dir/commandagent"
```

Zsh では `_commandagent` 関数を `fpath` 上のディレクトリへ置き、補完を初期化します。

```zsh
completion_dir="${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"
mkdir -p "$completion_dir"
commandagent --completions zsh > "$completion_dir/_commandagent"
fpath=("$completion_dir" $fpath)
autoload -Uz compinit && compinit
```

最後の 2 行は `.zshrc` に保存してください。Fish の場合:

```fish
set completion_dir (string join / (set -q XDG_CONFIG_HOME; and echo $XDG_CONFIG_HOME; or echo $HOME/.config) fish completions)
mkdir -p $completion_dir
commandagent --completions fish > $completion_dir/commandagent.fish
```

Fish はこのパスを自動的に読み込みます。PowerShell では生成したスクリプトを保存し、
現在のセッションまたは `$PROFILE` からドットソースします。

```powershell
commandagent --completions powershell > "$HOME/.commandagent-completion.ps1"
. "$HOME/.commandagent-completion.ps1"
```

Elvish も `commandagent --completions elvish` で生成できます。出力は利用中の
Elvish 設定に合わせて読み込むか保存してください。

生成した man ページを一般的なユーザー別パスへ導入する場合:

```bash
man_dir="${XDG_DATA_HOME:-$HOME/.local/share}/man/man1"
mkdir -p "$man_dir"
commandagent --generate-man > "$man_dir/commandagent.1"
man -l "$man_dir/commandagent.1"
```

明示パスなしの `man commandagent` で検索させるには、親の `man` ディレクトリを
`MANPATH` に追加してください。

## 例

```bash
# 既定値で対話型 TUI を起動する。
commandagent

# クラウドプロバイダで実行 prompt を 1 件処理する。
commandagent --provider gemini --model gemini-model-id \
  --prompt "現在のワークスペースを説明してください"

# 役割と profile を明示して UltraPlan を生成・実行する。
commandagent --provider ollama --model local-executor \
  --planner-provider openai --planner-model openai-model-id \
  --profile nextjs --ultra-plan-run 3011ポートでNext.jsアプリを作成してください

# 端末で固定フッターの描画が乱れる場合に無効化する。
commandagent --footer off
```
