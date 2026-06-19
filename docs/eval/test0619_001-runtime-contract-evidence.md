# Test0619 001 Runtime Contract Evidence

Date: 2026-06-19

## Input Evidence

The UAT log at `workspace/mvp/uat/test0619_001.md` showed repeated failures
where the saved repair input did not preserve enough deterministic context:

- tool protocol failure: malformed `Write` calls such as missing `path`
- step-policy failure: read-only steps attempting mutation
- verifier failure: `npm run build` failure after partial repair and setup

The failure class was not lack of retry budget. The missing piece was that
already-classified deterministic failures were not consistently rendered into
the bounded repair/replan packet.

## Change

Runtime repair now feeds shared `ContractEvidence` from these producers:

- `tool_protocol`: failed tool, reason code, missing field, required fields,
  and target path when known
- `step_policy`: read-only mutation violation with failed tool and contract
- `verifier`: failed command, verifier reason, bounded diagnostic, and optional
  setup diagnostic context after one approved setup attempt

Dependency setup remains runtime-owned and bounded. It is not a standalone
evidence producer and does not add another retry loop.

## Expected Effect

Standalone repair receives the actionable contract failure first, before broad
feature work:

- malformed tool calls are fixed as tool calls, not treated as generic build
  failures
- read-only mutation is moved into a mutation-allowed step by an explicit
  repair plan instead of being hidden
- verifier repair sees the failed command and whether dependency setup already
  ran

This should improve attribution and repair input quality. It does not guarantee
that a generated Next.js game is visually good, and it does not auto-resume a
failed ultra phase.

## Validation

Local checks passed:

- `cargo test correction_evidence`
- `cargo test repair_loop::tests`
- `cargo test repair::tests`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`
- `scripts/eval_smoke.sh`

Focused UAT:

- root: `/private/tmp/commandagent-uat-test0619-001`
- command: `target/release/commandagent --yes --context-budget 65536 --provider gemini --model gemini-3.1-flash-lite --planner-model gemini-3.5-flash "/ultra-plan-run --profile nextjs ..."`
- result: failed boundedly at `verify-build`, then profile verification reported
  route integration drift
- setup: one approved `npm install` ran and produced `.commandagent/setup/`
  logs
- runtime repair packet:
  `.commandagent/repairs/repair-verify-build-1781859786408.md`
- profile repair packet:
  `.commandagent/repairs/repair-profile-core-game-engine-1781859786408.md`

The `verify-build` repair packet contains the new structured verifier evidence:

```text
Contract evidence:
- evidence 1:
  Contract correction evidence:
  - guard: verifier
  - failed_step: verify-build
  - violated_contract: command_failed:1
  - reason_code: command_failed:1
  - command: npm run build
```

The UAT did not prove end-to-end app quality. It did prove that a verifier
failure now reaches the saved repair packet as structured evidence instead of
only prose. The later `nextjs_route_not_integrated` result remains a profile
contract failure handled by the existing profile repair packet.
