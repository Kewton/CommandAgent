# uat-test0722-circle-elev-001 execution runbook

Status: all three human-terminal runs completed; phase 3 evidence harvested.

## Elevated model selection

The discovery command was:

```sh
ollama list | grep -i cloud
```

It returned no match during the initial preparation. The subsequent explicit
measurement directive nevertheless fixed the executor to
`gemma4:31b-cloud` through provider `ollama`; that directive is the final model
selection for this campaign.

The execution terminal and phase 3 recovery both confirmed cloud reachability
with `ollama show gemma4:31b-cloud`. The recovered model facts were
architecture `gemma4`, 32,682,372,656 parameters, context length 262,144,
BF16, with completion/thinking/tools/vision capabilities.

The repository corpus contains a historical real provider event for this exact
model/provider form:

```text
tests/corpus/apps/test0708_011/fixtures/events-anchor-miss.jsonl:1
event=run_start
model=gemma4:31b-cloud
provider=ollama
```

The v0.1 workflow schema permits only executor overrides. Therefore both the
investigate and fix nodes are fixed to model `gemma4:31b-cloud` and provider
`ollama`; the workflow launcher's global planner remains the local
`qwen3.6:27b-coding-nvfp4` / `ollama` configuration.

Materialization commit: `f3e7605e4b788870a51400f73156b14d5e723a28`.

## Credential safety boundary

The credential supply path is Ollama's internal account/configuration state.
No credential value or credential environment variable is passed through a
CommandAgent argument, the workflow YAML, or a repository file. Never paste an
Ollama credential into this runbook, YAML, a command argument, console log, or
Codex message.

Pre-run safety checks:

```sh
python3 workspace/management/scripts/bench.py scrub --path workflows
grep -En 'AIza[0-9A-Za-z_-]{35}|ghp_[A-Za-z0-9]{36,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{16,}' workflows/recovery-circle-data-elevated.yaml
```

The scrub returned `{"ok": true, "findings": []}`; the explicit value-pattern
grep returned no matches during preparation.

## Preflight record

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- `git status --porcelain=v1 --untracked-files=all`: only the nine accepted raw
  logs under `uat-test0722-circle-001/`, `002/`, and `003/`.
- HEAD: `f3e7605e4b788870a51400f73156b14d5e723a28`, later than required
  baseline `5fffda9`.
- Privileged `cargo test`: green, 1747 passed / 30 ignored / 0 failed.
- Focused workflow schema tests: 4 passed / 0 failed.
- Release/install commands:

  ```sh
  cargo build --release --locked
  cargo install --path . --locked --root /Users/maenokota/.local --force
  ```

- `which commandagent`: `/Users/maenokota/.local/bin/commandagent`
- Installed `commandagent --version`:
  `commandagent 0.1.0 f3e7605+dirty 2026-07-22T07:46:37Z`
- Release `./target/release/commandagent --version`: identical.
- Installed and release SHA-256:
  `78ba1b6694006195862ad5ac015338a82b53727b5fe813bbaa4bedf5d81d477e`
- Version and binary digest match: yes.
- The `+dirty` suffix is caused solely by the nine accepted, untracked raw
  logs; `git status` reported no tracked modification before this runbook.
- `printenv NODE_ENV`: `production`.

## Origin procurement

Each archived source and fresh copy was verified with these commands:

```sh
rg -n '"event":"run_stop".*"status":"failed"' <source-or-origin>/.anvil/runs
find <source-or-origin>/.anvil/plans -name 'recovery-*.yaml' -print
diff -qr <source> <origin>
find <origin> -name .git -print
```

`diff -qr` and `.git` searches produced no output for all three fresh copies.

| Origin | Archived source | Failed create run | Recovery YAMLs |
|---|---|---|---|
| 1 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002` | `019f6951-e16e-7fc0-84a9-86f7657258ba`; `run_stop.status=failed` | `recovery-ultra-plan-read-only-stagnation-019f695c-3253-7910-9f51-6c0c104e56ef.yaml`; `recovery-ultra-plan-phase-data-aggregation-019f695c-3255-7160-b624-cd19ecf8cf4d.yaml` |
| 2 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_001` | `019f6940-466d-7912-9d32-6f4369fbfeeb`; `run_stop.status=failed` | `recovery-ultra-plan-phase-data-inspection-019f6948-b537-7d50-9f89-c1234229bbaf.yaml`; `recovery-ultra-plan-read-only-stagnation-019f6948-b536-7813-8ebb-04fdd16523ef.yaml` |
| 3 | `workspace/management/runs/uat-test0715-data-005/artifacts/data5_qwen35_profile_001` | `019f6476-d96f-7e20-abda-0094031f600e`; `run_stop.status=failed` | `recovery-ultra-plan-phase-data-cleaning-019f6482-0775-7241-bf2c-28a13c81849f.yaml`; `recovery-ultra-plan-read-only-stagnation-019f6482-0773-77b2-9ac8-29316e6eada7.yaml` |

Fresh origins:

1. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev001_origin_1`
2. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev001_origin_2`
3. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev001_origin_3`

## Phase 2: human-terminal execution

Run from `/Users/maenokota/share/work/github_kewton/CommandAgent-develop` in a
normal terminal. Confirm that `which commandagent` prints
`/Users/maenokota/.local/bin/commandagent` and `commandagent --version`
contains `f3e7605`.

Execute exactly one command at a time. After pressing Enter, do not monitor,
inspect, collect, run another command, or manipulate the session until the
prompt returns. Only after the prompt returns may the next command be started.
Do not run these in parallel and do not interrupt them.

### Run 1

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev001_origin_1 ; date +%s
```

### Run 2

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev001_origin_2 ; date +%s
```

### Run 3

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev001_origin_3 ; date +%s
```

When all three prompts have returned, send `完了` to Codex. Do not alter any
origin after completion; phase 3 will recover and scrub evidence from these
paths.
