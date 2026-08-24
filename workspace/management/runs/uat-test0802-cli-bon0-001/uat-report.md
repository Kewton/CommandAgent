# uat-test0802-cli-bon0-001: BoN-0初計測

実施日: 2026-08-03 (JST)

## 0. 事前宣言（実行前固定）

記載時刻: 2026-08-03 00:28:21 JST (`+0900`)

Luna-007/008の同一Responses/native世代で観測した単発fullは`2/12`である。
この実測値を独立試行の基準率 `p̂=1/6` とすると、6試行で1件以上fullを得る
事前検算は次のとおり。

```text
P(>=1 full in 6) = 1 - (1 - 2/12)^6
                  = 1 - (5/6)^6
                  = 0.665102... ≈ 66.5%（約66%）
```

この確率検算がBoN-0を制度計測する理由である。今回の6runがこの見積りから外れても
補正、刈り込み、予測、追加試行は行わず、その外れ自体を実測として保存する。

事前に固定する境界:

- configuration: `bon:6`
- suite: `cli-filter-bon0`、単一goal `filter`のexact bytesを6回反復
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0802_bon0`
- 各runは別workspace。時分割実行を許容し、product terminalの再実行はしない
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gpt-5.6-luna` / `openai`
- endpoint: `responses`、tool protocol: `native`
- Luna-008 baseline suite SHA-256:
  `e4c04d2374aa2c0d45eaeb6cdc51a2ea3fd0692533fd66070e0df505b01da895`
- filter goal SHA-256:
  `87977bbb84c010dc158eba0246c3f04794899f65303c3585292070d9b2ef4070`
- BoN suite SHA-256:
  `209843c8bab4fcfdc028b3220383694c50b0b5e46360c284e4a83a8586cbf500`
- 実行revision: `1c64d87f`（第1コミット）
- pack pin: Luna-008同様に未指定。absenceを6runで同一性確認する

選別規則は実行前に封緘した`commandagent.bon-selection/v0`だけを使う。
earned fullを最優先し、複数fullは到達スコア降順、所要昇順、費用昇順、run名昇順。
全滅時は同じ順序で最高到達スコア個体を「最有望敗者」として特定するだけで、
v0から修復へは接続しない。非選別5runの証拠も削除せず較正資産として保持する。

## 1. 実行結果

campaign `cli-filter-bon0-20260802-153030`を、事前宣言後にrevision
`1c64d87f4904005136863a0861b50a5b8120c6eb`のclean detached worktreeから
順次実行した。preflightは`cargo test`、release build、install、version確認を全て
greenとし、built/installed binary SHA-256は一致した。6runは追加・打切り・再投入
なしで完走した。

- earned full: **1/6**
- 選別: **`filter_bon0_005`をadopted fullとして採用**
- non-full: 5/6
- reached score: 6/6
- prediction / pruning / repair connection: **false / false / false**
- measurement validity: **valid**（同一性不一致0件）

唯一のfullなのでタイブレークは発火しなかった。選別器はpersist済み`run_stop`の
`ok=true`、`final_acceptance_status=full_success`、`assurance_level=full`を全て満たす
個体だけをearnedとした。`filter_bon0_005`はこの3条件を満たし、score 100だった。

## 2. 個体別到達スコアベクトルと選別

封緘済みF-1式（pass = `+w`、absent = `0`、violation = `-w/2`）を、重み1の
CLI 4原子へ適用した。表の`P`はpass、`V`はviolationを表す。

| run | cli_probe | help_binding | cli_output_claims | rerun | score | full | selected | 秒 | 費用 |
|---|---|---|---|---|---:|---|---|---:|---:|
| `filter_bon0_001` | P | P | V | P | 62.5 | no | no | 1,539 | $0.0567084 |
| `filter_bon0_002` | V | V | P | P | 25.0 | no | no | 833 | $0.0418348 |
| `filter_bon0_003` | V | V | P | P | 25.0 | no | no | 1,352 | $0.0577423 |
| `filter_bon0_004` | V | P | P | P | 62.5 | no | no | 1,308 | $0.0502942 |
| `filter_bon0_005` | P | P | P | P | **100.0** | **yes** | **yes** | 1,430 | $0.0390891 |
| `filter_bon0_006` | V | P | V | P | 25.0 | no | no | 1,329 | $0.0291967 |

到達スコア五数要約は **min 25.0 / Q1 25.0 / median 43.8 / Q3 62.5 /
max 100.0**。full個体は1件で、その1件が100であることを全数検算した。
non-fullに100を与えた個体もない。

## 3. 同一性表

| run | binary SHA-256 | input-pin SHA-256 | requested / returned | tier | fingerprint | pack pin | equal |
|---|---|---|---|---|---|---|---|
| `filter_bon0_001` | `5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a` | `17ea4c1c82797c44434dac6681436d47d58e77bcbfd1c0b90b75e6f263fd843c` | Luna / Luna | default | null | absent | yes |
| `filter_bon0_002` | `5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a` | `17ea4c1c82797c44434dac6681436d47d58e77bcbfd1c0b90b75e6f263fd843c` | Luna / Luna | default | null | absent | yes |
| `filter_bon0_003` | `5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a` | `17ea4c1c82797c44434dac6681436d47d58e77bcbfd1c0b90b75e6f263fd843c` | Luna / Luna | default | null | absent | yes |
| `filter_bon0_004` | `5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a` | `17ea4c1c82797c44434dac6681436d47d58e77bcbfd1c0b90b75e6f263fd843c` | Luna / Luna | default | null | absent | yes |
| `filter_bon0_005` | `5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a` | `17ea4c1c82797c44434dac6681436d47d58e77bcbfd1c0b90b75e6f263fd843c` | Luna / Luna | default | null | absent | yes |
| `filter_bon0_006` | `5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a` | `17ea4c1c82797c44434dac6681436d47d58e77bcbfd1c0b90b75e6f263fd843c` | Luna / Luna | default | null | absent | yes |

