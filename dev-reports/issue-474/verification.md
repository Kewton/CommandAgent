# Issue #474 worker verification

- Status: `passed`

## Checks

- `git fetch origin develop`: `passed`
- `git rev-parse HEAD origin/develop`: `passed`
- `cargo test --lib issue474 -- --nocapture`: `passed`
- `cargo test --lib issue439 -- --nocapture`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `shasum -a 256 target/release/commandagent`: `passed`
- `git diff --check`: `passed`

## Results and scope

These are the required worker-local checks. The dispatch explicitly assigns
targeted UAT orchestration, PR/CI, merge, and the integrated release check to the
parent. This status does not mark those later gates or live capability evaluation
complete.

The isolated branch started at fetched origin/develop
`e43b2d86760a77b5250afbb260106ea21abba947`. The focused suite passed 9 tests;
existing #439 passed 16. The final full `cargo test` exited 0 outside the sandbox
so local HTTP/socket tests could run: 2,553 library tests passed, 19 existing
library tests were ignored, and all integration targets and both doctests passed.
Corpus (7), generality guardrails (10), conformance, and protection coverage were
included in that successful full run. Existing ignored integration/PTY helpers
remain ignored and are not counted as passed. No new ignored tests were added.

Rust/cargo 1.94.0 and Node v24.1.0 were used. The verified 14 production, test,
and corpus files are bound by SHA256 in [verification-evidence.json](verification-evidence.json).
No product/test source changed after the final Clippy and full test run.

## Deterministic acceptance evidence

The original command was copied without changing bytes. Original contract hash
`47e90522229c8d82fb715085c83835978e1dd4a5000e6420746ba25d09bd5157` and central
events hash `8671fd95b592b5fa9ef76266db161a5903806e0b1b9ca08d06c21ea70ea1be82`
were rechecked against their read-only source after implementation.

Before production edits, the focused saved-command regression failed with
`Weak("node_smoke_without_assertion")` versus expected StaticSyntax. After the
change, it passes without satisfying bound business verification or input-handler
evidence. Actual Node executions individually fail for missing primary, input,
and state, with the corresponding `Error: missing ...` in captured stderr.

The final acceptance test calls the product boundary with a generic profile to
isolate command execution from npm/browser infrastructure. It separately calls
the actual Next.js repair mapping and tests Next.js generated registry admission,
refresh, and recovery binding. This is not a new Next.js business-app/browser UAT.
All 21 unsupported/masked/deleted/opaque command controls remain rejected.
Alternate-path passes, stale final passes, and unrelated assertion/API additions
do not substitute for the retained original check. Genuine Test commands remain
registered and classified independently.

The old weak-only report reproduces the recorded API `repair_changed` selection.
Recognized, genuinely failing hook commands select their source page through
`contract_attribute`; unrelated API edits do not pass, while repairing the hook
and freshly verifying does. The independently reproduced attribute-diagnostic
defect remains open for parent handoff in
[diagnostic-follow-up.md](diagnostic-follow-up.md). Its reproduction passing means
the defect was demonstrated, not fixed. Codex2 agreed with the separation in two
read-only discussions, both returned with exit 0 and source `history`.

Initial guardrail feedback rejected test placement/counting; tests were moved
to leaf wiring and marked test-only without changing baselines. Clippy's iterator
feedback and captured-stderr test plumbing were corrected before the final run.
The failure gate was never weakened to address these findings.

## Local release identity

The release was built from the verified modified tree before this worker commit:

```text
commandagent 0.1.0 e43b2d86+dirty 2026-09-12T19:48:28+09:00
SHA256 4e9451f10df75357fadfcbbcc24e7397e7fda1e2c44547b7ba07df663884e6e3
```

The dirty marker and embedded timestamp are precommit source metadata, not an
integrated release identity or the build completion time. This binary was not
installed into a shared launcher or GUI runtime.

## Parent gates and CI provenance

The public GitHub Actions API was queried for the full fetched parent SHA after
`gh auth status` reported an invalid token. Exact-SHA results were all
`completed / success`: [CI](https://github.com/Kewton/CommandAgent/actions/runs/34689412790),
[acceptance](https://github.com/Kewton/CommandAgent/actions/runs/34689412791), and
[Next.js domain oracles](https://github.com/Kewton/CommandAgent/actions/runs/34689412802).
Those runs verify the parent, not this new worker commit.

The parent must run the assigned targeted UAT and this commit's CI, merge when
appropriate, and check the integrated release version/hash. The independent
diagnostic follow-up has a concrete reproduction and ownership proposal; no new
Issue was created. No PR, push, merge, lifecycle mutation, model campaign, or
historical evidence/runtime migration was performed by this worker.
