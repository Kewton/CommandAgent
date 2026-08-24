# circle-elev-005 runbook

Implementation under test: `b5649e0` (with `3efb09f`). The elevated
executor is `gemma4:31b-cloud` via Ollama. Each origin is a fresh copy of a
real failed create×data run and contains `.anvil/runs/*/events.jsonl` with a
failed `run_stop` plus `.anvil/plans/recovery-*.yaml`.

Run these exact commands one at a time from the repository root. Record the
two epoch values from each command. Do not inspect, monitor, interrupt, or
run another command until the prompt returns; then start the next command.

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev005_origin_1 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev005_origin_2 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev005_origin_3 ; date +%s
```

After all three prompts return, report completion. Recovery will collect
workflow events, diagnosis carry and I2 binding, `selection_reason`, fix
turn counts, node verdicts, and scrub results.
