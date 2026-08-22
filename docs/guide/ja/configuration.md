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
| `base_url`、`api_key_env` | CLI > preset。`openai-compatible` のみが使用し、base URL は必須 |
| `api` | `--api` > preset > `chat_completions`（OpenAI と LM Studio） |
| `tool_protocol` | CLI > preset > プロバイダ能力の既定値 |
| `planner_model`、`planner_provider` | CLI > preset > 実行役割から継承。異なるプロバイダには planner model が必要 |
| `planner_think` | 明示した `--think` > preset > `false`。CLI flag は executor の上書きとしても維持 |
| `classifier_model`、`classifier_provider` | preset > planner 役割から継承。異なるプロバイダには classifier model が必要 |
| `profile` | CLI > preset > ゴール／ワークスペース推論 > `generic` |
| `pack` | CLI `--pack` > 選択 preset。明示値が矛盾すると失敗 |
| `extension_root` | CLI `--extension-root` > トップレベルキー > repository のみ探索 |
| `ollama_host`、`think`、`lm_studio_host`、`num_predict`、`max_iterations`、`chat_retries`、`style`、`state_dir`、その他の CLI 専用フィールド | CLI 値または CLI 宣言／組み込み既定値。設定ファイルでは受け付けない |

timeout の既定値は、executor、planner、classifier のいずれかが Ollama または LM Studio なら `600` 秒、
全役割がリモートなら `180` 秒です。
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

`.anvil/` の設定名は legacy read として引き続きサポートされます。新しい run、plan、repair、
evidence は対応する `.commandagent/` subdirectory に書き込み、run inventory、resume 探索、
evidence consumer は適用可能な `.anvil/` read を維持します。既定の session と workspace history は
platform の `commandagent` state directory に書き込み、既存 state の load または copy-forward 時は
`anvilminimal` へ fallback します。

## Preset

`--preset <name>` で preset を選択します。preset セクションでは、現在の 21 キーすべてを
受け付けます。文字列／列挙値はダブルクォートで囲み、数値はクォートなしの整数で指定します。

| Preset キー | 受け付ける値 | どの層にもない場合の実効 fallback |
| --- | --- | --- |
| `extends` | 親 preset 名 1 個 | 親なし |
| `pack` | exact な `"id@MAJOR.MINOR.PATCH"` selector | pack なし |
| `model` | model ID 文字列 | `qwen3.6:27b-coding-nvfp4` |
| `provider` | `"ollama"`、`"lm-studio"`、`"openai"`、`"openai-compatible"`、`"gemini"` | `"ollama"` |
| `base_url` | credential、query、fragment を含まない HTTP(S) URL | `openai-compatible` では必須 |
| `api_key_env` | プロセス環境変数名 | bearer 認証なし |
| `api` | `"chat_completions"` または `"responses"` | `"chat_completions"` |
| `tool_protocol` | `"native"` または `"text"` | プロバイダ能力の既定値 |
| `planner_model` | model ID 文字列 | プロバイダが同じなら実行モデル。それ以外は必須 |
| `planner_provider` | `"ollama"`、`"lm-studio"`、`"openai"`、`"openai-compatible"`、`"gemini"` | 実行プロバイダ |
| `planner_think` | `"true"`、`"false"`、`"low"`、`"medium"`、`"high"` | `"false"` |
| `classifier_model` | model ID 文字列 | プロバイダが同じなら planner model。それ以外は必須 |
| `classifier_provider` | `"ollama"`、`"lm-studio"`、`"openai"`、`"openai-compatible"`、`"gemini"` | planner provider |
| `context_budget` | 非負のプラットフォームサイズ整数 | `65536` |
| `chat_timeout_secs` | 非負の 64 bit 整数 | プロバイダ依存の `600` または `180` |
| `profile` | profile 文字列 | 推論後に `"generic"` |
| `narration` | `"normal"` または `"quiet"` | `"normal"` |
| `footer` | `"on"` または `"off"` | `"on"` |
| `stream` | `"on"` または `"off"` | REPL ではオン、それ以外はオフ |
| `prompt_layout` | `"stable"` または `"legacy"` | トップレベル値、その後 `"legacy"` |
| `plan_preset` | `"none"` または `"profile"` | トップレベル／計算された planner 値 |

`extends` は単一継承です。子が定義したフィールドを優先し、不足フィールドを親から継承します。
親も 1 個の親を継承できますが、親が見つからない場合や `a -> b -> a` のような循環はエラーです。
継承したフィールドの診断 source は `preset:<parent>` のままです。

クォートした値には `${ENV_NAME}` 参照を 1 個以上含められます。CommandAgent はフィールド検証前に
process environment から展開します。文字列／列挙フィールドに加え、
`context_budget = "${COMMANDAGENT_CONTEXT_BUDGET}"` のようなクォートした数値にも使えます。
未設定、非 Unicode、または不正な変数参照は設定解決エラーとなり、`--doctor` では `✗` と表示します。
変数の値は表示しません。

