# E2 claims-binding違反の一次資料調査

## 調査範囲

- UAT id: `uat-test0715-data-004`
- 記録先: `workspace/management/runs/uat-test0715-ff1-002/`
- Run 1: `data4_qwen35_profile_001`
- Run 4: `data4_qwen35_none_002`
- 対象: `evidence/claims-binding.json`, `output/results.json`, `output/report.md`
- 本文は一次資料の転記と機械的分類のみを記録する。修正案は含めない。

対象6ファイルは調査開始時点ですべて `artifacts/<run名>/` に退避済みだったため、再コピーしていない。各artifactと元ワークスペースのSHA-256は一致した。

| run | ファイル | SHA-256 |
|---|---|---|
| Run 1 | `evidence/claims-binding.json` | `8154d2ac6701f1a9d5d0c307c1e4b4c8b80a4514a80a48714f81c23bbf3f2c60` |
| Run 1 | `output/results.json` | `01eff12ef7edd9894028ff670648d7aee4648b92bbe4604630c26e0bbc803be6` |
| Run 1 | `output/report.md` | `983906ca327986bd7bb01116c83e8b3e35ddea767b6b9769db5b613bffad5c4c` |
| Run 4 | `evidence/claims-binding.json` | `d8ea03d3cd51d8b0a3155d9a94202360ba995828df3539c32cb8a591cb23edcb` |
| Run 4 | `output/results.json` | `24f19215726b9e48a2212ccc522c2515bbcd069d09d404c36c2f8133da1b0d79` |
| Run 4 | `output/report.md` | `e269f308bd513dcd51aba97ad9a4ef94ddfb7e5bd87a51d3c711711bb2bd4df2` |

## Evidence概要

| run | `status` | claims認識数 | `ok=true` | `ok=false` | `failure_kinds`数 |
|---|---|---:|---:|---:|---:|
| Run 1 | `failed` | 43 | 13 | 30 | 30 |
| Run 4 | `failed` | 29 | 10 | 19 | 19 |
| 合計 | — | 72 | 23 | 49 | 49 |

49件すべてで evidence の `matched_key`, `matched_result_value`, `rounded_result_value` は `null` だった。`failure_kinds` の基本種別は全件 `claims_binding_violation` である。

| failure_kind基本種別 | Run 1 | Run 4 | 合計 |
|---|---:|---:|---:|
| `claims_binding_violation` | 30 | 19 | 49 |

## 機械的分類基準

1. `report.md` の数値トークンを `[+-]?[0-9]+(?:,[0-9]{3})*(?:\.[0-9]+)?%?` で独立抽出し、`claims[].raw` と順序および多重度を照合した。
2. 数値クレームはカンマと単位を外し、全角数字があれば半角化し、`%` があれば百分率として比較し、`printed_precision` の桁で丸めた値を `results.json.values` の全数値と比較する。本資料の49違反にはカンマ・全角数字・`%` がなく、全件 `printed_precision=0`, `percent=false` だった。Run 4の除外理由4件だけは `unit="row"` だった。
3. **kind-A**: 数量を述べる文脈のクレームで、上記変換後も `.values` のどの数値とも一致しないもの。`results.json.reconciliation` に同値があっても `.values` に無ければ、指定定義に従いkind-Aとした。
4. **kind-B**: evidenceでは失敗だが、上記表記変換後に `.values` の数値と一致するもの。
5. **kind-C**: 数量クレームではないトークンの抽出、想定外構造、その他。ISO月ラベル `YYYY-MM` から別々に抽出された `YYYY` と `-MM` はkind-Cとした。
6. 判定が曖昧なものはkind-Cとする。本資料には追加の曖昧判定はなかった。

### 分類集計

| run | kind-A | kind-B | kind-C | 合計 |
|---|---:|---:|---:|---:|
| Run 1 | 6 | 0 | 24 | 30 |
| Run 4 | 7 | 0 | 12 | 19 |
| **合計** | **13** | **0** | **36** | **49** |

