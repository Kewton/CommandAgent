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

## v2: nextjs集計0化の走査実態（2026-07-22）

### 結論

0化はabort-on-gap不変条件の発火ではない。現行のnextjs経路は24セットを
`scanned_sets`へ加えた後、全セットを静かに0件へフィルタし、例外なしで
0行バンドを生成する。したがって集計器自身の正直性欠陥である。

`discover_records()`の実測値は次のとおりだった。

```text
{'scanned_sets': 24, 'aggregate_rows': 0, 'aggregate_records': 0, 'records': 0}
```

唯一のassertは`aggregate_record_total == aggregate_row_total`であり、今回は
両辺が0なので成立した。入力セットごとの形式適合、1件以上のrecord実在、
旧Source Setsの実在を検証するgap gateは存在しない。

### 24セットの落下地点

全セットが`WINDOW_START = "uat-test0711-bs-003"`以降という辞書順条件だけで
走査対象になった。いずれにも`aggregate.json`はなく、
`parse_report_only()`へ進んだ。除外条件や例外で落ちたセットは0件である。

| セット | 導入コミット | 落下地点 | 理由 |
| --- | --- | --- | --- |
| uat-test0713-data-001 | 97f9b94 | report fallback | `uat-report.md`不在のため空配列 |
| uat-test0714-ff1-001 | dac63de | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0714-m4-001 | e6a6697 | report fallback | `uat-report.md`不在のため空配列 |
| uat-test0714-m4-004 | cbe5fe2 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0715-data-005 | cc829bc | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0715-data-006 | 8b97959 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0715-data-007 | 88e0a69 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0715-ff1-001 | 00a406c | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0715-ff1-002 | f049af7 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0716-d0c-001 | 29bc14b | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0716-data-008 | 46d9e34 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0716-data-009 | c4d5727 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-dfix-001 | 9682f45 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-dfix-002 | 3d012e3 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-dfix-003 | 0c1ff4a | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-dfix-004 | 2066ef2 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-fix-001 | 8754592 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-fix-002 | 61ab3f5 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-fix-003 | 0fedc86 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0717-fix-004 | 2f45863 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0718-dfix-005 | b3d730e | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0718-inv-001 | df1afdb | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0718-inv-002 | ea00fa7 | report format | `## Smoke Result`見出し不在のため空配列 |
| uat-test0719-dfix-006 | 967c572 | report fallback | `uat-report.md`不在のため空配列 |

内訳はreport不在3、report形式不一致21、除外0、例外0である。

### 旧出力との構成差

tracked `band_summary.md`は2026-07-13の`ffbdd9b`で、12セット・77
`aggregate.json`行・report由来1行から78 recordsを生成したと記録する。
その12 Source Setsは現在の`workspace/management/runs/`に全て不在で、
`git rev-list --all --objects`にも対象パスが0件だった。したがって旧集計は
Gitで再現可能な入力集合から生成されていない。

一方、現在検出される24セットは全て旧出力コミット後に追加されたdata、
FF-1/M4、D-0c、fix、investigationキャンペーンで、nextjs用
`aggregate.json`形式ではない。広すぎる`RUNS_DIR.glob("uat-*")`により、
これらの追加セットはnextjsの「走査済み」件数だけを増やす。

厳密には、後続24セットの追加それ自体は旧78 recordsを消さない。0化の直接条件は
recordを持っていた旧12ディレクトリが現構成に存在しないことであり、後続セットの
増加は「無関係セットを数えて入力があるように見せる」欠陥を露呈させた。つまり
構成差は、旧ローカル入力12セットから、追跡済みだがnextjs形式ではない24セットへの
置換である。

### 裁定候補

- 旧出力維持側: 旧78 recordsの数値は当時の入力に対する測定記録として凍結し、
  再生成には同一12セットの監査可能なアーカイブ復元を必須とする。
- 現行再生成側: 現runs構成からはnextjsバンドを導出不能と正直に失敗させるべきで、
  0行バンドを正常出力してはならない。profile別の明示入力集合、全セット形式検証、
  非空assert、Source Sets実在検証が必要になる。

どちらを正準化するか、および旧12セットを追跡資産として復元するかはレビュー裁定と
する。本調査では集計器・trackedバンド本体を変更していない。
