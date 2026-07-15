Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: data-cleaning
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for pipeline/main.py; objective: Repair step `implement-pipeline`. Verification failed: data_claims_binding:claims_binding_violation:output/report.md:54:60; claims_binding_violation:output/report.md:73:57; claims_binding_violation:output/report.md:92:3; claims_binding_violation:output/report.md:170:1; claims_binding_violation:output/report.md:193:1; claims_binding_violation:output/report.md:214:1; Paths: - recovery prompt saved: .anvil/re

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- implementation

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
