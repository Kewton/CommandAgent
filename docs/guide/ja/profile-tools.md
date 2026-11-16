# operator root・profile tools・run budget

[English](../en/profile-tools.md) | [ガイド目次](../README.md)

3 つの機能が、ワークスペースの外に置く operator（運用者）のファイルを読みます。
**operator root**、**profile tools**、**run budget** です。モデル・プロファイル・
ワークスペース内のファイルは、そこにあるものを作ったり変えたりできません。この頁では、
そのファイルに何を書くか、結果をどう読むかを説明します。

## operator root

operator root は、環境変数 `COMMANDAGENT_ADAPTER_CONFIG_ROOT` で指す 1 つの絶対パスの
ディレクトリです。ワークスペースの外に置きます。

```bash
export COMMANDAGENT_ADAPTER_CONFIG_ROOT=/operator/config
```

同じ root に複数の機能の宣言が入ります。機能ごとに専用のディレクトリと専用の protocol があり、
root が共有するのは場所であって形式ではありません。

| operator root の下のパス | 入れるもの | protocol |
| --- | --- | --- |
| `profile-tools/<id>.json` | profile tool 1 つにつき descriptor 1 つ | `profile-tool.descriptor/v1` |
| `run-budgets/<id>.json` | run budget 1 つにつき宣言 1 つ | `run-budget.policy/v1` または `run-budget.policy/v2` |

- 変数が未設定、または相対パスのとき、root を要する機能は、run が何も送る前に拒否されます。
- root の下ではシンボリックリンクをたどらずにファイルを読みます。ワークスペースの中へ解決される
  ファイルは拒否されます。
- run の command が書き込める場所に run budget の宣言があるときも拒否されます。

## profile tools

profile tool は、プロファイルまたは eval pack が参照できる、中身の見えない tool ID です。
プロファイルや pack は「どの ID を使うか」だけを書き、各 ID が何を実行するかは operator root に
operator が書きます。モデルに届くのは、operator が pin（固定）したものだけです。

### プロファイルまたは pack で tool を宣言する

`schema_version = "v2"` のプロファイル manifest と eval pack は、`[profile_tools]` の表を
持てます。v1 の manifest・overlay・assist pack は持てません。表が無ければ機能は off で、
何も変わりません。

```toml
[profile_tools]
schema_version = "profile-tool.declaration/v1"
builtin_tools = ["Read", "Grep"]
tools = [
  { id = "config-json-propose", effect = "propose-artifacts", max_output_bytes = 4096 },
]
```

- `builtin_tools` は必須です。残す組み込み tool を並べます。提出だけのプロファイルは `Write`・
  `Edit`・`Bash` を外します。
- 宣言は取り除くか狭めるだけです。実行ファイル・引数・hash・権限は書けず、外した組み込み tool を
  戻すことも、上限を広げることもできません。
- プロファイルと選ばれた pack の両方が宣言したときは、組み込み tool の集合はその共通部分になり、
  両方に現れる tool ID は拒否されます。

### descriptor を書く

`<operator root>/profile-tools/<tool id>.json` を作ります。この descriptor は、宣言した 1 つの
出力に対するバイト列を提案する、pin 済みの process を 1 つ実行します。採用の前に host がそのバイト列を
検査します。

```json descriptor-example
{
  "protocol": "profile-tool.descriptor/v1",
  "tool_id": "config-json-propose",
  "name": "ProposeConfigJson",
  "version": "1.0.0",
  "description": "Propose the contents of the configuration file.",
  "effect": "propose-artifacts",
  "parameters": [],
  "schema": {
    "protocol": "profile-tool.schema/v1",
    "root": {
      "type": "object",
      "additionalProperties": false,
      "required": ["mode"],
      "properties": {"mode": {"type": "string", "enum": ["strict", "lenient"]}}
    }
  },
  "schema_sha256": "<sha256 of the compact JSON of schema, keys sorted>",
  "runtime": {"kind": "pinned_process", "id": "config-proposer"},
  "process": {
    "protocol": "profile-tool.process/v1",
    "executable": {"path": "/operator/tools/propose", "sha256": "<sha256 of the file>"},
    "dependency_directories": [{"path": "/operator/tools", "sha256": "<directory pin>"}],
    "argv": ["/operator/tools/serializer.js"],
    "cwd": "/operator/tools",
    "env": {"TOOL_MODE": "strict"},
    "limits": {"timeout_ms": 30000, "max_result_bytes": 65536},
    "outputs": [{"id": "artifact", "path": "config/settings.json"}]
  },
  "limits": {"max_input_bytes": 1024, "max_output_bytes": 4096}
}
```

