 `# Diagnosis Report

## Observed Failure
The pipeline execution failed because the required main script is missing.

- **Error Citation**: `outcome: CommandFailed status: exit status: 1 elapsed_ms: 21 summary: command did not succeed: test -f pipeline/main.py stdout: stderr:`
- **Position**: N/A (File does not exist)
- **Code Citation**: N/A

## Root Cause
The file `pipeline/main.py` was not found in the workspace. The reproducer command `test -f pipeline/main.py` returned a non-zero exit status, indicating that the core pipeline logic has not been implemented or the file is missing from the expected directory.

## Reproduction Steps
1. Run `test -f pipeline/main.py` in the workspace.
2. Observe the exit status 1.

## Correction Plan
修正方針:
`pipeline/main.py` を新規作成し、`data/sales.csv` を読み込んで月次・地域別の売上集計、全体合計の計算、および無効行の除外処理を実装する。また、`output/inspection.json`, `output/results.json`, `output/report.md` を出力するパイプラインを構築する。
`