kind-Aのクレーム集合は次のとおりである。

- Run 1: `60`, `57`, `3`, `1`, `1`, `1`。これらは `reconciliation.input_rows=60`, `used_rows=57`, excludedの合計`3`と各`rows=1`には存在するが、`.values` 13値の範囲は `16824.0`〜`117438.0`であり、一致するキーはない。
- Run 4: `60`, `56`, `4`, `1`, `1`, `1`, `1`。これらは `reconciliation.input_rows=60`, `used_rows=56`, excludedの合計`4`と各`rows=1`には存在するが、`.values` 10値の範囲は `17324.0`〜`117938.0`であり、一致するキーはない。

kind-CはすべてISO月ラベルの分割抽出である。

- Run 1: 月ラベルが2表に6か月ずつあり、`2026` が12件、`-01`〜`-06` が各2件、計24件。
- Run 4: 月ラベルが1表に6か月あり、`2026` が6件、`-01`〜`-06` が各1件、計12件。

kind-Bは0件だった。表記変換の対照として、違反にならなかった次のクレームは evidence 上で数値一致している。

| run | claim原文 | `matched_key` | claim正規化値 | values値 | 判定 |
|---|---|---|---:|---:|---|
| Run 1 | `19990.00` | `monthly_region_2026-01_東京` | 19990.0 | 19990.0 | `ok=true` |
| Run 1 | `117438.00` | `grand_total` | 117438.0 | 117438.0 | `ok=true` |
| Run 4 | `117938.00` | `total_sales` | 117938.0 | 117938.0 | `ok=true` |
| Run 4 | `40497.00` | `regional_名古屋` | 40497.0 | 40497.0 | `ok=true` |

## Run 1 違反全件

表の「期待→実測」は `normalized_value → matched_result_value`、キーは evidence の `matched_key` である。全行でキーと実測値は原文どおり `null`。

