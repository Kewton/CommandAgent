# uat-test0722-circle-001 execution runbook

Status: all three human-terminal runs completed; evidence harvested in phase 3.

## Preflight record

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- `git status --porcelain=v1 --untracked-files=all` before preparation: empty
- HEAD: `5fffda9b740cfddaedbf580be7f6912f5e9e47e8`
- `git merge-base --is-ancestor 5fffda9 HEAD`: exit 0
- Stash isolation: not required (no retained work was present)
- Privileged `cargo test`: green, 1737 passed / 30 ignored / 0 failed
- Release/install command:

  ```sh
  cargo build --release && cargo install --path . --locked --root /Users/maenokota/.local --force
  ```

- Install result: `/Users/maenokota/.local/bin/commandagent` replaced successfully
- `which commandagent`: `/Users/maenokota/.local/bin/commandagent`
- `commandagent --version`: `commandagent 0.1.0 5fffda9 2026-07-21T22:56:59Z`
- `target/release/commandagent --version`: `commandagent 0.1.0 5fffda9 2026-07-21T22:56:59Z`
- `git rev-parse --short HEAD`: `5fffda9`
- Dirty suffix: absent
- `printenv NODE_ENV`: `production`

## Origin procurement

The following commands were used for each archived source and again for each
fresh origin copy:

```sh
rg -n '"event":"run_stop"' <source>/.anvil/runs
find <source>/.anvil/plans -name 'recovery-*.yaml' -print
```

The copy was also checked with `diff -qr <source> <origin>` (no output for all
three) and `find <origin> -name .git -print` (no output for all three).

| Origin | Archived source | Failed create run | Recovery YAMLs |
|---|---|---|---|
| 1 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002` | `019f6951-e16e-7fc0-84a9-86f7657258ba`; `run_stop.status=failed` | `recovery-ultra-plan-phase-data-aggregation-019f695c-3255-7160-b624-cd19ecf8cf4d.yaml`; `recovery-ultra-plan-read-only-stagnation-019f695c-3253-7910-9f51-6c0c104e56ef.yaml` |
| 2 | `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_001` | `019f6940-466d-7912-9d32-6f4369fbfeeb`; `run_stop.status=failed` | `recovery-ultra-plan-phase-data-inspection-019f6948-b537-7d50-9f89-c1234229bbaf.yaml`; `recovery-ultra-plan-read-only-stagnation-019f6948-b536-7813-8ebb-04fdd16523ef.yaml` |
| 3 | `workspace/management/runs/uat-test0715-data-005/artifacts/data5_qwen35_profile_001` | `019f6476-d96f-7e20-abda-0094031f600e`; `run_stop.status=failed` | `recovery-ultra-plan-phase-data-cleaning-019f6482-0775-7241-bf2c-28a13c81849f.yaml`; `recovery-ultra-plan-read-only-stagnation-019f6482-0773-77b2-9ac8-29316e6eada7.yaml` |

Fresh copies:

1. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle001_origin_1`
2. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle001_origin_2`
3. `/Users/maenokota/share/work/localwork/commandagent_mvp/01/circle001_origin_3`

## Phase 2: human-terminal execution

Run from `/Users/maenokota/share/work/github_kewton/CommandAgent-develop` in a
normal terminal. Execute exactly one command at a time. After pressing Enter,
do not monitor, inspect, collect, run another command, or manipulate the
session until the prompt returns. Only after the prompt returns may the next
command be started. Do not run these in parallel and do not interrupt them.

### Run 1

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle001_origin_1 ; date +%s
```

### Run 2

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle001_origin_2 ; date +%s
```

### Run 3

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle001_origin_3 ; date +%s
```

When all three prompts have returned, send `完了` to Codex. Do not alter any
origin after completion; phase 3 will recover the evidence from these paths.
