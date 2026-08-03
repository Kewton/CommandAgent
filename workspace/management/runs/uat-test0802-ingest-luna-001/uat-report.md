# uat-test0802-ingest-luna-001: ingest×gpt-5.6-luna計測

実施日: 2026-08-03 (JST)

## 0. 事前宣言（実行前固定）

gemma4:31b-cloudの基準線`4/6 (66.7%)`に対するLunaの事前予測は
**置かない**。CLI系列で観測したとおり、モデル×試験の相互作用は
他セルの優劣から予測できない。C3で優位だったLunaがtable族の欠落dateを
理由つきで除外できるかは、この6試行の観測だけで判定する。

- 比較対象: `uat-test0726-ingest-elev-008` (`gemma4:31b-cloud`)
- 実行対象: list×3 / table×3、計6run、retryによる標本追加なし
- executor: `gpt-5.6-luna / openai / responses / native`
- planner: `qwen3.6:27b-coding-nvfp4 / ollama`
- workspace root: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0802_ingest_luna`
- 入力とcontract: elev-008と同一のsnapshot pin・candidate ID凍結注入・`fixed v0.1`
- 主検算: table族の空ddate候補を理由つき除外できるか
- 用途: 実需要セルのモデル因子観測とBoN-1配分表のp・token費用の供給

## 1. 結論——対照が主役

**P0-a / P0-b / P0-c / P1-a / P1-bは全てPASS。Lunaはfull相当
6/6 (100%)で、gemma4:31b-cloud基準線4/6 (66.7%)に対し
+2run / +33.3 percentage pointsだった。**

差は予測せず観測したtable族に集中した。gemmaは空`date`候補を
2/3runでrecordへ採用しN2に拒否されたのに対し、Lunaは3/3runで
各々理由を記録して除外した。list族は両モデルとも3/3 full。したがって
このセルでのモデル因子の実測差は、欠落値の除外規律への従順差である。

- Full meaning label: N1–N5 pass, including source-bound record values and
  complete candidate accounting; testimony/source binding is active as N2.
- BoN-1供給値: `p̂(full)=6/6=1.0` (ただしn=6の観測値)
- F-1 reached score: `reached 6/6`、五数要約
  `100.0 / 100.0 / 100.0 / 100.0 / 100.0`
- 6run推定費用: `$0.1803377`、1run平均`$0.0300563`

| 観測 | gemma31-cloud (elev-008) | gpt-5.6-luna |
|---|---:|---:|
| full相当 | 4/6 (66.7%) | **6/6 (100%)** |
| N1 pipeline | 6/6 | 6/6 |
| N2 source binding | 4/6 | **6/6** |
| N3 candidate accounting | 6/6 | 6/6 |
| N4 format/schema | 6/6 | 6/6 |
| N5 rerun consistency | 6/6 | 6/6 |
| list full | 3/3 | 3/3 |
| table full | 1/3 | **3/3** |
| table空dateのrecord採用 | 2/3 | **0/3** |
| table空dateの理由つき除外 | 1/3 | **3/3** |
| reached score五数要約 | 70.0 / 77.5 / 100.0 / 100.0 / 100.0 | **100.0 / 100.0 / 100.0 / 100.0 / 100.0** |

## 2. Suite・preflight

- suite: `ingest-create-luna`
- suite SHA-256:
  `8258f2dc00ce9279b9c4900f1b016fcab857fcc8348029055dfb35cee3202111`
- campaign: `ingest-create-luna-20260802-235526`
- measurement revision:
  `51ba4d2a557ad6cc6dd9c82a06b1f650c7cad1d3`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gpt-5.6-luna / openai / responses / native`
- contract: `docs/ingest-profile-contract.md` (`fixed v0.1`)
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0802_ingest_luna`
- retry / interruption / 標本追加: 0 / 0 / 0

suiteのgoal bytes、list×3/table×3行列、snapshot pin、planner、context budget、
minimum ancestorはelev-008と同一。差分はexecutor/provider/API/tool protocolと
run nameのみで、この同一性はPython fixtureで拘束した。

| family | asset | SHA-256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input hash一致とzero-exit precheckは6/6。preflightはclean detached HEADから実行した。

| preflight項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `51ba4d2a Record BoN zero measurement` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 51ba4d2a 2026-08-02T23:57:54Z` |
| built / installed SHA-256 | `52ca70e15fd0fa474a5021f265ff81db4a3662e7816a17792d5b86ea7a8d118b` / 同一 |
| `NODE_ENV` | `production` |

