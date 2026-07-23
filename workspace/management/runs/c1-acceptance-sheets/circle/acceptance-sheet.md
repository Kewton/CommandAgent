# Acceptance Sheet

## 1. 依頼

- goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。
- profile: 記録なし
- intent: 記録なし
- model/provider: 記録なし / 記録なし

## 2. 判定

- verdict: **circle_full**
- assurance: 定義された検証を全て実行し成立

## 3. 完成の定義

- `data_claims_binding`: 主張が検証結果に束縛されること
- `data_inspection_schema`: data_inspection_schema
- `data_reconciliation`: 集計結果と入力の整合性
- `data_rerun_consistency`: 再実行結果が一貫すること
- `data_results_schema`: results.jsonが契約スキーマに合うこと
- `pipeline_probe`: パイプラインを実行できること

## 4. 検証の実録

- claims-binding.json: 実在ファイルを参照（観測値は原文のまま）
- data-assurance.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-adjudication.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-after.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-before.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_claims_binding.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_reconciliation.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_rerun_consistency.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_results_schema.json: 実在ファイルを参照（観測値は原文のまま）
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-pipeline_probe.json: 実在ファイルを参照（観測値は原文のまま）
- inspection-schema.json: 実在ファイルを参照（観測値は原文のまま）
- investigation-binding.json: 実在ファイルを参照（観測値は原文のまま）
- investigation-run.json: 実在ファイルを参照（観測値は原文のまま）
- node-runs/019f8c8c-c8a1-7c62-b1ee-e1fb01785b18/state/externally-bound-reproducer.json: 実在ファイルを参照（観測値は原文のまま）
- node-runs/019f8c8c-e18b-7811-9b94-a6f743b38632/state/externally-bound-reproducer.json: 実在ファイルを参照（観測値は原文のまま）
- pipeline-run.json: 実在ファイルを参照（観測値は原文のまま）
- reconciliation.json: 実在ファイルを参照（観測値は原文のまま）
- rerun-consistency.json: 実在ファイルを参照（観測値は原文のまま）
- results-schema.json: 実在ファイルを参照（観測値は原文のまま）
- workflow-circle.json: 実在ファイルを参照（観測値は原文のまま）

## 6. 証拠台帳

この紙の主張は全てここから機械生成された。
- claims-binding.json sha256=1e90527a6533cf708b9c2b315518f28de437863c538bf772ab5a640ceb208c44
- data-assurance.json sha256=cfbb54d7b8a95cf71d04688bb349cedf9e70b2324e88790da7a3e489bfde433e
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-adjudication.json sha256=2b53bf3412b0ed03e8a910291dc0dca30981ec307b77e6f28c8fab7ab20ccbda
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-after.json sha256=f28f6ed8d532eb4ab1ee54c73e150e7fe02279a7ca6e10d8c07600fbf3d418c1
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-before.json sha256=eadb84c0c89653d445f20630293bea5e3536c8d6fef35ad5c5c479123a136a79
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_claims_binding.json sha256=b452bf0ef52e93f602d1e510c22983926eef54ba18c497174eabd3103070c41d
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_reconciliation.json sha256=0c7c7ea43e21240176767afdf0ad2736dbd1a4904bd1900f9c04e0fa5aacdd78
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_rerun_consistency.json sha256=0bf79b5cc42f1fa3c82a9a7bd7bba75c3e417b8f4b72483e4936c11d599e1f2f
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-data_results_schema.json sha256=8dd5d344e39efc2a8152d3891b4107dc623ad13170bde060205a4b0c058ec4e3
- fix-019f8c8c-e18e-7821-b9ca-d3474938b33f-regression-pipeline_probe.json sha256=5c7e2331bfa5cd408415b74df5678482eb89c41f7f5ed464d6792dbe1eea33d0
- inspection-schema.json sha256=7f54465679d11d2318f1e1316e14ee0f34aaa3b2371f202ade08ddf53c556413
- investigation-binding.json sha256=36061e5fbef8f75bedc56bd6fe0d0e1ecfcb0f75f3412c709d1dd77611579665
- investigation-run.json sha256=68aca2e7f43126cd4237228e3683d5453c529737c47fb03fac9454c775a15494
- node-runs/019f8c8c-c8a1-7c62-b1ee-e1fb01785b18/state/externally-bound-reproducer.json sha256=68034c7c6ea3f62ca15685ea02b042601ad733922038a9f91aa5d72e09b336a1
- node-runs/019f8c8c-e18b-7811-9b94-a6f743b38632/state/externally-bound-reproducer.json sha256=f01c28a0f260242d1097d90ab9d96422ad51d3929c766ef365c343c3887fb3ce
- pipeline-run.json sha256=f1f899dc57633be032adae9da58e2e2ae349268438694c2c81db5527fed7198d
- reconciliation.json sha256=d3389ce0eb0b557339751e41fcc2066b9a2484c6008d5111eb55cbd350df7f96
- rerun-consistency.json sha256=1b4704ee966e01aeb8845be4e9130c58c679b646a5f21641dc2aeef9ac83ac08
- results-schema.json sha256=2c87a756b68f03bf4b305861df578cec984a55b6b3729d78007d043a1736bcb7
- workflow-circle.json sha256=c18428f4747c9d130da10062eb8a7a9b7e7b30d38d1c5020e0002b04f123e0e8
