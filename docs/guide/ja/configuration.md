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

`--preset <name>` で preset を選択します。preset セクションでは、現在の 22 キーすべてを
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
| `openrouter_provider` | `only`・`order`（provider slug の配列）と `allow_fallbacks`（boolean）だけを持つ 1 行の inline table。`openai-compatible` の役割が `provider` object として送る。例：`api = "chat_completions"` と `{ only = ["open-inference/fp4"], allow_fallbacks = false }`。`{}` は継承した table を解除（[プロバイダ](providers.md) を参照） | routing なし |

`extends` は単一継承です。子が定義したフィールドを優先し、不足フィールドを親から継承します。
親も 1 個の親を継承できますが、親が見つからない場合や `a -> b -> a` のような循環はエラーです。
継承したフィールドの診断 source は `preset:<parent>` のままです。

クォートした値には `${ENV_NAME}` 参照を 1 個以上含められます。CommandAgent はフィールド検証前に
process environment から展開します。文字列／列挙フィールドに加え、
`context_budget = "${COMMANDAGENT_CONTEXT_BUDGET}"` のようなクォートした数値にも使えます。
未設定、非 Unicode、または不正な変数参照は設定解決エラーとなり、`--doctor` では `✗` と表示します。
変数の値は表示しません。

**秘密情報の登録と伏せ字。** run は 1 個の SecretCatalog（秘密値の集合）を持ち、記録・表示・
provider 送信の各境界で同じ exact-value scrub を適用します。`sk-` や `AIza` などの形式による
推定 scrub も併用しますが、形式に合わない資格情報はこの値集合で保護します。登録元は次の範囲に
限り、process 全体の環境変数や任意の credential ファイルを再帰的に走査することはありません。

- provider key: 選択した role に関係なく、非空の `OPENAI_API_KEY`、`GEMINI_API_KEY`、
  `LM_STUDIO_API_TOKEN`、および `openai-compatible` の `api_key_env`。
- workspace root 直下の `.env` と non-template の `.env.*`（`.env.example` 系は対象外）。
  収集は 64 ファイル・各 1 MiB・catalog 1024 値が上限です。読取失敗、上限超過、symlink の
  dotenv source は秘密を黙って捨てず、**設定解決を honest に拒否**します。
- preset の `${ENV}` 展開値。展開は validation 前に登録するため、後続の検証が失敗してもその値は
  error chain に現れません。`${ENV}` 展開値が URL として不正な場合、エラーは field と検証種別
  のみを示し、展開値は stderr・error chain・`--doctor` のいずれにも表示しません。

値の登録規則は次のとおりです。**8 文字未満**の資格情報の値（資格情報名 `TOKEN`、`API_KEY`、
`PASSWORD`、`PASSWD`、`SECRET`、`CREDENTIAL`、`PRIVATE` などを持つ値と provider／`${ENV}` の
値）、**marker 候補と一致する**資格情報の値、および**合算で 1024 値を超える**登録は、
**登録せずに起動時に honest に拒否**します（値は表示しません）。資格情報名でない一般の dotenv 値は
8 文字以上を保守的に登録し、`PORT=3000`、`true`、`dev` のような短い普通の値は秘密にしません。
登録済みの値はすべて**完全一致の置換**で扱い、自由入力 field を丸ごと伏せる処理は持ちません。
event 名・schema・status・verdict・type などの固定識別子と tool 名は、登録値と衝突しても
書き換えません。

伏せ字は session/events/UI feed/spool/failsafe/evidence/trace/summary、provider に送る会話コピー
と tool schema、表示用 stream callback に及び、切り詰め・escape・chunk 分割の前に完全値へ適用
します。認証ヘッダー、運用 Config、実行引数、検証元の観測値は元の値を保ち、event 名・schema・
key・type・順序・verdict・source_refs は変えずに値だけを置換します。置換 marker は登録値と衝突
しないものを選び（最終候補まで検査）、再適用は冪等です。秘密を含む動的 key は、要素数を保ち
無関係な key を変えずに安全な key へ投影し、投影できない場合は共有 API が honest に拒否できます。
登録していない秘密は形式推定で捕捉できる範囲に限られ、format 外・未登録の値は保護されません。
過去の記録は表示時に保護し、書き戻しません。

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
受け付ける 22 キーと、早期停止条件の 11 キーを同じものと仮定しないでください。

## トップレベルキー

`config.toml` のトップレベルで有効なキーは次の 8 個だけです。

