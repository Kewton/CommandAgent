# Structured stages

Issue #975 (G4). A profile or pack can declare stages over its `propose-artifacts`
tools. A stage is complete only when, for its current inputs, the host adopted the
bytes and every check the operator requires passed. A new upstream version takes
that state away from everything below it. What a model or a tool says never
decides this.

Without a declaration none of this runs: nothing is written here, and the wire, the
events, and the artifacts of a run stay as they were
(`tests/corpus/apps/structured-stage-receipts/`). A run with no pack, or with a decoded
pack, reads no contents for this. A run whose pack was left to admission, with no stage
in it, also resolves the workspace's path. Its final gate reads the pack's contents when
no admission of this process recorded it, and a pack that cannot be used then ends that
run as a failure (see "A pack only admission can read" and "What this costs a run with no
stage").

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
| The `fix` intent | Before its own adjudication, and again after its reproducer and regression commands have run, before it reports completion |
| The `investigate` intent | Before its own adjudication, which runs no command |

The profile that is reported is the profile the stages are asked of. A run is scored and reported
as its plan's profile, and its stages are asked of its config's. A plan file and `/resume` bring a
profile of their own, so when either one declares stages, a plan naming another profile is refused
at the entry of an ultra plan run, before any provider call: `stage_contract_refused`. The names are
compared as profiles, so `next.js` and `nextjs` are one. A generated plan already has the config's
profile, and the one change during a run, a promotion, moves both.

Two forms of this say something else or come about unasked. A pack of stages for the plan's profile,
under a config of another, stops first, when the config's admission finds the pack is not its own:
`selected_pack_invalid`, with no names compared and no provider call. A plan file with no `profile:` is
read as `generic`, so `--profile P`, with stages in `P`, refuses it although the file names no profile.
To run it: write `profile: P` in the file.

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
output depends on that tool's stage. A stage can also list `depends_on`, and `required_checks` adds
to what the operator requires. Declared limits can only narrow, under `[profile_tools.stages.limits]`.
An eval pack carries the same fields in YAML under `profile_tools.stages`.

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

`--fresh-session` starts the stages over, replacing a record it can read at the first stage call.
It wins over `--resume` and `/resume`. It is the only operation of a run that removes the mark a
secret value left; moving or deleting the file by hand removes it too. Without it, a new process
that does not resume starts the counts and the deadline over, except on such a record, which it
refuses. A damaged record is refused either way (below).

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
  `WorkflowChild`) after that call. The same goes for `RecoversByItself`: only the REPL's filter for
  free text refuses before the first provider call, and `--ultra-plan-run` is refused at the session.
  Measured on `--recovery-plan-auto-runs 1 --ultra-plan-run`: one request for the phase's plan, no
  treatment, no second attempt.
- **A wrong declaration's wording (F10).** The text serde prints for an invalid declaration now
  lists `stage` and `stages`. This is the one wording that changes for a run with no stages.
- **Packs (N6), by reading the code, not measured.** `stages_declared_without_a_read` does not see a pack passed as `Selected`, or a
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
- **A damaged record.** `stage_ledger_refused: corrupt`, on any entry, `--fresh-session` included:
  it stays to be looked at. To recover: move the file the notice names away, then start a new session.
- **A record that changed underneath (diverged).** `stage_ledger_refused: diverged`. To recover:
  `--fresh-session` in a new process.
- **Another declaration (contract_changed).** A resume or a later session of this process, under
  stages the record was not written under: `stage_ledger_refused: contract_changed`. To recover:
  `--fresh-session`, or any new process that does not resume. A tool declared again in a stage set
  where it has other upstream inputs used to be refused at admission. It is now refused here, at its
  first stage call: the session starts, the tool's process does not, and no seal is left
  (`b1_another_upstream_set_stops_at_contract_changed_and_starts_no_process`).
- **One record per workspace and profile.** Another profile declaring the same tool, or a copy of
  the workspace, starts a record of its own, with no counts and no secret's mark.
