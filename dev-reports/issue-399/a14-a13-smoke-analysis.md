# A14-A13 stratified Recovery smoke analysis

## Outcome

A14-A13 smoke-01 stopped fail-closed after 3 of the frozen 10 pairs. The run is
instrument evidence only and must not be resumed, rescored, or pooled with a later
effect estimate.

- Repository run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a13-smoke-01`
- Product execution root: `/Volumes/SSD_NX/tmp/commandagent_trial/phase6-recovery-v4-20260830-a14-a13-smoke-01`
- Exact product/code SHA: `4c5d81909e2be2b1c230f36a5e7c603c35366b73`
- Completed: 3 CLI pairs; generic, nextjs, and dependency sentinel were not invoked

The three completed CLI outcomes were:

| Pair | Recovery runs | Frozen external transition | Attributed effect | Harm / regression | Incremental total tokens | Incremental wall time |
| --- | ---: | --- | --- | --- | ---: | ---: |
| `phase6-main-c05-task-05--pair-01` | 1 | fail to pass | improved | 0 / 0 | 45,116 | 172,132 ms |
| `phase6-main-c05-task-05--pair-02` | 0 | pass retained | no recovery needed | 0 / 0 | 0 | 0 ms |
| `phase6-main-c05-task-05--pair-03` | 0 | pass retained | no recovery needed | 0 / 0 | 0 | 0 ms |

These outcomes are diagnostics only. In particular, one rescue in one profile is
not a population effect claim.

## Stop condition and cause

Before the first generic product invocation, task binding raised:

`completion contract fix reproducer mismatch:phase6-main-c07-task-01`

A14-A13 added `completion_contract.fix_reproducer_command` to generic and nextjs
tasks, but did not add the corresponding structured
`operational_constraints.reproducer`. The task binder requires both fields to name
the same argv. The generated registry passed its shape validator, while this
cross-field corpus binding was still performed lazily inside the pair loop. That
allowed three valid CLI records to be committed before the first invalid generic
binding was reached.

## Amendment

A14-A13-1 makes two fail-closed changes without changing Recovery strength or any
success oracle:

1. Register the generic and nextjs before-failure argv in both the completion
   contract and typed operational constraint, with expected exit 1 before and 0
   after.
2. Bind and validate every selected task before campaign manifest creation and
   before the first product invocation.

A14-A13-1 uses a new contract and run ID. A14-A13 smoke-01 remains immutable at
3/10 and is excluded from A14-A14 full-experiment inference.
