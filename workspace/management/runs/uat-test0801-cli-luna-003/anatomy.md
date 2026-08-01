# uat-test0801-cli-luna-003: text protocol停止の個別解剖

実施日: 2026-08-01 (JST)

対象campaign: `cli-create-luna-20260801-093100`

## 1. 調査境界と重要な観測限界

ローカル計測workspaceを再度`bench.py scrub`に通し、`find`と`rg`で各runの
`.anvil`、console、成果物を全数検索した。scrub結果は`ok: true`, findings
0である。調査対象の歴史evidenceは変更していない。

結論からいうと、**停止を生んだLuna応答本文の原文は保存されていない**。
`provider_response`は応答メタデータとnative tool call数だけを記録し、text
protocolの本文を記録しない。`tool_call_raw`もtext/XMLの解析が成功した後の
型付き引数要約だけを記録する。この6停止では、失敗応答の
`provider_response`から`ultra_phase_failed`までに`tool_call_raw`がない。
consoleとrepair artifactにも本文はなく、parserのerror文字列だけが残る。

production経路でも、text replyは
`src/minimal_loop/loop_run.rs:1404-1405`で
`normalize_text_reply(...)?`へ渡され、失敗時は
`tool_call_raw`発行より前にreturnする。従って、本報告は存在しない原文を
復元・発明しない。以下の「原文抜粋」は、scrub済み一次資料に実在する停止
直前イベントをそのまま転記し、応答本文が非永続である箇所を明記したもの
である。この観測限界により、3件の`malformed XML tool call`は分類`c`、
2件のtrailing停止は構文上の`a`候補までを裁定上限とする。

なお、`provider_response.tool_calls=0`はnative tool callがなかったことを
示す。今回のsuiteは`tool_protocol=text`であり、本文中XML tool callの有無を
直接表す値ではない。

### 一次資料の固定値

| run | events run UUID | events.jsonl SHA-256 | console SHA-256 |
|---|---|---|---|
| `stats_luna_001` | `019fbcaa-f7ee-77d2-80aa-ca386d2b94b5` | `179d6594dc4071f6bafda821f1224ad8c003b8743d1fe6ccfcb751fbd2b176a4` | `23c9890d698c9fde13a064a04816a6e52bbc0d523c1be2b33ad441671bbd840f` |
| `stats_luna_002` | `019fbcb0-a569-7a01-822d-0a242bf1c119` | `ac5b915883055dec66707641b5b5ba4fade4e916e983184f05778be7ac8fcfbd` | `b8a4b9852ae5ed7452bef7b0cd126c5b99a7f2e6110c9c73fe7dc956e6890e1e` |
| `stats_luna_003` | `019fbcb6-2b50-70b1-bcd3-628cc843d1bf` | `314f3f2a8ca17bd7cceb83538a7799574ef59a919598adebbf99bb3895b3be62` | `ef0291656694da831e23590e88c5663cdc2bc8c4ee25e7d958c2b3b9dfc2b15b` |
| `filter_luna_001` | `019fbcbb-6029-7b31-ac50-5830052d7763` | `5802db4678a2c44fb747b0f4b7da238c794377bffbd6330dd227803abc380394` | `eb45a42b4574c75ec4a41b5db42800e6f659a0473c08d8a62e815641a20752f9` |
| `filter_luna_002` | `019fbcc0-79d7-7ac0-b1bf-1cebf60a0e05` | `149bec4c456fbdaab241eb57ad4250683b09d1adfefc6e55f144d025696a313a` | `62c74379f4f9c10171379cb53138edb22144e3ac89f7fbc7d06d148286e8a63c` |
| `filter_luna_003` | `019fbcc5-dd31-7031-8143-c9d02e447b06` | `07f1dd0b342d6066600eff249daef4a61b9e77d349dfeb2b0d157c141e61cafa` | `092784cc9c7be9890c6e3f6a83566698ba4fdbc75d5e73d2b16e899c32e65a42` |

## 2. parserと既存修復器の境界

text protocolは`src/minimal_loop/tool_protocol.rs:25-35`から
`xml_fallback::extract_tool_calls`を使う。既存の
`parse_json_relaxed` (`src/providers/xml_fallback.rs:241-260`)は、厳密JSON、
Markdown fence除去、single quote・trailing comma・bare keyの補修、括弧の
balanceを順に試す。plain tagではJSONが閉じて見える場合の閉じtag欠落も
救済する。`malformed XML tool call`は複数の抽出分岐が共有するerrorであり、
本文なしには、どの既存修復器が「惜しかった」かを個別確定できない。

