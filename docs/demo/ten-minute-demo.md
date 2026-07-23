# CommandAgent 検収デモ（10分）

実在する計測証跡を順に再生する。新規の演出物・モック・合成データは使わない。

| 幕 | 話すこと | 見せる実物 | 目安 |
|---|---|---|---:|
| 1. 依頼と完成の定義 | dataのgoalと、実行前に束縛された契約チェックを示す。「合格の意味は実行前に決まっている」。 | `workspace/management/runs/c1-acceptance-sheets/full/acceptance-sheet.md` §1–3 | 1分 |
| 2. 無人実行と検収 | full形のE2対照表を示す。「数字は全て機械が検算済みで、読む人の再計算は不要」。 | 同 `full/acceptance-sheet.md` §4 | 2分 |
| 3. わざと失敗させる | failed形の平易な停止理由とrecovery YAMLを示す。「謝罪ではなく回収情報が出る」。 | `workspace/management/runs/c1-acceptance-sheets/failed/acceptance-sheet.md` §5 | 1.5分 |
| 4. 調査したフリの拒否 | 実計測の診断束縛で、`diagnosis_unbound` と引用×出力不在が落ちることを示す。「嘘の報告書はここで落ちる」。 | `workspace/management/runs/uat-test0722-circle-elev-003/` の実証レポート | 1分 |
| 5. 円環 | 起点失敗→I2 5/5→3辺のearned確認→F1–F3→verify_origin→circle_full、所要18秒を上から読む。 | `workspace/management/runs/c1-acceptance-sheets/circle/acceptance-sheet.md` §円環時系列 | 2.5分 |
| 6. 値札と正直さ | bandの低い値（時系列0%を含む）も隠さない。「できないことを、できないと機械が言う」。 | `workspace/management/runs/band_summary_circle.md`、`workspace/management/runs/band_summary_fix.md` | 1.5分 |

## 一次資料台帳

- `workspace/management/runs/c1-acceptance-sheets/full/acceptance-sheet.md`
- `workspace/management/runs/c1-acceptance-sheets/failed/acceptance-sheet.md`
- `workspace/management/runs/c1-acceptance-sheets/circle/acceptance-sheet.md`
- `workspace/management/runs/uat-test0722-circle-elev-003/`
- `workspace/management/runs/uat-test0722-circle-elev-008/run1/`
- `workspace/management/runs/band_summary_circle.md`
- `workspace/management/runs/band_summary_fix.md`
