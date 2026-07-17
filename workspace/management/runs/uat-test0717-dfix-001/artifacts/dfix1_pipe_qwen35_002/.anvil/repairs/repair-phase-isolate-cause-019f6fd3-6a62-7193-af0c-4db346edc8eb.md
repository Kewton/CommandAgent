Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。

Profile: data

Failure scope:
- phase: isolate-cause
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for pipeline/main.py; objective: Repair step `verify-pipeline`. Verification failed: command failed: python pipeline/main.py outcome: CommandFailed status: exit status: 1 elapsed_ms: 159 summary: TypeError: list.append() takes exactly one argument (2 given) stdout: stderr: Traceback (most recent call last): File "/Users/<user>/share/work/localwork/commandagent_mvp/01/test0717_dfix_001/dfix1_pipe_qwen35_002/pipeline/main.py", line 181, in 

Missing paths:
- output/report.md

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
