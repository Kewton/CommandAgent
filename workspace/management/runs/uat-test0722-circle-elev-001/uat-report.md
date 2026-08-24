# uat-test0722-circle-elev-001 report

Date: 2026-07-22
Contract: `docs/workflow-circle-contract.md` v0.1
Product commit: `f3e7605e4b788870a51400f73156b14d5e723a28`
Workflow: `workflows/recovery-circle-data-elevated.yaml`

## Outcome

All three sequential human-terminal runs reached an honest workflow terminal.
Each was adjudicated `circle_failed` with
`reason=node_failed:investigate`; no panic, hang, timeout, interruption,
confinement violation, layout error, or edge misfire occurred. The
create-to-investigate edge earned E-A through E-D and fired once in every run.
The investigate node then stopped honestly because its first-phase StepPlan
did not contain the contract-shaped failing reproducer R.

The node configuration was elevated exactly as declared:
`workflow_node_run_created` and the concrete node `run_start` both record
model `gemma4:31b-cloud` and provider `ollama` in 3/3 runs. However, the
reproducer gate rejected the local qwen27 planner output before any executor
turn. Every `time_profile` therefore has `executor_ms=0`, and every observed
provider turn is `planner_step` using `qwen3.6:27b-coding-nvfp4 / ollama`.
This campaign measures the local-planner choke point in an elevated-node
configuration; it does not establish a Gemma executor capability rate.

## Preflight and model materialization

The complete preflight, source selection, copy verification, credential
boundary, installed binary identity, and exact commands are recorded in
`circle-elev001-run.md`.

- Privileged full suite: 1747 passed / 30 ignored / 0 failed.
- Focused workflow schema tests: 4 passed / 0 failed.
- Installed and release binary both reported
  `commandagent 0.1.0 f3e7605+dirty 2026-07-22T07:46:37Z` and shared SHA-256
  `78ba1b6694006195862ad5ac015338a82b53727b5fe813bbaa4bedf5d81d477e`.
- `NODE_ENV=production`.
- `ollama show gemma4:31b-cloud` confirmed architecture `gemma4`,
  32,682,372,656 parameters, context length 262,144, BF16, and
  completion/thinking/tools/vision capabilities.
- Credential supply remained inside Ollama account/configuration state; no
  secret value or credential environment variable was passed to CommandAgent,
  YAML, or a repository file.
- The three commands ran sequentially and without monitoring or interruption;
  each prompt returned before the next command started.

## Run matrix

| Run | Investigate run_id | Wall epoch | Wall seconds | Node profile ms | Circle verdict / reason | Node verdict / assurance | Node stop class |
|---|---|---|---:|---:|---|---|---|
| 1 | `019f88d7-e2d5-7fe1-9855-eb52d9a16567` | 1784707277–1784707742 | 465 | 464,836 | `circle_failed` / `node_failed:investigate` | `failed` / `static` | `investigation_reproducer_not_identified` |
| 2 | `019f88df-2bc5-7d33-98d2-344224c70ae9` | 1784707754–1784708266 | 512 | 511,789 | `circle_failed` / `node_failed:investigate` | `failed` / `static` | `investigation_reproducer_not_identified` |
| 3 | `019f88e7-259f-7981-a0f5-a0238154fd1a` | 1784708277–1784708412 | 135 | 135,199 | `circle_failed` / `node_failed:investigate` | `failed` / `static` | `investigation_reproducer_not_identified` |

All node `run_stop` events carry `status=failed`,
`assurance_level=static`, `assurance_reason=investigation_probe_not_run`, and
`stop_reason=investigation reproducer was not identified`.

## Elevated configuration and synthesis audit

| Requirement | Result | Evidence |
|---|---|---|
| Effective executor configuration | PASS 3/3 | workflow `node_run_created` and concrete `run_start`: `gemma4:31b-cloud / ollama` |
| Planner remains local qwen27 | PASS 3/3 | `run_start.planner_model=qwen3.6:27b-coding-nvfp4`, `planner_provider=ollama` |
| Effective profile | PASS 3/3 | `run_start.profile=data`, `run_stop.effective_profile=data` |
| Default investigate preset | PASS 3/3 | `plan_preset=profile`, source/origin `default_investigate_data` |
| Investigation synthesis | PASS 3/3 | one `investigation_plan_synthesized`, `phase_count=3`, `profile=data` per node |
| Fixed three phases | PASS 3/3 | harvested UltraPlan order is `reproduce-candidate → diagnose → bind-verify` |
| Origin confinement | PASS 3/3 | every declared node `run_dir` is below its matching `circle_elev001_origin_N/.anvil/runs/` |
| Run identity | PASS 3/3 | workflow event, circle node map, UUID directory, and node events agree |
| Honest terminal | PASS 3/3 | exactly one `workflow_adjudicated`; no `circle_full` and no missing terminal |
| Elevated executor invoked | NOT REACHED 0/3 | `executor_ms=0`; provider turns were qwen27 planner only |

