# Issue #377 design: bounded terminal failure explanation and recovery

## Baseline and predecessor integration

This branch starts from `develop` at `6d45c714` and incorporates the committed
Issue #370 result-detail routes, Issue #374 session workspace projection, Issue
#375 typed StepPlan lifecycle events, and Issue #376 task projection. Issue #374
and #376 both extended the shared browser smoke; their merge keeps both path and
task contracts.

The current GUI failure projection is limited to a stop reason, release-gate
reasons, and probe findings. It does not correlate a failed `plan_step_failed`
record with `step_verify_failure`, `ultra_phase_failed`, or
`recovery_prompt_saved`, and it can only expose recovery data embedded in one
long terminal string. The result-detail surface therefore cannot identify an
exact task or offer safe recovery-document actions.

## Shared projection contract

Add a public leaf module below `src/eval_events/` containing a serializable
`FailureExplanation` projection. The GUI server consumes it, but its event
correlation and display-safe text are not GUI-specific, so CLI, TUI, and human
summary consumers can reuse the same typed model without another stop-reason
parser.

The projection contains:

- a category enum: `planning`, `execution`, `verification`, `release_gate`,
  `infrastructure`, `interrupted`, or `unknown`;
- a location with continuation interval, exact Issue #375
  `plan_execution_id` / `step_execution_id`, phase ID/position, and task
  ID/kind/position;
- a primary explanation and separate machine failure kind/reason code;
- evidence fields for command, exit code, verification/probe/acceptance
  observations, missing and changed paths, and evidence paths;
- progress for completed/total phases and tasks, repair attempts, an explicit
  Issue #374 workspace state, and an artifact state which distinguishes
  observed changes, an available but unproven workspace, a missing workspace,
  and an unknown legacy state;
- recovery fields for the next-action code, viable actions, repair prompt,
  Recovery UltraPlan, both suggested commands, and continuation eligibility.

Every string and list is capped independently. Commands use their own larger
cap and a truncation bit; a truncated command is displayable as evidence but is
not copyable as an executable recommendation. Free-form fields are never joined
back into a terminal stop-reason blob. All path and text values still pass
through the existing execution-root public redaction before serialization.

## Interval and event correlation

The GUI status handler already locates the latest
`human_directive_continuation_started`. It will pass only events after that
boundary to the shared projection and record the one-based continuation
interval. A successful final interval produces no failure explanation even if
an earlier interval failed.

Within the selected interval, the projection validates Issue #375 schema
version 1 identities and selects the latest exact failed/interrupted task. It
uses that task's `plan_execution_id`, `step_execution_id`, phase, task position,
terminal outcome, verification summary, changed paths, and repair count. A
matching `step_verify_failure` supplies primary reason, missing paths, and viable
actions; a matching command/verification event supplies command, exit status,
and bounded observation. `ultra_phase_failed` supplies the phase location and
reason when no exact task exists. Release/probe/acceptance failures and terminal
infrastructure or interruption records are classified explicitly. Legacy or
malformed streams are never guessed: they receive `unknown` with direct
`summary.md` and `events.jsonl` evidence paths.

## GUI and safe recovery documents

Replace the Gate 4 failure block on `/try/history/detail/` with one ordered
card:

1. failed location;
2. primary cause;
3. bounded evidence;
4. completed progress and partial-workspace state;
5. concrete recovery actions;
6. collapsed technical details with machine codes.

Native buttons will copy the Issue #374 working directory, copy either complete
suggested command, open the repair prompt, open the Recovery Plan, and prefill
the existing additional-request textarea. Clipboard and action results use a
polite live region. Prefill only changes local form text. It does not persist,
confirm, dispatch, or execute anything; the existing directive preparation and
confirmation boundary remains mandatory.

Add a GET-only recovery-document endpoint. It accepts only one of the exact
repair prompt or Recovery Plan paths projected from the current terminal
interval, requires the existing Trial token, rejects traversal/symlinks and a
missing workspace, confines the target to the Issue #374 execution workspace,
and retains the existing text-size limit. It does not add a general workspace
file browser or any mutating endpoint.

## Fixtures and verification

Add an Issue #377 corpus with failure, continuation-success, and legacy JSONL
fixtures. Focused Rust tests cover interval isolation, exact typed task
correlation, all seven categories, workspace/artifact states, recovery fields,
legacy fallback, malformed/oversized input, redaction, and the confined recovery
reader. GUI source and browser smoke cover both `/` and
`/proxy/commandagent/`, desktop/mobile layout, headings, native keyboard
activation, screen-reader labels/live feedback, copy/open/prefill actions,
command truncation safety, and the absence of automatic recovery requests.

After focused checks, run formatting, Clippy both without and with `gui`, full
Rust tests, GUI lint/typecheck/build, and the relevant browser smoke. The final
verification report follows the issue-worker exact `Status` / `Checks` contract
and records only commands that completed successfully.

## Preserved boundaries

This change is additive to the session response and GET routes. It does not
rename or rewrite events, weaken verification/acceptance/release gates, change
Gate 1 hashes or directive confirmation, execute recovery commands, enlarge
unbounded evidence, or modify the live `.anvil/` namespace.