- **Another pack in one process.** A process can run several selections, as a REPL does. Each pack
  selection is a set of stages of its own to final acceptance: the seals of pack A do not pass pack B,
  whose run with no session ends as `no_session_bound_the_declared_stages`, and a run with no stage after
  either is not asked about them and reads no record. Going back to A finds A's stages as they were. The
  record is still one per workspace and profile, so a session of B calling a stage after A in that
  process gets `contract_changed`, which only a new process puts right.
- **Another declaration in one process.** By reading the code, no production path reaches another
  extension root within one process: `register` is called once per process, by the one mode that runs,
  and only the library API and tests call it again. Registering concurrently is not supported, which
  also no production path does (read). A later root would replace the registered profiles, so one name
  could come to mean another manifest. Final acceptance's table is keyed
  by the manifest's hash too, taken from memory. Neither a seal nor a "no stage" answer of one
  declaration passes another, whose run with no session ends as `no_session_bound_the_declared_stages`
  when it declares stages. Going back finds the first declaration's record as it was. The same bytes
  under another root are the same declaration, and another byte is another one, so a manifest changed
  only in its wording is also no success until a session binds it. A builtin profile has no hash, and no
  root can declare its name. The record is still one per workspace and profile and nothing is removed
  from it, so a session of the new declaration calling a stage gets `contract_changed`.
- **What a record's seals are held to.** Before it opens a record, final acceptance's gate compares
  what the record's run was admitted under with what is declared now: the stage contract's digest, and
  the digest of each staged tool's descriptor. Any difference is
  `no_session_bound_the_declared_stages`, with the record left as it was. That covers another registry
  root and a descriptor file edited in place, as measured, whether the change alters a tool's upstream
  inputs, its required checks, or any other byte. Without this, a run with no session passed on the old
  seals: a seal is judged by the bytes it covers, and the entry's own check does not refuse a tool whose
  upstream inputs changed. A session of the changed contract still stops at `contract_changed`.
  - A descriptor's digest holds what the descriptor says, and not the contents of what it points at. So
    the gate also holds those to their pins: the files in `dependencies`, the executable, and the pinned
    directories. Bytes off their pin, a descriptor that cannot be read, and a contract now refused end as
    `the_declared_stages_cannot_be_read`. A selected pack that cannot be used ends as
    `the_selected_pack_cannot_be_read`. The same bytes put back pass again, as long as no session of
    another descriptor came in between: such a session takes the table's place of the first, even when
    the record then refuses it (`ledger_refused`). The gate then says
    `no_session_bound_the_declared_stages` over seals still in the record, until a session of the first
    descriptor binds it again. New bytes with a new pin are another descriptor, so
    `no_session_bound_the_declared_stages`.
  - What the pins do not hold: bytes in a place the sandbox lets a tool read, such as `/usr/lib`; what a
    symlink inside a pinned directory points at, of which only the link's text is pinned; a file's mode;
    and the host's own binary.
  - The comparison only reads. It adopts no input, runs no probe, starts no process, and opens no record.
    When what it compares matches, the gate goes on to open the record and judge it, as it always did.
    A test of the whole gate can only see that the record's bytes stayed the same.
  - What it costs a run with stages: each time the gate asks, it reads each staged tool's descriptor and
    hashes its dependencies, executable, and pinned directories, and reads a pack left to admission.
  - A descriptor changed only in its description or spacing is no success until a session binds it, and
    the entry refuses such a session while the first adoption stands.
  - A compiled schema is not compared. What it adds to a descriptor's digest is the compiler, and only
    the library API can put another compiler under the same ID.
  - A descriptor changed during a `--prompt` session is judged by that session's contract. So a check
    added to a descriptor after every stage sealed and before the last reply still lets the session
    succeed. This is on the side of a false success. A plan's run stops at the final gate: there, a change
    not put back is `no_session_bound_the_declared_stages` even after a session has run, which stretches
    what those words say.
  - `state_dir` is no part of the table's key, so another value finds the same record (read, no test).
  - A pack rewritten after a session, with the same selection in the environment, is not read again by
    a record of no stage. It is one more change during a run.