| # | offset / report行 | claim原文と周辺文脈 | キー / 期待→実測 | failure_kind原文 | 分類 |
|---:|---|---|---|---|---|
| 1 | 54 / L4 | `60` — `- 入力行数: 60` | `null` / `60.0 → null` | `claims_binding_violation:output/report.md:54:60` | A |
| 2 | 73 / L5 | `57` — `- 使用行数: 57` | `null` / `57.0 → null` | `claims_binding_violation:output/report.md:73:57` | A |
| 3 | 92 / L6 | `3` — `- 除外行数: 3` | `null` / `3.0 → null` | `claims_binding_violation:output/report.md:92:3` | A |
| 4 | 170 / L11 | `1` — `\| invalid_date \| 1 \|` | `null` / `1.0 → null` | `claims_binding_violation:output/report.md:170:1` | A |
| 5 | 193 / L12 | `1` — `\| missing_amount \| 1 \|` | `null` / `1.0 → null` | `claims_binding_violation:output/report.md:193:1` | A |
| 6 | 214 / L13 | `1` — `\| missing_date \| 1 \|` | `null` / `1.0 → null` | `claims_binding_violation:output/report.md:214:1` | A |
| 7 | 295 / L18 | `2026` — `\| 2026-01 \| 東京 \| 19990.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:295:2026` | C |
| 8 | 299 / L18 | `-01` — `\| 2026-01 \| 東京 \| 19990.00 \|` | `null` / `-1.0 → null` | `claims_binding_violation:output/report.md:299:-01` | C |
| 9 | 327 / L19 | `2026` — `\| 2026-02 \| 大阪 \| 18657.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:327:2026` | C |
| 10 | 331 / L19 | `-02` — `\| 2026-02 \| 大阪 \| 18657.00 \|` | `null` / `-2.0 → null` | `claims_binding_violation:output/report.md:331:-02` | C |
| 11 | 359 / L20 | `2026` — `\| 2026-03 \| 名古屋 \| 20730.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:359:2026` | C |
| 12 | 363 / L20 | `-03` — `\| 2026-03 \| 名古屋 \| 20730.00 \|` | `null` / `-3.0 → null` | `claims_binding_violation:output/report.md:363:-03` | C |
| 13 | 394 / L21 | `2026` — `\| 2026-04 \| 東京 \| 16824.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:394:2026` | C |
| 14 | 398 / L21 | `-04` — `\| 2026-04 \| 東京 \| 16824.00 \|` | `null` / `-4.0 → null` | `claims_binding_violation:output/report.md:398:-04` | C |
| 15 | 426 / L22 | `2026` — `\| 2026-05 \| 大阪 \| 21470.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:426:2026` | C |
| 16 | 430 / L22 | `-05` — `\| 2026-05 \| 大阪 \| 21470.00 \|` | `null` / `-5.0 → null` | `claims_binding_violation:output/report.md:430:-05` | C |
| 17 | 458 / L23 | `2026` — `\| 2026-06 \| 名古屋 \| 19767.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:458:2026` | C |
| 18 | 462 / L23 | `-06` — `\| 2026-06 \| 名古屋 \| 19767.00 \|` | `null` / `-6.0 → null` | `claims_binding_violation:output/report.md:462:-06` | C |
| 19 | 541 / L28 | `2026` — `\| 2026-01 \| 19990.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:541:2026` | C |
| 20 | 545 / L28 | `-01` — `\| 2026-01 \| 19990.00 \|` | `null` / `-1.0 → null` | `claims_binding_violation:output/report.md:545:-01` | C |
| 21 | 564 / L29 | `2026` — `\| 2026-02 \| 18657.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:564:2026` | C |
| 22 | 568 / L29 | `-02` — `\| 2026-02 \| 18657.00 \|` | `null` / `-2.0 → null` | `claims_binding_violation:output/report.md:568:-02` | C |
| 23 | 587 / L30 | `2026` — `\| 2026-03 \| 20730.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:587:2026` | C |
| 24 | 591 / L30 | `-03` — `\| 2026-03 \| 20730.00 \|` | `null` / `-3.0 → null` | `claims_binding_violation:output/report.md:591:-03` | C |
| 25 | 610 / L31 | `2026` — `\| 2026-04 \| 16824.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:610:2026` | C |
| 26 | 614 / L31 | `-04` — `\| 2026-04 \| 16824.00 \|` | `null` / `-4.0 → null` | `claims_binding_violation:output/report.md:614:-04` | C |
| 27 | 633 / L32 | `2026` — `\| 2026-05 \| 21470.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:633:2026` | C |
| 28 | 637 / L32 | `-05` — `\| 2026-05 \| 21470.00 \|` | `null` / `-5.0 → null` | `claims_binding_violation:output/report.md:637:-05` | C |
| 29 | 656 / L33 | `2026` — `\| 2026-06 \| 19767.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:656:2026` | C |
| 30 | 660 / L33 | `-06` — `\| 2026-06 \| 19767.00 \|` | `null` / `-6.0 → null` | `claims_binding_violation:output/report.md:660:-06` | C |

## Run 4 違反全件

