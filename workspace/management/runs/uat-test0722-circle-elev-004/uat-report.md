# UAT workflow circle elevated-004

## Result

Three sequential, non-interactive runs completed without interruption. All
three investigate nodes reached full assurance and the earned
`investigate->fix` edge fired. All fix nodes failed honestly, so the circle
verdict is `circle_failed` (3/3) and `circle_full` is 0/3. No
`verify_origin` was attempted after fix failure.

| run | investigate run_id | fix run_id | elapsed | terminal reason |
|---|---|---|---:|---|
| 1 | `019f8a8a-32d4-7710-96c2-0dc722f794bf` | `019f8a8a-55a2-7012-9f9e-e8b4382cd607` | 17 s | `planner_error`: repair target could not be resolved |
| 2 | `019f8a8b-bbe3-7a83-b321-453cff126f5a` | `019f8a8b-0208-70c2-ab38-436a81327202` | 19 s | `planner_error`: repair target could not be resolved |
| 3 | `019f8a8b-31f9-7262-a8bb-58d3418940ba` | `019f8a8b-5973-7cc1-a54b-ccf11a52f6ef` | 26 s | `model_stagnation:read_only_loop:write_required` |

The exact terminal events were:

```json
{"event":"workflow_adjudicated","reason":"node_failed:fix","verdict":"circle_failed"}
{"event":"workflow_adjudicated","reason":"node_failed:fix","verdict":"circle_failed"}
{"event":"workflow_adjudicated","reason":"node_failed:fix","verdict":"circle_failed"}
```

## I2 output-anchor audit

`evidence/investigation-binding.json` was read for each run. Claims were
anchored to measured R output (including `CommandFailed` and
`inspection_schema_violation` forms): run 1 = 6/6 matched, run 2 = 5/5,
run 3 = 10/10. Thus matched claims are non-zero in all three runs; no
I2 violation was recorded.

The audit command was:

```sh
python3 - <<'PY'
import json
for n in (1,2,3):
    d=json.load(open(f'workspace/management/runs/uat-test0722-circle-elev-004/run{n}/investigation-binding.json'))
    c=d['claims']; print(n, len(c), sum(x.get('matched') is True for x in c))
PY
```

## Edge and containment evidence

`workflow-events.jsonl` records `create->investigate` and
`investigate->fix` with E-A, E-B, E-C and E-D all true for every run. The
node run directories recorded in `workflow-circle.json` are under their
respective origin paths; the copied evidence preserves the event streams,
workflow-circle records, investigation bindings and fix adjudications.
The fix failures are node/model outcomes, not workflow-layer mechanical
failures. Since no fix reached full acceptance, F1–F3 and verify_origin have
no successful evidence to report.

## Security and scrub

The complete campaign directory, including copied evidence and the runbook,
was scrubbed with:

```sh
python3 workspace/management/scripts/bench.py scrub \
  --path workspace/management/runs/uat-test0722-circle-elev-004
```

Result: `{"ok":true,"findings":[]}`. The credential-pattern scan over
console logs and events (AIza/ghp/xox/AKIA/sk-/private-key forms) was empty.
Raw `run1.log`–`run3.log` remain untracked and are intentionally excluded
from the commit.

## Acceptance

The release binary used for the measurement was commit `47cc8ac` with the
elevated executor `gemma4:31b-cloud` via Ollama. The privileged full suite
was green: 1776 passed, 30 ignored, 0 failed. Commit 1 CI and acceptance
workflow runs were both green (`29934629315`, `29934629252`); the post-
measurement commit's CI results are recorded in the handoff after push.
