# uat-test0722-circle-elev-003 execution runbook

Status: phase 1 complete; waiting for three sequential human-terminal runs.

## Implementation and preflight

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- E-A commit: `7d683ed` (`Read workflow edges from adjudication`).
- Diagnose injection commit: `922d08d` (`Inject reproducer output into diagnosis`).
- Both commits are pushed to `origin/develop`.
- Existing worktree entries before preparation were only the fifteen accepted,
  untracked raw logs from circle-001 through circle-elev-002. No tracked
  modification was present.
- Privileged `cargo test`: green, 1773 passed / 30 ignored / 0 failed.
- `cargo fmt --all -- --check`: green.
- `cargo clippy --all-targets -- -D warnings`: green.
- Release/install commands:

  ```sh
  cargo build --release --locked
  cargo install --path . --locked --root /Users/maenokota/.local --force
  ```

- `which commandagent`: `/Users/maenokota/.local/bin/commandagent`
- Installed and release `--version` (identical):
  `commandagent 0.1.0 922d08d+dirty 2026-07-22T14:52:18Z`
- Installed and release SHA-256 (identical):
  `1440b4fc13dce84119a47d2cef4f961870971caf8bc06cf9a4f9fbde0a85a458`
- The `+dirty` suffix is caused only by the fifteen accepted raw logs.
- `printenv NODE_ENV`: `production`.

## Elevated model and credential boundary

`workflows/recovery-circle-data-elevated.yaml` fixes both investigate and fix
executors to `gemma4:31b-cloud` through provider `ollama`. `ollama show
gemma4:31b-cloud` succeeded. Planner configuration remains the launcher's
local `qwen3.6:27b-coding-nvfp4` / `ollama` configuration.

Credentials are supplied only through Ollama's internal account/configuration
state. No credential value or credential environment variable is present in a
command argument, workflow YAML, or repository file. Preparation checks:

```sh
python3 workspace/management/scripts/bench.py scrub --path workflows
grep -En 'AIza[0-9A-Za-z_-]{35}|ghp_[A-Za-z0-9]{36,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{16,}' workflows/recovery-circle-data-elevated.yaml
```

Scrub returned `{"ok":true,"findings":[]}` and grep returned no match.

## Origin procurement and prevalidation basis

The three archived sources are the same distinct failed create×data runs used
by elev-002, copied into entirely new origins. The following commands confirmed
the failed `run_stop`, recovery YAML, byte-identical copy, and absence of `.git`:

```sh
grep -RniE '"event"\s*:\s*"run_stop".*"status"\s*:\s*"failed"' <origin>/.anvil/runs
find <origin>/.anvil/plans -name 'recovery-*.yaml' -print
diff -qr <archived-source> <origin>
find <origin> -name .git -print
```

All three `diff -qr` and `.git` searches returned no output.

| Origin | Archived source | Failed create run | Recovery YAML | Expected canonical R source and precheck |
|---|---|---|---|---|
| 1 | `workspace/management/runs/uat-test0715-data-007/artifacts/data7_qwen35_none_002` | `019f65d3-ae61-7b81-b96d-9d5f871768b1` | `recovery-ultra-plan-phase-validate-and-clean-data-019f65d7-4121-7541-8333-d36a4d73f8f6.yaml` | (c) `verify_default_bound`: `test -f pipeline/main.py`; expected exit 1 |
| 2 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_none_002` | `019f6961-6c4e-7020-a97b-5af570d660db` | `recovery-ultra-plan-phase-load-and-validate-data-019f6969-8372-7593-a21d-f8516e810352.yaml` | (c) `anvil-catalog-check:data_inspection_schema`; `output/inspection.json` absent |
| 3 | `workspace/management/runs/uat-test0715-data-007/artifacts/data7_gemma31_none_001` | `019f65f7-b518-7241-b015-9a0e7272c241` | `recovery-ultra-plan-phase-load-and-inspect-data-019f6602-718c-74c3-ab04-f057f28a6b41.yaml` | (a) pipeline candidate is expected to pass and be rejected; (c) inspection schema candidate is expected to fail |

The workflow itself performs and records the definitive R prevalidation on
each fresh origin before binding it. The catalog checks were not manually run
because they write evidence and the copies must remain unchanged until launch.

Fresh origins:

1. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev003_origin_1`
2. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev003_origin_2`
3. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev003_origin_3`

## Phase 2: human-terminal execution

Run from `/Users/maenokota/share/work/github_kewton/CommandAgent-develop` in a
normal terminal. Before Run 1, confirm:

```sh
which commandagent
commandagent --version
```

The first command must print `/Users/maenokota/.local/bin/commandagent`; the
version must contain `922d08d`.

Execute exactly one command at a time. After pressing Enter, do not monitor,
inspect, collect, run another command, or manipulate the session until the
prompt returns. Only after the prompt returns may the next command be started.
Do not run in parallel and do not interrupt.

### Run 1

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev003_origin_1 ; date +%s
```

### Run 2

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev003_origin_2 ; date +%s
```

### Run 3

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev003_origin_3 ; date +%s
```

When all three prompts have returned, send `完了` to Codex. Do not alter any
origin after completion; phase 3 will harvest, audit, and scrub the evidence.
