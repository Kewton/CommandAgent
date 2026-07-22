# uat-test0722-circle-elev-004 execution runbook

Status: phase 1 complete; waiting for three sequential human-terminal runs.
Date: 2026-07-23 (preflight recorded after commit `47cc8ac`)

## Implementation and preflight

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Calibration commit: `47cc8ac` (`Calibrate investigation error claims to output anchors`), pushed to `origin/develop`.
- `cargo fmt --all -- --check`: green.
- `cargo clippy --all-targets -- -D warnings`: green.
- Privileged full suite: `1776 passed / 30 ignored / 0 failed`.
  The first full-suite attempt had one known baseline runner-test fluctuation;
  the exact test passed three consecutive isolated runs, and the final full
  suite was green.
- `cargo build --release --locked` and install completed.
- `which commandagent`: `/Users/maenokota/.local/bin/commandagent`.
- Installed and release version: `commandagent 0.1.0 47cc8ac+dirty 2026-07-22T15:43:00Z`.
- Installed and release SHA-256 (both identical):
  `f49216c36e791846d4ceac31ac582a56b049f9e67e21d6a71425704bafd57eb0`.
- `printenv NODE_ENV`: `production`.
- The `+dirty` suffix is caused only by accepted untracked raw campaign logs.

## Elevated model and credential boundary

`workflows/recovery-circle-data-elevated.yaml` fixes investigate and fix to
`gemma4:31b-cloud` through `ollama`; `ollama show gemma4:31b-cloud` succeeded.
The planner remains the local qwen27 configuration. `bench.py scrub --path
workflows` returned `{"ok":true,"findings":[]}`. Secret-value patterns in the
elevated YAML produced no matches. Credentials are supplied only through
Ollama account/configuration state; no value is passed in CLI arguments, YAML,
or repository files.

## Origin procurement and prevalidation basis

Three distinct archived failed create×data runs were copied into fresh
directories. The following searches confirmed failed `run_stop`, recovery YAML,
byte-identical copies, and no `.git` history:

```sh
grep -RniE '"event"\s*:\s*"run_stop".*"status"\s*:\s*"failed"' <origin>/.anvil/runs
find <origin>/.anvil/plans -name 'recovery-*.yaml' -print
diff -qr <archived-source> <origin>
find <origin> -name .git -print
```

| Origin | Archived source | Failed create run | Recovery YAML | R basis |
|---|---|---|---|---|
| 1 | `workspace/management/runs/uat-test0715-data-007/artifacts/data7_qwen35_none_002` | `019f65d3-ae61-7b81-b96d-9d5f871768b1` | `recovery-ultra-plan-phase-validate-and-clean-data-019f65d7-4121-7541-8333-d36a4d73f8f6.yaml` | origin bound `test -f pipeline/main.py` (failure) |
| 2 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_none_002` | `019f6961-6c4e-7020-a97b-5af570d660db` | `recovery-ultra-plan-phase-load-and-validate-data-019f6969-8372-7593-a21d-f8516e810352.yaml` | origin bound `anvil-catalog-check:data_inspection_schema` (inspection path absent) |
| 3 | `workspace/management/runs/uat-test0715-data-007/artifacts/data7_gemma31_none_001` | `019f65f7-b518-7241-b015-9a0e7272c241` | `recovery-ultra-plan-phase-load-and-inspect-data-019f6602-718c-74c3-ab04-f057f28a6b41.yaml` | passing pipeline candidate rejected; failed inspection schema bound |

Fresh origins:

1. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev004_origin_1`
2. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev004_origin_2`
3. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev004_origin_3`

The workflow performs definitive R prevalidation on each fresh origin and
records the attempts in `workflow-circle.json`; origins must not be modified
before launch.

## Phase 2: human-terminal execution

Run from the repository root in a normal terminal. Before Run 1 confirm:

```sh
which commandagent
commandagent --version
```

The path must be `/Users/maenokota/.local/bin/commandagent` and the version
must contain `47cc8ac`. Execute exactly one command at a time. After pressing
Enter, do not monitor, inspect, collect, run another command, or manipulate the
session until the prompt returns. Do not run in parallel or interrupt. Start
the next command only after the previous prompt returns.

### Run 1

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev004_origin_1 ; date +%s
```

### Run 2

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev004_origin_2 ; date +%s
```

### Run 3

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev004_origin_3 ; date +%s
```

When all three prompts have returned, send `完了` to Codex. Do not alter any
origin after completion; phase 3 will collect complete workflow and node
evidence. If `circle_full` occurs, the complete I1/I2/F1–F3/verify_origin
evidence and original `workflow_adjudicated` event will be retained.
