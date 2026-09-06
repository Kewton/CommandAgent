# Issue #425 Reopen Design

## Evidence and scope

The current Issue body and owner comment 5550459250 (2026-09-05) supersede
the original repair. This new worktree starts at current origin/develop
0296d779 plus mechanical WIP-port commit
3d222aaee5c17b0b3b699a064a9f0cd070bd5e76. The original dirty worktree at
6ccc3913 remains read-only. The inherited PR #427 implementation summary and
verification are historical, not completion evidence for this run.

The original generated-contract binder only fills an empty contract from a nonempty
failure handoff. Read-only stagnation reads the empty step contract, while
phase execution, invariant and final-acceptance failures pass empty commands.
This explains the empty-contract / empty-handoff cycle. The existing bounded
repair 4/1/1 command handoffs must remain usable.

Inspection of the carried code confirms early run-contract initialization,
admitted StepPlan registration, and fallback wiring in shared phase handoff
and bounded-repair construction. These changes are unfinished: review their
authority boundaries and all callers, complete focused/corpus coverage, and
run fresh verification. Issues #428 and #430 are independently committed and
will be reused by the orchestrator; do not duplicate their tokenizer, JSON
policy or UTF-8 fixes here.

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
- Scope generation provenance to the active UltraPlan call, including automatic
  Recovery binding after the initial failure. Only a contract actually generated
  by the acceptance binder in that scope can replace an internal step contract.
  An explicit contract with the generated filename or a canonical alias remains
  authoritative. Fresh runs ignore old commands even for identical profile/goal;
  same-run acceptance refresh retains admitted checks. No on-disk provenance
  schema or runtime namespace migration is introduced.
- Register only success-expecting implement/verify step checks, keeping setup,
  inspection, reporting and negative reproducer expectations out of the
  final-success command registry. The profile boundary may exclude exact
  product configuration assertions already enforced by its final verifier;
  Next.js identifies its three generated package-script assertions this way.
  This does not exempt arbitrary Node commands or alter the assertion policy.
  Regression tests must still reject invalid build, dev-port and start-port
  settings through the final profile gate.
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
