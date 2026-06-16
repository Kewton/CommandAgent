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
```

## Results

| Check | Result | Notes |
| --- | --- | --- |
| Release binary help | Pass | `target/release/commandagent --help` prints expected CLI usage. |
| Eval slice dry-run | Pass | Run root created under `/private/tmp/commandagent-uat-eval`. |
| Large eval dry-run | Pass | Run root created under `/private/tmp/commandagent-uat-large`. |
| Ollama connection | Pass | Local Ollama is reachable and includes coding models. |
| REPL prompt loop | Covered by unit tests | `agent::repl` tests pass in smoke. |
| Simple file create live UAT | Not run yet | Requires a live model run after slash execution gap is resolved. |
| Next.js small app live UAT | Pending | `/ultra-plan-run` REPL dispatch is now wired; live model run still needed. |
| Repair fallback live UAT | Pending | Runtime dispatch exists; live failure/repair scenario still needed. |
| Planner/executor split live UAT | Pending | Config and dispatcher exist; live mixed-provider run still needed. |
| Python/Rust smoke live UAT | Not run yet | Should run after planner/step execution wiring lands. |

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
