# Structured stages

Issue #975 (G4). A profile or pack can declare stages over its `propose-artifacts`
tools. A stage is complete only when, for its current inputs, the host adopted the
bytes and every check the operator requires passed. A new upstream version takes
that state away from everything below it. What a model or a tool says never
decides this.

Without a declaration none of this runs: nothing is read or written here, and the
wire, the events, and the artifacts of a run stay as they were
(`tests/corpus/apps/structured-stage-receipts/`).

## Design

| Part | File | What it owns |
| --- | --- | --- |
| Contract | `src/planner/structured_stages/contract.rs` | Admits the declaration before any provider call |
| State machine | `state.rs` | Phases, generations, the D7 table, limits. No file, no process, the clock as an argument |
| Invalidation | `invalidation.rs` | What an upstream change takes away |
| Receipt | `receipt.rs` | Binds a target's state to G3's commit receipt, which it does not change |
| Resume | `resume.rs` | Lowers a saved record. A saved record never raises a state |
| Ledger | `ledger.rs` | Attempts and generations across sessions, and a file for a later process |
| Session wiring | `mod.rs`, `implementation_completion/structured_stage_completion.rs` | The one judgment, the hold, the funnel, the final gate |

### State and generations

A target is `not_ready`, `proposed`, `locally_valid`, `sealed`, or `unverified`.
Only a commit that stands and is durable seals. Each change of a target's bytes
moves its generation, and the revision a model passes as `expected_revision` names
a generation in this run. `initial` is the revision of a target at generation zero.

Moved bytes take the state of the target and of everything below it, however
they moved. A stage that does not depend on the change keeps its state. Bringing
old bytes back does not bring the old state back below it.

### One judgment

`StageLedger::verdict` compares real bytes and is the only judgment. `complete()`
on what it returns is the only thing that says a stage set is complete. A record
that cannot be vouched for answers no, and says so.

### The ledger

One file per workspace and profile, below `--state-dir`, outside the workspace.
A state directory inside the workspace, or in a cache commands may write, is refused.
Every change reaches the disk before it takes effect, so an attempt is counted before its
process starts and an intent exists before its rename. A count is never rebuilt, and never
starts at zero on a resume.

### The D7 table

`begin` answers each call, in this order. Only `Proceed` starts a process or spends an attempt.

| Row | Condition | Answer | Ends the run |
| --- | --- | --- | --- |
| 1 | A secret value ended this run | `ended_by_secret_value` | Yes |
| 2 | An intent is unanswered | refused (`OutOfOrder`) | Yes |
| 3 | The deadline passed | `deadline_exceeded` | Yes |
| 4 | Another call in this reply committed | `stale`, with the current revision | No |
| 5 | The revision is older | `stale`, with the current revision | No |
| 6 | The revision is not one of this run | `unknown_revision` | No |
| 7 | The same call on the bytes already sealed | `no_change` | No |
| 8 | A stage above is not sealed | `not_ready`, naming what it waits on | No |
| 9 | An input is not what the stage above adopted | `input_not_current` | No |
| 10 | A fixed input is not what the run pinned | `fixed_input_changed` | Yes |
| 11 | The target's attempts are spent | `attempts_exhausted_for_target` | Yes |
| 12 | The run's attempts are spent | `attempts_exhausted_for_run` | Yes |
| 13 | Otherwise | `Proceed`: the attempt is on disk, then the process starts | |

### Resubmission

A failed check goes back to the model as data (`status: resubmit`, `code: check_failed`) and
leaves what is sealed as it was. Any other bad result ends the run. Calls that start no process
spend no attempt, so they have a limit of their own: `max(8, number of targets)` in a row, then
`no_progress_limit`. A call that starts a process clears that count.

### The hold

A session with an unmet obligation that wants to end is sent back with
`Stage obligations are not met: <stage>/<target> at revision <revision>`, at most 8 times.
A step that could end at a tool result goes back first if the result is one its model has to
read. A shortfall no submission can mend is not held.

### The funnel and the final gate

Every way out of a session passes the funnel: a success with an unmet obligation becomes
`stage_incomplete`. A plan step answers only for the stages it called. The rest is judged at the
plan's end:

| Entry | What judges every obligation |
| --- | --- |
| `--prompt` | The funnel |
| `--plan-run`, `--run-plan`, `/plan-run`, `/run-plan` | The end of `run_step_plan_with_ui` |
| `--ultra-plan-run`, `/ultra-plan-run`, a confirmed REPL request | Final acceptance, whose report gets the failure |
| The `fix` and `investigate` intents | Before their own adjudication |

## Declaring stages

The manifest uses `profile-tool.declaration/v2`:

```toml
[profile_tools]
schema_version = "profile-tool.declaration/v2"
builtin_tools = ["Read"]
tools = [
  { id = "config-upper", effect = "propose-artifacts" },
  { id = "config-lower", effect = "propose-artifacts" },
]

[profile_tools.stages]
stages = [
  { id = "upper", targets = [{ id = "main", tool_id = "config-upper" }] },
  { id = "lower", targets = [{ id = "main", tool_id = "config-lower" }] },
]
```

An edge between stages comes from the schemas: a tool whose `schema.inputs` names another tool's
output depends on that tool's stage. Declared limits can only narrow.

The operator's descriptor carries the floor. Without it the tool is refused at admission:

