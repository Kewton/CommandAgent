# A14-A6 pre-execution failure

## Outcome

A14-A6 smoke-01 stopped before any product or model execution. It produced zero
raw records, so it provides no Recovery-effect observation and must not be
resumed or rescored. A14-A6.1 uses a new contract and run ID.

## Working directories

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Exact-SHA source: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a6-source`
- Exact-SHA target: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a6-final-target`
- Failed run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a6-smoke-01`

## Evidence and observed failure

- Code SHA: `1c688aefc3ffa0dcfbe8b899d1e4ad5f3743e03c`
- Binary SHA-256: `904ba8fadfa97707accf4f0d84148554761484d379d20ecb7e913ef92e62cd14`
- Exact-SHA CI and acceptance: completed successfully.
- The run directory contains only `campaign-manifest.json` and the runtime lock.
- Raw record count: 0.
- Terminal error: `ValueError: task execution goal missing:phase6-main-c05-task-01`.

## Direct and root causes

The A14-A6 generator changed the task-registry schema version from the
registered `commandagent.goal_verify.task_contracts.v4_a14_a2` value to an
unregistered A14-A6 value. The loader uses shared `goal` binding only for its
closed registered schema set; otherwise it follows the legacy
`execution_goal` path. The A14-A6 rows intentionally contain `goal`, not
`execution_goal`, so binding failed before execution.

The root cause was an incomplete schema change: the generator changed a schema
identifier without registering and testing the corresponding loader semantics.
The existing unit test built the generated dictionaries and checked selected
fields, but did not pass the generated registry through the real validator and
corpus-binding path.

## A14-A6.1 correction and acceptance criteria

1. Preserve the registered A14-A2 shared-goal schema instead of declaring a new
   task-registry migration.
2. Register the optional `fix_reproducer_command` completion field in the
   closed validator.
3. Require it to be a non-empty string and, when present, exactly match the
   candidate-visible typed reproducer argv after deterministic shell joining.
4. Test registry validation and real corpus binding for all three selected
   smoke cases.
5. Generate new A14-A6.1 input paths, contract ID, and run ID. Never reuse the
   A14-A6 run ID.
6. Freeze and execute only after exact-SHA CI and acceptance pass for the
   correction commit.

These changes correct the measurement harness only. They do not weaken product
verification and do not convert the failed A14-A6 attempt into an observation.
