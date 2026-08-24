# FF-1 historical full audit

監査日: 2026-07-14  
対象窓: `test0711_bs_003` 以降、`docs/generality.md` の Measured capability bands に算入された78 run。

走査対象は `/Users/maenokota/share/work/localwork/commandagent_mvp/01/` および同配下の `test0711_*`〜`test0713_*` UATアーカイブ。履歴アーカイブは読み取り専用で扱った。`scan_full_interaction_contract.py` の判定規則（`probe_mode=contract`、primary hook、非空state dimensions）で、full_successを報告した最終受理イベントを照合した。

## 集計

| 分類 | ウィンドウ内件数 |
|---|---:|
| contract-mode full | 31 |
| report-documented contract-mode | 0 |
| heuristic-only full | 0 |
| unverifiable | 0 |

`band_summary.md` の集計（full 31、総run 78）と一致した。確認できたfull証跡は、契約フックのprimaryと非空の状態次元（例: `ballX`/`ballY`、`score`、`bricksRemaining`、`selectedOption`、`currentQuestionIndex`等）を伴う。ウィンドウ外の旧runは有効性判定から除外した。

## heuristic-only / unverifiable 全件

該当なし（0件）。したがって列挙対象もない。

## 結論

ウィンドウ内のfull判定にheuristic-only fullまたはunverifiable fullは検出されなかった。FF-1による既存バンドセルの再計算は不要であり、既存バンドは有効と判定する。