trailing停止に対して検討可能な新規則は、streaming JSON parserで先頭の
JSON valueを一つだけ切り出し、そのvalueが既存のobject/name/allowed-tool/
arguments検証を全て通る場合に限り、後続を厳密なallowlist（説明文または
閉じfence）で受けるものになる。ただし今回の原文fixtureがないため、救済を
実装してよいという裁定材料にはまだ達していない。

## 3. 6停止の個別解剖

### 3.1 `stats_luna_001`

- 到達点: `setup-sample-data` phase。これ以前にLunaは
  `data/sample.csv`と`cli/main.py`への`Write`をtext protocol経由で成功。
  停止stepの不足成果物は`smoke_check.py`。
- 分類: **c（その他: raw非永続のため形状裁定不能）**。
- 既存修復器との距離: 最寄り層は`xml_fallback`だが、tag欠落、fence、
  escape、JSON修復のどれだったかを一次資料から選べない。新修復規則を
  実測原文なしに発明してはならない。

停止直前の保存原文（events 63--71、有界）:

```json
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"attempt":1,"attempt_limit":3,"event":"artifact_stagnation_feedback","last_model_action":"no_tool_missing_artifacts","missing_paths":["smoke_check.py"],"non_edit_streak":3,"schema_version":"1","target_attempt":1,"target_path":"smoke_check.py"}
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"event":"ultra_phase_failed","final_phase":false,"ok":false,"phase_id":"setup-sample-data","phase_index":1,"reason":"malformed XML tool call","schema_version":"1","stage":"execute","step_count":null,"total_phases":4}
```

2つ目の`provider_response`本文は保存されず、その直後に
`tool_call_raw`は存在しない。

### 3.2 `stats_luna_002`

- 到達点: `create-sample-data` phase。`data/sample.csv`への`Write`は成功。
  停止stepの不足成果物は`cli/main.py`, `test_cli.py`。
- 分類: **c（その他: raw非永続のため形状裁定不能）**。
- 既存修復器との距離: `stats_luna_001`と同じ。generic errorだけでは
  修復分岐を特定できない。

停止直前の保存原文（events 51--59、有界）:

```json
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"attempt":1,"attempt_limit":3,"event":"artifact_stagnation_feedback","last_model_action":"no_tool_missing_artifacts","missing_paths":["cli/main.py","test_cli.py"],"non_edit_streak":3,"schema_version":"1","target_attempt":1,"target_path":"cli/main.py"}
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"event":"ultra_phase_failed","final_phase":false,"ok":false,"phase_id":"create-sample-data","phase_index":1,"reason":"malformed XML tool call","schema_version":"1","stage":"execute","step_count":null,"total_phases":4}
```

失敗応答本文と`tool_call_raw`は存在しない。

### 3.3 `stats_luna_003`

- 到達点: 最初の`create-sample-data` phase。成果物`data/sample.csv`は未作成。
- 分類: **a候補（機械修復可能な近似形。ただしtool callとしての有効性は
  未確定）**。
- 既存修復器との距離: `parse_json_relaxed`がJSON全体を単一valueとして
  parseする境界で停止した。
- trailing判定: serde_jsonの`trailing characters at line 1 column 121`は、
  先頭に構文上完結したJSON valueがあり、その後に非空白文字があったことを
  示す。しかし原文がないため、先頭valueがobjectか、tool nameが登録済みか、
  arguments schemaを満たすかは判定不能。従って「有効なtool callだった」とは
  裁定しない。
- 救済候補: 上述のleading-value規則。ただし既存の型・語彙・引数検証を
  bypassしないことが条件。

停止直前の保存原文（events 41--49、有界）:

```json
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"attempt":1,"attempt_limit":3,"event":"artifact_stagnation_feedback","last_model_action":"no_tool_missing_artifacts","missing_paths":["data/sample.csv"],"non_edit_streak":3,"schema_version":"1","target_attempt":1,"target_path":"data/sample.csv"}
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"event":"ultra_phase_failed","final_phase":false,"ok":false,"phase_id":"create-sample-data","phase_index":1,"reason":"trailing characters at line 1 column 121","schema_version":"1","stage":"execute","step_count":null,"total_phases":4}
```

### 3.4 `filter_luna_001`

- 到達点: `create-sample-data` phaseのverify step。
  `data/sample.txt`への`Write`は成功済み。
- 分類: **b（根本的不遵守）**。parserで弾かれた形ではなく、actionが必要な
  stepで、empty/non-tool応答をfeedback後も反復した。
- 修復可能性: JSON/XML parser拡張では救えない。書式・行動の遵守か、native
  tool境界の比較対象である。

