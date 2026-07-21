# uat-test0722-circle-001 — workflow circle first local-arm measurement

Measured revision: `5fffda9b740cfddaedbf580be7f6912f5e9e47e8`

## Outcome

All three processes returned without a panic, hang, or user interruption, and
all three emitted an honest `workflow_adjudicated` terminal. The circle result
was `circle_failed / node_failed:investigate` in 3/3 runs. No `circle_full`
was emitted.

The intended local `data` arm is **not a valid distribution measurement**,
because the node's declared `profile: data` was not propagated into the
resolved child Config. The actual node `run_start` records `profile: generic`
and `plan_preset: none` in all three runs. This is a workflow-layer mechanical
class, not a model-distribution result.

## Run matrix

| Run | Wall epoch | Wall seconds | Circle verdict / reason | Declared node run_id | Actual detailed run_id | Node verdict / assurance | Stop class |
|---|---|---:|---|---|---|---|---|
| 1 | `1784676772..1784676914` | 142 | `circle_failed` / `node_failed:investigate` | `019f8706-6b52-7ac0-b2f1-8735266768e7` | `019f8706-6b50-78a3-b743-a11454c21132` | `failed` / `static` (`investigation_probe_not_run`) | `model_stagnation:no_progress_recorded` |
| 2 | `1784676923..1784677147` | 224 | `circle_failed` / `node_failed:investigate` | `019f8708-b82f-7b22-831c-65264fb7d144` | `019f8708-b82e-7671-aab9-47e18b233efc` | `failed` / `static` (`investigation_probe_not_run`) | `model_stagnation:no_progress_recorded` |
| 3 | `1784677154..1784677270` | 116 | `circle_failed` / `node_failed:investigate` | `019f870c-3f95-7cc1-96c0-00ca6174c125` | `019f870c-3f94-7ae3-b601-10e4c1bfa620` | `failed` / `static` (`investigation_probe_not_run`) | `model_stagnation:no_progress_recorded` |

The prescribed command recorded the two epochs but not `$?`; therefore no
shell exit code is inferred. Completion is established by prompt return plus
the terminal events.

## Event firing table

Counts below are from the harvested JSONL recursively. Each
`workflow_node_run_created` and `run_stop` count is two because both the
origin-side declared node stream and the separately located actual detailed
stream are preserved.

| Event | Run 1 | Run 2 | Run 3 | Observation |
|---|---:|---:|---:|---|
| `workflow_started` | 1 | 1 | 1 | entry `create` |
| `workflow_edge_fired` | 1 | 1 | 1 | `create->investigate`; checks list contains E-A, E-B, E-C, E-D |
| `workflow_node_started` | 1 | 1 | 1 | investigate only |
| `workflow_node_run_created` | 2 | 2 | 2 | one workflow record plus one origin-side declared stream |
| `intent_resolved` | 2 | 2 | 2 | workflow-facing record and actual child record; intent is investigate |
| `investigation_plan_synthesized` | 0 | 0 | 0 | child resolved with `plan_preset: none` |
| `investigation_adjudicated` | 0 | 0 | 0 | no investigation evidence was earned |
| `run_stop` | 2 | 2 | 2 | origin-side summary plus actual detailed terminal |
| `workflow_adjudicated` | 1 | 1 | 1 | honest `circle_failed` terminal |
| fix `workflow_node_started` | 0 | 0 | 0 | investigate failure correctly gated fix |

Search commands used:

```sh
rg -n '"event":"(intent_resolved|investigation_plan_synthesized|investigation_adjudicated|run_stop|workflow_node_run_created)"' workspace/management/runs/uat-test0722-circle-001/run{1,2,3} --glob '*.jsonl'
rg -o '"event":"<event-name>"' workspace/management/runs/uat-test0722-circle-001/run{1,2,3} --glob '*.jsonl' | wc -l
rg -n -i 'panic|deadlock|hang|workspace_confinement_violation|edge_not_earned' workspace/management/runs/uat-test0722-circle-001/run{1,2,3} --glob '*.jsonl'
```

The final negative search returned zero matches.

## Death attribution and mechanical findings

The immediate node stop in every run is model-side
`model_stagnation:no_progress_recorded`: 15 successful, non-timeout Ollama
turns were consumed but the model never materialized `output/diagnosis.md` or
an `investigation-*.json` evidence file. That immediate stop class is honest.

However, all three runs are contaminated by the same workflow-layer defects:

1. **Node profile/config re-resolution missing.** The workflow declares
   `profile: data`, while each actual `run_start` says `profile: generic` and
   `plan_preset: none`. The absence of `investigation_plan_synthesized` is
   consistent with this incorrect effective configuration.
2. **Detailed event path is not confined to the origin.** The actual child
   events were written under the repository's ignored `.anvil/runs/`, not
   `<origin>/.anvil/runs/`. The model workspace itself was correctly recorded
   as the origin, but telemetry/runtime evidence escaped the origin copy.
3. **Declared and actual run IDs diverge.** The workflow's
   `workflow_node_run_created.run_id` names a two-line origin-side summary
   stream, while `run_config` creates a different UUID for the detailed child
   stream. Both IDs are listed in the run matrix and both streams are retained.
4. **Origin goal lineage is reduced to a placeholder.** Each actual stop
   reason contains `『起点run』`; the archived create goal text was not carried
   into the derived investigate goal. This weakens the node input before model
   behavior is measured.
5. **Circle evidence is sparse.** `workflow-circle.json` contains only final
   verdict/reason; the edge checks and node/run mapping live only in
   `workflow-events.jsonl`. This is sufficient to observe the terminal here,
   but not the promised self-contained circle evidence record.

Therefore the per-run attribution is the same for runs 1–3: the immediate
terminal is a node-model stagnation, while the intended data-arm measurement
is invalidated by workflow-layer configuration and evidence-routing classes.
No model-distribution conclusion should be entered into a band from these
runs.

## Cost record

Provider: local Ollama, model `qwen3.6:27b-coding-nvfp4`. No monetary billing
field is emitted, so time and token telemetry are the auditable cost measures.

| Run | Provider turns | Provider/total ms | Estimated prompt tokens sent | Prompt eval count | Eval count | Timeout/failed provider turns |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 15 | 141,042 | 33,606 | 51,195 | 3,956 | 0 / 0 |
| 2 | 15 | 224,045 | 22,828 | 32,941 | 7,226 | 0 / 0 |
| 3 | 15 | 114,555 | 35,196 | 51,919 | 3,267 | 0 / 0 |
| Total | 45 | 479,642 | 91,630 | 136,055 | 14,449 | 0 / 0 |

Human-terminal wall time totals 482 seconds.

## Predeclared criteria

| Criterion | Result | Evidence |
|---|---|---|
| P0-a | PASS | 3/3 `workflow_adjudicated`; classified reason; panic/hang zero |
| P0-b | PASS | `circle_full` zero; each failed node lacks earned investigate evidence |
| P1-a | **FAIL** | profile/config propagation and detailed-event/run_id confinement defects reproduced in 3/3 |

## Evidence layout

For each `runN/`:

- `workflow-events.jsonl`: complete workflow-facing stream
- `workflow-circle.json`: final circle evidence document
- `investigate-events.jsonl`: workflow-facing intent resolution
- `declared-node-run/events.jsonl`: origin-side run_id announced by workflow
- `actual-node-run/events.jsonl`: detailed child stream recovered from the
  repository-side leaked `.anvil/runs/` location

No `output/diagnosis.md` or `investigation-*.json` existed in any origin at
harvest time. The human terminal logs are summarized above and remain local;
they are not part of the commit because repository guardrails prohibit
committing raw logs.
