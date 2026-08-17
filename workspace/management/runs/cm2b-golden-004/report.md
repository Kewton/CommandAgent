# 結果サマリ

## 事前宣言

分母は3 suite × 3変種 × 4 run = 36 run。閾値は一発full ≥60%、修復込みfull ≥90%、所要p50 ≤180秒、1生成 ≤$0.067、予算上限$5。直接priorは置かず、最近接間接実測 ingest×Luna 100% (n=6)、Quiz 92%を記録しWilson 95% CIを併記する。001〜003のwarikan-001〜003窓は床返済前のため除外。計器pinはproduct HEAD `6800293d`、sealed suite manifestは既存SHAと照合し不変。

## ①失敗帰属と返済

CM-2b-003 warikan_001の原文は `AppSpec check failed: entity.fields must be a mapping`。生成物の`app.spec.yaml`ではentityのfields形状がschema要求のmappingに適合しておらず、モデル生成字義の違反である。warikan_002は `dependency_setup_authority_required: node smoke-check.js` で、verify指示が依存設営境界へ入ったことが根拠。schema fixture側は変更せず、正しい最小spec例をDATA-1 guidanceへ追加し、pinned schemaの`fields`定義と機械照合するテストを追加した。

## ②verify自己完結化

計画内preferred verifyを`commandagent --offline --profile community-mini-app --prompt "Validate app.spec.yaml against the pinned Community AppSpec schema and exit non-zero on violation."`へ変更した。製品内蔵のoffline profile verificationでworkspace内完結・依存設営不要。`node smoke-check.js`は計画語彙から撤去し、B系acceptance側の役割に限定した。dependency authority境界そのものは変更していない。

## cm2b-golden-004実測

製品実行前のbench preflightで停止した。原文は `preflight release install failed: install: sandbox detected, falling back to direct copy` に続き `install: /Users/maenokota/.local/bin/INS@qSzUI8: Operation not permitted`。run workspace・provider events・verdictは生成されていないため、run非消費の環境中断として扱い、36分母へ算入しない。従って4閾値は判定停止（製品性能の未達とは主張しない）。

## 前回CI/acceptance確定値

前回push `5117743124291a37e30cc1697a60c7f91cf72fa8` は CI run `32036890871` conclusion `success`、acceptance run `32036890834` conclusion `success`。CM-2e実装push後のworkflowは別途確定値を追記する。

## 封緘・bytes

既存profileのbytes互換テストとsealed suite SHAは不変。変更はcommunity profile guidance/quality語彙と専用テストに限定した。

## 判定

CM-2eの製品再計測は環境床で開始前停止。Phase 2 GOは宣言しない。
