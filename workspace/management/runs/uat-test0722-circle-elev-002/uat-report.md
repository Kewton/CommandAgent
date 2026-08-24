# uat-test0722-circle-elev-002 report

Date: 2026-07-22
Contract: `docs/workflow-circle-contract.md` v0.1 plus the D-3a-3f §7 carry revision
Product commit: `c41cc9fee40ed073a70f65bf6a3af3a77166ea27`
Workflow: `workflows/recovery-circle-data-elevated.yaml`

## Outcome

All three sequential human-terminal runs reached an honest classified workflow
terminal with no panic, hang, timeout, interruption, confinement violation, or
false edge firing. Every run was `circle_failed`: Runs 1 and 2 ended at
`edge_not_earned:investigate->fix:evidence`, while Run 3 ended at
`node_failed:investigate`. No `circle_full` was projected without evidence.

D-3a-3f's R-supply objective worked in 3/3 runs. Each origin produced a
canonical candidate, the candidate was executed on that origin, a subject
failure was observed, and the exact command plus lineage was persisted in
`workflow-circle.json`, node state, `workflow_reproducer_bound`, and
`investigation-run.json`. The reproduce-candidate phase then executed the same
R and lineage in 3/3 runs.

The elevated executor was reached in 3/3 runs. Each node used
`gemma4:31b-cloud / ollama` for three executor turns plus one repair turn: 12
Gemma turns total. All wrote `output/diagnosis.md` and completed the fixed
three-phase sequence. Runs 1 and 2 earned investigation `partial` because the
diagnoses yielded zero machine-checkable claims. Run 3 produced four claims,
but only one bound to evidence and therefore ended `failed / diagnosis_unbound`.
This is a node-model capability distribution, not an R-resolution failure.

The run also exposed one workflow-layer reporting defect. On Runs 1 and 2 the
investigation adjudication was `partial`, but the `investigate->fix` E-A check
was recorded as passed because `terminal_status_matches` treats any
`run_stop.status=completed` as route verdict `full`. E-B independently required
`investigation_adjudicated.assurance_level=full`, failed, and prevented the
edge from firing. Safety and contract §6 were preserved, but the E-A result and
terminal reason attribution are not contract-faithful. P1-a therefore fails
its “workflow mechanical class zero” clause even though R binding itself passes
3/3. This measurement records the defect; it does not rewrite the evidence or
repair production code.

## Preflight and provenance

Complete commands, source selection, source/copy comparisons, binary identity,
credential boundary, and prevalidation expectations are in
`circle-elev002-run.md`.

- Privileged full suite: 1758 passed / 30 ignored / 0 failed.
- fmt and clippy `-D warnings`: green.
- Installed and release binary:
  `commandagent 0.1.0 c41cc9f+dirty 2026-07-22T13:45:59Z`.
- Installed and release SHA-256:
  `daa4fc3a823fd7876b83f1951c7536c97c2ce0a1f3b2b2bdd31d2b00f3e2fdbe`.
- `NODE_ENV=production`.
- Commit-1 CI: acceptance run `29925343228` success; CI run
  `29925343452` success. Every job in each workflow completed successfully.
- `ollama show gemma4:31b-cloud` confirmed the configured cloud model.
- Credential supply remained inside Ollama account/configuration state; no
  credential value or credential environment variable was passed through
  CommandAgent arguments, YAML, or repository files.

## Run matrix

| Run | Investigate run_id | Wall seconds | Node ms | Circle verdict / reason | Investigate verdict / stop class | Elevated turns |
|---|---|---:|---:|---|---|---:|
| 1 | `019f8a24-dd03-72c2-b80b-9d85ba612c6e` | 47 | 46,342 | `circle_failed` / `edge_not_earned:investigate->fix:evidence` | `partial` / `diagnosis_claims_absent` | 4 |
| 2 | `019f8a25-c7ab-73c0-9f4d-d4e045a31c30` | 33 | 32,631 | `circle_failed` / `edge_not_earned:investigate->fix:evidence` | `partial` / `diagnosis_claims_absent` | 4 |
| 3 | `019f8a26-64cb-7a91-9f28-8faf000f7a25` | 33 | 32,554 | `circle_failed` / `node_failed:investigate` | `failed` / `diagnosis_unbound` | 4 |

