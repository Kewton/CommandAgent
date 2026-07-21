# D-3a-2c workflow smoke

Command:

`commandagent --workflow workflows/recovery-circle-data.yaml --origin workspace/management/runs/d3a2-smoke`

The orchestrator reached `workflow_started`, validated the recovery YAML, and
terminated honestly at the create→investigate edge because the origin fixture
has no `events.jsonl`/`run_stop` evidence. No node run was consumed; therefore
there are no node run IDs. The recorded terminal verdict is
`circle_failed` with reason `edge_not_earned:create_to_investigate:run_stop`.
