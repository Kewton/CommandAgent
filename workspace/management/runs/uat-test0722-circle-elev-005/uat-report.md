# UAT workflow circle elevated-005

Three fresh origins were run sequentially with `b5649e0` installed. All
investigate nodes completed the three UltraPlan phases and all three
`investigate->fix` earned edges fired. The fix phase did not reach full
acceptance in any run; therefore the honest distribution is
`circle_failed 3/3`, `circle_full 0/3`.

| run | investigate run | fix run | elapsed | terminal |
|---|---|---|---:|---|
| 1 | `019f8a9e-78a4-7332-ac39-58c82f45b051` | `019f8a9f-2034-7473-94ef-2bc8c591b7be` | 44 s | `repair_target_unresolved` (planner error) |
| 2 | `019f8a9f-4f92-77f1-ac37-b548c9b4a6a5` | `019f8a9f-6314-7a71-810f-2b2e18721d09` | 6 s | `repair_target_unresolved` (planner error) |
| 3 | `019f8a9f-9e44-7fc3-aa2b-ba4e789a3244` | `019f8a9f-b2e8-79e1-878a-95c057250a6a` | 31 s | schema verification failure, then `model_stagnation:read_only_loop` |

The investigate IDs above and all node paths are recorded in each copied
`workflow-circle.json`; the fix IDs are also present in the corresponding
`workflow-events.jsonl` `workflow_node_run_created` records. Each run ended
with the exact event:

```json
{"event":"workflow_adjudicated","reason":"node_failed:fix","verdict":"circle_failed"}
```

## Diagnosis carry and I2

`diagnosis` carry was present on the investigate→fix route and the copied
origins contained both `output/diagnosis.md` and
`evidence/investigation-binding.json`. I2 output-anchor results were:

| run | claims | matched | violations |
|---|---:|---:|---:|
| 1 | 4 | 4 | 0 |
| 2 | 5 | 5 | 0 |
| 3 | 5 | 5 | 0 |

The fix logs show that runs 1 and 2 terminated at the new honest
`repair_target_unresolved` boundary. Run 3 selected `pipeline/main.py`,
wrote a candidate repair, failed the inspection-schema verification, and
then entered the existing read-only stagnation guard. No run emitted a
successful `selection_reason=verified_diagnosis_mapped` record, so the
targeting path is implemented and exercised up to honest resolution, but
the desired successful target selection is not claimed as a measurement.

## Evidence and safety

For each run, `workflow-events.jsonl`, `workflow-circle.json`, investigate
and fix event streams, investigation binding, and fix adjudication/before
evidence are copied under `run1/`, `run2/`, and `run3/`. All node run
directories recorded there are under the corresponding
`circle_elev005_origin_N/.anvil/runs/` roots. No `verify_origin` or F1–F3
full evidence exists because fix failed before that gate.

Scrub command and result:

```sh
python3 workspace/management/scripts/bench.py scrub \
  --path workspace/management/runs/uat-test0722-circle-elev-005
# {"ok":true,"findings":[]}
```

Credential-pattern grep over logs and events was empty. Raw `run1.log`,
`run2.log`, and `run3.log` remain untracked and are intentionally excluded
from the commit.
