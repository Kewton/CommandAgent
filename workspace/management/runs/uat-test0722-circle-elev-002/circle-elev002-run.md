# uat-test0722-circle-elev-002 execution runbook

Status: all three human-terminal runs completed; phase 3 evidence harvested.

## Implementation and preflight

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Implementation commit: `c41cc9fee40ed073a70f65bf6a3af3a77166ea27`
  (`Bind origin reproducers into workflow nodes`), pushed to `origin/develop`.
- The only pre-existing worktree entries are the twelve accepted, untracked raw
  logs under the circle-001, circle-002, circle-003, and circle-elev-001 run
  directories. No tracked modification was present before this runbook.
- Privileged `cargo test`: green, 1758 passed / 30 ignored / 0 failed.
- `cargo fmt --all -- --check`: green.
- `cargo clippy --all-targets -- -D warnings`: green.
- Release/install commands:

  ```sh
  cargo build --release --locked
  cargo install --path . --locked --root /Users/maenokota/.local --force
  ```

- `which commandagent`: `/Users/maenokota/.local/bin/commandagent`
- Installed and release `--version` (identical):
  `commandagent 0.1.0 c41cc9f+dirty 2026-07-22T13:45:59Z`
- Installed and release SHA-256 (identical):
  `daa4fc3a823fd7876b83f1951c7536c97c2ce0a1f3b2b2bdd31d2b00f3e2fdbe`
- The `+dirty` suffix is caused only by the twelve accepted raw logs.
- `printenv NODE_ENV`: `production`.

## Elevated model and credential boundary

The workflow fixes both the investigate and fix executors to
`gemma4:31b-cloud` through provider `ollama`. `ollama show
gemma4:31b-cloud` confirmed the configured cloud model (gemma4,
32,682,372,656 parameters, context 262,144, BF16, completion/thinking/tools/
vision). Planner configuration remains the launcher's local
`qwen3.6:27b-coding-nvfp4` / `ollama` configuration.

Credentials are supplied only through Ollama's internal account/configuration
state. No credential value or credential environment variable is present in a
command argument, workflow YAML, or repository file. The preparation checks
were:

```sh
python3 workspace/management/scripts/bench.py scrub --path workflows
grep -En 'AIza[0-9A-Za-z_-]{35}|ghp_[A-Za-z0-9]{36,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{16,}' workflows/recovery-circle-data-elevated.yaml
```

Scrub returned `{"ok": true, "findings": []}` and grep returned no match.

## Origin procurement and R prevalidation basis

The campaign deliberately uses three distinct failed create×data origins whose
archived state contains a prevalidation-negative canonical R. This preserves
the elevated arm's three-origin design while making the D-3a-3f acceptance
target (a bound, actually executed R in 3/3 nodes) measurable. Each source and
fresh copy was checked with:

```sh
grep -RniE '"event"\s*:\s*"run_stop".*"status"\s*:\s*"failed"' <source-or-origin>/.anvil/runs
find <source-or-origin>/.anvil/plans -name 'recovery-*.yaml' -print
diff -qr <source> <origin>
find <origin> -name .git -print
```

All three `diff -qr` and `.git` searches returned no output.

| Origin | Archived source | Failed create run | Recovery YAML | Expected canonical R source and precheck |
|---|---|---|---|---|
| 1 | `workspace/management/runs/uat-test0715-data-007/artifacts/data7_qwen35_none_002` | `019f65d3-ae61-7b81-b96d-9d5f871768b1` | `recovery-ultra-plan-phase-validate-and-clean-data-019f65d7-4121-7541-8333-d36a4d73f8f6.yaml` | (c) archived `verify_default_bound`: `test -f pipeline/main.py`; fresh-copy execution exit 1 |
| 2 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_none_002` | `019f6961-6c4e-7020-a97b-5af570d660db` | `recovery-ultra-plan-phase-load-and-validate-data-019f6969-8372-7593-a21d-f8516e810352.yaml` | (c) archived `verify_default_bound`: `anvil-catalog-check:data_inspection_schema`; `output/inspection.json` is absent, so the profile-owned check is expected to fail |
| 3 | `workspace/management/runs/uat-test0715-data-007/artifacts/data7_gemma31_none_001` | `019f65f7-b518-7241-b015-9a0e7272c241` | `recovery-ultra-plan-phase-load-and-inspect-data-019f6602-718c-74c3-ab04-f057f28a6b41.yaml` | (a) `python3 -B pipeline/main.py` passed on a disposable source copy and is not bindable; (c) then quotes `anvil-catalog-check:data_inspection_schema`, whose archived inspection keys are `columns,sample_rows,validation_rules` rather than the five required keys |

The definitive prevalidation is performed by the workflow itself on each
fresh origin. It must emit `workflow_reproducer_prevalidated` and bind only an
observed subject failure; phase 3 will compare those execution records with
the table above. The catalog checks were not manually run on these fresh
origins, because they write evidence and the copies must stay byte-identical
until workflow launch.

Fresh origins:

1. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev002_origin_1`
2. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev002_origin_2`
3. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev002_origin_3`

## Phase 2: human-terminal execution

Run from `/Users/maenokota/share/work/github_kewton/CommandAgent-develop` in a
normal terminal. Before Run 1, confirm that `which commandagent` prints
`/Users/maenokota/.local/bin/commandagent` and `commandagent --version`
contains `c41cc9f`.

Execute exactly one command at a time. After pressing Enter, do not monitor,
inspect, collect, run another command, or manipulate the session until the
prompt returns. Only after the prompt returns may the next command be started.
Do not run these in parallel and do not interrupt them.

### Run 1

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev002_origin_1 ; date +%s
```

### Run 2

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev002_origin_2 ; date +%s
```

### Run 3

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev002_origin_3 ; date +%s
```

When all three prompts have returned, send `完了` to Codex. Do not alter any
origin after completion; phase 3 will harvest and scrub the evidence.

## Phase 2 execution record

All commands ran sequentially in a normal terminal. The prompt returned before
the next command was started, and no run was monitored, interrupted, or run in
parallel.

| Run | Start epoch | End epoch | Wall seconds | Console observation |
|---|---:|---:|---:|---|
| 1 | 1784729099 | 1784729146 | 47 | all three phases complete |
| 2 | 1784729159 | 1784729192 | 33 | all three phases complete |
| 3 | 1784729199 | 1784729232 | 33 | all three phase verifiers passed; node events contain the terminal record |