Run 3's console capture ends after the Phase 3 verifier, but its node
`investigation_adjudicated` and `run_stop`, workflow
`workflow_adjudicated`, and complete `workflow-circle.json` establish the
terminal independently of the console summary line.

## Origin R derivation and binding audit

| Run | Derivation attempts | Bound R / lineage | Prevalidation | Node execution identity |
|---|---|---|---|---|
| 1 | (c) origin `verify_default_bound` | `test -f pipeline/main.py` / `reproducer:445133cb672eb360` | failure, `subject_failure=true` | exact command and lineage match |
| 2 | (c) origin `verify_default_bound` | `anvil-catalog-check:data_inspection_schema` / `reproducer:33eefa99a5d98b68` | failure: inspection path absent, `subject_failure=true` | exact command and lineage match |
| 3 | (a) pipeline probe passed and was rejected; then (c) origin bound check | `anvil-catalog-check:data_inspection_schema` / `reproducer:33eefa99a5d98b68` | pipeline success not bound; schema failure bound with `subject_failure=true` | exact command and lineage match |

The failed prevalidation in each bound record is also the attempt referenced by
`reproducer_suggestion.bound`; no passing candidate was supplied. The node
state file `externally-bound-reproducer.json`, event
`workflow_reproducer_bound`, synthesized plan `r_basis`, and
`investigation-run.json` agree in every run. This removes the elev-001
planner-dependent R-construction choke point.

## Elevated node and synthesis audit

| Requirement | Result | Evidence |
|---|---|---|
| Effective model/provider | PASS 3/3 | node-created event and concrete `run_start`: `gemma4:31b-cloud / ollama` |
| Planner declaration remains local | PASS 3/3 | `run_start`: `qwen3.6:27b-coding-nvfp4 / ollama`; deterministic synthesis required no planner turn |
| Effective profile | PASS 3/3 | `run_start.profile=data`, `run_stop.effective_profile=data` |
| Investigate preset | PASS 3/3 | `plan_preset=profile`, origin `default_investigate_data` |
| Investigation synthesis | PASS 3/3 | `investigation_plan_synthesized`, `phase_count=3`, bound `r_basis` |
| Three-phase execution | PASS 3/3 | `reproduce-candidate → diagnose → bind-verify`, all phase-complete events |
| Bound R actually executed | PASS 3/3 | `investigation-run.executed=true`, `outcome=failure`, exact lineage |
| Diagnosis materialized | PASS 3/3 | `output/diagnosis.md` exists and is retained with SHA-256 |
| Elevated executor invoked | PASS 3/3 | 3 executor + 1 repair Gemma turns per run |
| Origin confinement | PASS 3/3 | every run_dir is below its corresponding `circle_elev002_origin_N/.anvil/runs/` |
| Run identity | PASS 3/3 | workflow event, circle node map, UUID directory, and node events agree |
| Workflow mechanical-class zero | FAIL | partial node was recorded as E-A pass in Runs 1/2; E-B safely prevented firing |

Diagnosis SHA-256 values are:

- Run 1: `7675bb5881df10a2e36446947688d72a9a3edaf37d528f9197ddfdaf8cf99166`
- Run 2: `513ba1821e9ae905513b4ccd8dd7097705d67ac97cd07cc9c0ee2918de135c6c`
- Run 3: `24088a9c243802b16de918ffc49f1a7dfeb6bf4b9c73465a06289fe7d5b96009`

## Edges, evidence, and contract closure

| Event or closure point | Run 1 | Run 2 | Run 3 | Interpretation |
|---|---:|---:|---:|---|
| `workflow_started` | 1 | 1 | 1 | workflow entry |
| `workflow_reproducer_prevalidated` | 1 | 1 | 2 | Run 3 rejected a passing candidate before binding the failure |
| create→investigate edge fired | 1 | 1 | 1 | E-A through E-D passed |
| investigate node started / UUID created | 1 | 1 | 1 | route-gated and origin-confined |
| `workflow_reproducer_bound` | 1 | 1 | 1 | externally fixed R reached node state |
| `investigation_plan_synthesized` | 1 | 1 | 1 | fixed three phases with bound R |
| elevated provider turns | 4 | 4 | 4 | Gemma executor reached |
| `investigation_adjudicated` | partial | partial | failed | model-produced claim distribution |
| investigate→fix edge fired | 0 | 0 | 0 | no investigate full verdict |
| fix node / F1–F3 evidence | 0 | 0 | 0 | edge not earned |
| verify_origin | 0 | 0 | 0 | fix not reached |
| `workflow_adjudicated` | 1 | 1 | 1 | honest `circle_failed` terminal |

