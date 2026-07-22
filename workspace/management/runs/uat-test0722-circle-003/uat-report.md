# uat-test0722-circle-003 report

Date: 2026-07-22
Contract: `docs/workflow-circle-contract.md` v0.1
Product commit: `a983725c8ac4d43bc17d3a0a10a04d0ee172a6c3`
Workflow: `workflows/recovery-circle-data.yaml`

## Outcome

The three human-terminal runs all reached an honest workflow terminal without
panic, hang, timeout, interruption, route misfire, confinement violation, or
layout error. All three were adjudicated `circle_failed` with
`reason=node_failed:investigate`. The investigate node reached the real
UltraPlan `reproduce-candidate` phase and emitted
`investigation_plan_synthesized` with the fixed three-phase plan in every run.

The D-3a-3d mechanical acceptance boundary therefore passes. This measurement
is the first valid local-arm price: `circle_full` 0/3 and
`circle_failed(node_failed:investigate)` 3/3. The common node stop was
`investigation reproducer was not identified`; it is attributed to the node
planner/model output because every generated first-phase StepPlan lacked the
contract-required single `verify`, `expected_result=fail` reproducer command.
The workflow routing, child mode, and evidence plumbing had already completed
successfully before that honest node-contract rejection.

## Preflight and execution protocol

The complete preflight, source selection, copy verification, installed binary
identity, and exact commands are recorded in `circle003-run.md`.

- Privileged full suite: 1747 passed / 30 ignored / 0 failed.
- `cargo fmt --all -- --check`: green.
- `cargo clippy --all-targets -- -D warnings`: green.
- Installed and release binary both reported
  `commandagent 0.1.0 a983725+dirty 2026-07-22T01:24:31Z` and shared SHA-256
  `a8a9ff5c323ea6712acc910c3b011899ed3610c903bafcefc3eaaeb7f8664193`.
- `NODE_ENV=production`.
- GitHub Actions for the implementation commit:
  - CI run `29882983088`: success.
  - acceptance run `29882983139`: success.
- The three commands were run sequentially from the repository root. The next
  run was not started until the preceding prompt returned; no monitoring,
  parallel execution, interruption, or timeout was applied.
- The prescribed command records wall-clock epochs but not `$?`; no process
  exit code is inferred from the timestamps. Terminal status is taken from the
  node `run_stop` and `workflow_adjudicated` records.

## Run matrix

| Run | Investigate run_id | Wall epoch | Wall seconds | Circle verdict / reason | Node verdict / assurance | Node stop class |
|---|---|---|---:|---|---|---|
| 1 | `019f8774-1882-7ef1-85ce-119f3cd24591` | 1784683960–1784684319 | 359 | `circle_failed` / `node_failed:investigate` | `failed` / `static` | `investigation_reproducer_not_identified` |
| 2 | `019f8779-b0be-7dd0-a9bb-d0067db050f6` | 1784684327–1784684638 | 311 | `circle_failed` / `node_failed:investigate` | `failed` / `static` | `investigation_reproducer_not_identified` |
| 3 | `019f877e-97bb-7693-bcff-fd7834152a5f` | 1784684648–1784684778 | 130 | `circle_failed` / `node_failed:investigate` | `failed` / `static` | `investigation_reproducer_not_identified` |

All node `run_stop` events carry `status=failed`,
`assurance_level=static`, `assurance_reason=investigation_probe_not_run`, and
the original `UltraPlanRun(...)` action. No fix node was started because the
investigate node did not earn the `investigate->fix` edge.

## D-3a-3c/D-3a-3d audit

| Requirement | Result | Evidence |
|---|---|---|
| Effective profile is `data` | PASS 3/3 | node `run_start.profile=data`, `run_stop.effective_profile=data`, and side `intent_resolved.profile=data` |
| Real UltraPlan child mode | PASS 3/3 | node `run_start.action=UltraPlanRun(...)` and `time_profile.command=--ultra-plan-run` |
| `investigation_plan_synthesized` | PASS 3/3 | one event per node with `phase_count=3`, `profile=data` |
| Fixed three-phase plan exists | PASS 3/3 | archived `plans/ultra-plan.yaml` contains `reproduce-candidate`, `diagnose`, `bind-verify` in that order |
| First UltraPlan phase entered | PASS 3/3 | `ultra_phase_start`, `ultra_phase_scaffold_complete`, and `ultra_phase_plan_validated` for `reproduce-candidate`; step counts 3/5/4 |
| Default plan preset | PASS 3/3 | `plan_preset=profile`, source/origin `default_investigate_data` |
| Origin-goal fidelity | PASS 3/3 | `workflow-circle.json.origin.goal` is reproduced verbatim inside the derived child `run_start.action`; no placeholder fallback |
| Workspace confinement | PASS 3/3 | each declared and actual `run_dir` is under its matching `circle003_origin_N/.anvil/runs/`; repository `.anvil/` gained no file after epoch 1784683960 |
| Run identity consistency | PASS 3/3 | workflow event, circle node mapping, actual UUID directory, and node event all agree |
| Honest workflow terminal | PASS 3/3 | exactly one `workflow_adjudicated` per run; no `circle_full` and no missing terminal |

P1-a, as redefined for circle-003, is satisfied: the workflow mechanical
class count is zero, all prior (a)–(f) propagation checks remain satisfied,
and real three-phase investigation synthesis is 3/3. Consequently the 0/3
circle-full distribution above is a valid local-arm measurement rather than a
configuration-invalid sample.