## 3. Run行列と2軸値札

admissionはoffのため表示は`static (profile_not_admitted)`へcapされるが、
earned assuranceはN1〜N5の契約値であり6run全てfull。F-1 scoreは
封緘式`pass=1 / absent=0 / violation=-1/2`をtyped final evidenceに適用した
report/allocation用の加法値で、earnedに介入しない。

| run | family | verdict | earned | N1 | N2 | N3 | N4 | N5 | score | 秒 | 費用 |
|---|---|---|---|---|---|---|---|---|---:|---:|---:|
| `list_luna_001` | list | complete | full | pass | pass | pass | pass | pass | 100.0 | 32 | $0.0310053 |
| `list_luna_002` | list | complete | full | pass | pass | pass | pass | pass | 100.0 | 44 | $0.0391492 |
| `list_luna_003` | list | complete | full | pass | pass | pass | pass | pass | 100.0 | 25 | $0.0254559 |
| `table_luna_001` | table | complete | full | pass | pass | pass | pass | pass | 100.0 | 40 | $0.0330717 |
| `table_luna_002` | table | complete | full | pass | pass | pass | pass | pass | 100.0 | 28 | $0.0250995 |
| `table_luna_003` | table | complete | full | pass | pass | pass | pass | pass | 100.0 | 30 | $0.0265561 |

fullの6runが100であることは6/6。score分布と到達率を分離した2軸表示は
`band_summary_ingest.md`にLuna別アームとして追加し、historical
`f1-retrospective-001`は書き換えない。T2Fは未計測。

## 4. N2 / N3実物監査

### 4.1 凍結・候補勘定

snapshot structure injection、pre-run candidate ID injection、candidate freeze、
N1〜N5 evidenceは全て6/6。候補は全run 10件で、N2の216 field bindingと
N3の60 candidate参照は全て接頭辞込み正準IDの`exact`一致だった。

table 001のN3原文:

```json
{"capability_id":"ingest_candidate_accounting",
 "status":"pass","ok":true,
 "selector":{"kind":"css","value":"table > tbody > tr"},
 "detected":10,"accepted":9,
 "excluded_by_reason":{"missing required date":1},
 "equation":"10 = 9 + 1",
 "candidate_id_resolutions":[{
   "provided_id":"data/snapshots/events-table.html#0",
   "status":"exact",
   "matched_ids":["data/snapshots/events-table.html#0"],
   "resolved_id":"data/snapshots/events-table.html#0"}],
 "failure_kinds":[]}
```

3つのtable runは空date候補`#8`をそれぞれ次の理由で除外した。

| run | accepted / excluded | 除外理由 |
|---|---:|---|
| `table_luna_001` | 9 / 1 | `missing required date` |
| `table_luna_002` | 9 / 1 | `date is not a supported complete date` |
| `table_luna_003` | 9 / 1 | `missing or invalid date` |

従ってtableの空date record、N2 violation、偽fullは全て0。gemmaで2/3だった
欠落値規律の壁はLunaで0/3に下がった。

### 4.2 日本語日付正規化と両断片

list 3runは全て、候補内の`8/3(月)`と文書見出しの`2026年`を
`document_year_context`で`2026-08-03`へ正規化し、両断片のsource pathと
byte位置を3/3 relevant runで記録した。list 001のN2原文:

```json
{"record_index":1,
 "candidate_id":"data/snapshots/events-list.html#1",
 "candidate_id_resolution":{
   "provided_id":"data/snapshots/events-list.html#1",
   "status":"exact",
   "matched_ids":["data/snapshots/events-list.html#1"],
   "resolved_id":"data/snapshots/events-list.html#1"},
 "source_path":"data/snapshots/events-list.html",
 "candidate_byte_start":434,"candidate_byte_end":656,
 "field":"date","output_value":"2026-08-03",
 "declared_normalizations":["japanese_date_to_iso","document_year_context"],
 "raw_source":"8/3(月)","normalized_source":"2026-08-03",
 "transformations":["japanese_date_to_iso","document_year_context"],
 "candidate_fragment":{
   "source_path":"data/snapshots/events-list.html",
   "byte_start":533,"byte_end":541,"raw_source":"8/3(月)"},
 "document_context":{
   "source_path":"data/snapshots/events-list.html",
   "byte_start":87,"byte_end":94,"raw_source":"2026年"},
 "matched":true,"nearest_miss":null}
```

