# Acceptance Sheet

## 1. 依頼

- goal (run_start.action): data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。
- profile: data
- intent (intent_resolved): 記録なし
- effective model/provider: gemma4:31b / ollama
- planner model: qwen3.6:27b-coding-nvfp4
- elapsed (epoch difference): 記録なし秒

## 2. 判定

- verdict: **failed**
- assurance: 未完了。回収情報あり

## 3. 完成の定義

- `data_claims_binding`: 主張が検証結果に束縛されること
- `data_reconciliation`: 集計結果と入力の整合性
- `data_results_schema`: results.jsonが契約スキーマに合うこと

## 4. 検証の実録

- E2 claims-binding: claims=0, matched=0

## 5. 失敗・次の一手

- runtime Bash is not deterministic verifier evidence
- recovery YAML: .anvil/plans/recovery-ultra-plan-phase-inspect-and-define-schema-019f640b-bc98-7520-ae05-5f90a226dfe5.yaml
- repair prompt: .anvil/repairs/repair-phase-inspect-and-define-schema-019f640b-bc98-7520-ae05-5f868419db26.md

## 6. 証拠台帳

この紙の主張は全てここから機械生成された。
- evidence/claims-binding.json sha256=a99c808928e06f04af1341748daa4f96138b0570f88972f5a7491d3639f1a2bd
- evidence/reconciliation.json sha256=6cdda9c188997c8bb235e756bee1a2de2fa3da504cffa51ad72887ef6da37354
- evidence/results-schema.json sha256=a0319c9f24add442edab39e6a5cb819c1cd73b284488eb4d4f47b367f6960beb
- output/inspection.json sha256=44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a