## Three-phase and first-phase artifacts

Each run contains a byte-for-byte harvest of the generated fixed UltraPlan and
the model-produced `reproduce-candidate` StepPlan:

| Run | UltraPlan phases | First-phase StepPlan | Planner recovery |
|---|---|---|---|
| 1 | `reproduce-candidate → diagnose → bind-verify` | 3 steps; no single fail-expected verify reproducer | one quality retry; recovered and linted |
| 2 | same fixed three phases | 5 steps; no single fail-expected verify reproducer | one lint error and retry sequence through attempt 3; recovered/degraded and linted |
| 3 | same fixed three phases | 4 steps; no single fail-expected verify reproducer | no retry; linted |

The `investigation_runtime::extract_reproducer` gate requires exactly one
verify step with `expected_result=fail` and exactly one command. None of the
three accepted model StepPlans has that shape, so the gate stopped before tool
execution. This explains `executor_ms=0`, absent `output/diagnosis.md`, absent
investigation evidence, and the common honest stop reason. This is a node
planner/model distribution result, not a workflow-layer machine failure.

## Event firing table

Counts are per run. Workflow-stream events and concrete node-stream events are
kept separate so the duplicated `workflow_node_run_created` record is not
misread as two nodes.

| Event | Run 1 | Run 2 | Run 3 | Stream / interpretation |
|---|---:|---:|---:|---|
| `workflow_started` | 1 | 1 | 1 | workflow |
| `workflow_edge_fired` (`create->investigate`) | 1 | 1 | 1 | workflow; E-A through E-D passed |
| `workflow_node_started` (`investigate`) | 1 | 1 | 1 | workflow |
| `workflow_node_run_created` | 1 | 1 | 1 | workflow, plus one matching copy in each node stream |
| `intent_resolved` (`investigate`) | 1 | 1 | 1 | concrete node, plus one side-node summary record |
| `plan_preset_resolved` | 1 | 1 | 1 | concrete node |
| `ultra_phase_start` | 1 | 1 | 1 | concrete node, first of three declared phases |
| `investigation_plan_synthesized` | 1 | 1 | 1 | concrete node |
| `ultra_phase_scaffold_complete` | 1 | 1 | 1 | concrete node |
| `ultra_phase_plan_validated` | 1 | 1 | 1 | concrete node |
| `investigation_adjudicated` | 0 | 0 | 0 | not reached because R was not identified |
| `run_stop` | 1 | 1 | 1 | concrete node, failed/static |
| `workflow_adjudicated` | 1 | 1 | 1 | workflow, honest `circle_failed` |
| fix `workflow_node_started` | 0 | 0 | 0 | edge not earned, therefore correctly not started |

Search commands used:

```sh
rg -n '"event":"(workflow_started|workflow_edge_fired|workflow_node_started|workflow_node_run_created|workflow_adjudicated)"' \
  workspace/management/runs/uat-test0722-circle-003/run{1,2,3}/workflow-events.jsonl
rg -n '"event":"(run_start|intent_resolved|plan_preset_resolved|ultra_phase_start|investigation_plan_synthesized|ultra_phase_scaffold_complete|ultra_phase_plan_validated|run_stop)"' \
  workspace/management/runs/uat-test0722-circle-003/run{1,2,3}/node-runs/*/events.jsonl
jq -e '.verdict == "circle_failed" and .reason == "node_failed:investigate" and (.edges | length) == 1' \
  workspace/management/runs/uat-test0722-circle-003/run{1,2,3}/workflow-circle.json
find .anvil -type f -newermt '2026-07-22 10:32:40' -print
```

The last command produced no output.

## Cost record

| Run | Wall seconds | Provider/planner ms | Estimated prompt tokens sent | Prompt eval count | Eval count | Planner retry count |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 359 | 358,661 | 3,207 | 3,041 | 11,847 | 1 |
| 2 | 311 | 311,457 | 4,478 | 4,270 | 10,336 | 3 |
| 3 | 130 | 130,591 | 1,711 | 1,598 | 4,330 | 0 |
| Total | 800 | 800,709 | 9,396 | 8,909 | 26,513 | 4 |

All three report `executor_ms=0`; the rejection occurred at the investigation
reproducer gate immediately after first-phase plan generation and validation.

## Evidence retention

For each `runN/`, this report retains:

- `workflow-events.jsonl` and `workflow-circle.json`;
- `investigate-events.jsonl`;
- the actual node UUID directory with `events.jsonl` and `summary.md`;
- `plans/ultra-plan.yaml` and
  `plans/reproduce-candidate-step-plan.yaml`.

The copied workflow and node event streams were compared to their origin files
with `diff -q`; all comparisons were byte-identical. The human-terminal
`run1.log`/`run2.log`/`run3.log` files remain untracked raw logs and are not part
of the committed audit package.

## Declared acceptance result

- P0-a: PASS — 3/3 honest workflow terminals, classified reasons, no
  panic/hang/timeout/interruption.
- P0-b: PASS — no evidence-free `circle_full`; all three remained
  `circle_failed` under contract §6.
- P1-a: PASS — workflow mechanical class zero, propagation audit (a)–(f)
  remains 3/3, and real fixed three-phase synthesis is 3/3.
- Recorded distribution: valid local arm, `circle_full` 0/3;
  `node_failed:investigate` 3/3 at reproducer identification.
