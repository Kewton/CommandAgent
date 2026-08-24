# Fix Intent Contract (v0, fixed before D-1 implementation)

Status: fixed (2026-07-16). 変更は明示的な契約改訂として台帳に記録する。

## 1. スコープ

対象: 既存ワークスペース内の成果物に対する修正タスク。前提として
「失敗する再現」（再現コマンドまたは再現チェック、以下 reproducer R）が
特定可能であること。Rは依頼者が与えるか、runの冒頭で構築される。

## 2. full の意味（最重要・不変条件）

full は「**開始時に失敗した再現 R が、修正後に同一の R として成功し、
束縛された回帰検証が全件成功した**」ことを意味する。
修正の設計品質（筋の良さ・最小性・美しさ）は本intentの恒久的な
スコープ外であり、assurance のいかなる階層でも主張しない。

## 3. 要求 evidence（full の必須ゲート）

F1 before_fails: R が stage=before で実際に実行され、失敗すること
（must_execute・expected=failure）。

F2 after_passes: **同一 lineage の R** が stage=after で実行され、
成功すること。after の evidence は before より新しい
run-local epoch を持つこと。

F3 no_regression: profile が束縛した回帰チェック集合が全件実行され
成功すること。回帰集合は契約・manifest 由来であり、
run 内での縮小・改変を禁止する。

## 4. assurance 階層

- full = F1〜F3 全成立（実行プローブによる実測）
- partial = F1・F2 成立、ただし F3 に inconclusive / unavailable が残る
- static = 修正は書かれたが F系が未実行（構文検証のみ）
- failed = F2 失敗／実行済み回帰の失敗／または
  **baseline_not_reproduced**（stage=before で R が成功してしまい、
  修正の前提が成立しない。理由コードで「直せなかった」と区別する）
  【要裁定①の封緘】

## 5. 実行プローブ

R および回帰集合の実行は隔離・有界（timeout・出力上限）。
プローブは修正の生成過程を参照せず、stage・lineage・epoch を
evidence（evidence/fix-*.json）に記録する。

## 6. 偽装耐性（conformance ネガティブテストの要求）

- 開始時から成功する R で full を獲得できないこと（F1の実効性）
- before と after で異なるコマンド/チェックへの**すり替え**が
  lineage 不一致として拒否されること
- 回帰集合を縮小した run が full を獲得できないこと
- after evidence の epoch が before 以前である場合の拒否
- 未実行のプローブからの獲得不可（earned assurance 継承）

## 7. スコープ外（明示）

修正の設計品質／複数障害の網羅（R が示す障害のみが対象）／
非決定的・環境依存障害の修正保証（R が決定的に構築できない場合、
run は正直に failed(baseline_not_reproduced) または partial に落ちる）。

## 8. 生成側への制約（契約由来ガイダンス)

R は決定的であること（時刻・乱数・外部ネットワーク非依存）。
R の特定が依頼に含まれない場合、修正着手前に R を構築し
stage=before の実行で失敗を確認してから修正に入ること。
