# 設定

[English](../en/configuration.md) | [ガイド目次](../README.md)

CommandAgent は `--cwd`、またはプロセスの現在のディレクトリからアクティブなワークスペースを
正規化した後に設定を解決します。設定ファイルを読み込みますが、ファイルや preset を自動作成・
自動追加することはありません。

## 解決の優先順位

すべての層をサポートするフィールドの順序は次のとおりです。

```text
CLI フラグ > 選択した preset のフィールド > トップレベル設定キー > 組み込み既定値
```

解決はフィールド単位で行われ、すべての設定が 4 層すべてをサポートするわけではありません。

| フィールド | 実装されている順序 |
| --- | --- |
| `prompt_layout`、`plan_preset` | CLI > preset > トップレベルキー > 計算値／組み込み既定値 |
| `narration` | `--quiet` > preset > トップレベルキー > `normal` |
| `footer` | `--no-footer` または `--footer` > preset > トップレベルキー > `on` |
| `stream` | `--stream` > preset > トップレベルキー > REPL ではオン、直接アクションではオフ |
| `model`、`provider`、`context_budget`、`chat_timeout_secs` | CLI > preset > 組み込み／プロバイダ依存の既定値 |
| `api` | `--api` > preset > `chat_completions`（OpenAI のみ） |
| `tool_protocol` | CLI > preset > プロバイダ能力の既定値 |
| `planner_model`、`planner_provider` | CLI > preset > 実行役割から継承。異なるプロバイダには planner model が必要 |
| `profile` | CLI > preset > ゴール／ワークスペース推論 > `generic` |
| `ollama_host`、`num_predict`、`max_iterations`、`chat_retries`、`style`、`state_dir`、その他の CLI 専用フィールド | CLI 値または CLI 宣言／組み込み既定値。設定ファイルでは受け付けない |

timeout の既定値は、いずれかの役割が Ollama なら `600` 秒、両方がリモートなら `180` 秒です。
`context_budget` の既定値は `65536` です。`plan_preset` は通常 `none` ですが、明示した `data` と
`fix` または `investigate` の組み合わせでは、planner model の既定値を適用する前に `profile` を
計算値として選ぶことがあります。

## 設定ファイルの探索パス

アクティブなワークスペースは、正規化した `--cwd` のパス、または `--cwd` がなければ正規化した
プロセスの現在のディレクトリです。TOML 形式のファイルは、優先度の高い順に次のとおり探索します。

1. `<workspace>/.commandagent/config.toml`
2. `<workspace>/.anvil/config.toml`
3. `$HOME/.commandagent/config.toml`
4. `$HOME/.anvil/config.toml`