- **A plan of a name this process does not know.** A plan naming a profile no registered root declares
  has stages the process cannot know of, and those are outside what the host holds: nothing here asks
  about them. Leaving out `--extension-root` is one way there. This is on the side of a false success,
  and only when the config has no stage either: a config with stages refuses such a plan like any other.
- **A plan of another profile with no stage.** With no stage on either side, the two names may differ as
  they always did, and the run is reported as the plan's profile.
- **`cli` with stages.** Write `--profile python-cli` for a run with stages. The catalog lists `cli` as an
  alias of `python-cli`, but the entry's comparison keeps `cli` as it is, so a plan named `python-cli`
  under `--profile cli` is refused at the entry: `stage_contract_refused`. This refusal is false, and on
  the side of no success (`plan_profile_the_cli_alias_is_falsely_refused_with_stages`). A run with no
  stage is not affected.
- **What a refused `/resume` leaves.** By reading the code, what comes before the entry's refusal writes
  `tui_command_start` and `resume_start` to the events, and `recovery_plan_auto_run_configured` with
  automatic runs above 0. The refusal then adds `tui_command_stop`, with `ok` false, and `loop_stop`, as
  any failed command does. Which events appear is measured; that a later run's success rests on none of
  them is a reading of what reads them. No record or session comes of it. One thing is wrong to read:
  a later command with the same events file shows `- Resumed from:` with the refused resume's value, and
  so does its `tui_command_stop`. That is a display error, and the code did this before stages.
- **A pack only admission can read.** A confirmed TUI dispatch, or a selection inherited in the
  environment, says nothing in memory about stages. A session that admits such a pack with no stage
  records that in final acceptance's table. The table belongs to the process and not to a run: a later
  run with the same workspace, profile, declaration, and selection uses the record, whether or not it
  ran a session of its own. Apart from comparing a record of stages, as above, the final gate reads the selected pack only when the current key has no
  record and memory cannot say whether stages are declared. Two ways there: no session of this process
  admitted that key, as in a first plan whose steps all short-circuit, or the workspace's path cannot be
  resolved, so that a record could be neither kept nor found, even after a session. What the read finds
  is not kept, so each such gate reads again. `the_selected_pack_cannot_be_read` is what that read ends
  as when the pack cannot be used: its files cannot be read, it fails verification (retired, identity,
  hash, pin), or the merge with the profile's declaration is refused.
- **What this costs a run with no stage.** Against a build without stages, its result differs in one
  way: the failure above. What it looks at differs too. A run with no pack, or with a decoded pack, and
  no declared stage returns at the gate's first line and looks at nothing. A run whose pack was left to
  admission, with no stage in it, also resolves the workspace's path (`canonicalize`) once in each
  session and once in each final gate; that looks at no pack, schema, or record bytes. Its gate reads the
  pack only as above.
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
- **`Implement` steps (N4), by reading the code, not measured.** A resubmission there counts as
  read-only to the loop's stagnation count. From the third in a row the read-only intervention
  starts, and at the seventh it asks for a write, before the limit of 8 calls.
- **Counts per session.** The hold's count and the count of calls that start nothing go back to
  zero with each session. The session's iteration budget and the ledger's attempts still hold.