```toml
[preset.team_base]
provider = "openai-compatible"
base_url = "${TEAM_LLM_URL}"
model = "${TEAM_EXECUTOR_MODEL}"
planner_model = "${TEAM_PLANNER_MODEL}"

[preset.alice]
extends = "team_base"
model = "${ALICE_EXECUTOR_MODEL}"
```

次の complete preset は、現在のローカル実測で推奨できる役割分割を示します。built-in default では
ありません。利用前に
[役割別の実測根拠と適用範囲](../model-probe-results/2026-08-22-local-role-pairs.md)
を確認してください。

```toml
[preset.local_role_split]
model = "qwen3.8:27b-mlx"
provider = "ollama"
api = "chat_completions"
tool_protocol = "native"
planner_model = "qwen3.8:27b-mlx"
planner_provider = "ollama"
planner_think = "false"
classifier_model = "qwen3.5:4b"
classifier_provider = "ollama"
context_budget = 65536
chat_timeout_secs = 600
profile = "generic"
narration = "normal"
footer = "on"
stream = "on"
prompt_layout = "legacy"
plan_preset = "none"
```

この exact local probe が支持する小型化は classifier だけです。計測した 9B／4B 候補による planner
置換は支持しません。model digest、host、context、provider、build が変わった場合は、
[model probe](model-probe.md#役割別の計測手順)と scenario admission check を再実行してください。

構文解析した preset に未知のキーまたは不正な値がある場合、ファイル、行、
`preset.<name>.<key>` を示すエラーになります。探索パスのどこにもない名前を選んだ場合もエラーです。
名前付き preset が見つからないときに暗黙で既定値を使うことはありません。

### Preset の完全性の罠

preset のマージは、`model`、`provider`、`planner_model`、`planner_provider`、
`context_budget`、`chat_timeout_secs`、`plan_preset`、`profile`、`narration`、`footer`、
`stream` の 11 フィールドが揃った時点で早期停止します。

`prompt_layout`、`api`、`tool_protocol`、`pack`、`planner_think`、
`classifier_model`、`classifier_provider`、`base_url`、`api_key_env`、`extends` は受け付けるキーですが、
この完全性判定には**含まれません**。優先度の高い preset が
11 個の完全性フィールドをすでに持ちながら `prompt_layout` を省略している場合、探索が停止し、
優先度の低いファイルにある同じ preset の `prompt_layout` は継承されません。`prompt_layout` を
優先度の高い同じ preset に置くか、意図した下位層まで探索されるように完全性フィールドを残してください。
受け付ける 21 キーと、早期停止条件の 11 キーを同じものと仮定しないでください。

## トップレベルキー

`config.toml` のトップレベルで有効なキーは次の 6 個だけです。

| キー | 受け付ける値 | 対応する CLI 上書き |
| --- | --- | --- |
| `extension_root` | ディレクトリパス文字列 | `--extension-root` |
| `narration` | `"normal"` または `"quiet"` | `--quiet` が `quiet` を強制 |
| `footer` | `"on"` または `"off"` | `--footer`、`--no-footer` |
| `stream` | `"on"` または `"off"` | `--stream` |
| `prompt_layout` | `"stable"` または `"legacy"` | `--prompt-layout` |
| `plan_preset` | `"none"` または `"profile"` | `--plan-preset` |

```toml
extension_root = "extensions"
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

## Pack 選択

Pack selector は `nextjs-acme@1.0.0` のように exact version を必ず含めます。
CommandAgent は `<workspace>/packs/<id>/<version>` より先に
`<extension_root>/<id>/<version>`（互換レイアウト
`<extension_root>/packs/<id>/<version>` も可）を探索します。選択ディレクトリには、読み込んだ
assist/eval の exact-byte hash と一致する `pack.sha256` が必要です。pin 不在、古い pin、
`--pack-hash` 不一致、profile/intent 不一致は run-start evidence の出力前に終了コード 2 で停止します。

```toml
extension_root = "extensions"

[preset.nextjs_acme_cagentpack]
pack = "nextjs-acme@1.0.0"
profile = "nextjs"
```

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

OpenAI キーと任意の `LM_STUDIO_API_TOKEN` はプロセス環境だけから読みます。Gemini キーは最初にプロセス環境、次に
ワークスペースの `.env` から読みます。[プロバイダ](providers.md)も参照してください。
通常のユーザー向け動作には次の環境変数も影響します。

| 環境変数 | 効果 |
| --- | --- |
| `LM_STUDIO_API_TOKEN` | LM Studio のサーバー認証を有効にした場合に任意の Bearer token を指定します。 |
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
