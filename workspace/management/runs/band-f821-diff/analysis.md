# band_aggregate F821 差分調査

## 採取方法

tracked出力を変更しないよう、`workspace/temp/band_f821.py`を一時コピーとして実行し、profileごとの出力を`/tmp/band_summary*.md`へ書いた。比較コマンドは次のとおり。

```sh
diff -u workspace/management/runs/band_summary.md /tmp/band_summary.md
diff -u workspace/management/runs/band_summary_data.md /tmp/band_summary_data.md
diff -u workspace/management/runs/band_summary_fix.md /tmp/band_summary_fix.md
diff -u workspace/management/runs/band_summary_investigation.md /tmp/band_summary_investigation.md
```

## 差分全文（意味を保持した行単位抜粋）

nextjs出力のみ差分が発生した。先頭差分は次のとおり。

```diff
- Scanned UAT sets: `12`
- Aggregate.json rows asserted: `77`
- Total run records: `78`
- Record sources: `{'aggregate': 77, 'report': 1}`
+ Scanned UAT sets: `24`
+ Aggregate.json rows asserted: `0`
+ Total run records: `0`
+ Record sources: `{}`
```

続く差分では、旧出力のPlanner Coverage（gemma 4 / qwen 27）、Scenario x Final State（Breakout 5/1/5/6、Quiz 23/0/2/1、Space 3/2/14/16）、Scenario x Executor、Full Run Durations、FF-1 ledger、Stop-Class Distribution、Provisional Comparison、Source Setsが消え、新走査では各件数0とprovisional 85/30/7が出力された。data/fix/investigationの3ファイルは`diff`出力ゼロだった。

## 根因

`window_a_records`は`build_investigation_summary`のWindow A（records全体）用に導入された。旧実装では同関数内で`window_a_records`を定義せず、2260行の呼出しだけが残ったためF821になった。過去の再生成・CIではinvestigation profile分岐が実行されない範囲、または古い生成物を直接検査する範囲が使われ、Ruff対象外の分岐として検出を逃した。修正は`window_a_records = records`であり、Window Aの定義（全investigation history）には意味的に整合する。

一方、nextjs差分はF821修正とは無関係で、今回の一時実行環境の走査対象がtracked生成時と異なり、24セットを見てaggregate rowsを0件としたことによる。旧出力が欠陥か新走査が欠陥かは、生成時の入力スナップショットを再現してレビュー裁定する必要がある。したがってバンド本体は変更せず凍結した。

結論はレビュー裁定待ちであり、本コミットは差分調査記録のみを収録する。
