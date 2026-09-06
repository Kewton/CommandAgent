# Issue #425 Reopen Design

## Evidence and scope

The current Issue body and owner comment 5550459250 (2026-09-05) supersede
the original repair. The worktree starts at 6ccc3913, the previous #425 fix;
its generated-contract binder only fills an empty contract from a nonempty
failure handoff. Read-only stagnation reads the empty step contract, while
phase execution, invariant and final-acceptance failures pass empty commands.
This explains the empty-contract / empty-handoff cycle. The existing bounded
repair 4/1/1 command handoffs must remain usable.

## Design before implementation

- Initialize the generated run contract before phase execution, with validated
  verification commands supplied by the product's profile runtime. Keep
  step-scoped acceptance contracts narrow: no eager whole-app build there.
- Register validated commands from the admitted StepPlan before its execution
  in the generated Next.js/generic run contract. Preserve those registrations
  when acceptance refreshes the run contract. Do not augment configured or data
  contracts, and do not invent generic commands when no authority exists.
- Centralize read-only selection of the authoritative run contract for failure
  handoffs. Use it for stagnation, phase and final-acceptance failures and empty
  bounded-repair handoffs; retain nonempty bounded-repair checks.
- Retain the existing automatic candidate bind and typed business observers.
  Build success alone cannot establish business acceptance. Keep existing event
  names/schema and readable stop summaries; any telemetry addition is additive.
- Add focused generation, empty/registered contract, step verify present/absent,
  phase/final-acceptance/bounded exhaustion and candidate competition tests, plus
  a corpus fixture. Use independent temporary sessions and deterministic local
  checks for Recovery execution and post-Recovery failure measurement.

## Verification and limits

Run focused Rust tests and corpus regression before `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test`. Check guardrails
without raising baselines. A live provider campaign is expressly excluded by the
approved worker decision; report that limit separately from deterministic tests.
No external session, historical evidence, runtime state, tokenizer, JSON policy,
UTF-8 fix, unrelated documentation, remote mutation or service operation is in scope.