`Luna / Luna`は全101 provider turnsでrequested/returned modelがともに
`gpt-5.6-luna`だったことを表す。service tierは全turn `default`、providerが
system fingerprintを返さなかったため全turn `null`である。pack pinはbaselineと
BoN suiteの双方で未指定であり、absenceが6/6一致した。入力pinはprofile、intent、
planner/executor、endpoint、tool protocol、context budget、workspace mode、goal exact
bytesから生成した。binary、入力pin、model meta、pack pinの不一致は0件だった。

## 4. 敗者evidence保持と較正collector

| loser | acceptance sheet | event files | preserved evidence files |
|---|---|---:|---:|
| `filter_bon0_001` | yes | 1 | 22 |
| `filter_bon0_002` | yes | 1 | 25 |
| `filter_bon0_003` | yes | 1 | 29 |
| `filter_bon0_004` | yes | 1 | 24 |
| `filter_bon0_006` | yes | 1 | 22 |

非選別5runは5/5とも削除せず、計122 evidence filesと各event streamをcampaignに
保持した。既存`calibration_corpus.py` collectorはcampaign report生成時に走査済み。
今回そのschemaが収集するC2/C3 `nearest_miss`候補はなく、較正records追加は**0件**
だった。これは敗者evidenceの削除を意味しない。raw evidence保持は5/5、specialized
nearest-miss corpusへの追加は0、と別々に記録する。

## 5. Responses/native使用量、費用、所要

- provider turns: 101
- native tool calls: 114
- input: 667,741 tokens（cached 592,395、uncached 75,346）
- output: 23,380 tokens（reasoning 5,767、output内数）
- 固定単価: uncached input $1.00/M、cached input $0.10/M、output $6.00/M
- 総費用: **$0.2748655**
- run所要合計: **7,791秒（2時間9分51秒）**
- run開始epoch: `1785684813`
- run終了epoch: `1785692604`

所要合計は6 workspaceの逐次実行時間の和で、並列短縮値ではない。費用は各
provider turnのreturned usageだけから加算し、推定turnや失敗個体を除外していない。

## 6. 事前宣言との検算

事前宣言は単発`p̂=2/12=1/6`から
`P(>=1 full in 6)=0.665102...`（約66.5%）と置いた。実測は**1件以上full = yes**、
full個体数1、採用個体`filter_bon0_005`だったため、この二値検算は実現した。
ただし1 campaignだけで独立性や母比率を再推定したとは読まない。事前値は選別へ
使わず、run追加、成功後の早期打切り、予測順位づけはいずれも0件だった。

## 7. band更新と既存値不変

`band_summary_cli.md`には単発bandと分離した`bon:6`行だけを加法更新した。

| configuration | full | selected | reached | five-number | seconds | cost | identity |
|---|---:|---|---|---|---:|---:|---|
| `bon:6` | 1 | `filter_bon0_005` | 6/6 | 25.0 / 25.0 / 43.8 / 62.5 / 100.0 | 7,791 | $0.274865 | matched |

BoN節より前の既存band bytesはSHA-256
`7adfb4d248466ca02b464150b6a45cf93004403edc4ee4a1aa34dfa13baf4657`のまま。
BoN値をLuna単発`2/48`や各窓`1/6`へ混入していない。

## 8. scrubと一次資料

- run別scrub: 6/6 green、findings 0
- campaign全体scrub再実行: green、findings 0
- `OPENAI_API_KEY`実値のcampaign・保存report・台帳・band完全一致scan: 0件
- selector validity: true、invalid reasons 0
- selector schema: `commandagent.bon-selection/v0`
- pre-execution report SHA-256:
  `eebcd2c951a276321ac5be7ce63e5776feaac277b778fc80df1a500385c7b425`
- suite SHA-256:
  `209843c8bab4fcfdc028b3220383694c50b0b5e46360c284e4a83a8586cbf500`
- selection evidence SHA-256:
  `6c99fad22f8ba429725ef0a9be796d86813495997b5eca2af15aa15077da3637`

機械可読な全個体表、同一性表、score vector、選別結果、費用、保持数は
`evidence/bon-selection.json`に保存した。raw log、workspace、`.anvil/`はrepoへ
転記せず、campaign原本をread-only一次資料として保持する。

## 9. 結論

BoN-0は第4因子（試行数）を初めて制度計測し、単一goalの6独立試行から1件の
earned fullを取得して採用した。採用は実測終端だけに基づき、予測ゼロ・刈り込み
ゼロ・修復接続ゼロ。5敗者も較正資産として保持した。`ρ=0.063`に基づくBoN-2
ゲートは維持し、本結果から先読み選別や途中刈り込みを解錠しない。