The exact workflow terminal events are:

```json
{"event":"workflow_adjudicated","reason":"edge_not_earned:investigate->fix:evidence","verdict":"circle_failed"}
{"event":"workflow_adjudicated","reason":"edge_not_earned:investigate->fix:evidence","verdict":"circle_failed"}
{"event":"workflow_adjudicated","reason":"node_failed:investigate","verdict":"circle_failed"}
```

No first live investigate→fix earned edge occurred, so there is no fix F1–F3
evidence or verify_origin record to infer. Contract §6 remains safe: neither a
partial nor failed investigate node was washed into `circle_full`.

Search commands used:

```sh
rg -n '"event":"(workflow_started|workflow_reproducer_prevalidated|workflow_edge_fired|workflow_node_started|workflow_node_run_created|workflow_adjudicated)"' \
  workspace/management/runs/uat-test0722-circle-elev-002/run{1,2,3}/workflow-events.jsonl
rg -n '"event":"(workflow_reproducer_bound|run_start|intent_resolved|investigation_plan_synthesized|ultra_phase_complete|investigation_adjudicated|run_stop|time_profile)"' \
  workspace/management/runs/uat-test0722-circle-elev-002/run{1,2,3}/node-runs/*/events.jsonl
jq -r 'select(.event=="provider_turn_duration") | [.caller_scope,.model,.provider] | @tsv' \
  workspace/management/runs/uat-test0722-circle-elev-002/run{1,2,3}/node-runs/*/events.jsonl
jq '.reproducer_suggestion,.edges,.nodes,.verdict,.reason' \
  workspace/management/runs/uat-test0722-circle-elev-002/run{1,2,3}/workflow-circle.json
```

## Cost record

| Run | Wall seconds | Provider ms | Executor ms | Repair ms | Prompt tokens sent | Prompt eval | Eval | Gemma turns |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 47 | 46,342 | 35,141 | 11,201 | 6,768 | 6,748 | 159 | 4 |
| 2 | 33 | 32,631 | 6,930 | 25,701 | 6,747 | 6,948 | 399 | 4 |
| 3 | 33 | 32,554 | 13,387 | 19,167 | 7,050 | 6,896 | 276 | 4 |
| Total | 113 | 111,527 | 55,458 | 56,069 | 20,565 | 20,592 | 834 | 12 |

## Evidence retention and credential scrub

The repository retains the complete workflow stream and circle evidence,
investigate side stream, investigation run/binding, concrete UUID node run and
external-R state, every workflow-generated plan, and diagnosis output. Each
copied workflow, circle, investigation, node event, and diagnosis file was
compared with its origin using `diff -q`; all were byte-identical. Human
terminal `run1.log` through `run3.log` remain untracked raw logs and are not
part of the committed evidence package.

Safety commands:

```sh
python3 workspace/management/scripts/bench.py scrub --path \
  workspace/management/runs/uat-test0722-circle-elev-002
grep -Ern 'AIza[0-9A-Za-z_-]{35}|ghp_[A-Za-z0-9]{36,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{16,}' \
  workspace/management/runs/uat-test0722-circle-elev-002
```

Final bench scrub returned `{"ok": true, "findings": []}`. The explicit
secret-value grep returned no matches across the complete campaign directory,
including all three raw console logs and retained event streams.

## Declared acceptance result

- P0: PASS — 3/3 honest classified terminals, contract §6 preserved, and no
  panic/hang/timeout/interruption.
- P1-a R binding: PASS — canonical R prevalidated, bound, and executed 3/3;
  elevated executor reached 3/3.
- P1-a mechanical-class zero: FAIL — Runs 1/2 expose completed/partial being
  reported as E-A full; E-B prevented unsafe firing.
- P1-b: PASS — final scrub findings zero and explicit secret-value matches
  zero.
- Recorded circle distribution: `circle_full` 0/3, `circle_failed` 3/3.
- Recorded investigate distribution: `partial/diagnosis_claims_absent` 2/3,
  `failed/diagnosis_unbound` 1/3; investigate→fix fired 0/3.
