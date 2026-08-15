# Workflow Circle Contract & Schema (v0, fixed before D-3a implementation)

Status: fixed (2026-07-21). 変更は明示的な契約改訂として台帳に記録する。

## 1. スコープ
workflow とは、単一intentのrun（create / fix / investigate）を
ノードとし、その earned verdict を辺の発火条件として接続する
宣言的ルーティングである。v0 は線形の recovery circle
（create失敗 → investigate → fix → verify_origin）のみを対象とする。
並列分岐・複数profileの混成・createの全面再実行はv0のスコープ外。

## 2. circle_full の意味（最重要・不変条件）
circle_full は「**起点runの失敗が、I1/I2を全て成立させた調査と、
F1〜F3を全て成立させた修正を経て、起点runが束縛していた契約検証
集合が再実行され全件成立した**」ことを意味する。
各ノードの成立は当該intent契約（fixed）の full と同一定義であり、
workflow層が緩和・代替することはない。

## 3. 辺の発火条件（earned edges）
辺は次の全てを機械確認して発火する:
E-A verdict条件: 遷移元ノードの verdict が宣言された値であること。
E-B evidence実在: verdict の根拠 evidence（investigate: I1/I2、
    fix: F1〜F3）がファイルとして実在し、当該runのadjudication
    イベントと整合すること。ラベルのみでの発火を禁止する。
E-C epoch順序: 遷移先ノードの evidence epoch が遷移元より
    新しいこと。
E-D carry整合: 運搬対象（workspace・recovery YAML・
    reproducer lineage）が宣言どおり存在すること。

## 4. lineage の連続（円環の接合部）
investigate が I1 で確立した reproducer R は、fix ノードに
**同一 lineage** で引き継がれ、fix の F1 は同一Rの stage=before
実行として成立する。lineage の断絶（fixが別のRを採用）は
接合部の偽装として辺発火を拒否する（fix契約§6のすり替え拒否を
workflow層で再確認する）。

## 5. verify_origin（閉門）
円環の完成判定は、起点create runが束縛していた契約検証集合
（起点runのevidence/recovery YAMLに記録された束縛）を再実行し、
全件成立することで行う。集合の縮小・差し替えを禁止する。
fix の F3（fix時点の回帰）は verify_origin を代替しない。

## 6. 裁定階層
- circle_full        = 全辺 earned 発火＋verify_origin 全件成立
- circle_failed      = いずれかのノードの正直な失敗
                       （reason: node_failed:<node> / 
                       origin_verify_failed / edge_not_earned:<edge>）
- circle_interrupted = 環境起因の中断（run非消費の裁定に従う）
partial に相当する中間階層は設けない。部分的な進捗は各ノードの
evidence がそのまま記録として残る（洗浄の禁止: fix full は
circle_full を意味しない）。

## 7. workflow schema v0.1（構成のみ・振る舞いの記述を禁止）

改訂記録（2026-07-22）: v0→v0.1。改訂対象は本§7のノードexecutor構成
のみであり、§1〜§6の裁定意味論は不変。既存v0定義は引き続き有効。

改訂記録（2026-07-22、D-3a-3f）: v0.1の固定carry語彙に
reproducer_suggestionを追加する。workflowは依頼者として起点workspace
から機械導出・事前失敗確認したRをinvestigateへ束縛できる。導出不能時は
Rを発明せず、investigate契約§8どおりノード冒頭での構築に委ねる。
改訂対象は本§7のcarry構成のみであり、§1〜§6の裁定意味論は不変。

```yaml
workflow: <id>
version: 0.1
entry: <node-id>
nodes:
  <node-id>: { intent: create|fix|investigate, profile: <profile-id>,
               model: <executor-id>?, provider: ollama|lm-studio|openai|gemini? }
routes:
  - { from: <node-id>, on: <verdict>, when: <condition-id>?,
      to: <node-id>, carry: [<carry-id>...] }
terminal:
  <node-id>: { on: <verdict>, verdict: circle_full|circle_failed }
```
制約: on は固定語彙（full / failed）。when は固定condition語彙
（v0: recovery_yaml_present のみ）。carry は固定語彙
（workspace / recovery_yaml / reproducer_suggestion /
reproducer_lineage）。reproducer_suggestionはv0.1だけで使用できる。
式・スクリプト・任意述語の記述は禁止し、パーサは未知語彙を
エラーとして拒否する。ノードのintent/profileは実在かつ
admission済みであること。model/providerはexecutorだけをノード単位で
指定する任意の組であり、片方だけの指定を拒否する。両方省略時は
workflow起動時のグローバルmodel/providerをそのまま継承する。
planner_model/planner_providerのノード指定はv0.1のスコープ外であり、
未知キーとして拒否する。v0定義にmodel/providerを追加することも拒否し、
当該構成はversion 0.1への明示改訂を要求する。

## 8. 偽装耐性（conformance ネガティブテストの要求）
- evidence を欠く verdict ラベルのみでの辺発火の拒否（E-B）
- investigate→fix の lineage 断絶の拒否（§4）
- verify_origin 集合の縮小・差し替えの拒否（§5）
- epoch 逆転（過去evidenceの再利用）の拒否（E-C）
- 未admissionセルをノードに含むworkflowの実行拒否（§7）
- fix full を circle_full として投影する洗浄の拒否（§6）

## 9. スコープ外（明示）
原因の最適性・修正の設計品質（各intent契約に従う）／
複数障害の同時円環／profile混成・並列分岐／createの全面再実行
（verify_origin は起点束縛の再検証であり再生成ではない）。

## 10. 生成側への制約
workflow YAML は人間または上位ツールが記述する構成であり、
モデルが実行中に書き換えることはできない。ノードrunの内部では
当該intentの契約・合成計画がそのまま適用される。