- 2 つの `sha256` と directory pin は placeholder です。実際のファイルから計算してください。
  pin が合わない descriptor は `invalid_pin` で拒否されます。
- pin するパスはすべて絶対パスで、正規化済みで、ワークスペースの外にあります。
- process の環境変数は、宣言した key だけです。何も引き継がれず、loader が検索パスやコードとして
  読む key は拒否されます。
- `outputs` はちょうど 1 件です。`path` はワークスペース相対で、process はそれを見ることも
  選ぶこともできません。
- pin は実行の直前にも毎回比べ直されます。合わなければ、tool を実行せずに run が終わります。

### コマンドラインのプログラムで動くもの

- `propose-artifacts` の effect を持つ `pinned_process` の tool は、`commandagent` の binary で
  動きます。macOS の Seatbelt sandbox が probe に通ることが必要で、それ以外の環境では run の開始前に
  `unavailable` として拒否されます。
- runtime が `in_process` の tool には、CommandAgent を組み込むプログラムが登録する runtime が
  必要です。binary は何も登録しないので、その ID は `unavailable` として拒否されます。
- tool の結果だけで、必須の成果物や段が満たされることはありません。受入の gate はこれまでどおり
  判定します。

### tool が拒否されるとき

admission（受付）の拒否は、起動時、最初の provider 呼び出しの前に起きます。メッセージは
`profile_tool_refused: <kind>: <reason>` の形で、command は終了コード 2 で終わります。

pin は、実行の直前にも毎回比べ直されます。受付の後に pin したファイルが変わっていれば、
モデルが tool を呼んだ時点で、この確認が実行時にその呼び出しを拒否します。tool は実行されず、
run は終わります。この拒否は、provider 呼び出しが既に行われた後に起きます。

| Kind | よくある原因 |
| --- | --- |
| `registry_unavailable` | operator root が未設定、または解決できない |
| `unknown_tool_id` | その ID の descriptor が登録されていない |
| `invalid_descriptor` | descriptor が decode できない、または検証に通らない |
| `invalid_pin` | schema・実行ファイル・依存が pin と合わない |
| `workspace_descriptor` | descriptor や依存がワークスペースの中へ解決される、または descriptor がシンボリックリンク |
| `unavailable` | sandbox や runtime など、必要な能力が使えない |

kind の全一覧、schema の部分集合、上限は、開発者向けの文書 `docs/dev/profile-tools.md` にあります。

## run budget

run budget は、1 回の run の provider 呼び出しに対する上限の財布（wallet）です。送信のたびに、
host は最悪の場合の額をディスクに予約します。上限を超える送信は出ず、費用を数えられない送信が
あれば run は止まります。wallet を宣言するのは operator で、run は選ぶだけです。

### 2 つの基準

宣言には 2 つの版があります。数え方が違い、互いを読み替えることはありません。

| | `run-budget.policy/v1` | `run-budget.policy/v2` |
| --- | --- | --- |
| 数えるもの | provider が請求したと報告した額 | 報告された token 使用量に、operator が宣言した単価表を掛けた額（`usage_tariff`） |
| route | generic client 経由の OpenAI Chat Completions（`route.api` は `chat_completions`） | OpenAI 直接の、非 streaming の Responses（`route.api` は `responses`） |
| ファイルの中の単価 | 予約に使う 1 組の価格 | 入力長の tier ごとの価格と、endpoint・応答モデル・入力の上限 |
| ledger の請求額 | 報告されたとおりに記録 | 常に `unreported`。額は使用量から計算する |

