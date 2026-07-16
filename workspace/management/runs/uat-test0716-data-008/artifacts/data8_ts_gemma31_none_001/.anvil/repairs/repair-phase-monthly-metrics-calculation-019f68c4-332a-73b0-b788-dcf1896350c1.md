Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: monthly-metrics-calculation
- step: unknown
- kind: phase_scaffold_error

Failure evidence:
- invalid StepPlan after corrective retries: verify command may not use shell control syntax; allowed alternatives: use one deterministic command such as `npm run build`, `cargo test`, `python -m compileall -q src`, or `test -f relative/path`; split multiple checks into separate verify commands

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- python -c "import pandas as pd; df = pd.read_csv('output/monthly_metrics.csv'); assert 'month' in df.columns; assert 'total_sales' in df.columns; assert 'mom_pct' in df.columns; assert 'moving_avg' in df.columns; print('Verification passed')"

Changed paths:
- none

Repair targets:
- phase_scaffold

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
