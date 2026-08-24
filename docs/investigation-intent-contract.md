# Investigation Intent Contract (v0, fixed before D-3b implementation)

Status: fixed (2026-07-18). 変更は明示的な契約改訂として台帳に記録する。

## 1. スコープ
対象: 既存ワークスペース内の障害に対する原因調査タスク。
納品物は「検証可能な再現 reproducer R」と「診断レポート
（正準成果物 output/diagnosis.md）」の2点。Rは依頼者が与えるか、
runの冒頭で構築される。

## 2. full の意味（最重要・不変条件）
full は「**検証可能な再現Rが提出され、実際に実行されて期待どおり
失敗し、診断レポートの機械照合可能な主張のすべてがRの実出力および
実在ファイルと照合一致した**」ことを意味する。
Rの最小性、原因仮説の正しさ（修正せずには検証できない）、
修正方針の妥当性は、本intentの恒久的なスコープ外であり、
assurance のいかなる階層でも主張しない。

## 3. 要求 evidence（full の必須ゲート）
I1 reproducer_fails: R が stage=diagnosis で実際に実行され、
    失敗すること（must_execute・expected=failure）。
    R自体の欠陥による失敗（reproducer_defect: 対象評価前の
    構文エラー等）はI1を成立させない。defect時は決定的フィードバック
    のもとR再構築を1回許す（I1確定前に限る。fix契約と同機構）。
I2 diagnosis_bound: output/diagnosis.md 中の機械照合可能な主張が
    全て実在と一致すること:
    (a) 引用されたエラー（例外型・メッセージ）が R の実出力
        （stdout/stderr）に実在すること
    (b) 参照されたファイルパスがworkspaceに実在し、行番号が
        当該ファイルの行数の範囲内であること
    (c) 引用されたコード断片が当該ファイルに実在すること
    照合結果（対象主張の列挙・一致/違反・nearest情報）は
    evidence/investigation-binding.json に記録する。
    機械照合可能な形で書かれていない叙述（因果の考察等）は
    照合対象外であり、I2の成否に影響しない。

## 4. assurance 階層
- full    = I1・I2 全成立（実行プローブによる実測）
- partial = I1成立・I2実行済みだが、照合対象主張が0件
            （diagnosis_claims_absent: 診断が機械照合可能な主張を
            含まない）
- static  = 診断は書かれたが R が未実行
- failed  = R が失敗しない（baseline_not_reproduced: 調査の前提が
            成立しない）／または I2 で違反検出
            （diagnosis_unbound: 実在しないエラー・ファイル・行・
            コードを引用した診断。虚偽診断は partial ではなく
            failed である）

## 5. 実行プローブ
R の実行は隔離・有界（timeout・出力上限）。プローブは診断の
生成過程を参照せず、stage・epoch を evidence
（evidence/investigation-*.json）に記録する。

## 6. 偽装耐性（conformance ネガティブテストの要求）
- 開始時から成功する R で full を獲得できないこと
- R の実出力に存在しないエラー名・メッセージを引用した診断が
  full を獲得できないこと（diagnosis_unbound の実効性）
- 実在しないファイル・行番号・コード断片を引用した診断の拒否
- reproducer_defect（R自壊）が I1 を成立させないこと
- 未実行のプローブからの獲得不可（earned assurance 継承）
- 照合対象主張が0件の診断が full を獲得できないこと（partial上限）

## 7. スコープ外（明示）
R の最小性／原因仮説の真偽／修正方針の妥当性／非決定的・環境依存
障害の診断保証（R が決定的に構築できない場合、run は正直に
failed(baseline_not_reproduced) に落ちる）。

## 8. 生成側への制約（契約由来ガイダンス）
R は決定的であること（時刻・乱数・外部ネットワーク非依存）。
診断レポートの主張は機械照合可能な形式で書くこと
（エラーの正確な引用・ファイルパス:行番号・コード断片の引用）。
R が依頼に含まれない場合、診断着手前に R を構築し
stage=diagnosis の実行で失敗を確認してから診断に入ること。

