# uat-test0722-circle-elev-003 report

Date: 2026-07-22
Contract: `docs/workflow-circle-contract.md` v0.1 plus the D-3a-3f §7 carry revision
Product commit: `922d08d4975105b5dbdd5863ef66eaf87c9f0ace`
Workflow: `workflows/recovery-circle-data-elevated.yaml`

## Outcome

All three sequential human-terminal runs reached an honest classified workflow
terminal with no panic, hang, timeout, interruption, confinement violation, or
false edge firing. Every run was `circle_failed`. Run 1 ended at
`node_failed:investigate`; Runs 2 and 3 ended at
`edge_not_earned:investigate->fix:assurance_below_required`. No
`circle_full` was projected without evidence.

D-3a-3g's E-A correction worked in every applicable edge evaluation. The two
completed-but-partial investigate nodes were recorded as
`assurance=partial is below required=full`, E-A was false, and the terminal
reason named `assurance_below_required`. Run 1 was already an honestly failed
node, so the orchestrator terminated it before evaluating the outgoing edge.
E-B remained false for the two partial nodes, preserving the independent
evidence-existence defense.

The diagnose input objective also worked mechanically in 3/3 runs. Every
concrete diagnose plan contains the executed R command, failure summary,
deterministic excerpt, bounded stdout/stderr tails, traceback availability, and
the minimum-claim requirement. Every generated diagnosis quoted the injected
failure text exactly. The matcher was not relaxed.

End-to-end I2 nevertheless produced zero matches: 1 claim / 0 matches / 1
violation in Run 1 and 0 / 0 / 0 in Runs 2 and 3. Inspection of the fixed
matcher explains the boundary mismatch. Inline error claims are recognized
only when the quoted value contains `Error`, `Exception`, or `Traceback`; the
actual canonical R outputs are `CommandFailed` or
`inspection_schema_violation`, so exact quotes in Runs 2 and 3 remained
`diagnosis_claims_absent`. Run 1 additionally fenced the reproducer command;
the generic fence parser treated it as a source-code claim without a subject
file and recorded one violation, producing `diagnosis_unbound`. This is a new
investigation generation-to-binding mechanical defect exposed by the
measurement. It is recorded without changing the checker or historical
evidence.

Accordingly, the workflow routing subset of P1-a passes (E-A precision,
confinement, route gating, and honest terminal), but the predeclared end-to-end
mechanical-class-zero P1-a fails because the deterministic material and I2
accepted vocabulary do not meet. No investigate→fix edge fired, so no fix
F1–F3 or verify_origin evidence exists to transfer.

## Preflight and provenance

Complete commands, source selection, source/copy comparisons, binary identity,
credential boundary, and prevalidation expectations are in
`circle-elev003-run.md`.

- Privileged full suite: 1773 passed / 30 ignored / 0 failed.
- fmt and clippy `-D warnings`: green.
- Installed and release binary:
  `commandagent 0.1.0 922d08d+dirty 2026-07-22T14:52:18Z`.
- Installed and release SHA-256:
  `1440b4fc13dce84119a47d2cef4f961870971caf8bc06cf9a4f9fbde0a85a458`.
- `NODE_ENV=production`.
- Implementation CI: acceptance run `29930650687` success; CI run
  `29930649796` success. Every job in both workflows completed successfully.
- `ollama show gemma4:31b-cloud` confirmed the configured cloud model.
- Credentials remained inside Ollama account/configuration state; no credential
  value or credential environment variable was passed through CommandAgent
  arguments, YAML, or repository files.

## Run matrix

| Run | Investigate run_id | Wall seconds | Node ms | Circle verdict / reason | Investigate verdict / stop class | Gemma turns |
|---|---|---:|---:|---|---|---:|
| 1 | `019f8a55-2d3f-7a43-ae26-c9cc92032498` | 6 | 5,858 | `circle_failed` / `node_failed:investigate` | `failed` / `diagnosis_unbound` | 4 |
| 2 | `019f8a55-7b60-7c22-ac8e-6c24c1b8f1cb` | 10 | 9,448 | `circle_failed` / `edge_not_earned:investigate->fix:assurance_below_required` | `partial` / `diagnosis_claims_absent` | 4 |
| 3 | `019f8a55-ca2a-7b93-91f6-a0107c687493` | 4 | 3,462 | `circle_failed` / `edge_not_earned:investigate->fix:assurance_below_required` | `partial` / `diagnosis_claims_absent` | 2 |

All three console captures reached the final epoch after the prompt returned.
The wall times were 6, 10, and 4 seconds, respectively. The node event streams
contain `investigation_adjudicated` and `run_stop`, and each workflow stream
contains `workflow_adjudicated`; the result does not rely on console wording.

## Origin R derivation and binding audit

