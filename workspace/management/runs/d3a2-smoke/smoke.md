# D-3a-2c workflow smoke

Command:

`commandagent --workflow workflows/recovery-circle-data.yaml --origin workspace/management/runs/d3a2-smoke`

The orchestrator reached `workflow_started`, validated the recovery YAML, and
terminated honestly at the create→investigate edge because the origin fixture
has no `events.jsonl`/`run_stop` evidence. No node run was consumed; therefore
there are no node run IDs. The recorded terminal verdict is
`circle_failed` with reason `edge_not_earned:create_to_investigate:run_stop`.

## v8 closeout

The v8 live smoke was the first complete workflow-circle traversal to an
adjudicated terminal. It ran for 346 seconds (`1784647739` to `1784648085`),
exited 0, and confined the investigate node run
`019f854b-69ad-7852-86a0-c0d46c064ccf` to the origin workspace at
`.anvil/runs/019f854b-69ad-7852-86a0-c0d46c064ccf`. The model did not
materialize `output/diagnosis.md`; the node stopped honestly with
`model_stagnation:no_progress_recorded`, and the workflow emitted the following
terminal event without starting fix:

```json
{"event":"workflow_adjudicated","reason":"node_failed:investigate","verdict":"circle_failed"}
```

Across v1 through v8, the live smokes removed five defect classes: invented
origin layout, panic-boundary initialization deadlock, missing earned-edge
gating, missing origin-workspace propagation, and stale PATH binaries (observed
twice). With those layers removed, v8 completed the first full orchestration
round to a truthful `circle_failed` terminal. The verdict records model
stagnation; it does not wash a failed investigate node into circle success.