The accepted StepPlans had 6, 4, and 4 steps. None supplied the single verify
step with `expected_result=fail` and exactly one command required by
`investigation_runtime::extract_reproducer`: Run 1 used only pass-expected
file checks, Run 2 had one pass-expected `python pipeline/main.py`, and Run 3
used pass-expected checks with inconsistent `pipeline/main.py` versus
`pipeline/reproducer.py` paths. The honest gate stopped before tool execution,
so no `output/diagnosis.md`, I1/I2 investigation evidence, or executor turn was
created.

## Route and contract closure

| Event or closure point | Run 1 | Run 2 | Run 3 | Interpretation |
|---|---:|---:|---:|---|
| `workflow_started` | 1 | 1 | 1 | workflow entry |
| `workflow_edge_fired` (`create->investigate`) | 1 | 1 | 1 | E-A through E-D passed |
| investigate `workflow_node_started` | 1 | 1 | 1 | route-gated node start |
| investigate `workflow_node_run_created` | 1 | 1 | 1 | real UUID and elevated config |
| `investigation_plan_synthesized` | 1 | 1 | 1 | fixed three-phase contract |
| investigate `run_stop` | 1 | 1 | 1 | failed/static, R not identified |
| `workflow_adjudicated` | 1 | 1 | 1 | honest `circle_failed` |
| `investigate->fix` edge fired | 0 | 0 | 0 | investigate verdict was not full |
| fix node started / F evidence | 0 | 0 | 0 | edge not earned |
| `verify_origin` | 0 | 0 | 0 | fix was not reached |

No first live `investigate->fix` earned-edge firing occurred. There is no fix
F1–F3 evidence and no origin verification record to infer. Contract §6 is
preserved: a non-full investigate node cannot be washed into `circle_full`.

Search commands used:

```sh
rg -n '"event":"(workflow_started|workflow_edge_fired|workflow_node_started|workflow_node_run_created|workflow_adjudicated)"' \
  workspace/management/runs/uat-test0722-circle-elev-001/run{1,2,3}/workflow-events.jsonl
rg -n '"event":"(run_start|intent_resolved|plan_preset_resolved|investigation_plan_synthesized|run_stop|time_profile)"' \
  workspace/management/runs/uat-test0722-circle-elev-001/run{1,2,3}/node-runs/*/events.jsonl
jq -r 'select(.event=="provider_turn_duration") | [.caller_scope,.model,.provider] | @tsv' \
  workspace/management/runs/uat-test0722-circle-elev-001/run{1,2,3}/node-runs/*/events.jsonl
```

## Credential scrub and evidence retention

The harvested workflow/circle/node streams were compared with their origin
files using `diff -q`; all comparisons were byte-identical. For every run the
repository retains the workflow stream, complete circle evidence, investigate
side stream, concrete UUID node events and summary, fixed UltraPlan, and
first-phase StepPlan. The human-terminal `run1.log` through `run3.log` remain
untracked raw logs and are not part of the committed audit package.

Safety commands:

```sh
python3 workspace/management/scripts/bench.py scrub --path \
  workspace/management/runs/uat-test0722-circle-elev-001
grep -Ern 'AIza[0-9A-Za-z_-]{35}|ghp_[A-Za-z0-9]{36,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{16,}' \
  workspace/management/runs/uat-test0722-circle-elev-001
```

Final bench scrub result: `{"ok": true, "findings": []}`. The explicit
value-pattern grep returned no matches across the complete directory,
including all three console logs and event streams. P1-b therefore passes.

## Cost record

| Run | Wall seconds | Provider/planner ms | Estimated prompt tokens sent | Prompt eval count | Eval count | Planner turns | Executor ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 465 | 464,836 | 4,512 | 4,294 | 15,418 | 3 | 0 |
| 2 | 512 | 511,789 | 4,493 | 4,279 | 17,062 | 3 | 0 |
| 3 | 135 | 135,199 | 1,711 | 1,598 | 4,481 | 1 | 0 |
| Total | 1,112 | 1,111,824 | 10,716 | 10,171 | 36,961 | 7 | 0 |

## Declared acceptance result

- P0-a: PASS — 3/3 honest classified workflow terminals; no
  panic/hang/timeout/interruption.
- P0-b: PASS — no evidence-free `circle_full`; all three remained
  `circle_failed` under contract §6.
- P1-a: PASS at the declared configuration/mechanical boundary — effective
  node model, profile, synthesis, confinement, and route mechanics are 3/3.
  Capability caveat: Gemma executor invocation is 0/3 because the local
  planner did not earn R, so this is not yet an elevated-executor performance
  denominator.
- P1-b: PASS — final scrub findings zero and explicit secret-value pattern
  matches zero across harvested evidence and raw console logs.
- Recorded distribution: `circle_full` 0/3,
  `node_failed:investigate` 3/3; `investigate->fix` earned-edge firing 0/3.
