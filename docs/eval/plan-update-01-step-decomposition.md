# Plan Update 01 Step Decomposition

Date: 2026-06-20

## Problem

Recent Next.js UAT showed a step-decomposition failure: a generated `setup`
step attempted to own source/style artifacts such as `app/globals.css` or
`src/app/globals.css`. The execution layer correctly rejected source mutation
under `SetupMutationOnly`, but the planning layer should have rejected the bad
step before tool execution.

Responsible layer:

- Primary: Planning Contract / step-plan lint.
- Supporting: Profile Contract / artifact classification.
- Fallback: Execution Contract / setup mutation policy.
- Reporting: Recovery Task Contract / setup-source violation evidence.

## Adopted Design Lesson

The useful Anvil-side idea is not a hidden job arbiter. The useful idea is that
task contracts, artifact roles, workspace scope, and recovery targets are
control data when they are deterministic and visible.

This slice adopts the smallest piece:

```text
profile + expected_path -> classified artifact role
step kind + artifact role -> deterministic ownership lint
```

## Implemented Contract Change

Code changes:

- `src/agent/step_runner/profile_artifact.rs`
  - adds `ArtifactKind::StyleSource`;
  - classifies `app/globals.css`, `src/app/globals.css`, `styles/globals.css`,
    and `src/styles/globals.css` as `source/style`;
  - keeps global CSS out of Next.js route-integration eligibility;
  - adds helper labels and setup ownership checks.
- `src/agent/step_runner/plan_lint/mod.rs`
  - rejects `StepKind::Setup` when `expected_paths` includes a classified
    non-setup artifact;
  - emits `plan_lint.step_decomposition` contract evidence with failed step,
    rejected path, observed role, allowed setup roles, and required action.

The first enforced rule is intentionally narrow:

```text
setup step + source/route/component/style/test/docs/generated/build expected path
  -> planning contract violation
```

Broader checks for `verify`, `inspect`, and `report` are deferred because
existing flows may use `expected_paths` as read-only existence gates.

## Tests

Focused tests run during implementation:

```text
cargo test profile_artifact
cargo test plan_lint
```

Results:

- `profile_artifact`: passed, including global CSS classification.
- `plan_lint`: passed, including setup ownership rejection and correction
  evidence.

Full verification run during implementation:

```text
cargo fmt --check                         passed
cargo test                                passed, 410 unit tests plus integration/doc test suites
python3 tests/test_eval_report.py          passed
python3 -m py_compile scripts/eval_report.py passed
cargo build --release                     passed
git diff --check                          passed
```

## Evaluation Interpretation

Success for this slice means the prior setup/source ownership violation is
caught at plan lint or corrected before execution. It does not mean that the
generated Next.js app is visually complete, playable, or high quality. Visual,
gameplay, audio, and route-content quality remain separate explicit profile or
eval obligations.

## Focused Gemini UAT

Command:

```text
target/release/commandagent --yes --context-budget 65536 --model gemini-3.1-flash-lite --planner-model gemini-3.5-flash --provider gemini '/ultra-plan-run --profile nextjs あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。'
```

Run root:

```text
/private/tmp/commandagent-plan-update-01-WrB2P2
```

Observed result:

- The setup/source ownership issue did not recur.
- The project setup phase assigned `app/globals.css` to a `create` step, not a
  `setup` step.
- `setup` steps owned only `package.json` and config files such as
  `next.config.js`, `tailwind.config.js`, `postcss.config.js`, and
  `tsconfig.json`.
- The run stopped later in phase `game-engine-core` with an independent
  `tool_protocol` failure: `Write` was emitted without the required `path`
  field for `app/components/SpaceInvaders.tsx`.
- Profile verification then reported
  `nextjs_integration_artifact_missing` for the missing explicit component
  artifact before route integration.

Interpretation:

- The targeted plan-decomposition failure moved: the observed setup/source
  mismatch was not present in this run.
- The remaining blocker is not this slice's Planning Contract ownership rule.
  It is a tool-call/protocol conformance failure followed by the expected
  profile missing-artifact report.
- The app is still incomplete and should not be treated as product-quality
  success.
