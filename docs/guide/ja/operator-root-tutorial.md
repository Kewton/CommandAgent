# 2 段の拡張を作る

[English](../en/operator-root-tutorial.md) | [ガイド目次](../README.md)

この頁は [`examples/operator-root/`](../../../examples/operator-root/README.md) の
見本を順にたどります。見本は、2 つの structured stage（段）を持つプロファイルと、
段ごとに operator root で pin（固定）する 1 つの process と、provider への呼び出しを
すべて数える run budget から成ります。偽のモデルを相手に動かすので、鍵も本物の
provider も要りません。最後まで読むと、同じ形で自分の 2 段を作るのに要るものが揃います。

ここで使う各ファイルの参照は
[operator root・profile tools・run budget](profile-tools.md) にあります。この頁は
その表を繰り返さず、それらが一緒に動くところを示します。

## 始める前に

| 要るもの | 理由 |
| --- | --- |
| macOS | pin した process は、probe が通った macOS の Seatbelt sandbox の下でだけ動きます。それ以外では、run の開始前に tool が `unavailable` として拒否されます |
| `PATH` 上の `commandagent` | 以下のすべてのコマンドがこれを実行します |
| `/bin/bash` | 見本の tool が pin する唯一の実行体です。tool は bash の組み込み機能だけを使います |
| `python3` | 予行演習の偽のモデル `rehearse/fake_model.py`（標準ライブラリだけ）のためだけに要ります。tool は使いません |

見本が玩具のデータで行うこと:

- 段 `parse` は `input/items.txt`（`name,quantity` の行）を読み、`work/items.json` を
  提案します。
- 段 `check` は `work/items.json` を読み、名前が重複しないことと数量が 0 以上であることを
  確かめ、最終の成果物 1 つ、`out/report.json` を `{"items":<件数>,"total":<合計>}` として
  提案します。

## 1. root を配置する

pin するパスはすべて絶対パスで正規形でなければならないので、見本は `@ROOT@` と書いた
雛形と、それを埋める script を同梱しています。repository の checkout から、新しい
ディレクトリを渡して実行します:

```bash
bash examples/operator-root/setup.sh "$HOME/two-stage-example"
ROOT=$(cd "$HOME/two-stage-example" && pwd -P)
```

`setup.sh` は 5 つのディレクトリを複製し、雛形の `@ROOT@` を正規形のパスに置き換え、
group と other の書き込み権を外します。他人が書き込める tool のディレクトリを host が
拒否するためです。

| `$ROOT` の下のパス | 入っているもの | 持ち主 |
| --- | --- | --- |
| `extension/profiles/two-stage-example/manifest.toml` | プロファイル: 2 つの tool、2 つの段、必須の成果物 | 拡張の作者 |
| `operator/profile-tools/example-parse.json`、`example-check.json` | tool 1 つにつき descriptor 1 つ: 何を動かすか、その pin、段の下限 | operator |
| `operator/run-budgets/example-wallet.json` | v1 の run budget | operator |
| `tools/lib.sh`、`parse.sh`、`check.sh` | 2 つの process と共有の補助関数 | operator |
| `workspace/` | ワークスペース: `input/items.txt` と preset を書いた `.commandagent/config.toml` | run の利用者 |
| `rehearse/fake_model.py`、`replies.json` | 偽のモデルとその台本 | 予行演習のためだけ |
| `ledgers/` | run budget の台帳。最初の run が作ります | host |

operator root（`$ROOT/operator`）と tool はワークスペースの外にあります。そうでなければ
なりません。モデル・プロファイル・ワークスペースのファイルはこれらを変えられません。

## 2. manifest を読む

プロファイルは tool を中身の見えない ID で宣言し、段として並べます。実行体・パス・hash は
書きません。

```toml
[profile_tools]
schema_version = "profile-tool.declaration/v2"
builtin_tools = ["Read"]
tools = [
  { id = "example-parse", effect = "propose-artifacts" },
  { id = "example-check", effect = "propose-artifacts" },
]

[profile_tools.stages]
stages = [
  { id = "parse", targets = [{ id = "main", tool_id = "example-parse" }] },
  { id = "check", targets = [{ id = "main", tool_id = "example-check" }] },
]
```

- `builtin_tools = ["Read"]` により、モデルには `Write`・`Edit`・`Bash` がありません。
  ワークスペースを変える方法は段の tool を呼ぶことだけです。
- `parse` から `check` への辺は descriptor から決まります。`example-check` は
  `example-parse` の出力である `work/items.json` を読みます。
- `[artifacts] required = ["out/report.json"]` が最終の成果物を名指しします。

