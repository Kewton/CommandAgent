# uat-test0722-circle-003 execution runbook

Status: phase 1 complete; awaiting three sequential human-terminal runs.

## Preflight record

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- `git status --porcelain=v1 --untracked-files=all`: only the six previously
  accepted raw logs under `uat-test0722-circle-001/` and
  `uat-test0722-circle-002/`.
- HEAD: `a983725c8ac4d43bc17d3a0a10a04d0ee172a6c3`
- Privileged `cargo test`: green, 1747 passed / 30 ignored / 0 failed.
- Focused workflow tests: 18 passed / 0 failed.
- `cargo fmt --all -- --check`: green.
- `cargo clippy --all-targets -- -D warnings`: green.
- Release/install commands:

  ```sh
  cargo build --release
  cargo install --path . --locked --root /Users/maenokota/.local --force
  ```

- `which commandagent`: `/Users/maenokota/.local/bin/commandagent`
- Installed `commandagent --version`:
  `commandagent 0.1.0 a983725+dirty 2026-07-22T01:24:31Z`
- Release `./target/release/commandagent --version`:
  `commandagent 0.1.0 a983725+dirty 2026-07-22T01:24:31Z`
- Installed and release SHA-256:
  `a8a9ff5c323ea6712acc910c3b011899ed3610c903bafcefc3eaaeb7f8664193`
- Version and binary digest match: yes.
- The `+dirty` suffix is caused solely by the six accepted, untracked raw logs;
  `git status` reported no tracked modification before this runbook was made.
- `printenv NODE_ENV`: `production`.

## Origin procurement

Each archived source and fresh copy was verified with the following commands:

```sh
rg -n '"event":"run_stop".*"status":"failed"' <source-or-origin>/.anvil/runs
find <source-or-origin>/.anvil/plans -name 'recovery-*.yaml' -print
diff -qr <source> <origin>
find <origin> -name .git -print
```

`diff -qr` and `.git` searches produced no output for all three copies.

| Origin | Archived source | Failed create run | Recovery YAMLs |
|---|---|---|---|
| 1 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002` | `019f6951-e16e-7fc0-84a9-86f7657258ba`; `run_stop.status=failed` | `recovery-ultra-plan-read-only-stagnation-019f695c-3253-7910-9f51-6c0c104e56ef.yaml`; `recovery-ultra-plan-phase-data-aggregation-019f695c-3255-7160-b624-cd19ecf8cf4d.yaml` |
| 2 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_001` | `019f6940-466d-7912-9d32-6f4369fbfeeb`; `run_stop.status=failed` | `recovery-ultra-plan-phase-data-inspection-019f6948-b537-7d50-9f89-c1234229bbaf.yaml`; `recovery-ultra-plan-read-only-stagnation-019f6948-b536-7813-8ebb-04fdd16523ef.yaml` |
| 3 | `workspace/management/runs/uat-test0715-data-005/artifacts/data5_qwen35_profile_001` | `019f6476-d96f-7e20-abda-0094031f600e`; `run_stop.status=failed` | `recovery-ultra-plan-phase-data-cleaning-019f6482-0775-7241-bf2c-28a13c81849f.yaml`; `recovery-ultra-plan-read-only-stagnation-019f6482-0773-77b2-9ac8-29316e6eada7.yaml` |

Fresh origins:

1. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle003_origin_1`
2. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle003_origin_2`
3. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle003_origin_3`

## Phase 2: human-terminal execution

Run from `/Users/maenokota/share/work/github_kewton/CommandAgent-develop` in a
normal terminal. Before starting, confirm that `which commandagent` prints
`/Users/maenokota/.local/bin/commandagent` and `commandagent --version` contains
`a983725`.

Execute exactly one command at a time. After pressing Enter, do not monitor,
inspect, collect, run another command, or manipulate the session until the
prompt returns. Only after the prompt returns may the next command be started.
Do not run these in parallel and do not interrupt them.

### Run 1

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle003_origin_1 ; date +%s
```

### Run 2

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle003_origin_2 ; date +%s
```

### Run 3

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle003_origin_3 ; date +%s
```

When all three prompts have returned, send `完了` to Codex. Do not alter any
origin after completion; phase 3 will recover the evidence from these paths.