### v1 の budget を宣言する

`<operator root>/run-budgets/<budget id>.json` を作ります。ファイルの中の `budget_id` は、
`.json` を除いたファイル名と同じにします。

```json run-budget-v1-example
{
  "protocol": "run-budget.policy/v1",
  "budget_id": "wallet-example",
  "limits": {
    "cost_usd": "5",
    "physical_sends": 200,
    "logical_global": 100,
    "logical_per_stage": 20,
    "tokens": 2000000,
    "wall_seconds": 3600,
    "final_repair_attempts": 3,
    "final_repair_seconds": 600
  },
  "tariff": {
    "revision": "tariff-example-1",
    "input_per_million": "0.5",
    "cached_input_per_million": "0.05",
    "cache_write_per_million": "0.5",
    "output_per_million": "2",
    "request_fee": "0"
  },
  "route": {
    "api": "chat_completions",
    "model": "example-model",
    "upstream_only": ["example-upstream"],
    "allow_fallbacks": false,
    "effort": "low",
    "reasoning_in_output": true
  },
  "max_output_tokens": 4096,
  "max_input_tokens": 120000,
  "input_overhead": 256,
  "ledger_dir": "/operator/ledgers"
}
```

- 金額は数値ではなく、10 進の文字列で書きます。入力または出力の価格が 0 のときは拒否されます。
  無料の route では費用の上限が何も止めなくなるためです。
- `upstream_only` は、request が使ってよい上流の provider を、1 つずつ重複なく並べます。
  `allow_fallbacks` は `false`、`reasoning_in_output` は `true` にします。
- `wall_seconds` と `final_repair_seconds` は 0 より大きくします。
- 未知の欄・重複した key・型の違う値があると、ファイル全体が拒否されます。

### v2 の budget を宣言する

v2 のファイルは、OpenAI 直接の Responses の run 用です。provider は token の使用量を報告しますが、
請求額は報告しません。そこで wallet は、検証済みの使用量に、宣言した単価表を掛けて数えます。

```json run-budget-v2-example
{
  "protocol": "run-budget.policy/v2",
  "accounting_basis": "usage_tariff",
  "budget_id": "wallet-example",
  "limits": {
    "cost_usd": "5",
    "physical_sends": 200,
    "logical_global": 100,
    "logical_per_stage": 20,
    "tokens": 2000000,
    "wall_seconds": 3600,
    "final_repair_attempts": 3,
    "final_repair_seconds": 600
  },
  "route": {
    "api": "responses",
    "endpoint": "https://api.openai.com/v1/responses",
    "model": "example-model",
    "response_models": ["example-model-snapshot"],
    "effort": "high",
    "service_tier": "default"
  },
  "tariff": {
    "revision": "tariff-example-1",
    "price_conditions": {"region": "global", "regional_surcharge": "none"},
    "cache_write_mode": "none",
    "tiers": [
      {
        "above_input_tokens": 0,
        "input_per_million": "2",
        "cached_input_per_million": "0.2",
        "output_per_million": "8",
        "request_fee": "0"
      }
    ]
  },
  "input_bound": {
    "method": "body_bytes_plus_overhead_or_provider_max",
    "input_overhead": 256,
    "provider_max_input_tokens": 400000
  },
  "max_output_tokens": 4096,
  "max_input_tokens": 400000,
  "ledger_dir": "/operator/ledgers"
}
```

- `route.endpoint` は、direct client が POST する endpoint、`https://api.openai.com/v1/responses`
  でなければなりません。transport が実際に POST する先と比べられ、違う値ではすべての送信が拒否されます。
  書いてあるとおりに使ってください。
- モデル・応答モデル・価格・上限は、自分の値に置き換えてください。上の値は例で、実際の価格ではありません。
- `response_models` は、受け入れる応答モデル名を完全一致で並べます。別のモデルの応答は、その送信を
  不明のまま残します。