宣言の各欄は [structured stages](profile-tools.md#structured-stages) で説明しています。

## 3. descriptor を読む

`$ROOT/operator/profile-tools/example-parse.json` を開きます。大事なところ:

```json
"schema": {
  "protocol": "profile-tool.schema/v1",
  "root": {
    "type": "object",
    "additionalProperties": false,
    "required": ["expected_revision"],
    "properties": {"expected_revision": {"type": "string"}}
  },
  "inputs": ["input/items.txt"]
},
"process": {
  "executable": {"path": "/bin/bash", "sha256": "0000…"},
  "dependency_directories": [{"path": "<ROOT>/tools", "sha256": "0000…"}],
  "argv": ["<ROOT>/tools/parse.sh"],
  "cwd": "<ROOT>/tools",
  "env": {"LC_ALL": "C"},
  "outputs": [{"id": "items", "path": "work/items.json"}]
},
"stage": {"required_checks": ["lines"], "attempts_per_target": 3, "attempts_per_run": 8, "deadline_seconds": 600}
```

- `schema.inputs` は、host が読んで process に渡すワークスペースのファイルを並べます。
  process 自身はワークスペースのパスを開きません。
- schema は必須の文字列 `expected_revision` を 1 つ取ります。段の tool には必須で、
  host はこれを process への問い合わせから外します。
- `schema_sha256` は本物の値で同梱しています。schema はどの機械でも同じだからです。
  実行体の pin とディレクトリの pin は機械ごとに違うので 0 で同梱し、手順 5 で写します。
- `outputs` はちょうど 1 つです。その `path` は operator が決め、process には見えません。
- `stage` は operator の下限です。これが無いと段の tool は拒否されます。

## 4. 段の process を書く

host は `/bin/bash <ROOT>/tools/parse.sh` を起動し、標準入力に要求を 1 行書き、標準出力から
結果の文書を 1 つ読みます。両方の欄は
[process の封筒](profile-tools.md#process-の封筒) に並べています。

### 要求

compact な JSON 1 行です。見本は、繰り返さなければならない値と 1 つの入力を読みます:

```bash
source "${BASH_SOURCE[0]%/*}/lib.sh"
read_request || exit 3          # REQ_ID, REQ_TOOL, REQ_RUNTIME, REQ_REVISION, REQ_SCHEMA, REQ_OUTPUT
b64_decode "$REQ_INPUT" || exit 4   # the input's bytes, as integers in BYTES
bytes_to_text                       # the same bytes as TEXT
```

### 結果

JSON の文書 1 つです。束縛の欄を繰り返し、検査を並べ、候補の SHA-256 と正規形の base64 を
運びます:

```bash
add_check lines "$status"         # passed or failed
emit_result "{\"items\":[$items]}"$'\n'
```

- 通らなかった検査は **exit 0** で返します。モデルに伝わり（`status: resubmit`、
  `code: check_failed`、あなたの diagnostics つき）、tool をもう一度呼べます。0 以外の
  exit は run を終わらせます。
- host がバイトを採用するのは、すべての検査が `passed` で、`stage.required_checks` の
  検査がすべて含まれているときだけです。
- process が言うことはデータです。exit 0 や結果の中の文は証拠になりません。host が
  digest とバイトを自分で突き合わせます。

### sandbox の中で bash ができること

process は `deny default` の方針の下で動きます。fork、pin した実行体以外の exec、
ファイルの書き込み、どこかへの接続はできません。読めるのは実行体、その依存ディレクトリ、
システムのライブラリだけです。だから見本の tool は bash の組み込み機能だけを使います:

| してよい | してはいけない |
| --- | --- |
| `[[ =~ ]]`、`${var#…}`、`printf -v`、算術、配列、依存ディレクトリの中のファイルの `source` | `$(…)` やバッククォート（fork します）、パイプ、`<<<`（一時ファイルを書きます）、`sed` や `sha256sum` のような外部プログラム |

`tools/lib.sh` が base64 と SHA-256 を bash の算術で計算するのはこのためです。そのまま
写して使ってください。repository が参照実装と突き合わせて試験しています。

## 5. root を確かめて pin を写す

環境を一度設定し、宣言したものを run が受け入れるかを host に尋ねます。確認は provider を
呼ばず、何も書きません。

```bash
export COMMANDAGENT_ADAPTER_CONFIG_ROOT="$ROOT/operator"
export COMMANDAGENT_RUN_BUDGET=example-wallet
export COMMANDAGENT_COMPATIBLE_REASONING_EFFORT=low
FLAGS=(--cwd "$ROOT/workspace" --extension-root "$ROOT/extension"
  --profile two-stage-example --preset example
  --base-url http://127.0.0.1:8765/v1 --allow read,write)
commandagent --operator-check --json "${FLAGS[@]}"
```

最初の確認は `1` で終わります。pin が 0 なので `profile_tools` が `invalid_pin` で
失敗します。報告はすべての pin を `declared` と `calculated` で並べます:

```json
{"tool_id": "example-parse", "pin": "executable", "path": "/bin/bash",
 "declared": "0000000000000000000000000000000000000000000000000000000000000000",
 "calculated": "<64 hex digits>"}
```

tool ごとに、`executable` の `calculated` の値を `process.executable.sha256` へ、
`dependency_directory` の値を `process.dependency_directories[0].sha256` へ写します。
写す前に、それぞれの値が何を pin するかを確かめてください。代わりに書くものはありません。
もう一度確認すると、`status: pass` で `0` で終わります。

- `schema_sha256` は最初から一致しています。
- 後で `tools/` の下のファイルを直すと、ディレクトリの pin が変わります。確認をやり直し、
  新しい値を写してください。

## 6. 偽のモデルで予行演習する

別の端末で偽のモデルを起動します。要求ごとに `replies.json` の次の返事を返し、合成した
請求額を報告します:

```bash
python3 -I "$ROOT/rehearse/fake_model.py" --replies "$ROOT/rehearse/replies.json" \
  --port 8765 --record "$ROOT/requests.jsonl"
```

そのうえで、同じ環境とフラグで prompt を実行します:

```bash
commandagent "${FLAGS[@]}" --summary-json --prompt "Produce out/report.json."
cat "$ROOT/workspace/out/report.json"
```

run は `0` で終わり、`out/report.json` は `{"items":3,"total":15}` になります。

- 台本は `ExampleParse` を呼び、次に host が名指しした revision で `ExampleCheck` を呼び、
  最後に文で答えます。`replies.json` の `{revision}` は host の最後の答えから埋まり、
  `stale` の答えには、それが名指しする revision で同じ呼び出しを繰り返します。
- `requests.jsonl` には偽のモデルが受けた接続と要求がすべて入ります。

preset に `openrouter_provider` がある理由: v1 の run budget が数えるのは汎用の
OpenAI-compatible Chat Completions の経路だけです。provider が応答ごとに請求額を報告する
唯一の経路だからです。そして wallet は、要求が wallet の宣言する routing の表を名指しして
いるときだけ送信を認めます。`openrouter_provider` はその表のための CommandAgent の設定の
鍵で、値の `example-upstream` は架空のものです。`--operator-check` は要求の本文を
見ないので、表が無い、または違っていても確認は通ります。そのとき run は最初の送信で
`the run budget refused the send: Wallet(Budget(RouteNotAdmitted))` で止まります。表は
wallet の `upstream_only` と `allow_fallbacks` に、preset の `model` は wallet の
`route.model` に、`COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` は wallet の `route.effort` に
合わせてください。

### summary を読む

`--summary-json` は最後に JSON を 1 行出します。host が何を決めたかは 2 つの object に
出ます:

| 欄 | 期待する値 |
| --- | --- |
| `structured_stages.complete` | `true` |
| `structured_stages.stages[].sealed` | `parse` と `check` の両方で `true`。それぞれにそのファイルの `committed_sha256` |
| `run_budget.record` | `observed` |
| `run_budget.stop` | `null` |
| `run_budget.sends.settled` | 偽のモデルが答えた要求の数。wallet の生涯を通して数えます |

wallet は生涯を通して 1 つの wallet です。送信・金額・経過時間は最初の送信から run を
またいで数えます。上限に達したら、`setup.sh` で新しい複製を配置するか、新しい budget ID を
宣言してください。既存の wallet の宣言を直したものは拒否されます。

## 7. 拒否される pin

pin した script に 1 バイト足して、もう一度確かめます:

```bash
printf '\n' >> "$ROOT/tools/check.sh"
commandagent --operator-check --json "${FLAGS[@]}"
commandagent "${FLAGS[@]}" --prompt "Produce out/report.json."
```

確認は `profile_tools` が `invalid_pin` で失敗して `1` で終わります。run は開始の前に
`error: profile_tool_refused: invalid_pin: …` で `2` で終わり、`requests.jsonl` には
1 行も増えません。戻すには、何が変わったかを確かめてから、手順 5 のように新しい
ディレクトリの pin を写します。

## 8. 自分の 2 段に合わせる

1. `setup.sh` で見本を新しいディレクトリに複製し、プロファイルのディレクトリと
   `[metadata] id` の名前を変えます。
2. manifest で自分の tool と段に ID を付け、`[artifacts] required` を最終のファイルに
   します。
3. tool ごとに descriptor を書きます: `tool_id` と `name`、読む `schema.inputs`（段 2 は
   段 1 の出力を読む）、`outputs` 1 つ、自分の script を名指しする `argv`、そして
   script が報告する検査の ID を `required_checks` に持つ `stage` の下限。`schema` を
   変えたら `schema_sha256` を計算し直します。これは `schema` を key の順に並べた compact な
   JSON の SHA-256 で、`--operator-check` が `calculated` の値を示します。
4. `lib.sh` を使って各 script を書きます: 要求を読み、入力を復号し、検査を決め、出力の
   文字列で `emit_result` を呼びます。
5. `replies.json` を自分の tool の名前に向け、手順 5 と 6 を実行します。

この版の制約を覚えておいてください: macOS だけ、tool 1 つにつき出力 1 つ、そして偽の
モデルでの予行演習には v1 の run budget だけ（v2 の wallet が数えるのは OpenAI Responses を
直接呼ぶ送信だけです）。