| キー | 受け付ける値 | 対応する CLI 上書き |
| --- | --- | --- |
| `extension_root` | ディレクトリパス文字列 | `--extension-root` |
| `narration` | `"normal"` または `"quiet"` | `--quiet` が `quiet` を強制 |
| `footer` | `"on"` または `"off"` | `--footer`、`--no-footer` |
| `stream` | `"on"` または `"off"` | `--stream` |
| `prompt_layout` | `"stable"` または `"legacy"` | `--prompt-layout` |
| `plan_preset` | `"none"` または `"profile"` | `--plan-preset` |
| `request_pattern` | `"off"` または `"jev"`。jev は direct の `--prompt` の依頼を判定して案内を足します | `--request-pattern` |
| `request_pattern_threshold` | `0` から `1` の数（既定 `0.8`）。jev の確信度がこれ未満なら案内を足しません | なし |

```toml
extension_root = "extensions"
narration = "quiet"
footer = "off"
stream = "on"
prompt_layout = "stable"
plan_preset = "none"
```

`model` などのキーは preset 内では有効ですが、トップレベルでは無効です。

> **互換性の変更。** 4 つの層（`<workspace>/.commandagent`、`<workspace>/.anvil`、
> `~/.commandagent`、`~/.anvil`）に存在する `config.toml` は、`--preset` や CLI の
> override の有無によらず、起動時にすべて構文解析されます。未知のトップレベルキー、
> 不正な TOML、読み取れないファイルは起動時のエラーになり、コマンドは非ゼロで終了します。
> `--doctor` も provider へ接続せず、Configuration の失敗として報告します。
> 以前は、そのようなファイルを黙って飛ばして優先度の低いファイルや既定値を使っていたため、
> 打ち間違いや壊れた上位ファイルに気づけませんでした。診断に出すのはファイル・行・キーだけで、
> 値は出しません。起動時に確認するのは構文・未知のトップレベルキー・読み取り失敗だけで、`${ENV}` の展開や preset の中身は、その preset を選んだときだけ検証します。報告された行を直すか削除してください。

有効な設定の解決は従来どおりです（flag、preset、設定ファイル、既定値の順で、source の表示も同じ）。
`[preset.<name>]` 以外のセクションは現在の小さなパーサーが設定として扱わず無視します。
拡張子のない legacy の `config` ファイルも、従来どおり寛容な行走査のままです。

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
| `COMMANDAGENT_STEP_WALL_CLOCK_CAP_MS` / `ANVIL_STEP_WALL_CLOCK_CAP_MS` | step の時間上限をミリ秒で指定する運用向け設定です。`RunSessionOptions` の指定が環境値より優先されます。値は trim して `u64` として解析し、未設定または解析不能なら既定の 15 分を使います。`0` も解析上は有効です。 |
| `COMMANDAGENT_PACK_DIRECTORY` | 選択済み pack の directory を内部で受け渡します。手で設定せず、`--pack` または preset で指定してください。 |
| `COMMANDAGENT_PACK_ID` | 選択済み pack の ID を内部で受け渡します。手で設定せず、`--pack` または preset で指定してください。 |
| `COMMANDAGENT_PACK_VERSION` | 選択済み pack の version を内部で受け渡します。手で設定せず、`--pack` または preset で指定してください。 |
| `COMMANDAGENT_PACK_HASH` | 選択済み pack の exact-byte hash を内部で受け渡します。手で設定せず、`--pack` または preset で指定してください。 |
| `COMMANDAGENT_UX_DEMO_FAST` / `ANVIL_UX_DEMO_FAST` | 開発・デモ用に `--ux-demo` の待機を短くします。値の真偽ではなく変数の存在を判定します。 |
| `COMMANDAGENT_SYSTEM_GUIDANCE` | direct の `--prompt` の run で、provider へのすべての request の system prompt に共通の案内文を 1 つ足します。未設定・空文字なら何も変わりません。下を参照してください。 |
| `COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` | OpenAI 互換の transport（`openai-compatible` と `lm-studio`）を使う role に推論の強さを宣言します。未設定・空文字なら何も送りません。下を参照してください。 |

`COMMANDAGENT_*` 項目では現在名が優先されます。現在名がない場合だけ対応する `ANVIL_*` 名を使い、
1 回だけ非推奨警告を出します。それでも表中の旧名は、現在サポートされている正確な綴りです。

`COMMANDAGENT_PACK_*` の 4 変数は例外で、直接読み込まれ、`ANVIL_*` の旧名を持ちません。これらは
選択済み pack の内部受け渡しであり、利用者向け設定ではありません。pack の選択には `--pack` または
preset を使ってください。

