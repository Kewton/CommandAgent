# Workflow circle smoke v8 — human-terminal run sheet

## Phase 1 preparation

Selected origin: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke8_origin`

The origin is a fresh copy of the real failed create×data run
`uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002`.

Existence checks (run from the repository root):

```sh
rg -l '"event":"run_stop"' \
  /Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke8_origin/.anvil/runs/*/events.jsonl
find /Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke8_origin/.anvil/plans \
  -name 'recovery-*.yaml' -print
```

Observed matches:

- `.anvil/runs/019f6951-e16e-7fc0-84a9-86f7657258ba/events.jsonl`
- `.anvil/plans/recovery-ultra-plan-read-only-stagnation-019f695c-3253-7910-9f51-6c0c104e56ef.yaml`
- `.anvil/plans/recovery-ultra-plan-phase-data-aggregation-019f695c-3255-7160-b624-cd19ecf8cf4d.yaml`

## Phase 2 exact command (human terminal)

Run the following as one command, then do not monitor, inspect, or interrupt it
until the shell prompt returns:

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke8_origin ; date +%s
```

Expected observations are `workflow_started`, origin E-B confirmation,
`investigate` node startup and run creation, followed by an honest terminal
(`workflow_adjudicated`, regardless of verdict). After the prompt returns,
tell Codex only: `終わった`.

Phase 1 is complete; the human-terminal run has not been started here.