| # | offset / report行 | claim原文と周辺文脈 | キー / 期待→実測 | failure_kind原文 | 分類 |
|---:|---|---|---|---|---|
| 1 | 57 / L5 | `60` — `- Input rows: 60` | `null` / `60.0 → null` | `claims_binding_violation:output/report.md:57:60` | A |
| 2 | 73 / L6 | `56` — `- Used rows: 56` | `null` / `56.0 → null` | `claims_binding_violation:output/report.md:73:56` | A |
| 3 | 93 / L7 | `4` — `- Excluded rows: 4` | `null` / `4.0 → null` | `claims_binding_violation:output/report.md:93:4` | A |
| 4 | 142 / L11 | `1` — `- **invalid_date**: 1 row(s)` | `null` / `1.0 → null` | `claims_binding_violation:output/report.md:142:1` | A |
| 5 | 173 / L12 | `1` — `- **missing_amount**: 1 row(s)` | `null` / `1.0 → null` | `claims_binding_violation:output/report.md:173:1` | A |
| 6 | 202 / L13 | `1` — `- **missing_date**: 1 row(s)` | `null` / `1.0 → null` | `claims_binding_violation:output/report.md:202:1` | A |
| 7 | 238 / L14 | `1` — `- **non_positive_amount**: 1 row(s)` | `null` / `1.0 → null` | `claims_binding_violation:output/report.md:238:1` | A |
| 8 | 359 / L24 | `2026` — `\| 2026-01 \| 19990.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:359:2026` | C |
| 9 | 363 / L24 | `-01` — `\| 2026-01 \| 19990.00 \|` | `null` / `-1.0 → null` | `claims_binding_violation:output/report.md:363:-01` | C |
| 10 | 382 / L25 | `2026` — `\| 2026-02 \| 18657.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:382:2026` | C |
| 11 | 386 / L25 | `-02` — `\| 2026-02 \| 18657.00 \|` | `null` / `-2.0 → null` | `claims_binding_violation:output/report.md:386:-02` | C |
| 12 | 405 / L26 | `2026` — `\| 2026-03 \| 20730.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:405:2026` | C |
| 13 | 409 / L26 | `-03` — `\| 2026-03 \| 20730.00 \|` | `null` / `-3.0 → null` | `claims_binding_violation:output/report.md:409:-03` | C |
| 14 | 428 / L27 | `2026` — `\| 2026-04 \| 17324.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:428:2026` | C |
| 15 | 432 / L27 | `-04` — `\| 2026-04 \| 17324.00 \|` | `null` / `-4.0 → null` | `claims_binding_violation:output/report.md:432:-04` | C |
| 16 | 451 / L28 | `2026` — `\| 2026-05 \| 21470.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:451:2026` | C |
| 17 | 455 / L28 | `-05` — `\| 2026-05 \| 21470.00 \|` | `null` / `-5.0 → null` | `claims_binding_violation:output/report.md:455:-05` | C |
| 18 | 474 / L29 | `2026` — `\| 2026-06 \| 19767.00 \|` | `null` / `2026.0 → null` | `claims_binding_violation:output/report.md:474:2026` | C |
| 19 | 478 / L29 | `-06` — `\| 2026-06 \| 19767.00 \|` | `null` / `-6.0 → null` | `claims_binding_violation:output/report.md:478:-06` | C |

## claim抽出の観測

独立抽出した数値トークン列と `claims[].raw` は、両runとも順序および多重度を含め完全一致した。

| run | report内の独立数値トークン | evidenceが認識 | evidenceが未認識 | 認識内訳 |
|---|---:|---:|---:|---|
| Run 1 | 43 | 43 | 0 | 成功13、違反30（うち日付断片24） |
| Run 4 | 29 | 29 | 0 | 成功10、違反19（うち日付断片12） |
| 合計 | 72 | 72 | 0 | 成功23、違反49（うち日付断片36） |

この数値トークン定義では抽出漏れは0件だった。反対に、ISO月ラベルから分割された36トークンが数量claimとして認識され、すべて違反になっている。

## 両run間の異同

両runの違反種別は同じで、行数・除外件数が `.reconciliation` には存在するが `.values` には無いkind-Aと、ISO月ラベルを `2026` / `-MM` に分割したkind-Cだけだった。kind-Bは両runとも0件で、金額クレームはRun 1の13件、Run 4の10件がすべて数値一致でPASSした。件数差は、Run 1が月ラベルを「月次×地域別売上」と「月次合計」の2表に掲載したためkind-Cが24件、Run 4は月次表1つのため12件だったこと、およびRun 4には除外理由が1項目多くkind-Aが7件、Run 1は6件だったことに対応する。