保存された時系列原文（events 49--66、有界）:

```json
{"attempt":1,"event":"empty_response_escalation","phase_scope":"create-sample-data","schema_version":"1","session_scope":"plan-run-step","stage":"nudge_1","step_kind":"verify"}
{"after_empty_responses":1,"event":"empty_response_recovered","fresh_session_retry":false,"phase_scope":"create-sample-data","schema_version":"1","session_scope":"plan-run-step","step_kind":"verify"}
{"attempt":1,"event":"empty_response_escalation","phase_scope":"create-sample-data","schema_version":"1","session_scope":"plan-run-step","stage":"nudge_1","step_kind":"verify"}
{"after_empty_responses":1,"event":"empty_response_recovered","fresh_session_retry":false,"phase_scope":"create-sample-data","schema_version":"1","session_scope":"plan-run-step","step_kind":"verify"}
{"event":"ultra_phase_failed","final_phase":false,"ok":false,"phase_id":"create-sample-data","phase_index":1,"reason":"missing tool call for action prompt after feedback","schema_version":"1","stage":"execute","step_count":null,"total_phases":3}
```

「feedback後欠落」の詳細は次のとおり。

1. 最初のexecutor応答はemptyで、実装のfeedback原文は
   `The previous assistant response was empty. Continue the task by calling the appropriate tool, or provide a concise final answer if no tool is needed.`
2. 次の応答は`empty_response_recovered`なので非空だが、`tool_call_raw`がなく、
   action未達として次のfeedbackへ進んだ。実装のfeedback原文は
   `The task appears to require workspace changes, but no Write/Edit tool call has happened yet. Create or modify the required files before final response, or explain why no file change is required.`
3. その次は再びempty。empty feedback後の最終応答は非空に回復したが、再び
   `tool_call_raw`がなく、`missing tool call for action prompt after feedback`
   で停止した。

モデルが代わりに出した**非空本文そのものは非永続**であり、散文か独自形式か
は判定不能である。保存事実から確定できるのは「空→非空だがtoolなし→空→
非空だがtoolなし」という行動列までである。

### 3.5 `filter_luna_002`

- 到達点: `setup-sample-data` phase。`data/sample.txt`への`Write`は成功。
  停止stepの不足成果物は`cli/main.py`。
- 分類: **c（その他: raw非永続のため形状裁定不能）**。
- 既存修復器との距離: generic `malformed XML tool call`のため特定不能。

停止直前の保存原文（events 51--59、有界）:

```json
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"attempt":1,"attempt_limit":3,"event":"artifact_stagnation_feedback","last_model_action":"no_tool_missing_artifacts","missing_paths":["cli/main.py"],"non_edit_streak":3,"schema_version":"1","target_attempt":1,"target_path":"cli/main.py"}
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"event":"ultra_phase_failed","final_phase":false,"ok":false,"phase_id":"setup-sample-data","phase_index":1,"reason":"malformed XML tool call","schema_version":"1","stage":"execute","step_count":null,"total_phases":4}
```

### 3.6 `filter_luna_003`

- 到達点: 最初の`implement-cli-tool` phase。成果物は未作成。
- 分類: **a候補（機械修復可能な近似形。ただしtool callとしての有効性は
  未確定）**。
- 既存修復器との距離: `parse_json_relaxed`の単一value境界。
- trailing判定: column 230より前に構文上完結したJSON valueがあったこと
  までは確定する。原文非永続のため、そのvalueが既存のtool call schemaを
  通るかは判定不能であり、「先頭が有効なtool call」とは裁定しない。
- 救済候補: `stats_luna_003`と同じleading-value規則。

停止直前の保存原文（events 27--35、有界）:

```json
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"attempt":1,"attempt_limit":3,"event":"artifact_stagnation_feedback","last_model_action":"no_tool_missing_artifacts","missing_paths":["data/sample.txt"],"non_edit_streak":3,"schema_version":"1","target_attempt":1,"target_path":"data/sample.txt"}
{"api":"chat_completions","event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
{"event":"ultra_phase_failed","final_phase":false,"ok":false,"phase_id":"implement-cli-tool","phase_index":1,"reason":"trailing characters at line 1 column 230","schema_version":"1","stage":"execute","step_count":null,"total_phases":4}
```

## 4. このコミット時点の裁定境界

本コミットは個別解剖だけを固定する。停止の既存
`process_failure / model`帰属は変更せず、修復器拡張もF-0b昇格も実施しない。
分類集計、救済見込みの上下限、台帳追記案は次コミットで追加し、その後
レビュー裁定待ちとする。
