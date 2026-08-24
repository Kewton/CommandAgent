# Workflow Circle Contract & Schema (v0–v0.2)

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

v0.2 で draft profile を含む円環は、各ノードで得た保証にかかわらず
`circle_full` へ投影しない。`verify_origin` が全件成立しても既存の
二値終端語彙を保ったまま `circle_failed` / `profile_not_admitted` として
閉じる。これは draft の既存上限 `static` を円環終端でも再適用する
admission cap であり、新しい中間保証の発明ではない。

## 7. workflow schema v0.1 / v0.2（構成のみ・振る舞いの記述を禁止）

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
version検証で拒否する。v0定義にmodel/providerを追加することも拒否し、
当該構成はversion 0.1への明示改訂を要求する。

### v0.2 差分

改訂記録（2026-08-22、Issue #253）: v0.1→v0.2。改訂対象は登録済み
draft profile と planner role のノード構成、および円環終端での admission
cap のみである。v0/v0.1 YAML の解釈、既存 event 名・field、§2〜§5 の
earned/lineage/verify_origin 意味論は不変。

```yaml
workflow: <id>
version: 0.2
entry: <node-id>
nodes:
  <node-id>: { intent: create|fix|investigate, profile: <profile-id>,
               model: <executor-id>?, provider: ollama|lm-studio|openai|gemini?,
               planner_model: <planner-id>?,
               planner_provider: ollama|lm-studio|openai|gemini? }
routes:
  - { from: <node-id>, on: full|failed, when: <condition-id>?,
      to: <node-id>, carry: [<carry-id>...] }
terminal:
  <node-id>: { on: full|failed, verdict: circle_full|circle_failed }
```

v0.2 の profile は compiled registry または起動時に読み込んだ extension
registry に実在しなければならない。admitted と draft のどちらも指定できるが、
未登録IDは draft とみなさず拒否する。draft を1つでも含む円環には §6 の
`static` admission cap を適用する。executor の model/provider と planner の
planner_model/planner_provider はそれぞれ独立した任意の組で、各組は両方指定
または両方省略に限る。planner組は v0.2 のみで指定でき、省略時は workflow
起動時の global planner_model/planner_provider のbyteをそのまま継承する。
classifier のノード指定は v0.2 のスコープ外であり、未知キーとして拒否する。

### condition 固定語彙の追加手順

condition は v0.2 でも `recovery_yaml_present` のみである。語彙追加は次を
すべて同一の明示的 schema 改訂で行う。どれかを欠く追加は受け入れない。

1. 新しい schema version と Rust の typed `Condition` variant を追加し、旧version
   では新語彙を拒否する。
2. model/YAML が実行ロジックを供給しない deterministic leaf evaluator を追加する。
   任意式、script、shell、alias、parameter付き述語は許可しない。
3. 新語彙が成立・不成立となる positive execution test と、unknown token・旧version・
   不正値を拒否する negative schema test を追加する。
4. `tests/corpus/apps/` に positive/negative YAML と実行結果の fixture を追加し、
   corpus regression で固定する。
5. 本契約と `docs/dev/workflow-smoke-runbook.md` に証拠source、評価時点、失敗reason、
   live smoke の確認方法を追記する。

## 8. 偽装耐性（conformance ネガティブテストの要求）
- evidence を欠く verdict ラベルのみでの辺発火の拒否（E-B）
- investigate→fix の lineage 断絶の拒否（§4）
- verify_origin 集合の縮小・差し替えの拒否（§5）
- epoch 逆転（過去evidenceの再利用）の拒否（E-C）
- v0/v0.1 の未admissionセル、およびv0.2の未登録profile IDの実行拒否（§7）
- draft を含むv0.2円環の `circle_full` 投影拒否（§6、§7）
- planner model/provider半組、旧versionでのplanner組、未知conditionの拒否（§7）
- fix full を circle_full として投影する洗浄の拒否（§6）

## 9. スコープ外（明示）
原因の最適性・修正の設計品質（各intent契約に従う）／
複数障害の同時円環／profile混成・並列分岐／createの全面再実行
（verify_origin は起点束縛の再検証であり再生成ではない）。

## 10. 生成側への制約
workflow YAML は人間または上位ツールが記述する構成であり、
モデルが実行中に書き換えることはできない。ノードrunの内部では
当該intentの契約・合成計画がそのまま適用される。