```json
"stage": {
  "required_checks": ["shape"],
  "attempts_per_target": 4,
  "attempts_per_run": 16,
  "deadline_seconds": 600
}
```

The tool's schema must take a required string argument `expected_revision`. The host keeps it out
of what the tool is asked and out of the digest of the call.

**What a tool's process has to do.** A list of `checks: []` ends the run with `no_checks`: it is
not an empty pass. A failing check has to come back with the process exiting 0. A failing exit
ends the run with `failure_code`, and the model gets no chance to resubmit.

## `--fresh-session`

`--fresh-session` starts the stages over, replacing the record at the first stage call. It wins
over `--resume` and `/resume`. It is the only thing that removes the mark a secret value left.
Without it, a new process that does not resume starts the counts and the deadline over, except on
such a record, which it refuses.

## Known limits

Each row names what an operator sees and what to do.

### Declaring and admitting

- **No narrowing by upstream content (D1).** Version 1 does not narrow a schema by what the stage
  above produced. The check belongs to the pinned process.
- **Automatic recovery (B4).** A stage declaration with `--recovery-plan-auto-runs` above 0 is
  refused at admission: `stage_contract_refused`, `RecoversByItself`. A treatment works in a
  workspace of its own, under a ledger that would start over. To recover: run with
  `--recovery-plan-auto-runs 0`.
- **Entries before the session (F8).** An entry that calls a provider before `session_registry`
  gets the refusals for no OS boundary (D6, `OsSandboxNotEnforced`) and for a workflow child (D12,
  `WorkflowChild`) after that call.
- **A wrong declaration's wording (F10).** The text serde prints for an invalid declaration now
  lists `stage` and `stages`. This is the one wording that changes for a run with no stages.
- **Packs (N6).** `stages_declared_without_a_read` does not see a pack passed as `Selected`, or a
  pack it would have to read from its bytes as another profile. It matters only for a run where no
  session ever bound: then final acceptance does not report
  `stage_incomplete: no_session_bound_the_declared_stages`.

### Time and attempts

- **The deadline does not come back (P3).** A resume keeps the run's clock, so a resume across a
  night ends at `stage_run_ended: deadline_exceeded`. To recover: `--fresh-session`.
- **A narrower limit on a resume (P3).** Narrowing `attempts_per_target` changes the contract, and
  the resume is refused: `stage_ledger_refused: contract_changed`. To recover: `--fresh-session`.
- **Where the clock starts (B).** At the run's first stage call, not at admission and not at the
  first judgment.
- **The real upper bound (F7).** The deadline is read only in `begin`. A process that started just
  before it can run to `MAX_TIMEOUT_MS`, 30 seconds, so the upper bound is 3630 seconds against a
  deadline of 3600.

### The record

- **Missing.** `--resume` with no ledger, including for a session that never ran a stage, is
  refused: `stage_ledger_refused: missing`. To recover: `--fresh-session`.
- **One record per workspace and profile.** Another profile declaring the same tool, or a copy of
  the workspace, starts a record of its own, with no counts and no secret's mark.
- **A fixed input changed (P5, R2).** A resume finding a changed fixed input is refused:
  `stage_ledger_refused: fixed_input_changed`. To recover: `--fresh-session`, or any new process
  that does not resume. A change before the first `begin` cannot be seen, because nothing was
  pinned yet.
- **The lock (N8 of stage 1).** A background process started through Bash can inherit and hold the
  `flock`. A resume is then refused: `stage_ledger_refused: held`. To recover: end that process.
- **A secret value.** A run a secret ended: `stage_run_ended: ended_by_secret_value`. A later
  process with no flag: `stage_ledger_refused: ended_by_secret_value`. To recover: `--fresh-session`.
- **A mark that could not be saved.** If the record itself cannot be written at that moment, the
  run ends with `stage_ledger_refused` and the record's own code, and the notice names the file.

### The model's view

- **500 characters.** A tool result is cut in the history. Diagnostics have about 500 characters,
  for the whole answer. `src/state/tool_history.rs` is not changed; an answer falls to a fixed form
  that still says what happened, with `truncated: true`.
- **`--prompt` (P1).** A session holds until everything is sealed, so a question asked on a stage
  profile ends as a non-success after at most 8 holds. Ask questions on another profile.
- **`Implement` steps (N4).** A resubmission there counts as read-only to the loop's stagnation
  count. From the third in a row the read-only intervention starts, and at the seventh it asks for
  a write, before the limit of 8 calls.
- **The same bytes again (N8 of stage 3).** Not measured on the path this was found on, where a
  repair seals with the same bytes and `reprobe_skipped` ends the repair with no new judgment.
  Measured: seals a resume lowered are not met at final acceptance, and the run is no success. In
  that run the repair ended with `model_stagnation:read_only_loop` before any new seal.

### What the host cannot see

- **After the judgment (S18).** A background process of the same uid can change bytes after a
  judgment. Only macOS has real isolation. Case and Unicode aliases of a path are not judged as the
  same path.
- **What a seal means (D8).** A seal says the stage's obligations were met. It is not final
  acceptance's Full and not a release pass, and nothing turns it into one.
- **Evidence.** The process runs only on macOS, so **a stage's success end to end is exercised only
  locally on macOS. A green CI is no evidence of a stage's success.** Linux runs the same sessions
  in its unit tests with an unconfined launch only a test build has.
