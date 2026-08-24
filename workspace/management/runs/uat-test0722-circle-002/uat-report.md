# uat-test0722-circle-002 — workflow circle second local-arm measurement

Measured product revision: `ec768eca26a33a41f18e196527d64aec6935f94b`

The phase-1 runbook-only commit `71aa1e1` is layered above that product
revision and does not change `src/`, `tests/`, `docs/`, or the installed
binary.

## Outcome

All three measured invocations returned to the prompt without interruption,
panic, hang, or provider timeout. Each emitted exactly one
`workflow_adjudicated` terminal and ended honestly as
`circle_failed / node_failed:investigate`. No `circle_full` was emitted.

D-3a-3c corrected five of the six live acceptance observations: effective
profile, preset provenance, origin-goal fidelity, workspace confinement, and
run identity all pass 3/3. The sixth observation does not pass:
`investigation_plan_synthesized` is absent 3/3 and no new three-phase
investigation plan exists. The actual child lifecycle is `--prompt`; the
persisted `run_start.action` is `Prompt(...)`. Thus the propagated
`plan_preset=profile` never enters the UltraPlan path that constructs
`reproduce-candidate -> diagnose -> bind-verify`.

This is a workflow execution-mode mechanical class, not a local-model
distribution result. P0 is satisfied, but P1-a is not; this set therefore does
not yet establish the formal local-arm price.

## Operational note

Before the measured sequence, one invocation was attempted from a leftover
working directory where the relative path
`workflows/recovery-circle-data.yaml` did not resolve. The operator returned
to the prescribed repository root and then ran the three commands exactly as
recorded in `circle002-run.md`. The failed lookup emitted no
`workflow_started`, created no measured node run, and is a non-consumed
pre-execution invocation. Each origin contains exactly one workflow start.

## Run matrix

| Run | Wall epoch | Wall seconds | Circle verdict / reason | Investigate run_id | Node verdict / assurance | Stop class |
|---|---|---:|---|---|---|---|
| 1 | `1784681019..1784681593` | 574 | `circle_failed` / `node_failed:investigate` | `019f8747-3a18-71b1-a715-8a1d25bc7f3f` | `failed` / `static` (`investigation_probe_not_run`) | `loop_progress_exhausted: empty assistant response` |
| 2 | `1784681604..1784681735` | 131 | `circle_failed` / `node_failed:investigate` | `019f8750-24b6-79f1-abcc-5a4f179d11b1` | `failed` / `static` (`investigation_probe_not_run`) | `model_stagnation:no_progress_recorded` |
| 3 | `1784681744..1784682079` | 335 | `circle_failed` / `node_failed:investigate` | `019f8752-49ec-7d03-b970-b3ce19856ac8` | `failed` / `static` (`investigation_probe_not_run`) | `model_stagnation:no_progress_recorded` |

The prescribed command records the two epochs but not `$?`; no process exit
code is inferred. Prompt return plus the terminal workflow and node events are
the completion evidence. Total measured wall time is 1,040 seconds.

Run 1 emitted `empty_response_escalation` at `nudge_1` and then
`empty_response_recovered`. A later empty response at the bounded loop limit
produced its honest terminal. All 15 provider turns record `ok=true` and
`timed_out=false`.

## D-3a-3c acceptance audit

| Check | Run 1 | Run 2 | Run 3 | Result |
|---|---|---|---|---|
| (a) effective `profile=data` in actual node `run_start` and `run_stop` | yes | yes | yes | PASS 3/3 |
| (b) `investigation_plan_synthesized` plus concrete three-phase plan | absent | absent | absent | **FAIL 0/3** |
| (c) preset resolution source `default_investigate_data` | yes | yes | yes | PASS 3/3 |
| (d) derived child goal contains the exact persisted origin goal | yes | yes | yes | PASS 3/3 |
| (e) actual node run and all detailed events remain under origin; repository `.anvil/` leakage | yes / zero | yes / zero | yes / zero | PASS 3/3 |
| (f) workflow event run_id = circle mapping = actual run directory UUID | yes | yes | yes | PASS 3/3 |

The expected synthesized phase IDs are `reproduce-candidate`, `diagnose`, and
`bind-verify`. A post-run recursive event search found zero synthesis events,
and `diff -qr` showed that each origin's `.anvil/plans/` remained byte-identical
to its archived source. No `output/diagnosis.md`,
`evidence/investigation-run.json`, or
`evidence/investigation-binding.json` exists in any run.

### Goal fidelity

- Runs 1 and 2 bind:
  `data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`
- Run 3 binds:
  `data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`

In all cases the exact string appears in `workflow-circle.json` at
`origin.goal` and inside the actual node's `run_start.action`; the former
`起点run` placeholder does not recur.

### Confinement and identity

The declared run directories are respectively:

- `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle002_origin_1/.anvil/runs/019f8747-3a18-71b1-a715-8a1d25bc7f3f`
- `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle002_origin_2/.anvil/runs/019f8750-24b6-79f1-abcc-5a4f179d11b1`
- `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle002_origin_3/.anvil/runs/019f8752-49ec-7d03-b970-b3ce19856ac8`