- **A final repair that seals a stage and changes nothing compared (N8 of stage 3, #986).** Measured
  (`a_final_repair_that_seals_the_open_stage_still_ends_as_a_failure`): a final repair that seals the
  open stage, on the right revision, and changes nothing the repair's before and after compare ends
  with no new judgment, through `final_acceptance_repair_no_source_change` and `reprobe_skipped`. The
  run fails with a message that calls the stage unmet although it is sealed by then, which misleads.
  It is a failure and not a false pass.
  - What is compared: the files a walk of the workspace finds with `.ts`, `.tsx`, `.js`, `.jsx`,
    `.mjs`, `.cjs`, `.css`, or `.py`, or with a few config names such as `package.json`, skipping dot
    directories and names such as `out`, `build`, and `target`, and the profile's expected and
    route-bound paths. When an admitted artifact check ran, the artifacts the profile declares and
    the names that check recorded are compared too, past the walk's usual limits on extension and on
    generated directories. The limits on paths remain: a dot directory, `node_modules`, or a credential
    path is not compared, and the counts of candidates and of bytes still bound what is
    (`artifact_repair_paths.rs`, `repair_workspace_snapshot.rs`). A stage output such as
    `out/lower.json` is none of these unless such a check names it.
  - If the repair also changes a compared file, acceptance asks again and judges the seal by the bytes
    there at that moment (`a_final_repair_that_seals_and_changes_a_source_is_asked_again`). Measured
    on one form only, a profile that keeps `Write` with its required file already there, and only with
    both in one reply: that repair session ended at its first batch of tools, so a write in one attempt
    and the seal in the next ended as above. This is what that form did and not a rule of the repair: a
    session has other feedback that sends a model back for another reply
    (`loop_run.rs`, `implementation_completion.rs`), and there the seal may come in a later reply.
  - To recover: submit every stage in the plan's own steps. In the form measured, a repair finished one
    only when the same reply also changed a compared file.
  A repair called on the wrong revision gets `stale`, and on an `Implement` repair with a missing
  write it can end at `model_stagnation:read_only_loop` first. Seals a resume lowered are not met at
  final acceptance either, and that run is no success.

### The terminal

- **The notice and the footer.** The record's notice shows once for each `StageRun`, and a session
  makes a new one, so a repair's session can show it again. Its race with the footer's drawing is not
  checked under load. Measured with `TerminalUi`, a spinner, and a footer in an ignored test under
  tmux, five times, not in a real REPL.

### What the host cannot see

- **After the judgment (S18).** A background process of the same uid can change bytes after a
  judgment. Only macOS has real isolation. Case and Unicode aliases of a path are not judged as the
  same path.
- **What a seal means (D8).** A seal says the stage's obligations were met. It is not final
  acceptance's Full and not a release pass, and nothing turns it into one: assurance reads nothing
  of the stages, and the gate only lowers a report. On the create path, the level on
  `ultra_plan_complete` comes from the kind of profile: it is not the result of a check, and not a
  seal. Only a run that passed final acceptance reaches it. A manifest profile with no base runtime
  reports `full` there, with or without stages. Final acceptance's own level is a separate value. The
  `fix` and `investigate` intents are different: they report their own adjudication's level and do not
  go through final acceptance. A run with every stage sealed and a run of the same profile with no
  stage declaration end at the same two levels, `static` and `full`, and with the profile's final
  checks failing the same seals make no success. Both those values are at their limit for that
  profile, so that comparison cannot see a raise; that a seal raises nothing rests on reading the
  code and the audit. An overlay on `generic`, whose level has room, cannot be compared: an overlay
  has no `profile_tools` table to declare stages in, and `generic` cannot be an overlay's base.
- **The final repair's clock.** Its 240 seconds use the real clock, so no test reaches that limit. The
  value and its use are held by the audit, and `final_acceptance.rs` reads nothing of the stages. Its
  two attempts are measured, with two stages open.
- **A fix's order.** A fix judges its stages again after its adjudication's file and event, so it can
  end as a non-success after an adjudication that reads well. No `ultra_plan_complete` follows.
- **The off comparison.** One ultra row is recorded with speculation off (`profile_explicit = false`)
  and compared whole. Another runs with speculation on, as an explicit profile does. There, only
  `provider_request` and `provider_response` may fall anywhere among the other events, and only between
  `speculative_phase_plan_started` and `speculative_phase_plan_adopted`. The two keep their order
  between themselves, and each line is compared whole. The requests are kept in two lists, by whether
  a request offers a tool, each in its order. The order across the two lists is not compared.
- **Evidence.** The process runs only on macOS, so **a stage's success end to end is exercised only
  locally on macOS. A green CI is no evidence of a stage's success.** Linux runs the same sessions
  in its unit tests with an unconfined launch only a test build has.
