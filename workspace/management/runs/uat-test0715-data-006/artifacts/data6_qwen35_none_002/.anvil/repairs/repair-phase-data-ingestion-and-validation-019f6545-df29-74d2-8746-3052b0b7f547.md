Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: data-ingestion-and-validation
- step: unknown
- kind: phase_scaffold_error

Failure evidence:
- invalid StepPlan after corrective retries: verify command may not use shell control syntax; allowed alternatives: use one deterministic command such as `npm run build`, `cargo test`, `python -m compileall -q src`, or `test -f relative/path`; split multiple checks into separate verify commands

Missing paths:
- pipeline/main.py
- output/inspection.json
- output/results.json
- output/report.md

Missing capabilities:
- none

Verification commands:
- python -c "import json; assert 'reconciliation' in json.load(open('output/results.json'))"
- python -c "import json; assert 'values' in json.load(open('output/results.json'))"

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
