# Logic 002 Contract Orchestration

Date: 2026-06-20

## Baseline

Primary evidence:

- `workspace/mvp/uat/test0620_006.md`
- `docs/eval/logic-001-contract-boundary-propagation.md`

The previous focused Gemini E2E improved verifier/setup handoff but still did
not converge. The remaining blocker was a Next.js Tailwind plan-lint failure:
the corrected plan wrote conceptual `Tailwind CSS` text while omitting exact
manifest literals:

- `tailwindcss`
- `postcss`
- `autoprefixer`

This is a planning/manifest contract failure, not a source implementation or
provider transport failure.

## Implemented Direction

This slice applies the contract-orchestrated design update:

- evidence can carry `active_job`, `artifact_role`, and explicit
  `disallowed_actions`;
- Recovery Task Contract renders the active job and artifact role;
- Next.js Tailwind plan-lint evidence classifies the failure as
  `active_job=manifest_repair`;
- the evidence targets the unambiguous package step when exactly one
  `package.json` step exists;
- generated step plans can be deterministically materialized with a
  plan-level manifest obligation before rerunning the same plan lint;
- bounded correction attempts attach an attempt ledger and final exhausted
  failures include the rendered evidence.

The materialized Next.js Tailwind manifest obligation is limited to setup
contract facts:

- `next`
- `react`
- `react-dom`
- `typescript 5.x`
- `@types/react 18.x`
- `tailwindcss`
- `postcss`
- `autoprefixer`
- `scripts.build=next build`
- `tailwind.config.js`
- `postcss.config.js`

## Design Boundary

This does not add:

- a second execution engine;
- hidden continuation;
- retry budget increases;
- provider/model-specific Gemini behavior;
- model-issued dependency installation;
- source/gameplay quality scoring;
- package-registry solving.

The new behavior clarifies the current repair job before execution. If the
target package step is ambiguous, CommandAgent does not patch source or guess a
target.

## Local Checks

Initial focused check during implementation:

```text
cargo test -q step_runner
```

Result:

```text
285 passed
```

Final local checks and focused Gemini E2E should be recorded after the
implementation is complete.

Final local checks:

```text
cargo fmt --check: pass
git diff --check: pass
cargo test -q step_runner: pass, 285 tests
cargo test: pass, 458 unit tests plus integration tests
cargo build --release: pass
```

## Focused Gemini E2E

Run roots:

```text
/private/tmp/commandagent-logic-002-e2e-20260620a
/private/tmp/commandagent-logic-002-e2e-20260620b
```

`20260620a` failed at `plan_lint.profile_obligations`, not at the Tailwind
source/style lint. This showed the same manifest contract needed active job
classification and materialization from the profile-obligation producer too.

After extending that producer, `20260620b` progressed past the previous
manifest exact-literal blocker. The saved plan contains a deterministic
manifest obligation block in the package step, dependency setup ran, and the
run reached profile verification after the game-engine phase.

Final `20260620b` stop:

```text
profile_verification:nextjs_route_not_integrated
app/game/engine.ts is not referenced from selected route graph rooted at app/page.tsx
app/game/types.ts is not referenced from selected route graph rooted at app/page.tsx
```

Repair packet:

```text
.commandagent/repairs/repair-profile-design-game-engine-and-core-loop-1781941785939.md
```

Interpretation:

- Targeted Logic 002 blocker improved.
- Full Gemini E2E is still not green.
- The remaining blocker is route integration profile repair, not manifest
  setup or Tailwind exact package literals.
- The repair packet has structured route-integration recovery tasks with
  `repair_kind=route_integration_repair`, `repair_target=app/page.tsx`,
  `setup_implication=none`, and `rerun_authority=profile_verification,npm run build`.

The detailed UAT record is:

- `workspace/mvp/uat/test0620_007.md`
