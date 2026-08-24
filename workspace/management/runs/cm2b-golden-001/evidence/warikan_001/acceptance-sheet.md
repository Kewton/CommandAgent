# Acceptance Sheet

## 1. 依頼

- goal (run_start.action): 友だちとの旅行のお金をあとで揉めないように割り勘できる小さなアプリを作って。誰が何を払ったかはざっくり入力できて、最後に誰が誰へいくら渡せばいいか見たい。
- profile: community-mini-app
- intent (intent_resolved): create
- effective model/provider: gpt-5.6-luna / openai
- planner model: qwen3.6:27b-coding-nvfp4
- 所要: 記録なし秒(run_start→run_stop)

## 2. 判定

- verdict: **failed**
- assurance: 未完了。回収情報あり

## 3. 完成の定義

- 記録なし

## 4. 検証の実録

- 記録なし

## 5. 失敗・次の一手

- required_artifacts_satisfied_after_tool
- recovery YAML: .anvil/plans/recovery-ultra-plan-phase-integration-and-verification-01a00f78-dd46-7f81-9a05-62134c077d05.yaml
- repair prompt: .anvil/repairs/repair-phase-integration-and-verification-01a00f78-dd45-7d63-b6b8-17d0f227c58e.md

## 6. 証拠台帳

この紙の主張は全てここから機械生成された。
- src/app-zone/promotion_decision.json sha256=f52f09d8f93ba753b2b198f2c3ec044c4758e1da24393cf587c10b03a437af89
