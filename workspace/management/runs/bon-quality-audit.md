# BoN採用個体の品質監査

実施日: 2026-08-04 (JST)

## 1. 結論

BoNが採用したfull 2個体と、単発窓で得たfull 2個体を一次成果物まで戻って
突合した。4個体はすべてC1–C4がpassの同一100点ベクトルで、C3のREADME claimは
合計9/9が保存stdoutと字義一致した。成果物ツリーは4/4が相異なり、課題familyを
filterへ揃えた比較でも3/3が相異なる。

この4例には、BoN選別が低品質または同型の個体を系統的に拾った徴候はない。
ただし `n=4`（各群2）、かつ単発群の一方はstatsである。これは記述的監査であり、
母集団差、同等性、有意差を主張しない。

## 2. 対象と分母

| arm | window / run | family | C1/C2/C3/C4 | score | 秒 | 費用 |
|---|---|---|---|---:|---:|---:|
| 単発full | `luna-007 / filter_luna_001` | filter | P/P/P/P | 100 | 1,526 | $0.0693643 |
| 単発full | `luna-008 / stats_luna_002` | stats | P/P/P/P | 100 | 1,316 | $0.0438062 |
| BoN採用full | `bon0-001 / filter_bon0_005` | filter | P/P/P/P | 100 | 1,430 | $0.0390891 |
| BoN採用full | `bon0-001r2 / filter_bon0_004` | filter | P/P/P/P | 100 | 1,431 | $0.0651932 |

単発2本はユーザー指定どおりLuna-007/008のfullを全数採った。BoN側は初回
`bon0-001`の採用個体と、系列計器ピン後に新たに採用された`bon0-001r2`の個体を
全数採った。除外窓のfullは品質監査の分母へ追加していない。

## 3. スコアベクトルとSelection gap

4個体とも `cli_probe / help_binding / cli_output_claims /
cli_rerun_consistency = pass / pass / pass / pass`、score 100である。単発個体には
選別器を後付けせず、同じF-1式へ保存済みC1–C4を代入して比較ベクトルを再構成した。

BoN 2窓はいずれもOracle@6（プール内にfullあり）かつSelector@6（実選別がfullを
採用）だった。条件付きfull回収は2/2、binary full selection gapは0/2である。
到達スコアでは採用100に対する最良非採用品が62.5 / 75.0で、marginはそれぞれ
+37.5 / +25.0だった。これは2つの機会でのselector整合を示すだけで、将来の回収率を
推定する分母ではない。

## 4. 成果物差分とDiversity

`.anvil/`、検収evidence、cache、console logを除き、相対pathと各file SHA-256を
辞書順に連結して再hashした。

| run | normalized product-tree SHA-256 |
|---|---|
| `filter_luna_001` | `c1ee84ed7723719ef261997652cec36414941d85ed5f390db470d3fc328859d5` |
| `stats_luna_002` | `f1d9d3701f8d134628f7c998aafb6b99dd139309d4a8f503a6aef923b10e47a9` |
| `filter_bon0_005` | `cec60db0b81d3f8900bff46015e9aff72922f7f979c8c96e96f66471a0b44517` |
| `filter_bon0_004` | `2c4c1aa3b1423cbf541ec083e83dafeb192d7c971e3e0d9d509d729e105c0c8a` |

全体のtree diversityは4 unique / 4、同一filter familyでも3/3である。filterの
共通主要3ファイル（`README.md`、`cli/main.py`、`data/sample.txt`）も、3個体の
各対応file SHAがすべて異なる。単発filter対初回BoNは6 filesに65 insertions / 72
deletions、BoN 2採用品間は6 filesに49 insertions / 67 deletionsだった。ファイル名
構成にも個体差があり、同一成果物の再採用ではない。

## 5. C3実物

保存済み`cli-probe.json`のclaim、argv、exit、stdoutを突合した。

- 単発`filter_luna_001`: ERROR行抽出2行、WARNING count `3`、INFO count `6`の
  3/3一致。
- 単発`stats_luna_002`: price列の `Count: 3 / Sum: 300 / Average: 100`が
  1/1一致。
- BoN`filter_bon0_005`: `test`抽出3行、count `3`、warning count `1`が
  3/3一致。
- BoN`filter_bon0_004`: error抽出3行、warning count `2`が2/2一致。

全9件でexit 0、stdout truncation false、nearest missなし。各個体はnormal rerunも
exit/stdout/stderrが一致し、invalid probeはexit 2だった。検収JSONのSHAと全file
hashは`f-bon-v-001/evidence/quality-audit.json`へ保存した。

## 6. 記述的比較と裁定

BoN採用群の平均所要は1,430.5秒、単発full群は1,421.0秒。平均費用はそれぞれ
$0.05214115 / $0.05658525だった。差は個体差・family差と分離できず、統計主張を
置かない。少なくとも採用群だけにスコア劣化、C3不一致、成果物重複が現れる形は
観測されなかったため、現分母での品質監査は「系統的偏りを検出せず」とする。

この裁定は「偏りがない」の証明ではない。BoN-1以後もOracle@N、Selector@N、
Selection gap、成果物Diversityを窓ごとに記録し、分母が増えてから再評価する。