- `cache_write_mode` は `ttl_30m_12h`（書き込みを TTL ごとに値付けする。すべての tier に
  `cache_write_30m_per_million` と `cache_write_12h_per_million` が要る）か、`none`（書き込みを
  値付けしない。どの tier も書き込みの価格を持てない）です。
- `tiers` は `above_input_tokens` が 0 から始まり、厳密に増えます。request の入力が入る tier が、
  その request 全体の価格を決めます。
- `price_conditions` は、地域と追加料金についての operator の申告です。host は検証できません。
- 不透明な reasoning item を再送する request は `provider_max_input_tokens` で上限が決まります。
  この値が無いと、そのような request は送られません。
- run のすべての role は、ファイルにある 1 つのモデルと effort を使わなければなりません。

### 共通のパス・ledger・基準の変更

- wallet は `COMMANDAGENT_RUN_BUDGET=<budget id>` で選びます。未設定は off で、budget について
  何も読みません。ID は英数字・`-`・`_`・`.` で、64 バイト以内、先頭はドットにしません。
- v1 と v2 のファイルは同じパス `<operator root>/run-budgets/<id>.json` に置きます。host は
  `protocol` を読んで decoder を選び、v2 と名乗らないファイルは v1 として読みます。
- ledger は両方の版で `<ledger_dir>/<budget_id>` にあり、1 つの lock の下にあります。基準と、
  宣言のバイト列そのものの hash を持ちます。
- wallet は基準を変えません。v1 の ledger を v2 のファイルで開く、またはその逆は拒否され、移行は
  ありません。新しい budget ID を使ってください。既にある wallet の宣言を編集することも拒否されます。
- wallet が数えられない run は、送信の前に拒否されます。workflow・model probe・request pattern の
  routing・OS sandbox を切った run などです。

### summary に出るもの

`--summary-json` には、`provider_cost_usd` の隣に 3 つの欄が足されます。provider が請求したと
報告した額を、provider の呼び出し 1 回につき 1 回だけ足したもので、価格表から計算したものでは
ありません。

| 欄 | 意味 |
| --- | --- |
| `provider_billed_cost_usd_total` | 合計の 10 進の文字列。額を報告した呼び出しが 1 件も無いときは `null` |
| `provider_billed_cost_complete` | すべての呼び出しが額を報告したら `true`、欠けがあれば `false`、provider の呼び出しが無い run は `null` |
| `provider_billed_cost_calls` | 額を報告した呼び出しの件数 |

18 桁より細かい額は、18 桁目へ切り上げて数えるので、合計が過小になることはありません。合計が
桁あふれしたときは、すべての呼び出しが額を報告していても `provider_billed_cost_usd_total` は
`null`、`provider_billed_cost_complete` は `false` になり、合計が無いので `summary.md` に行は
書かれません。

額が 1 件以上報告されたとき、`summary.md` の provider 使用量の表の後に 1 行が足されます。例：
`Provider billed cost: $0.50154 (3/3 calls reported)`。欠けがあれば末尾に ` (incomplete)` が付きます。
額の報告が 1 件も無い run には、この行は出ません。

**Responses の run は請求額を報告しません。** そのため v2 の wallet の下では、3 つの欄は `null`・
`false`・`0` になります。これは「無料」ではなく、provider が額を送らなかったという意味です。wallet が
数えた額（使用量に単価表を掛けたもの）は別の数で、次に説明する `run_budget` に出ます。Ollama のように
額を報告しない provider でも、3 つの値は同じになります。

### summary に出る wallet と段

run budget を選んだ run は、終わるときに wallet の状態を 1 回だけ `run_budget_summary` の event に
記録します。structured stages を宣言した run も、同じように `structured_stages_summary` に記録します。
`--summary-json` はこれらの event を読み、2 つの object を足します。宣言の無い機能の object は出ません。
summary は run 自身の event だけを読み、ledger は読みません。そのため、この機能より前に記録された run
には、どちらの object もありません。数えるのは今の run だけで、最後の `run_start` から後の event です。
events の path を使い回す run に、前の run の object が出ることはありません。

