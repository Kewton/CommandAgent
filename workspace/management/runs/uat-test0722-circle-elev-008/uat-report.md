# UAT workflow circle elevated-008

Binary `5742189` was installed and all three runs were sequential and free of
HTTP 500 interruption.

| run | investigate run | fix run | elapsed | circle verdict | selection/result |
|---|---|---|---:|---|---|
| 1 | `019f8c8c-c8a1-7c62-b1ee-e1fb01785b18` | `019f8c8c-e18b-7811-9b94-a6f743b38632` | 18 s | `circle_full` | fix full, verify_origin reached |
| 2 | `019f8c8d-8bfb-7132-a60b-50de6b36e31c` | `019f8c8d-c98b-7752-a7b5-7898ccac70f6` | 82 s | `circle_failed` | inspection schema failure/read-only stagnation |
| 3 | `019f8c8f-0077-7d22-aa45-793037bb6bd6` | `019f8c8f-157a-7b72-a8a0-618e2cd369f7` | 22 s | `circle_failed` | model read-only stagnation |

Run 1 is the first complete circle. Its workflow event tail is:

```json
{"checks":["E-A","E-B","E-C","E-D"],"edge":"fix->verify_origin","event":"workflow_edge_fired"}
{"event":"workflow_adjudicated","verdict":"circle_full"}
```

The run1 evidence directory includes I1/I2 binding, F1 before, F2 after,
all five regression evidence files, pipeline/results/reconciliation/rerun
evidence, both node event streams, and the complete node run directories.
The repair wrote `pipeline/main.py`; verify-regressions passed all frozen
data checks and verify_origin closed the origin-bound contract set.

Runs 2 and 3 ended honestly at `node_failed:fix` after model-generated writes
failed inspection or entered the read-only stagnation guard. No circle_full
claim is made for them.

## I2 and target audit

I2 matched claims were 5/5 (run1), 6/6 (run2), and 10/10 (run3). Run1's
target was the missing `pipeline/main.py`, selected from the carried verified
diagnosis/R path after the existence-gate removal. The copied fix events and
plan contain the target and the repair diff. Runs 2/3 retain their failed
target and model evidence for comparison.

## Security and containment

All node run paths are under their origin `.anvil/runs/` roots. The complete
campaign was scrubbed:

```sh
python3 workspace/management/scripts/bench.py scrub \
  --path workspace/management/runs/uat-test0722-circle-elev-008
# {"ok":true,"findings":[]}
```

Credential-pattern grep over events, evidence and logs was empty. Raw
`run1.log`–`run3.log` remain untracked and are excluded from the commit.