| Run | Derivation attempts | Bound R / lineage | Prevalidation | Node execution identity |
|---|---|---|---|---|
| 1 | (c) origin `verify_default_bound` | `test -f pipeline/main.py` / `reproducer:445133cb672eb360` | failure, `subject_failure=true` | exact command and lineage match |
| 2 | (c) origin bound catalog check | `anvil-catalog-check:data_inspection_schema` / `reproducer:33eefa99a5d98b68` | inspection path absent, `subject_failure=true` | exact command and lineage match |
| 3 | (a) pipeline probe passed and was rejected; then (c) origin bound check | `anvil-catalog-check:data_inspection_schema` / `reproducer:33eefa99a5d98b68` | schema missing-keys failure, `subject_failure=true` | exact command and lineage match |

The bound attempt is the failed attempt selected in
`reproducer_suggestion.bound`; no passing candidate was supplied. Node state,
`workflow_reproducer_bound`, synthesized `r_basis`, and
`investigation-run.json` agree in all runs.

## Elevated node and synthesis audit

| Requirement | Result | Evidence |
|---|---|---|
| Effective model/provider | PASS 3/3 | node-created and concrete `run_start`: `gemma4:31b-cloud / ollama` |
| Planner remains local | PASS 3/3 | `run_start`: `qwen3.6:27b-coding-nvfp4 / ollama` |
| Effective profile | PASS 3/3 | `run_start.profile=data`, `run_stop.effective_profile=data` |
| Investigate preset | PASS 3/3 | `plan_preset=profile`, origin `default_investigate_data` |
| Investigation synthesis | PASS 3/3 | `investigation_plan_synthesized`, `phase_count=3`, bound `r_basis` |
| Three-phase execution | PASS 3/3 | all `reproduce-candidate → diagnose → bind-verify` phase-complete events |
| Bound R executed | PASS 3/3 | `investigation-run.executed=true`, failure outcome, exact lineage |
| R output injected into diagnose | PASS 3/3 | concrete diagnose plan includes exact command, summary, excerpt, tails, and requirement |
| Injected quote materialized | PASS 3/3 | each diagnosis contains the exact R failure string |
| Elevated executor invoked | PASS 3/3 | Gemma provider turns: 4 / 4 / 2 |
| Origin confinement | PASS 3/3 | every run_dir is below its corresponding `circle_elev003_origin_N/.anvil/runs/` |
| Run identity | PASS 3/3 | workflow event, circle node map, UUID directory, and node events agree |
| E-A assurance precision | PASS 2/2 applicable | partial is displayed as partial and attributed `assurance_below_required` |
| Workflow routing mechanical class | PASS | no deadlock, misfire, leakage, or false projection |
| End-to-end mechanical class | FAIL | injected schema/command failure quotes are outside I2's accepted error vocabulary |

Diagnosis SHA-256 values:

- Run 1: `8d41d6415fc739a017cd187a9a5f4422d2688e80666f694fb4d0ba605865d15a`
- Run 2: `df03a3b93ac92f93101652dbf055f458eaad6aa4f4504f23b90f43ce23d1a4b5`
- Run 3: `95dddfbd38c0f0245186645f700a9cdf490fc560511b1c32122291bef4bb9057`

## I2 claim statistics

| Run | Claims | Matches | Violations | Assurance | Observed extraction |
|---|---:|---:|---:|---|---|
| 1 | 1 | 0 | 1 | failed / `diagnosis_unbound` | fenced `test -f pipeline/main.py` became an unbound `code_snippet`; exact `CommandFailed` quote was not recognized as an error claim |
| 2 | 0 | 0 | 0 | partial / `diagnosis_claims_absent` | exact `inspection_schema_violation:inspection_path:...` quote was not recognized |
| 3 | 0 | 0 | 0 | partial / `diagnosis_claims_absent` | exact `inspection_schema_violation:missing_keys:...` quote was not recognized |
| Total | 1 | 0 | 1 | full 0/3 | first in-circle match remains unobserved |

The injected prompt explicitly said that zero machine-checkable claims cannot
be full, and adjudication enforced that rule. The finding is therefore not an
assurance relaxation or unsafe projection; it is a producer/consumer vocabulary
gap that keeps honest assurance below full.

## Edges and contract closure

| Event or closure point | Run 1 | Run 2 | Run 3 | Interpretation |
|---|---:|---:|---:|---|
| `workflow_started` | 1 | 1 | 1 | workflow entry |
| `workflow_reproducer_prevalidated` | 1 | 1 | 2 | Run 3 rejected a passing candidate before binding the failure |
| create→investigate edge fired | 1 | 1 | 1 | E-A through E-D passed |
| investigate node started / UUID created | 1 | 1 | 1 | route-gated and origin-confined |
| `workflow_reproducer_bound` | 1 | 1 | 1 | externally fixed R reached node state |
| `investigation_plan_synthesized` | 1 | 1 | 1 | fixed three phases with bound R |
| injected R quote in diagnosis | 1 | 1 | 1 | deterministic diagnose material was consumed |
| `investigation_adjudicated` | failed | partial | partial | I2 result, not run_stop projection |
| investigate→fix edge fired | 0 | 0 | 0 | no investigate full verdict |
| fix node / F1–F3 evidence | 0 | 0 | 0 | edge not earned |
| verify_origin | 0 | 0 | 0 | fix not reached |
| `workflow_adjudicated` | 1 | 1 | 1 | honest `circle_failed` terminal |

