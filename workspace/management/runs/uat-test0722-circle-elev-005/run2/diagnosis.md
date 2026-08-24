# Diagnosis Report

## Failure Observation
The execution of the data pipeline failed during the inspection schema check.

エラー引用: `inspection_schema_violation:inspection_path:path does not exist: output/inspection.json`
位置: N/A (Runtime check failure)
コード引用: N/A (The file `output/inspection.json` was not generated)

## Analysis
The pipeline failed to produce the required artifact `output/inspection.json`. Based on the provided file list, `pipeline/main.py` does not exist in the workspace, which is the primary cause for the absence of any output files. The system expects a pipeline to be implemented and executed, but the current workspace only contains input data and evidence files.

## Reproduction Steps
1. Check for the existence of `pipeline/main.py`.
2. Attempt to run the pipeline.
3. Observe that `output/inspection.json` and `output/results.json` are not created.
4. Run the validation check `anvil-catalog-check:data_inspection_schema`.

## Proposed Fix
修正方針:
1. Implement `pipeline/main.py` to process `data/sales.csv` according to the requirements (monthly totals, % change, 3-month moving average).
2. Ensure the pipeline generates `output/inspection.json` containing the observed data profile.
3. Ensure the pipeline generates `output/results.json` and `output/report.md` as specified in the contract.
