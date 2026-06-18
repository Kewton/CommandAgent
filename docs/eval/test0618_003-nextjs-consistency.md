# test0618_003 Next.js Consistency Triage

Date: 2026-06-19
Failure class: `phase_profile_consistency`

## Input

UAT log:

- `workspace/mvp/uat/test0618_003.md`

Generated workspace:

- `/Users/maenokota/share/work/localwork/commandagent/test0618_003`

Command:

```text
commandagentdev --yes --context-budget 65536 \
  --model gemini-3.1-flash-lite \
  --planner-model gemini-3.5-flash \
  --provider gemini
```

Prompt:

```text
/ultra-plan-run --profile nextjs あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。
```

## Summary

Dependency setup recovery worked: the run reached `npm install` through the
bounded dependency recovery path and later produced a buildable Next.js
workspace. The remaining issue was not dependency setup. The issue was that
phase and profile contracts drifted across the ultra run.

The original ultra plan had five phases:

- `project-setup`
- `core-game-loop`
- `visual-effects-and-juice`
- `advanced-gameplay-and-boss`
- `UI-and-polish`

The original run stopped at phase 3 `visual-effects-and-juice` during
`verify-build`. The suggested repair commands created standalone repair ultra
plans. Those repair plans addressed build failures, but they did not resume and
complete the original phase 4 and phase 5 scope.

## Evidence

### App Root Drift

Phase 1 created `src/app/layout.tsx`, `src/app/page.tsx`, and
`src/app/globals.css`. The final workspace also contains root `app/` files:

- `app/layout.tsx`
- `app/page.tsx`
- `app/globals.css`

Later generated files include:

- `components/SpaceInvaders.tsx`
- `src/lib/game/entities.ts`
- `src/lib/game/engine.ts`

The final visible route is under `app/page.tsx`, while earlier game work was
created under `src/app` and `src/lib/game`. Build success did not prove that
the requested game implementation was integrated into the selected route.

### Port Contract Drift

An earlier step verified a `package.json` dev script containing:

```json
"dev": "next dev -p 3011"
```

The final generated `package.json` contains:

```json
"dev": "next dev"
```

The requested port contract was lost after a later package rewrite.

### Tailwind Contract Drift

The workspace contains Tailwind-related files and CSS intent:

- `tailwind.config.js`
- `postcss.config.js`
- `app/globals.css`

The final `package.json` dependencies contain `next`, `react`, and
`react-dom`, but not `tailwindcss`, `postcss`, or `autoprefixer`.

### Tool And Step Boundary Drift

The UAT log shows inspect steps mutating files and some tool/protocol failures
being followed by step success:

- `inspect-workspace` wrote `package.json`, `app/layout.tsx`, `app/page.tsx`,
  `tailwind.config.js`, `app/globals.css`, `postcss.config.js`, and
  `tsconfig.json`.
- A compound Bash command in `create-app-files` was blocked, but verifier
  commands then passed and the step was reported `ok`.
- A phase 3 inspect step edited `src/app/page.tsx` and
  `src/lib/game/engine.ts`, wrote `src/lib/game/audio.ts`, then hit
  `minimal loop reached max iterations`; the step was still reported `ok`.

## Decision

This UAT should be treated as a consistency failure, not as a dependency setup
failure. The runtime should:

- refresh workspace facts before each ultra phase
- make step kind tool boundaries explicit and enforced
- prevent fatal tool/turn errors from being hidden by empty or weak verifiers
- run read-only profile verification at phase boundaries
- keep standalone repair-plan success distinct from original ultra-plan
  completion

The implemented checks are intentionally deterministic. They do not judge UI
quality semantically and they do not add hidden retries or automatic original
plan resume.

## Post-Change Focused UAT

After adding phase workspace contracts, step tool policy, fatal turn-error
gating, and Next.js profile verification, a focused UAT was run from the
current release binary:

Workspace:

- `/private/tmp/commandagent-uat-test0618-003-current3`

Headline result:

- stopped explicitly with `initial turn error: invalid tool arguments: missing string field path`
- failed step: `edit-page-tsx`
- repair packet: `.commandagent/repairs/repair-edit-page-tsx-1781801824965.md`

Observed files at stop time:

- `package.json`
- `next.config.mjs`
- `tsconfig.json`
- `src/app/layout.tsx`
- `src/app/page.tsx`
- `src/components/SpaceInvaders.tsx`

Interpretation:

- The fatal tool/protocol error was no longer hidden by an empty or weak
  verifier.
- The run did not report original ultra-plan completion.
- The repair packet explicitly says the suggested command starts a standalone
  repair plan and that the original ultra plan remains incomplete until it is
  explicitly resumed or replanned.

This is an expected bounded stop for the current slice. It is not a completed
app-quality UAT.