For each run, the UUID in `workflow_node_run_created`, the node mapping in
`workflow-circle.json`, the `run_dir` basename, and the actual detailed event
directory are identical. A repository-side search over every `.anvil` file
and directory with mtime at or after epoch `1784681019` returned zero paths.

## Event firing table

Counts use the harvested JSONL recursively. The two
`workflow_node_run_created` records are the workflow-facing event and the
actual node stream; they contain the same UUID. The two `intent_resolved`
records are the workflow-facing intent record and the actual node event.

| Event | Run 1 | Run 2 | Run 3 |
|---|---:|---:|---:|
| `workflow_started` | 1 | 1 | 1 |
| `workflow_edge_fired` (`create->investigate`) | 1 | 1 | 1 |
| `workflow_node_started` (`investigate`) | 1 | 1 | 1 |
| `workflow_node_run_created` | 2 | 2 | 2 |
| `intent_resolved` (`investigate`) | 2 | 2 | 2 |
| `plan_preset_resolved` | 1 | 1 | 1 |
| `investigation_plan_synthesized` | **0** | **0** | **0** |
| `investigation_adjudicated` | 0 | 0 | 0 |
| `run_stop` | 1 | 1 | 1 |
| `workflow_adjudicated` | 1 | 1 | 1 |
| `empty_response_escalation` | 1 | 0 | 0 |
| `empty_response_recovered` | 1 | 0 | 0 |
| fix `workflow_node_started` | 0 | 0 | 0 |

Search and audit commands:

```sh
rg -n '"event":"(run_start|intent_resolved|plan_preset_resolved|investigation_plan_synthesized|investigation_adjudicated|run_stop|workflow_node_run_created)"' <origin>/.anvil/runs --glob events.jsonl
rg -o '"event":"<event-name>"' workspace/management/runs/uat-test0722-circle-002/run{1,2,3} --glob '*.jsonl' | wc -l
jq -r '.origin.goal, .nodes.investigate.run_id, .nodes.investigate.run_dir' <run>/workflow-circle.json
jq -r 'select(.event=="run_start") | [.profile,.plan_preset_source,.action,.workspace_root]' <node-events>
diff -qr <archived-source>/.anvil/plans <origin>/.anvil/plans
find .anvil -exec stat -f '%m %N' {} + | awk '$1 >= 1784681019'
```

The last command produced no output.

## Death attribution

| Run | Workflow-layer finding | Node/model terminal |
|---|---|---|
| 1 | profile, preset, goal, confinement, and identity corrected; synthesized plan path not entered | bounded empty-response exhaustion after one earlier automatic recovery |
| 2 | same mechanical synthesis omission | `model_stagnation:no_progress_recorded` after 15 successful provider turns |
| 3 | same mechanical synthesis omission | `model_stagnation:no_progress_recorded` after 15 successful provider turns |

The node stops are honest and correctly gate the investigate-to-fix edge.
They do not erase the independent workflow-layer omission: every child runs
as direct `Prompt`, so the required synthesized investigation plan is never
created. Because the measurement configuration is incomplete, the node stop
distribution is recorded but is not promoted to a formal model-band value.

## Cost record

Provider is local Ollama; executor and planner configuration is
`qwen3.6:27b-coding-nvfp4`. No monetary billing field is emitted.

| Run | Provider turns | Provider ms | Estimated prompt tokens sent | Prompt eval count | Eval count | Repair ms | Timeout / failed turns |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 15 | 572,943 | 53,292 | 96,028 | 16,353 | 34,316 | 0 / 0 |
| 2 | 15 | 130,883 | 26,290 | 35,009 | 4,066 | 0 | 0 / 0 |
| 3 | 15 | 334,036 | 47,151 | 72,532 | 10,206 | 0 | 0 / 0 |
| Total | 45 | 1,037,862 | 126,733 | 203,569 | 30,625 | 34,316 | 0 / 0 |

## Predeclared criteria

| Criterion | Result | Evidence |
|---|---|---|
| P0-a | PASS | 3/3 classified `workflow_adjudicated`; panic/hang/environment interruption zero |
| P0-b | PASS | `circle_full` zero; node failures are not washed into workflow success |
| P1-a | **FAIL** | five audit items pass 3/3, but synthesized investigation plan/event is absent 3/3 |

## Harvest layout

Each `runN/` contains:

- `workflow-events.jsonl`: complete workflow-facing stream
- `workflow-circle.json`: origin binding, detailed E-A through E-D record,
  node/run mapping, and terminal adjudication
- `investigate-events.jsonl`: workflow-facing intent record
- `node-runs/<actual-uuid>/events.jsonl`: complete detailed child stream

Every harvested file was compared with its source using `diff -q`; all copies
are byte-identical. The human terminal logs remain untracked and are not part
of the evidence commit because repository guardrails prohibit committing raw
logs.
