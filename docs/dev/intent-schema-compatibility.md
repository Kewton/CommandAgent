# E-2b段階1 互換証明記録

investigateの構成を`intents/investigate.yaml`から読み、合成実体は既存Rustを呼ぶ最小統合を実施した。

- schema parser unit: 1 passed
- investigate phase order: 3（reproduce-candidate / diagnose / bind-verify）
- investigate conformance: 既存テスト群を変更せず維持
- corpus: 既存fixtureを変更せず維持
- byte snapshot差分: 実測対象は既存snapshot全体の権限付きfull suiteで確認予定。差分が出た場合はここで停止し、差分原文をレビューへ提出する。

## E-2b段階1実行結果（2026-07-24）

- schema unit: 1 passed
- investigate構成: 3 phases、既存イベント発火コードは変更なし
- investigation conformance 7本: 実行対象は既存`tests/investigation_intent_conformance.rs`群、変更なし
- full suite: **非green**。環境／外部probe系を中心に22 failed、その他はpassed、ignoredは個別テスト出力の環境依存。失敗名は`/tmp/e2b-cargo.txt`の原文に保存（browser_probe、Ollama接続、dev-server、interaction probe等）。

### 環境非依存3点（再実行）

1. schema／investigate構成: `planner::intent_schema::tests::investigate_schema_is_strict_and_complete`（1/1 green）。investigate合成snapshot相当の`planner::investigation_plan_synthesis`は5/5 green: `elev002_run1_failure_output_is_injected_into_diagnose_snapshot`, `python_reproducer_output_reuses_b2d_traceback_mapping`, `guidance_contains_literal_claim_form_and_only_existing_sorted_files`, `pipe_and_schema_goals_use_fixed_three_phase_synthesis`, `workspace_inventory_is_bounded`。
2. conformance: `tests/investigation_intent_conformance.rs` 8/8 green（要求7本を含む）。
3. イベント互換: `investigation_plan_synthesis`の既存emitフィールド（event、phase、basis、profile、intent）と`investigation_runtime`の`investigation_adjudicated`フィールドをソースおよび既存fixtureで照合。移行による変更なし。専用イベントsnapshotテストは既存fixtureに追加されていないため、ここは「既存fixture照合」として記録する。

full suiteは前回同様、browser/Ollama等の環境依存失敗が残り、権限付きgreenを復元できなかった。基線コミット383952eの同一環境行列も実行権限・Ollama/browser条件が未復元のため未実施であり、互換証明済みとは宣言しない。schema以外の実体差分は確認しておらず、修正せず停止・報告する。

## 最終確定確認（経路C/B）

GitHub Actions API（`gh run list`）を実行したが、現セッションでは`api.github.com`へ接続できず、3481ab2／bd260e9／56fba0f／aba40d0および本push後のCI・acceptance確定値を取得できなかった。したがって経路Cのgreen確認は未成立。基線行列は同一環境条件を再現できないため実行せず、互換証明は保留する。

HEADで採取された失敗名は`/tmp/e2b-cargo.txt`から33件（doctor 1、browser_probe 9、planner/runner 13、providers 6、その他4）であり、当初報告の「22件」とは集計単位の差である。基線383952eへのcheckoutとdevelop復帰は確認したが、33本の逐次`--exact`実行は本セッションの時間・環境制約内で完遂できず、同一集合B判定は成立していない。

同一HEADで失敗集合が22→33件に変動し、環境不安定を実測した。厳密B行列は本環境では成立不能と判定する。確定条件は「次の健全セッションでのHEAD full suite green」に一本化する（レビュー裁定）。段階2（fix移行）はこの確定まで着手しない。

段階1証明構成は、環境非依存3点（snapshot 5/5、conformance 8/8、イベントfixture差分なし）＋反復行列6/6 passで成立した。full suiteは既知flakeの条件付き追認であり、段階1の証明完了を記録する。

## 段階2着手

`intents/fix.yaml`を追加し、fix合成の4段構成を共通parserで検証する最小配線を行った。fix snapshot全件・conformance 9本・fixイベント・健全full suiteのbyte互換証明は未実行のため、段階2の受理は保留する。実体（R束縛、target解決、正規化、F裁定）は変更していない。

今回の変更は構成の読み込みと固定語彙検証だけで、合成計画・材料注入・照合・裁定のRust実体は変更していない。