これは **direct command の run だけ**が対象です（`--prompt`・`--plan-run` などの direct command）。REPL の
session には、欄も節も出ません。

| object | 欄 | 意味 |
| --- | --- | --- |
| `run_budget` | `record` | `observed` または `unavailable`（理由は下に並べます）。`unavailable` の記録に額は無く、`0` も入りません。`accounting_basis` と `stop` は残ります |
| `run_budget` | `accounting_basis` | `provider_billed`（v1）または `usage_tariff`（v2） |
| `run_budget` | `limit_usd`・`spent_usd`・`held_usd` | 費用の上限・使った額・まだ押さえている額。10 進の文字列です。v1 は確定した送信ごとの消費額、v2 は送信ごとの単価表による額を使った額として数えます |
| `run_budget` | `declared_tariff_charge_usd_total` | v2 だけ。あなたが宣言した単価表を検証済みの使用量に当てた額の合計で、**provider の請求ではありません**。v2 では `spent_usd` と同じ値です |
| `run_budget` | `sends`・`unknown`・`latch` | 状態ごとの送信の数、unknown の送信とその理由、latch とその理由。語は ledger のものです |
| `run_budget` | `stop` | `null`、またはこの command で wallet が実際に拒否したこと。上限による拒否は `kind` が `limit` で、宣言の上限の名前（`cost_usd` など）が `limit` に入ります。それ以外は `refused` です。額から推し量った停止は出ません。run が ledger の外に持っているので、`unavailable` にも残ります |
| `structured_stages` | `record`・`complete`・`ledger_intact` | `observed` なら run の終わりの、ただ 1 つの段の判定の答え。`unavailable` なら他に何もありません。後で summary を作っても、ファイルを判定し直しません |
| `structured_stages` | `stages` | target ごとの `phase`・`sealed`・`attempts`（`used`・`limit`）・`artifact`（`path`・`committed_sha256`）。この run で検査が落ちた試行があれば `failed_checks` |

何も保証できないとき、記録は `unavailable` になります。

- wallet の ledger の lock か段の lock が、run の終わりの締め切りまでに空かなかった（締め切りは wallet と
  段で共有の合わせて 1 秒。たとえば `SIGINT` の時に送信がまだ lock を持っていた）。
- lock が poisoned だった。
- wallet の ledger のファイルが、run の書いたものではなくなっていた。
- 合計が桁あふれした。
- run が段の記録を一度も開かなかった（最初の段の呼び出しと段の gate の前に終わった）、または宣言した段に
  session が bind されなかった。
- 記録した event を読み戻せない。`observed` と言っているのに中身が decode できない記録は、JSON でも
  `summary.md` でも `unavailable` として出します。

`summary.md` には、ある object ごとに短い節が、請求額の行の後に足されます。`assurance` は変わりません。
段がすべて sealed でも、ここに出るだけで `static` は上がりません。

## run なしで operator root を確かめる

`commandagent --operator-check` は、この設定で operator root の宣言を run の受理が受け付けるかを、
run 自身の判定で確かめます。provider は呼ばず、何も残しません。run に渡すのと同じフラグ
（`--profile`、`--extension-root`、`--pack`、`--provider`、`--api`、`--sandbox`、`--allow`、
`--request-pattern` など）と、同じ環境変数（`COMMANDAGENT_ADAPTER_CONFIG_ROOT`、
`COMMANDAGENT_RUN_BUDGET`）を渡してください。設定は、空のゴールの非対話の `--prompt` run として
解決し、そのように判定します。ゴールからの推定に頼らず `--profile` を渡してください。
`--json` を付けると `commandagent.operator-check/v1` の report を出します。

```bash
commandagent --operator-check --json --profile my-profile --extension-root ./ext \
  --provider openai --api responses --model my-model
```