トップレベルキーでは、そのキーを含む最初のファイルが優先されます。選択した preset では、
フィールドごとに最初の値が優先され、不足フィールドを後続ファイルから補えます。これにより、
[完全性の罠](#preset-の完全性の罠)に該当しない限り、ワークスペースの preset でユーザー preset の
一部だけを上書きできます。

`.anvil/` の名前は引き続きサポートされます。新しい `.commandagent/` 設定名前空間だけを理由に、
既存の `.anvil/` runtime またはガイド中のパスを変更しないでください。

## Preset

`--preset <name>` で preset を選択します。preset セクションでは、現在の 14 キーすべてを
受け付けます。文字列／列挙値はダブルクォートで囲み、数値はクォートなしの整数で指定します。

| Preset キー | 受け付ける値 | どの層にもない場合の実効 fallback |
| --- | --- | --- |
| `model` | model ID 文字列 | `qwen3.6:27b-coding-nvfp4` |
| `provider` | `"ollama"`、`"openai"`、`"gemini"` | `"ollama"` |
| `api` | `"chat_completions"` または `"responses"` | `"chat_completions"` |
| `tool_protocol` | `"native"` または `"text"` | プロバイダ能力の既定値 |
| `planner_model` | model ID 文字列 | プロバイダが同じなら実行モデル。それ以外は必須 |
| `planner_provider` | `"ollama"`、`"openai"`、`"gemini"` | 実行プロバイダ |
| `context_budget` | 非負のプラットフォームサイズ整数 | `65536` |
| `chat_timeout_secs` | 非負の 64 bit 整数 | プロバイダ依存の `600` または `180` |
| `profile` | profile 文字列 | 推論後に `"generic"` |
| `narration` | `"normal"` または `"quiet"` | `"normal"` |
| `footer` | `"on"` または `"off"` | `"on"` |
| `stream` | `"on"` または `"off"` | REPL ではオン、それ以外はオフ |
| `prompt_layout` | `"stable"` または `"legacy"` | トップレベル値、その後 `"legacy"` |
| `plan_preset` | `"none"` または `"profile"` | トップレベル／計算された planner 値 |

```toml
[preset.local]
model = "qwen3.6:27b-coding-nvfp4"
provider = "ollama"
api = "chat_completions"
tool_protocol = "text"
planner_model = "qwen3.6:27b-coding-nvfp4"
planner_provider = "ollama"
context_budget = 65536
chat_timeout_secs = 600
profile = "nextjs"
narration = "normal"
footer = "on"
stream = "on"
prompt_layout = "legacy"
plan_preset = "none"
```

構文解析した preset に未知のキーまたは不正な値がある場合、ファイル、行、
`preset.<name>.<key>` を示すエラーになります。探索パスのどこにもない名前を選んだ場合もエラーです。
名前付き preset が見つからないときに暗黙で既定値を使うことはありません。

### Preset の完全性の罠

preset のマージは、`model`、`provider`、`planner_model`、`planner_provider`、
`context_budget`、`chat_timeout_secs`、`plan_preset`、`profile`、`narration`、`footer`、
`stream` の 11 フィールドが揃った時点で早期停止します。

`prompt_layout`、`api`、`tool_protocol` は受け付けるキーですが、この完全性判定には**含まれません**。優先度の高い preset が
11 個の完全性フィールドをすでに持ちながら `prompt_layout` を省略している場合、探索が停止し、
優先度の低いファイルにある同じ preset の `prompt_layout` は継承されません。`prompt_layout` を
優先度の高い同じ preset に置くか、意図した下位層まで探索されるように完全性フィールドを残してください。
受け付ける 14 キーと、早期停止条件の 11 キーを同じものと仮定しないでください。

## トップレベルキー

`config.toml` のトップレベルで有効なキーは次の 5 個だけです。

| キー | 受け付ける値 | 対応する CLI 上書き |
| --- | --- | --- |
| `narration` | `"normal"` または `"quiet"` | `--quiet` が `quiet` を強制 |
| `footer` | `"on"` または `"off"` | `--footer`、`--no-footer` |
| `stream` | `"on"` または `"off"` | `--stream` |
| `prompt_layout` | `"stable"` または `"legacy"` | `--prompt-layout` |
| `plan_preset` | `"none"` または `"profile"` | `--plan-preset` |

```toml
narration = "quiet"
footer = "off"
stream = "on"
prompt_layout = "stable"
plan_preset = "none"
```

`model` などのキーは preset 内では有効ですが、トップレベルでは無効です。未知のトップレベルキーが
あると、そのファイルの構文解析が失敗します。名前付き preset の読み込みは構文解析エラーを表示し、
トップレベルフィールドの検索は構文解析できなかったファイルを飛ばして優先度の低いファイルへ進みます。
現在の小さなパーサーは `[preset.<name>]` 以外のセクションを設定として扱わず、無視します。

## 旧形式の拡張子なし設定

4 個の `config.toml` パスに一致する値がなければ、トップレベルの検索では次の拡張子なしファイルも
この順で確認します。

1. `<workspace>/.commandagent/config`
2. `<workspace>/.anvil/config`

旧形式の `.anvil/config` は、その名前のまま引き続きサポートされます。行単位の `key = value` 形式で、
`narration`、`footer`、`stream`、`prompt_layout`、`plan_preset` だけの fallback です。
preset はサポートしません。値はクォートあり／なしの両方を使え、`#` からコメントになります。
新しい設定では `config.toml` を推奨します。

## 環境変数

OpenAI キーはプロセス環境だけから読みます。Gemini キーは最初にプロセス環境、次に
ワークスペースの `.env` から読みます。[プロバイダ](providers.md)も参照してください。
通常のユーザー向け動作には次の環境変数も影響します。

| 環境変数 | 効果 |
| --- | --- |
| `NO_COLOR` | ANSI color を無効にします。`ANVIL_*` の別名はありません。 |
| `COMMANDAGENT_NO_FOOTER` / `ANVIL_NO_FOOTER` | 空でない値で固定 footer を無効にします。 |
| `COMMANDAGENT_NO_SPINNER` / `ANVIL_NO_SPINNER` | 空でない値で進捗 spinner を無効にします。 |
| `COMMANDAGENT_NO_MARKDOWN` / `ANVIL_NO_MARKDOWN` | 空でない値で端末 Markdown 描画を無効にします。 |
| `COMMANDAGENT_NO_INTERRUPT` / `ANVIL_NO_INTERRUPT` | 空でない値で raw mode の interrupt monitor を無効にします。 |
| `COMMANDAGENT_NO_TERMINAL_TITLE` / `ANVIL_NO_TERMINAL_TITLE` | 空でない値で Ultra phase の端末タイトル進捗を無効にします。 |
| `COMMANDAGENT_NO_BELL` / `ANVIL_NO_BELL` | 空でない値で 10 秒以上かかったコマンドの完了ベルを無効にします。 |
| `COMMANDAGENT_EVAL_EVENTS` / `ANVIL_EVAL_EVENTS` | event JSONL パスを上書きします。 |
| `COMMANDAGENT_PLAYWRIGHT_DIR` / `ANVIL_PLAYWRIGHT_DIR` | interaction probe 用 Playwright module の明示的探索ディレクトリを追加します。 |
| `COMMANDAGENT_COMPLETION_CONTRACT` / `ANVIL_COMPLETION_CONTRACT` | 外部 completion-contract パスを指定します。 |
| `COMMANDAGENT_DEV_SERVER_PROBE` / `ANVIL_DEV_SERVER_PROBE` | false 値で planner の dev-server probe を無効にします。 |
| `XDG_STATE_HOME` | 既定の state directory が使うベースを変更します。 |
| `HOME` | ユーザー設定パスと state directory fallback のホームを指定します。 |
| `LC_ALL`、次に `LANG` | spinner が UTF-8 frame を使うか決めます。 |

`COMMANDAGENT_*` 項目では現在名が優先されます。現在名がない場合だけ対応する `ANVIL_*` 名を使い、
1 回だけ非推奨警告を出します。それでも表中の旧名は、現在サポートされている正確な綴りです。

## 実効設定の確認

TUI を起動して `/status` を実行してください。起動 banner と status card は、`flag`、
`preset:<name>`、`config:<file>`、推論／既定値の source、timeout source、footer、stream、
prompt layout など、主要な解決値と出所を表示します。

model、profile、layout、timeout、表示 mode が想定外の場合、診断前にこの表示を確認してください。
この表示は API キーを表示しません。設定の診断でもキー値を表示しないでください。
