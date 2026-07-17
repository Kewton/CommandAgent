Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair step `verify-pipeline`. Verification failed: command failed: python pipeline/main.py outcome: CommandFailed status: exit status: 1 elapsed_ms: 159 summary: TypeError: list.append() takes exactly one argument (2 given) stdout: stderr: Traceback (most recent call last): File "/Users/<user>/share/work/localwork/commandagent_mvp/01/test0717_dfix_001/dfix1_pipe_qwen35_002/pipeline/main.py", line 181, in <module> run() File "/Users/<user>/share/work/localwork/commandagent_mvp/01/test0717_dfix_0

Profile: data

Failure scope:
- phase: isolate-cause
- step: verify
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=6
- write_required exhausted without Write/Edit to pipeline/main.py: attempts=2/2
- write_required selected_targets=pipeline/main.py; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- pipeline/main.py

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
