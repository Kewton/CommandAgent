Recover this failed run by producing and executing a focused ultra plan.

Original goal:
output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。

Profile: data

Failure scope:
- phase: isolate-cause
- step: unknown
- kind: phase_execute_error

Failure evidence:
- step verify-top-level-keys failed verification after bounded repair: command failed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" outcome: CommandFailed status: exit status: 1 elapsed_ms: 146 summary: command did not succeed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" stdout: stderr: ; Paths: - repair prompt saved: .anvil/re

Missing paths:
- output/inspection.json

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