N1 pipeline実行は30 / 30 / 60 / 30 / 29 / 53 msで全6run exit 0。N4の
record countは全run 9でschema pass、N5は`output/records.json`と
`output/report.md`の再実行一致を6/6で確認した。

## 5. ドリフト探針・token・費用

55/55 executor provider turnでrequested modelとreturned modelはどちらも
`gpt-5.6-luna`、providerは`openai`、APIは`responses`、native toolsは
55/55 enabled。service tierは55/55 `default`、response IDは55個すべて一意、
failed provider turnは0。`system_fingerprint`は全turn `null`だった。
planner configは6/6一致し、profile presetのためplanner provider turnは0。

| run | turns | native tool calls | input | cached | output | reasoning | 費用 |
|---|---:|---:|---:|---:|---:|---:|---:|
| `list_luna_001` | 9 | 10 | 45,567 | 37,313 | 3,170 | 620 | $0.0310053 |
| `list_luna_002` | 14 | 14 | 79,255 | 69,602 | 3,756 | 703 | $0.0391492 |
| `list_luna_003` | 6 | 8 | 29,652 | 21,769 | 2,566 | 463 | $0.0254559 |
| `table_luna_001` | 12 | 12 | 60,387 | 51,837 | 3,223 | 724 | $0.0330717 |
| `table_luna_002` | 6 | 8 | 29,328 | 21,445 | 2,512 | 507 | $0.0250995 |
| `table_luna_003` | 8 | 9 | 38,020 | 30,491 | 2,663 | 600 | $0.0265561 |
| **合計** | **55** | **61** | **282,209** | **232,457** | **17,890** | **3,617** | **$0.1803377** |

Luna固定単価（uncached input $1.00/M、cached input $0.10/M、output
$6.00/M）を適用した。uncached inputは49,752 tokens。ingestは1run平均
47,034.8 input tokens、$0.0300563、33.2秒であり、BoN-1の配分表は
`p̂=1.0, n=6`とこの実測費用を并記できる。この小標本を予測確率へ
外挿してはいない。

| 区間 | 秒 |
|---|---:|
| preflight | 174 |
| formal run合計 | 199 |
| list / table | 101 / 98 |
| preflight start→run end | 374 |

## 6. Full verification

| check | 結果 |
|---|---|
| `cargo fmt --all -- --check` | green |
| `cargo clippy --all-targets -- -D warnings` | green |
| `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets` | **1995 passed / 0 failed / 32 ignored** |
| Python 3.12 management unittest | **99 passed / 0 failed** |
| Ruff (changed Python files, repository-root settings) | green |

Rust内訳はlib 1833 passed / 15 ignored、integration 162 passed / 17 ignored。
`src/`差分、production code差分、growth tripwire baseline変更、`.anvil/`保存は
いずれも0。

## 7. P0定型・scrub

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | completed 6、product exit 0、retry/interruption 0 |
| P0-b §assurance準拠 | **PASS** | N1〜N5 all pass→earned full 6/6、off capは表示のみ |
| P0-c 偽成功ゼロ | **PASS** | 空dateは3/3除外、N2/N3原証拠と一致 |
| P1-a 到達runでN1〜N5 | **PASS** | 各evidence 6/6 |
| P1-b sheet 6/6 | **PASS** | acceptance sheet 6/6生成 |

- per-run scrub: 6/6 green
- campaign scrub: green、finding 0
- input expected / observed / final SHA一致: 6/6
- panic / endpoint failure / 理由なし終端 / 新しいfailure class: 0

資格情報、token文字列、password、private keyをbench定型で走査し、finding 0。
repoに保存するのは本レポート、実行前宣言、集約済み
`evidence/campaign-summary.json`のみで、raw log、workspace、`.anvil/`
runtime stateは保存しない。