`COMMANDAGENT_SYSTEM_GUIDANCE` は、共通の指示文を比べるための opt-in です。direct の `--prompt` の run の
開始時に、プロセス環境だけから 1 回読みます。`ANVIL_*` の旧名はなく、設定ファイルや `.env` からは読みません。
plan の入口、TUI、`--model-probe` はこの変数を使いません。設定すると、その run の provider へのすべての
request（各 turn、tool 呼び出し後の turn、空応答の fresh retry、native と XML の tool protocol）の
「Request guidance」block に文を 1 回ずつ載せます。`--request-pattern jev` が pattern を採用したときは、
その pattern の案内の後に「Common guidance:」の行に続けて載せます。元の依頼、profile の案内、session の履歴、
`request_pattern_resolved` event、jev を呼ぶ条件は変わりません。値は UTF-8 で 8 KiB（8192 bytes）以下、
空白だけでないことが必要です。満たさないときは、値を含めず変数名と理由だけを示すエラーで run が失敗します。
この文は通常の案内であり、tool の権限を与えたり外したりせず、検証・completion・acceptance・予算の規則も
変えません。run の secret scrub は適用されますが、値に秘密を入れないでください。

`COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` は、OpenAI 互換 server に推論の強さを指定する宣言です。
provider の client を作るときにプロセス環境だけから読みます（`ANVIL_*` の旧名はなく、設定ファイルや
`.env` からは読みません）。その client の clone・worker・通信の再試行は同じ値を送ります。CLI flag はありません。

- 適用範囲：provider が OpenAI 互換の transport に解決される role、つまり汎用の `openai-compatible` と
  `lm-studio` のすべてで、executor・planner・classifier を区別しません。Ollama native・OpenAI・Gemini の
  role には送らず、この値がそれらの role の起動を止めることもありません。
- 値：前後の空白を除いた上で `none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max` のいずれか
  （小文字）。大文字小文字や別名は変換しません。未設定・空文字・空白だけなら field を送らず、変数がない
  ときと同じ request になります。それ以外の値（`LOW`、`true`、未知の語、UTF-8 でない byte 列）は、最初の
  request の前に、変数名と許容語を示し値は示さないエラーで失敗します。
- wire：Chat Completions（`--api chat-completions`）は最上位に `"reasoning_effort": "<値>"` を、
  Responses（`--api responses`）は `"reasoning": {"effort": "<値>"}` を足します。request のほかの部分は
  変わりません。`max_tokens` と `max_output_tokens` はそのままで、OpenAI 専用の field や Ollama の
  `think` は足しません。
- 対応しない値：CommandAgent が確かめるのは自分の語彙だけです。model がどの値に対応するかを server に
  問い合わせず、model 名や host 名で特別扱いもしません。server が値を拒んだとき（たとえば model が
  持たない語への HTTP 400）は、通常の再試行しない provider エラーとして turn が失敗します。field を
  外したり、別の強さ・別の API・`think` に置き換えたりして送り直すことはしません。
- ほかの設定との関係：`--think` は今までどおり Ollama native の role だけに効き、
  `COMMANDAGENT_OPENAI_REASONING_EFFORT` は `openai` provider だけに効きます。どちらもこの変数へ
  fallback せず、この変数もそれらへ fallback しません。
- 証跡：`--doctor` は `config.compatible_reasoning_effort` に role ごとの `declared`・`omitted`・
  `not_applicable`・`invalid` を示し（互換 role が使う値が不正ならこの check は失敗します）、headless
  summary は同じ宣言を `model_metadata.compatible_reasoning_effort` に記録します。どちらも CommandAgent が
  送る内容であり、server が適用したことではありません（`server_support_verified` は `false` のまま）。
  値を送ったときは `provider_request` event に `requested_reasoning_effort`・`reasoning_effort_source`
  （`env`）・`provider_role` が加わります。

たとえば executor が OpenAI 互換、planner が Ollama native の場合：

```bash
COMMANDAGENT_COMPATIBLE_REASONING_EFFORT=low commandagent \
  --provider openai-compatible --base-url http://127.0.0.1:11434/v1 \
  --model glm-5.3-flash:cloud \
  --planner-provider ollama --planner-model <planner-model> \
  --prompt '<依頼>'
```

executor は `reasoning_effort: "low"` を送ります。Ollama の planner は自分の `--think` の設定のままで、
effort の field を受け取りません。planner も `--planner-provider openai-compatible` なら、同じ `low` を送ります。

## 実効設定の確認

TUI を起動して `/status` を実行してください。起動 banner と status card は、`flag`、
`preset:<name>`、`config:<file>`、推論／既定値の source、timeout source、footer、stream、
prompt layout など、主要な解決値と出所を表示します。

model、profile、layout、timeout、表示 mode が想定外の場合、診断前にこの表示を確認してください。
この表示は API キーを表示しません。設定の診断でもキー値を表示しないでください。
