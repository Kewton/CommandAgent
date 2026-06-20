# Logic 001 Contract Boundary Propagation

Date: 2026-06-20

## Baseline

Commit before this slice:

```text
ce7f805ac00d5ec74d1df4216af7aff6e49ecfd7
```

Working tree was dirty before this slice from prior MVP work.

Primary observed evidence:

- `workspace/mvp/uat/test0620_005.md`

Baseline failure sequence:

1. Gemini `/ultra-plan-run --profile nextjs` stopped after
   `project-setup`.
2. Profile verification detected:
   - `nextjs_dependency_version_conflict`
   - `nextjs_tailwind_contract`
3. Explicit profile repair completed.
4. Re-running the original task stopped later after `core-game-engine`.
5. Manual build inspection showed missing `@tailwindcss/postcss`.
6. Profile verification reported disconnected artifacts:
   - `app/hooks/useGame.ts`
   - `app/types/game.ts`

## Implemented Direction

This slice implements the updated Contract Boundary Propagation design for the
observed Next.js failure classes.

Shared evidence now carries bounded propagation fields:

- `repair_kind`
- `setup_implication`
- `rerun_authority`

Next.js profile repair evidence projects these fields for:

- manifest dependency repair;
- Tailwind contract repair;
- route integration repair;
- integration artifact creation.

Verifier/setup recovery now treats a declared missing package diagnostic such
as `Cannot find module '@tailwindcss/postcss'` as `dependency_missing` when
the package is declared in `package.json` but absent from `node_modules`.

Setup command selection now checks whether `package-lock.json` reflects
declared dependencies. If the lockfile is stale after manifest repair,
approved setup recovery can select bounded `npm install` instead of `npm ci`.

Next.js route integration now uses a bounded static route graph from the
selected route through relative imports. This allows route -> component -> hook
integration to pass and lets repair packets target a deterministic route-tree
file when an artifact is disconnected.

Follow-up focused E2E also exposed two adjacent plan-contract gaps, now covered
by this slice:

- Next.js phase plans must not use `npx` verifiers. `npx tsc --noEmit` is not
  connected to verifier-owned setup recovery and is blocked by Bash policy, so
  Next.js source verification is steered to `npm run build`.
- Generated TypeScript Next.js apps must make the TypeScript toolchain family
  explicit in package.json work. If a plan creates `tsconfig.json`, `.ts`,
  `.tsx`, or TypeScript code, the package step must mention a stable
  TypeScript 5.x and `@types/react` 18.x family instead of allowing TypeScript
  6 or React 19 type packages with Next.js 14/React 18.

## Design Boundary

This does not add:

- hidden continuation;
- retry budget increases;
- model-issued dependency setup;
- provider/model-specific behavior;
- profile-owned workflow execution;
- visual/gameplay semantic scoring.

The profile detects deterministic facts. Recovery Task Contract renders the
repair task. Verifier/setup owns bounded setup recovery. The minimal loop still
executes one bounded task at a time.

## Local Checks

Focused check run during implementation:

```text
cargo test -q step_runner
```

Result:

```text
284 passed
```

Final local checks:

```text
cargo fmt --check
cargo test
cargo build --release
```

Result on 2026-06-20 after implementation:

```text
cargo fmt --check: pass
cargo test -q step_runner: pass, 284 passed
cargo test: pass, 457 unit tests plus integration tests passed
cargo build --release: pass
```

## E2E Plan

After local checks pass, run focused Gemini E2E with the release binary and the
same Space Invaders Next.js task class as `test0620_005`.

Acceptance for this slice:

- previous manifest/setup drift should either be fixed or become explicit
  verifier-owned setup evidence;
- missing declared package diagnostics should classify as `dependency_missing`;
- route integration failure should include route-tree target evidence;
- any remaining stop should be a new explicit layer-specific blocker.

## Focused E2E Results

Runs were executed with:

```text
provider=gemini
model=gemini-3.1-flash-lite
planner-model=gemini-3.5-flash
binary=target/release/commandagent
```

Observed progression:

- `20260620g`: stopped at `verify-compilation` because the plan used
  `npx tsc --noEmit`; this was classified as a plan-contract gap because
  `npx` is blocked by Bash policy and cannot trigger verifier-owned setup
  recovery.
- `20260620h`: after adding Next.js verifier lint, the run reached
  `verify-nextjs-build`, ran `npm install --include=dev`, and then failed on
  a source/type error caused by generated `typescript@6.0.3` and
  `@types/react@19.2.17` with Next.js 14/React 18. This was classified as a
  profile dependency-family gap.
- `20260620i`: after adding TypeScript toolchain contract, the run stopped
  earlier at Tailwind plan lint because bounded plan correction still did not
  replace the phrase `Tailwind CSS` with the exact package literals
  `tailwindcss`, `postcss`, and `autoprefixer` in the package step.

Conclusion:

- The original setup handoff problem improved: dependency setup now reaches
  runtime-owned `npm install --include=dev`, not model-issued Bash.
- The `npx` verifier problem is now caught before execution.
- The TypeScript toolchain major-version problem is now caught by plan/profile
  contracts.
- Full Gemini E2E is not yet green. The remaining blocker is bounded Tailwind
  plan correction failing to satisfy exact package literal requirements after
  three correction attempts.

## Residual Risk

Even if build/profile contracts improve, app quality may still be poor. Visual
quality, gameplay depth, and audio are separate obligations and should not be
treated as solved by this contract-boundary slice.
