# Test0620 001 Contract Boundaries

Date: 2026-06-20

## Baseline

`workspace/mvp/uat/test0620_001.md` showed that the previous
profile-artifact classifier fix worked for generated declarations, but the run
still failed to converge in later contract boundaries:

- `create-game-component` emitted an invalid `Write` call without `path`.
- `components/SpaceInvaders.tsx` was missing.
- Next.js profile verification reported `nextjs_route_not_integrated`, which
  mixed a missing artifact with route integration drift.
- The profile repair tried to read the missing component before creating it.
- A read-only inspection step attempted mutation and was correctly blocked.
- `npm run build` reached dependency/setup and build repair boundaries.
- Repair turns attempted direct `npm install`, which was correctly blocked.

## Change

This slice keeps the existing design constraints:

- no hidden continuation;
- no retry budget increase;
- no provider/model-specific branch;
- no model-issued dependency setup;
- no weakened verifier or profile checks.

The implementation separates three deterministic boundaries:

- Profile Contract: Next.js now reports
  `nextjs_integration_artifact_missing` before route integration drift is
  evaluated. `nextjs_route_not_integrated` applies only to existing explicit
  artifacts.
- Recovery Task Contract: profile repair evidence for a missing integration
  artifact targets the missing artifact and asks for creation before selected
  route integration.
- Verifier/Setup Contract: dependency setup attempts are bounded by verifier
  step, setup command, and package-manager manifest fingerprint. If repair
  changes `package.json`, `package-lock.json`, or `pnpm-lock.yaml`, approved
  runtime-owned setup may run once for that changed manifest state.

## Evaluation Guidance

A focused rerun should record:

- whether missing `components/SpaceInvaders.tsx` is reported as
  `profile_verification:nextjs_integration_artifact_missing`;
- whether `nextjs_route_not_integrated` only appears after the artifact exists;
- whether the saved profile repair packet targets the missing artifact before
  route integration;
- whether direct `npm install` in model repair remains blocked;
- whether runtime-owned setup reruns only after a manifest fingerprint changes.

Passing these checks means the contract boundary improved. It does not imply
the generated app is visually good or feature-complete.

## Focused UAT

Run:

```text
commit=caaebc4 dirty=true
binary=target/release/commandagent
provider=gemini
model=gemini-3.1-flash-lite
planner_model=gemini-3.5-flash
workspace=/private/tmp/commandagent-test0620-001-uat-20260620012546
```

Command:

```text
/ultra-plan-run --profile nextjs Create a Space Invaders style Next.js app that can run on port 3011.
```

Result:

- The run did not fully converge.
- Runtime-owned setup ran once and produced `.commandagent/setup/...npm-install...`
  logs.
- Direct model-issued `npm install` in repair remained blocked as
  `EnvSetup`.
- The later profile failure was
  `profile_verification:nextjs_route_not_integrated` for
  `app/lib/gameEngine.ts`.
- `app/lib/gameEngine.ts` existed, so this was route integration drift, not a
  missing-artifact false positive.
- The saved profile repair packet targeted `app/page.tsx` and named
  `app/lib/gameEngine.ts` as the candidate artifact.

Remaining failure:

- The app still stopped on profile route integration drift after a verifier
  repair failure. This is a narrower and more accurate failure class than the
  original missing-artifact/route-integration conflation, but it is not product
  success.

## Follow-Up Focused UAT

Run:

```text
commit=caaebc4 dirty=true
binary=target/release/commandagent
provider=gemini
model=gemini-3.1-flash-lite
planner_model=gemini-3.5-flash
workspace=/private/tmp/commandagent-test0620-001-uat-20260620013023
```

Result:

- The run stopped earlier at
  `profile_verification:nextjs_dependency_version_conflict`.
- The generated manifest combined `next=14.0.0` with `react=18.0.0` and
  `react-dom=18.0.0`, which is a package contract failure rather than a route
  integration or missing-artifact failure.
- Because the run stopped at manifest compatibility, it did not re-exercise
  the missing-artifact versus route-integration boundary.
- The follow-up implementation keeps the strict dependency contract and makes
  the repair packet state the required package action explicitly: edit
  `package.json` so `next`, `react`, and `react-dom` are mutually compatible;
  do not keep exact React pins below `18.2` with Next.js 14.

## Local Verification

Commit:

```text
caaebc4 dirty=true
```

Checks:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Result:

- All checks passed.
- `cargo test` passed 391 unit tests plus integration tests.
