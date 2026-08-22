# モデル動作プローブ

[English](../model-probe.md) | [ガイド目次](../README.md)

`commandagent --model-probe` と TUI の `/model-probe` は、設定済みの executor、
planner、classifier を対象に、有界な dialect probe を実行します。classifier は選択した
preset で設定します。probe は計測専用で、結果から runtime 設定を自動変更しません。
出力は人間が確認する JSON profile と Markdown card です。

throwaway workspace は system temp directory の下に作成され、終了後に削除されます。
package install は実行せず、各 task は通常の model/tool chokepoint を通る有界 session です。
固定 battery の version は `model-probe-v3` です。

## Battery

1. `write_simple`: 5 行の関数を `src/util/math.ts` に作成し、path 形式を測る。
2. `write_deep`: 5 階層下の長い filename を作成し、path 復元を測る。
3. `edit_provided`: prompt に exact content を示した file を編集し、anchor copy を測る。
4. `edit_own`: task 1 の file を同じ session で編集し、memory anchor を測る。
5. `verify_exist`: file 存在を確認し、Bash command shape を測る。
6. `verify_json`: `package.json` の build script を確認し、JSON verify dialect を測る。
7. `repair_appended`: 約 2k token の先行 context の後で 1 行 compile error を修復する。
8. `repair_compact`: fresh session で同じ error を修復し、context sensitivity を測る。
9. `regenerate`: `Write` で正しい file 全体を再生成する。
10. `csv_fixture_verify`: CSV fixture を作成して local program で検証し、redirect／heredoc
    の混在傾向を測る。
11. `json_schema`: StepPlan 風 schema の JSON だけを返し、planner の parse と不足 field を測る。
12. `classifier_closed`: 固定 closed list から一致する route を選び、exact 3 key を返す。
    設定済み classifier model と本番の `think=false`／生成上限を使います。

JSON profile は raw tool call と raw Bash command をそのまま記録します。

## Metrics

- executor、planner、classifier ごとの固定 probe band（`complete`、`partial`、`failed`）、
  provider 所要時間、turn 数、latency、token telemetry
- `absolute_path_rate`、`corrupted_path_count`
- `shell_control_rate` と `&&`、`;`、pipe、redirect、`cd` の内訳
- `edit_anchor`、appended/compact repair、full-file regeneration の follow-through
- `json_valid_rate`、`missing_field_kinds`、`classifier_valid_rate`
- empty response、malformed tool call、context truncation の signal
- call latency と provider token telemetry

band はこの固定 micro-task の完了状況だけを示し、本番能力 tier ではありません。
`model-probe-v1` は CSV task がなく N=10、v2 は classifier と役割別 table がなく N=11 です。
旧 card は宣言済み範囲では有効ですが、v3 の classifier evidence や N=12 band の代用にはできません。

## 役割別の計測手順

classifier が意図せず planner を継承しないよう、complete preset で 3 役割を明示します。
exact model ID、digest、provider、context budget、thinking、tool protocol、build、host を
arm 間で固定してください。

```toml
[preset.role_pair_probe]
model = "<executor-model-id>"
provider = "ollama"
api = "chat_completions"
tool_protocol = "native"
planner_model = "<planner-model-id>"
planner_provider = "ollama"
planner_think = "false"
classifier_model = "<classifier-model-id>"
classifier_provider = "ollama"
context_budget = 65536
chat_timeout_secs = 600
profile = "generic"
narration = "quiet"
footer = "off"
stream = "off"
prompt_layout = "legacy"
plan_preset = "none"
```

各 arm を最低 2 回実行し、split model の初回所要時間を解釈する前に model residency を
確認します。

```bash
commandagent --preset role_pair_probe --model-probe
ollama ps
commandagent --preset role_pair_probe --model-probe
```

同一 model の baseline から 1 役割ずつ変更し、aggregate ではなく役割別 duration を比較します。
executor task 数や retry の変動が対象役割を隠すためです。小さい model が自動的に速いとは限らず、
micro-probe の `complete` は smoke や full scenario acceptance の代わりになりません。

## 現在のローカル実測推奨

2026-08-22 の実測で推奨できる開始構成は、executor と planner に
`qwen3.8:27b-mlx`、classifier だけに `qwen3.5:4b` を使う組です。独立した 4B
classifier は対象 4 run で 4/4 完了し、最終 hybrid では 176〜304 ms でした。

小型 planner は推奨しません。warm 9B planner は 27B baseline より遅く、4B planner は
JSON 契約が 1/2 でした。詳細は
[exact digest、duration、checksum を含む実測記録](../model-probe-results/2026-08-22-local-role-pairs.md)
を参照してください。この構成は
local probe/smoke の開始点であり、built-in default や universal tier ではありません。

## 新しい model の手順

scenario UAT の前に次の順序を守ります。

1. 対象 3 役割で `commandagent --model-probe` または `/model-probe` を実行し、card を確認する。
2. CLI task 1 本と TOOL task 1 本の smoke を実行する。
3. 事前に landing criteria を commit して full scenario round を実行する。
4. probe profile を引用して tier table に追加する。

model version または digest が変わったら再実行してください。identity を pin できない cloud model は
measurement campaign ごとに再実行します。