何も作らず、書くために開かず、lock も取りません。state ディレクトリ、sandbox の一時ディレクトリ、
wallet のディレクトリ、lock ファイル、ledger、events のファイルは、どれもできません。機械に触れる
ことは 2 つです。proposal tool が宣言されていれば、隔離の probe がシステムの一時ディレクトリ
（`TMPDIR`）の下に一時領域を作り、loopback の socket を持ち、sandbox の中で子プロセスを起動します。
check はその一時領域を消し、残っていれば fail にします。probe の最中に中断すると、run 自身の probe と
同じく、空の一時領域が残ることがあります。OS sandbox は、run の開始と同じく backend を一度起動して
確かめます。

各検査は `id`、`status`（`pass`、`fail`、`unverified`、`not_declared`、`not_checked`）、
`code`、`detail` を持ちます。`code` は同じ拒否に run が使う語そのものです。profile tool の kind
（`invalid_pin`、`unavailable` など）、段の契約の `stage_contract_refused`、run budget の拒否
（`PolicyV2(UnsupportedServiceTier)`、`Wallet(Corrupt)`、`RouteNotCountable`、
`Unmetered(RequestPattern)` など）です。すべて通れば exit `0`、1 つでも `fail` か `unverified` なら
`1`、設定そのものが拒否されれば run と同じく `2` です。report は、どちらの形でも、出す前に全体を
run の secret の scrub に通します。

| 検査 | 走らせるもの |
| --- | --- |
| `run_budget.controls` | sandbox のモードとキーの変数を、run budget の制御と照らす |
| `pack` | pack の選択を、run と同じく解決する |
| `profile_tools` | 各 descriptor の decode・pin・依存・schema の採用・schema と proposal の preflight・proposal tool に要る隔離の probe |
| `isolation.probe_cleanup` | 隔離の probe の一時領域が消えたこと |
| `os_sandbox` | run が入れる OS sandbox を、入れずに probe する |
| `artifact_contract` | profile または pack が宣言する artifact contract の adapter |
| `structured_stages` | 段の契約。sandbox は probe の結果で判定する |
| `run_budget.declaration` | 宣言を、run と同じく読み decode する |
| `run_budget.ledger` | wallet のディレクトリへの道と、既にある ledger を宣言に照らす。ledger は **wallet の lock を取らずに** 読み、届かなかった場合も含め、この検査はいつもそう書きます。2 回の読みの間に変わった記録は `unverified` で、pass にはしません |
| `run_budget.route.executor` | executor の経路を wallet が数えられるか |
| `run_budget.route.planner` | いつも `not_checked`。`--prompt` の run は planner の client を作らないので、合否に入れません |
| `request_pattern` | request pattern の設定。wallet の下では `jev` の routing を `Unmetered(RequestPattern)` で拒否します |
| `secret_scope` | run の secret の scope を、その credential から作れること |

report には `pins` も出ます。選ばれた tool ごとに、`schema_sha256`、各依存、実行体、pin した各
ディレクトリ、`runtime_sha256`、descriptor 自身の digest を、`declared`（宣言値）と `calculated`
（計算値）を並べて示します。pin の食い違いで受理が止まるときも示します。合否は受理の比較のままです。
これらの値はどこにも書きません。pin の対象を確かめてから、descriptor に自分で書き写してください。

pass は「いまの宣言・設定・probe が受理される」という意味です。次のことは確かめず、report の
`not_checked_by_design` にも並べます。provider への接続と認証（キーがあるかを含む）。provider が実際に
返す model・tier・usage。実際の request body、token の上限と reservation。遅れた応答と latch。本番の
開始での sandbox の一時ディレクトリ・wallet のディレクトリと lock の作成、ledger の保存。別のプロセスが
wallet の lock や workspace の lock を持っているか。段の ledger の既存の状態。再開する session。作った
client 自身の wire に対する schema tool の preflight（check は設定された executor の wire を使います）。
確かめた後の pin の変更（run は各プロセスの前にもう一度比べます）。
