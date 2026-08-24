# F-BoN-V settlement

記録時刻: 2026-08-04T21:20:00+09:00。

## 1. 裁定

F-BoN-Vは、発行済み4検証を追い計測なしでCLOSEする。総合statusは
`issue_detected`。これはBoNの増幅機構を否定する語ではなく、系列計器事件、基準率の
予測帯事件、およびlocal補助sheetの投影差を清算に残すための語である。

BoN-1はsplit GO/NO-GOとする。earned-onlyのrun末選別、固定N、候補全保存、
Oracle@N/Selector@N/Selection gap/Diversity計上にはGO。17%の固定基準率、普遍適用、
途中刈り込み、修復接続、品質の母集団主張にはNO-GO。

## 2. 事前宣言 → 実測 → 検算

| 検証 | 事前宣言 | 実測 | 検算・裁定 |
|---|---|---|---|
| Luna確率反復 | 旧点p 17%、N=6×4、full期待1.02/窓、`>=1`期待2.69/4 | pinned restart `1,0,0,0`; pooled `4/42=9.52%` | restart分散比0.295、閾0.5未満でunderdispersed。Wilson 95% CI 3.77–22.07%。追い計測なし |
| Gemma負対照 | 旧p=0を既知とせず、0/6からJeffreys Beta-binomial帯0..2 | `0/6`、成果物6/6 unique | 帯内かつ最頻値。過去窓と合算0/12、Wilson上限24.25%。母率0の証明はしない |
| local Breakout | 2/11、Wilson 5.14–47.70%、Beta-binomial帯0..3、`P(>=1)=68.50%` | `1/6`、tree 6/6 unique、2,719秒 | 帯内、期待1.25との差-0.25。補助sheetのstatus投影差を`issue_detected`として保持 |
| 品質監査 | BoN採用full 2対単発full 2、記述的のみ | score 4/4、C3 9/9、tree 4/4 | 系統的偏りの徴候なし。ただしn=4なので不存在・同等性・有意差を主張しない |

Lunaの実用値はpool `p-hat=4/42`を点代入した場合でも
`1-(1-4/42)^6=45.15%`であり、単発9.52%から機会を増幅する方向は残る。ただしこれは
保証ではなく、Wilson端点を伝播した参考範囲も広い。増幅は採用機会の増加であって、
失敗個体をfullへ修復する機構ではない。

## 3. 除外窓と事件

- `bon0-002/003`は、同一commitでもtimestamp-bearing binary SHAが窓ごとに変わる
  計器事件のため理由付き除外。既支出`$0.5281295`は統計分母へ戻さない。
- `bon0-001r`は、別Cargo target由来のMach-O UUID/signature差をpreflight pinが
  支出・product起動前に遮断した。統計trial 0、費用`$0`。
- 決定的`build.rs`、同一commit再build version一致テスト、系列binary pin、負例により
  以後の計器床を固定した。
- 基準率事件では、2/12の点pを既知母率として狭い二項帯へ置いた誤りを収穫した。
  schema v2は基準分母、CI、Beta-binomial予測帯を必須にし、点pだけの宣言をlint拒否する。
- local 005の一次`run_stop`、summary、campaign metadataはfullで一致するが、補助
  `acceptance-sheet.md`は`status=completed`から「未完了」を表示した。事前full述語の
  本数は変えず、同sheetを独立selector入力にする前の修正要件として残す。

裁定者誤り系譜は、依頼値`3.4/4`を`2.69/4`へ直した事件1a、中間転記を
`2.692238506524...`へ直した事件1b、点p二項帯を不確実性伝播へ改めた事件2の3件を
全て保持する。事前宣言ファイルは証跡なので遡及修正していない。

## 4. SelectionとDiversity

Lunaの採用勘定は5窓30trialでOracle@6 `2/5`、Selector@6 `2/5`、機会があった
条件下のfull回収`2/2`、Selection gap 0の窓`5/5`、平均gap 0.0。treeは30/30 unique、
non-empty 29/30だった。Gemmaは6/6 unique/non-empty、localは6/6 unique/non-empty、
品質監査対象は4/4 unique/non-emptyである。成果物の分散は観測できたが、Gemmaの
0/6が示すように、分散そのものは受理境界到達を保証しない。

## 5. 費用と資源

有効なLuna pinned 4窓の記録費用は`$1.0267417`。計器事件で除外した旧2窓の
`$0.5281295`も実支出として別掲し、合計Luna実支出は`$1.5548712`である。
Gemma providerの料金はcampaign証跡へ記録されていないため推定しない。localはAPI費用
`$0`、AC給電45分19秒だが電力量計がないためkWh・電気料金を推定しない。品質監査は
保存証跡のread-only突合で、新規provider費用0。

## 6. BoN-1実装のGO/NO-GO裁定材料

GO:

- fixed Nの全候補を完走し、earned evidenceだけでrun末選別する。
- configuration別の基準分母・CI・Beta-binomial帯を事前固定する。
- 全候補を保存し、Oracle@N、Selector@N、Selection gap、Diversityを公開する。

NO-GO:

- `p=17%`、`P(>=1)=70%`を固定の製品保証や全model共通値にする。
- Gemma固執型へN増加だけを処方する。
- 途中刈り込み、repair接続、予測ranking、品質の統計主張へ進む。

再開条件は、系列pinと不確実性宣言の維持、dated model snapshotが存在する場合のpin替え、
補助acceptance sheet投影の整合、および相関・品質用の事前分母確保である。

機械可読清算は`evidence/settlement.json`へ固定した。
