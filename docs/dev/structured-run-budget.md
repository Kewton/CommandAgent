# Run budget for Chat Completions (`provider_billed`, Issue #976)

A run budget (wallet) is one operator-issued record that every provider send of a run is
reserved, started and settled against, before and after the HTTP POST. This is v1: it counts
the amount the provider reports it billed on generic OpenAI-compatible Chat Completions.
The direct Responses basis is a separate, versioned sibling: see
[responses-run-budget.md](responses-run-budget.md). Neither is ever read as the other. The
operator-facing declaration is in the Run Budget section of
[profile-tools.md](../guide/en/profile-tools.md#run-budget).

The code is `src/structured_budget/`. With `COMMANDAGENT_RUN_BUDGET` unset, nothing there reads
anything but the variable's presence, and a run's requests, messages, tools, artifacts and
events are as they were.

## Policy: `run-budget.policy/v1`

Read from `<operator root>/run-budgets/<id>.json` (`policy.rs`, `POLICY_PROTOCOL`). The decode
refuses unknown fields, repeated keys, wrong types, zero bounds and a zero price. It holds eight
limits (`cost_usd`, `physical_sends`, `logical_global`, `logical_per_stage`, `tokens`,
`wall_seconds`, `final_repair_attempts`, `final_repair_seconds`), one tariff used only for the
reservation, the route, per-send ceilings, and `ledger_dir`. No price or limit depends on a
model, a domain or a profile name.

## Reserve, start, settle

- A send goes only if `spent + held + reservation` stays within every limit together. Exactly at
  a limit passes; one past it does not.
- Reservation and start are on disk, for the same bytes, before the POST. Transport retries each
  reserve and start on their own.
- A send that was refused before its start gives its reservation back. Once started, a timeout,
  a cancel, a missing amount or a cut body leaves the send unknown: the reservation is held, the
  latch is set for good, and no further send or command success is allowed.
- A late valid reply settles the same send once. A billed amount of an explicit zero is a zero;
  an unreported one is never read as zero.
- A run that selected a wallet but cannot be counted is refused before it sends (`entry.rs`,
  `admission.rs`): a workflow, a model probe, request-pattern routing, the GUI's delegated
  runs, and a run with the OS sandbox off.

## Ledger: `run-budget.ledger/v1`

Kept at `<ledger_dir>/<budget_id>` under one lock, with the hash of the policy's exact bytes.
Any edit of the declaration, a v2 record, or a file that no longer matches what the run wrote is
refused. Resume and fresh starts never reissue a budget. `--summary-json` reads the run's own
`run_budget_summary` event; it never reads the ledger.

## Decisions kept from the plan

- **D3 (variable not in README or `--help`):** the variable is published in the guide's Run Budget
  section and in `configuration.md`, as every other `COMMANDAGENT_*` variable is. It stays out of
  the README and `--help` on purpose.
- **D8 (prefetch):** under a wallet a planner prefetch does not start; one fixed-word refusal is
  emitted instead (`a_prefetch_under_a_run_budget_does_not_start` in `src/planner/pipeline.rs`).
  A prefetch is a send no wallet would see settled. The Issue's "adopt / discard / unsettled
  prefetch" cases are therefore read as "refused before it starts". The user approved this
  reading with the plan.
- **D12 (independent judge):** no judge feature was built. The code makes no billed judge call
  (`SendPurpose` has Planner, Executor, Repair, FinalRepair and Classifier only). Any later
  judge goes through the same transport and so draws on the same wallet. The Issue's acceptance 1
  is checked with a provider call from a separate conversation. The user approved this reading
  with the plan.
- **D11:** see below.

## Known limits

These are records of what v1 does not cover. None of them lets a send past the provider, cost or
token limits, and none turns a refused command into a success, unless the entry says so.

- **Q6 (user decision):** the wallet counts the provider calls CommandAgent makes. A program
  inside an online sandboxed Bash can still reach a provider directly, and that is not counted.
- **A wallet directory made by another uid:** an already existing wallet directory is accepted whatever
  its owner. The owner check applies only to a directory CommandAgent made (`ledger.rs`,
  `refuse_what_was_not_made_here`). Another uid that can write the parent directory can create
  the wallet directory before the first run. That other uid is not a command of the run, so it is
  outside the threat model; this was not introduced by the later hardening either.
- **D11:** a writer with the same uid outside the sandbox can delete the ledger before a run or
  put an older file back. A sandboxed Bash cannot write it. What is caught: a ledger that vanishes
  during a run, a missing ledger on resume, corruption, and a hash mismatch. What is not: a delete
  or rollback before the run starts, which looks like a fresh wallet.
- **R4-I/O:** after a failed bind, "no G4 attempt is left in flight" holds only when the G4
  cleanup (`record_failure`) can be saved. If that write fails, an unresolved attempt can remain
  on disk. It stays fail-closed: no process starts, the command is not a success, and G4's
  ledger refuses what follows. In `src/tools/profile_tools/effects.rs` only the secret-in-output
  case returns the recording error; at the time of writing, other failures of that write are
  dropped without a notice to the operator.
- **L-1 (low):** a send's command number is taken when its body is built
  (`BudgetedBody::new`, called from `post_json` in `src/providers/lm_studio.rs`). A late worker
  that builds its body after the next command began carries the new number, so a refusal of the
  previous command does not stop it. Wallet limits still apply to it. Not fixed here.
- **L-2 (low, narrow form):** a command whose first refusal arrives after the command boundary
  keeps that refusal nowhere (`note_refused` in `src/structured_budget/transport.rs` keeps it
  only for the running command). Stopping every send that is not of the current command (the broad
  form) would close it and is out of scope.
- **L-3 (low):** the guard that stops dependency reconciliation after a refusal sits on the final
  acceptance wrapper only (`ultra_final_acceptance_report_with_deterministic_remedies`, calling
  `structured_budget::command::guard`). The reconciliations in `phase.rs` and
  `phase/phase_boundary.rs` do not ask it. `phase.rs` is held to its guardrail line budget, so a
  fix is out of scope. It could matter only when a refusal is swallowed and the declared
  dependencies then disagree. This is read from the code, not observed.
- **H2:** a send with overrides (a classifier) does not receive the final-repair marker
  (`provider_call.rs`, `overrides.is_none().then(current_final_repair)`). The current production
  callers (`tui/boundary_shell/ambiguity.rs`, `model_probe.rs`) run outside a session, so nothing
  is hurt. Calling a classifier from inside a session could send outside the final-repair window;
  the physical, cost and token limits still apply.
- **H5:** where a refusal is wrapped into a string (`planner/runner/phase/flow.rs`,
  `planner/runner/driver.rs`), its fixed type is lost on the way. G5 reads the handle's own record
  of the refusal, so none is missed, and no production code downcasts to find it. A reader of the
  error text sees the words, not the type.
- A whole reply that fails to parse, and calls the loop drops before the registry, are not logical
  submissions. The physical limit still applies.
- The v1 amount is the provider's report. Whether a real service reports `usage.cost`, and whether
  it is right, was not measured: no live request is part of this work.
