# MVP UAT

Date: 2026-06-16

## Scope

This UAT checks the current CommandAgent migration state against the MVP work
plan.

## Commands Run

```bash
target/release/commandagent --help
scripts/eval_agent_slice.sh --dry-run --out /tmp/commandagent-uat-eval --runs 1
scripts/eval_large_tasks.sh --dry-run --out /tmp/commandagent-uat-large --runs 1
curl -fsS http://127.0.0.1:11434/api/tags
/usr/bin/expect -c '<run /plan-run --profile docs with qwen3.6:27b-coding-nvfp4>'
```

## Results

| Check | Result | Notes |
| --- | --- | --- |
| Release binary help | Pass | `target/release/commandagent --help` prints expected CLI usage. |
| Eval slice dry-run | Pass | Run root created under `/private/tmp/commandagent-uat-eval`. |
| Large eval dry-run | Pass | Run root created under `/private/tmp/commandagent-uat-large`. |
| Ollama connection | Pass | Local Ollama is reachable and includes coding models. |
| REPL prompt loop | Covered by unit tests | `agent::repl` tests pass in smoke. |
| Simple file create live UAT | Pass | `/plan-run --profile docs` with `qwen3.6:27b-coding-nvfp4` created `README.md`, saved a plan, and saved a session. |
| Next.js small app live UAT | Pending | `/ultra-plan-run` REPL dispatch is now wired; live model run still needed. |
| Repair fallback live UAT | Pending | Runtime dispatch exists; live failure/repair scenario still needed. |
| Planner/executor split live UAT | Pending | Config and dispatcher exist; live mixed-provider run still needed. |
| Python/Rust smoke live UAT | Not run yet | Should run after planner/step execution wiring lands. |

## Live Notes

`qwen3:8b` was also tried for the same docs `/plan-run` flow. It reached the
planner path but returned invalid step-plan YAML even after one correction
attempt. This is recorded as a model capability/contract-following limitation,
not a runtime dispatch failure.

The successful 27B run produced:

- `/private/tmp/commandagent-live-uat-docs27b/README.md`
- `/private/tmp/commandagent-live-uat-docs27b/.commandagent/plans/plan-*.yaml`
- `/private/tmp/commandagent-live-uat-docs27b/.commandagent/sessions/*/session.json`

## Previous Blocking Finding

`/ultra-plan-run` is an MVP feature, but the current REPL only sends every
non-exit line to the minimal loop. `agent/slash_command` can parse plan
commands, and `agent/step_runner` contains schemas, verifier, repair artifacts,
profiles, and ultra execution core. The missing piece is runtime dispatch from
REPL slash commands into planner generation and step execution.

Status update: this gap has been addressed in code with a REPL dispatch path
and regression tests. Live UAT remains pending.

## Next Required Work

Rerun this UAT with live simple file, Next.js, repair fallback,
planner/executor split, Python, and Rust checks.