Exact workflow terminal events:

```json
{"event":"workflow_adjudicated","reason":"node_failed:investigate","verdict":"circle_failed"}
{"event":"workflow_adjudicated","reason":"edge_not_earned:investigate->fix:assurance_below_required","verdict":"circle_failed"}
{"event":"workflow_adjudicated","reason":"edge_not_earned:investigate->fix:assurance_below_required","verdict":"circle_failed"}
```

Search commands used:

```sh
grep -RnE '"event":"(workflow_started|workflow_reproducer_prevalidated|workflow_edge_fired|workflow_node_started|workflow_node_run_created|workflow_adjudicated)"' \
  workspace/management/runs/uat-test0722-circle-elev-003/run{1,2,3}/workflow-events.jsonl
grep -RnE '"event":"(workflow_reproducer_bound|run_start|intent_resolved|investigation_plan_synthesized|ultra_phase_complete|investigation_adjudicated|run_stop|time_profile)"' \
  workspace/management/runs/uat-test0722-circle-elev-003/run{1,2,3}/node-runs/*/events.jsonl
grep -RnE '実行済みRの失敗観測|deterministic excerpt|診断には最低1件のエラー引用' \
  workspace/management/runs/uat-test0722-circle-elev-003/run{1,2,3}/plans
jq '.reproducer_suggestion,.edges,.nodes,.verdict,.reason' \
  workspace/management/runs/uat-test0722-circle-elev-003/run{1,2,3}/workflow-circle.json
jq '{claims:(.claims|length),matched:([.claims[]|select(.matched==true)]|length),violations:([.claims[]|select(.matched!=true)]|length)}' \
  workspace/management/runs/uat-test0722-circle-elev-003/run{1,2,3}/investigation-binding.json
```

## Cost record

| Run | Wall seconds | Provider ms | Executor ms | Repair ms | Prompt tokens sent | Prompt eval | Eval | Gemma turns |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 6 | 5,858 | 2,211 | 3,647 | 7,456 | 7,772 | 314 | 4 |
| 2 | 10 | 9,448 | 4,574 | 4,874 | 7,628 | 7,660 | 426 | 4 |
| 3 | 4 | 3,462 | 1,022 | 2,440 | 3,981 | 3,966 | 339 | 2 |
| Total | 20 | 18,768 | 7,807 | 10,961 | 19,065 | 19,398 | 1,079 | 10 |

## Evidence retention and credential scrub

The repository retains the complete workflow stream and circle evidence,
investigate side stream, investigation run/binding, concrete UUID node run and
external-R state, every workflow-generated plan (including the actual diagnose
prompt), prevalidation catalog evidence when applicable, and diagnosis output.
Each copied file was compared with its origin and is byte-identical. Human
terminal `run1.log` through `run3.log` remain untracked raw logs and are not
part of the committed evidence package.

Safety commands:

```sh
python3 workspace/management/scripts/bench.py scrub --path \
  workspace/management/runs/uat-test0722-circle-elev-003
grep -Ern 'AIza[0-9A-Za-z_-]{35}|ghp_[A-Za-z0-9]{36,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{16,}' \
  workspace/management/runs/uat-test0722-circle-elev-003
```

Final bench scrub returned `{"ok":true,"findings":[]}`. The explicit
secret-value grep returned no matches across the complete campaign directory,
including all three raw console logs and retained event streams.

## Declared acceptance result

- P0: PASS — 3/3 honest classified terminals, contract §6 preserved, and no
  panic, hang, timeout, or interruption.
- P1-a routing and E-A precision: PASS — partial was never projected to full,
  reasons are precise, and workflow mechanics remained confined and gated.
- P1-a end-to-end mechanical-class zero: FAIL — injection succeeded, but the
  I2 extractor did not recognize the actual canonical failure vocabulary and
  misclassified a fenced reproducer as source code.
- P1-b: PASS — final scrub findings zero and explicit secret-value matches
  zero.
- Recorded circle distribution: `circle_full` 0/3, `circle_failed` 3/3.
- Recorded investigate distribution: `failed/diagnosis_unbound` 1/3,
  `partial/diagnosis_claims_absent` 2/3; investigate→fix fired 0/3.
- I2 distribution: claims 1, matches 0, violations 1; first in-circle match did
  not occur.
