# Recursive event and FIX-8/9 search audit

Event file discovery command:

```sh
find workspace/management/runs/uat-test0717-dfix-004/artifacts-v5 \
  -path '*/.anvil/runs/*/events.jsonl' -print
```

The command returned exactly six event files, one per formal v5 run.

Event search command:

```sh
rg --hidden -n -g 'events.jsonl' \
  '"event":"(intent_resolved|host_env_normalized|fix_reproducer_suggested)"' \
  workspace/management/runs/uat-test0717-dfix-004/artifacts-v5
```

Per-run counts, computed from each discovered JSONL with `jq -s`:

| Run | intent_resolved | host_env_normalized | fix_reproducer_suggested |
|---|---:|---:|---:|
| dfix4_pipe_qwen35_001 | 1 | 1 | 1 |
| dfix4_pipe_gemma31_001 | 1 | 1 | 1 |
| dfix4_pipe_qwen35_002 | 1 | 1 | 1 |
| dfix4_schema_qwen35_001 | 1 | 1 | 1 |
| dfix4_schema_gemma31_001 | 1 | 1 | 1 |
| dfix4_schema_qwen35_002 | 1 | 1 | 1 |

All `intent_resolved` records were `origin=cli, source=fix, value=fix`.
All `host_env_normalized` records used `strategy=unset_inherited` for
`variables=["NODE_ENV"]`. Pipe suggestions used
`basis=goal_failure_kind:pipeline_execution` and schema suggestions used
`basis=goal_profile_contract:data_results_schema`.

FIX search commands:

```sh
rg --hidden -n -g 'events.jsonl' -g 'summary.md' -g '*.yaml' -g '*.md' \
  'duplicate expected path ownership|verify step requires at least one verify command|path does not exist' \
  workspace/management/runs/uat-test0717-dfix-004/artifacts-v5

rg --hidden -n -g 'events.jsonl' -g 'summary.md' -g '*.log' -g '*.md' \
  'stream did not contain valid UTF-8' \
  workspace/management/runs/uat-test0717-dfix-004/artifacts-v5
```

Terminal `ultra_phase_failed` reasons extracted with `jq`:

| Run | Phase | Reason |
|---|---|---|
| dfix4_pipe_qwen35_001 | repair | `duplicate expected path ownership: pipeline/main.py in inspect-prior-evidence and implement-fix` |
| dfix4_pipe_gemma31_001 | repair | `duplicate expected path ownership: pipeline/main.py in fix-append-error and run-pipeline` |
| dfix4_pipe_qwen35_002 | repair | `duplicate expected path ownership: pipeline/main.py in synthesize-cause-isolation and implement-fix` |
| dfix4_schema_qwen35_001 | repair | `path does not exist: output/inspection.json` |
| dfix4_schema_gemma31_001 | repair | `model_stagnation:no_progress_recorded` at `execute-pipeline` |
| dfix4_schema_qwen35_002 | isolate-cause | `path does not exist: output/uat-console.log` |

Run 6 also has consecutive raw `Read` calls for both absent
`output/uat-console.log` and absent `output/inspection.json`; the first failure
became the terminal reason. The exact raw calls are events lines 59 and 60.

Class result: duplicate ownership 3 runs; empty-verify scaffold error 0;
absent-artifact requirement 2 terminal runs; invalid-UTF-8 phase death 0.
The zero counts are supported by the explicit recursive searches above, not
by absence of a guessed file path.
