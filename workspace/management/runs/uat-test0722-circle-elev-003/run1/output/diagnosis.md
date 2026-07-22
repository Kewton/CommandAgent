# Diagnosis Report

## Failure Summary
The execution of the requested pipeline failed because the entry point file `pipeline/main.py` does not exist in the workspace.

## Error Evidence
エラー引用: `outcome: CommandFailed status: exit status: 1 elapsed_ms: 25 summary: command did not succeed: test -f pipeline/main.py stdout: stderr:`
位置: N/A (File not found)
コード引用: N/A

## Root Cause
The reproducer failed at the first step `test -f pipeline/main.py`, confirming that the required pipeline source code is missing from the workspace.

## Reproducer
```bash
test -f pipeline/main.py
```
Expected: Exit 0
Actual: Exit 1

## Correction Strategy
修正方針:
`pipeline/main.py` を作成し、`data/sales.csv` を読み込んで月次×地域の売上集計、全体合計の計算、および無効行の除外処理を実装する必要があります。また、`output/inspection.json`, `output/results.json`, `output/report.md` を出力する仕様を満たす実装にします。
