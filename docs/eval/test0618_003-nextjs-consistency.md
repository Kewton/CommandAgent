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

## Focused Eval 001 Implementation

Date: 2026-06-19

This slice addresses the later observed phase/profile drift where a generated
Next.js app kept building but lost the requested `3011` dev-port contract.

Implemented changes:

- profile-owned, read-only obligations for Next.js phase planning
- phase workspace contract rendering of profile obligations
- obligation-aware step-plan lint for package.json create/edit/setup/repair
  steps
- reuse of the existing bounded generated-plan correction path when a phase
  step plan omits a profile obligation
- bounded profile repair packets when phase-boundary profile verification
  fails
- eval reason extraction for explicit profile verification failures as
  `profile_verification:<code>`

Design interpretation:

- The fix works on common step-runner boundaries: profile facts, phase contract,
  plan lint, profile verification, and eval classification.
- It does not add provider-specific behavior, hidden retries, profile workflow
  engines, or automatic original-plan resume.
- Next.js-specific logic remains limited to small deterministic facts and
  verification rules.

Expected evidence after rerun:

- If generated package.json work omits `3011`, the phase step plan should be
  rejected before execution and corrected by the bounded plan correction path.
- If execution still produces package.json drift, the phase should stop with
  `nextjs_dev_port_drift`, save a profile repair packet, and report
  `profile_verification:nextjs_dev_port_drift` in eval output rather than only
  `rc:1`.

## Focused Eval 001 Result

Run root:

- `/private/tmp/commandagent-eval-focused-nextjs-runs/20260619T090959`

Command shape:

```text
scripts/eval_agent_slice.sh \
  --cases-dir /private/tmp/commandagent-eval-focused-nextjs-current \
  --out /private/tmp/commandagent-eval-focused-nextjs-runs \
  --runs 1 \
  --binary target/release/commandagent \
  --provider gemini \
  --model gemini-3.1-flash-lite
```

Summary:

```text
case_id               run  rc  success  reason
large-nextjs-app-new  1    1   false    profile_verification:nextjs_dev_port_drift
```

Runtime error:

```text
profile verification failed for nextjs after phase engine:
nextjs_dev_port_drift: scripts.dev must preserve requested port 3011, got `next dev` (package.json).
profile repair prompt saved: .commandagent/repairs/repair-profile-engine-1781827827731.md
```

Interpretation:

- The run is not an app-quality pass.
- The targeted failure is now classified as profile-contract drift instead of
  a generic `rc:1` or semantic-only failure.
- A bounded profile repair packet was written with phase contract facts,
  profile facts before/after drift, expected paths, and explicit continuation
  semantics.
- The original ultra plan still stops visibly; no hidden retry or automatic
  original-plan resume was introduced.

## Focused Eval 002 Result

Date: 2026-06-19

Implemented changes:

- active profile contract carrier for step execution
- active contract facts rendered into step prompts
- active contract facts rendered into normal verifier repair prompts and
  exhausted repair packets
- inverse app-root drift lint coverage
- Tailwind dependency obligation admitted from the focused eval evidence where
  phase 1 requested Tailwind CSS but package.json omitted `tailwindcss`,
  `postcss`, and `autoprefixer`

Design interpretation:

- The common runtime remains a bounded carrier of profile facts. It does not
  parse Next.js semantics, mutate files automatically, add hidden retries, or
  auto-resume the original ultra plan.
- The new Tailwind behavior is profile-owned and evidence-based. It reuses the
  existing phase-planning obligation and bounded plan-correction path.

First focused run after active-contract projection:

- run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-002/20260619T124046`
- result:
  `profile_verification:nextjs_tailwind_contract`
- evidence:
  package.json preserved `dev: next dev -p 3011`, `build: next build`, and
  Next.js runtime dependencies, but Tailwind CSS/config existed without
  `tailwindcss`, `postcss`, and `autoprefixer`.

Second focused run after adding the Tailwind dependency obligation:

- run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-002/20260619T124417`
- command shape:

```text
COMMANDAGENT_PLANNER_MODEL=gemini-3.5-flash \
COMMANDAGENT_CONTEXT_BUDGET=65536 \
scripts/eval_agent_slice.sh \
  --cases-dir /private/tmp/commandagent-eval-focused-nextjs-current-002 \
  --out /private/tmp/commandagent-eval-focused-nextjs-runs-002 \
  --runs 1 \
  --binary target/release/commandagent \
  --provider gemini \
  --model gemini-3.1-flash-lite
```

Summary:

```text
case_id               run  rc  success  reason
large-nextjs-app-new  1    1   false    rc:1
```

Runtime error:

```text
initial turn error: invalid tool arguments: missing string field `path`
step setup-tailwind-layout failed verification
repair prompt saved: .commandagent/repairs/repair-setup-tailwind-layout-1781840739501.md
```

Observed contract state at stop time:

- `package.json` preserved `next dev -p 3011`
- `package.json` preserved `next build`
- `package.json` included `next`, `react`, `react-dom`, `tailwindcss`,
  `postcss`, and `autoprefixer`
- selected app root stayed `app`
- the repair packet included active profile contract facts, including package
  scripts, dependencies, selected app root, Tailwind CSS fact, and rendered
  profile obligations

Interpretation:

- The original `nextjs_dev_port_drift` class is not reproduced in this focused
  run.
- The newly observed `nextjs_tailwind_contract` class was addressed through a
  narrow profile obligation.
- The remaining failure is a model/tool-call quality issue: Gemini emitted an
  invalid tool call without a required `path` field during `setup-tailwind-layout`.
  CommandAgent treated it as a bounded turn error and saved a repair packet
  instead of silently continuing.
- This is still not an app-quality pass. It is a clearer bounded stop with the
  active profile contract preserved in the evidence packet.

## Focused Eval 003 Baseline

Date: 2026-06-19

This slice targets the remaining failure from the second focused `eval/002`
run:

- run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-002/20260619T124417`
- failed step:
  `setup-tailwind-layout`
- runtime error:

```text
initial turn error: invalid tool arguments: missing string field `path`
step setup-tailwind-layout failed verification
```

Classification:

- common execution/tool-call schema failure
- not Next.js profile verification
- not dependency setup
- not UI or gameplay quality

Anvil comparison:

- Anvil also rejects missing required tool fields in its tool registry.
- Anvil additionally classifies malformed tool-call behavior as
  `ToolProtocolFailure` / `tool_call_format_error` and has broader bounded
  recovery notes.
- CommandAgent does not port Anvil's broader reminder, sidecar, case-memory,
  or recovery-job stack. It adopts only the small common-contract idea:
  tool-call schema failure is distinct from verifier failure and profile
  failure.

Implemented changes for `eval/003`:

- `MinimalLoopError::ToolArgs` now carries structured `ToolArgError` data.
- Tool argument rejection includes the tool name, missing field, and required
  fields for the selected tool.
- Step-runner repair evidence classifies:
  - `tool_args_missing_required_field`
  - `tool_args_invalid_json`
- The existing bounded repair loop can spend one current-step schema-correction
  chance, then stops explicitly if malformed tool calls continue.
- Eval summary extraction can report `tool_args_missing_required_field:path`
  instead of collapsing this class to `rc:1`.
- The behavior is provider-independent and does not trigger dependency setup,
  weaken verifier/profile checks, or add a hidden retry loop.

## Focused Eval 003 Result

Date: 2026-06-19

Run root:

- `/private/tmp/commandagent-eval-focused-nextjs-runs-003/20260619T132916`

Command shape:

```text
COMMANDAGENT_PLANNER_MODEL=gemini-3.5-flash \
COMMANDAGENT_CONTEXT_BUDGET=65536 \
scripts/eval_agent_slice.sh \
  --cases-dir /private/tmp/commandagent-eval-focused-nextjs-current-002 \
  --out /private/tmp/commandagent-eval-focused-nextjs-runs-003 \
  --runs 1 \
  --binary target/release/commandagent \
  --provider gemini \
  --model gemini-3.1-flash-lite
```

Summary:

```text
case_id               run  rc  success  reason
large-nextjs-app-new  1    1   false    tool_args_missing_required_field:path
```

Runtime error:

```text
initial turn error: invalid tool arguments: Write missing string field `path` (required fields: path, content)
step create-tailwind-postcss-config failed verification
repair prompt saved: .commandagent/repairs/repair-create-tailwind-postcss-config-1781843393784.md
```

Repair packet evidence:

- initial turn reason: `tool_args_missing_required_field`
- repair turn reason: `tool_args_missing_required_field`
- diagnostic names `Write`, missing field `path`, and required fields
  `path, content`
- package contract facts were preserved in the packet:
  - `scripts.dev = next dev -p 3011`
  - `scripts.build = next build`
  - dependencies include `next`, `react`, `react-dom`, `tailwindcss`,
    `postcss`, and `autoprefixer`

Interpretation:

- The run is not an app-quality pass.
- The former coarse `rc:1` class is now attributed as
  `tool_args_missing_required_field:path`.
- The initial schema failure spent the step's schema-correction chance. The
  repair turn repeated the same malformed `Write` call, so CommandAgent stopped
  boundedly and saved the explicit repair packet.
- This is the expected behavior for persistent malformed tool calls under the
  `eval/003` design: classify, provide one bounded correction opportunity, then
  stop visibly rather than adding a hidden retry loop.

## Focused Eval 004 Result

Date: 2026-06-19

This slice makes tool-call schema recovery a strict protocol contract
correction rather than ordinary repair prose. The implemented behavior is:

- classify parsed malformed tool calls as `tool_args_*`
- issue one strict correction prompt with failed tool, missing field, required
  fields, and deterministic target path when available
- allow the same correction in a verifier repair turn, because that repair turn
  is an explicit mutation-allowed session
- rerun the same expected-path checks and verifiers after correction
- stop explicitly if malformed tool calls repeat

Baseline run after the first protocol-correction implementation:

- run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-004/20260619T142609`
- result:
  `tool_args_missing_required_field:path`
- failed step:
  `verify-nextjs-build`
- repair packet:
  `.commandagent/repairs/repair-verify-nextjs-build-1781846872494.md`

Interpretation of the baseline:

- dependency setup had already run and `node_modules` existed
- `npm run build` failed for an ordinary implementation/config reason
- the repair turn then emitted malformed `Edit` without `path`
- because the step kind was `verify`, the initial implementation treated that
  schema failure as terminal instead of issuing protocol correction

Final focused run after allowing protocol correction inside verifier repair
turns:

- run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-004/20260619T143133`
- command shape:

```text
COMMANDAGENT_PLANNER_MODEL=gemini-3.5-flash \
COMMANDAGENT_CONTEXT_BUDGET=65536 \
scripts/eval_agent_slice.sh \
  --cases-dir /private/tmp/commandagent-eval-focused-nextjs-current-004 \
  --out /private/tmp/commandagent-eval-focused-nextjs-runs-004 \
  --runs 1 \
  --binary target/release/commandagent \
  --provider gemini \
  --model gemini-3.1-flash-lite
```

Summary:

```text
case_id               run  rc  success  reason
large-nextjs-app-new  1    1   false    profile_verification:nextjs_route_not_integrated
```

Observed workspace state:

- `package.json` preserved `dev: next dev -p 3011`
- `package.json` preserved `build: next build`
- dependency setup completed and produced `node_modules`
- `npm run build` completed far enough to produce `.next`
- selected app root stayed `app`
- generated game hook existed at `app/hooks/useGame.ts`
- selected route `app/page.tsx` did not import or reference that hook

Profile repair packet:

```text
profile verification failed for nextjs after phase game-engine-hook:
nextjs_route_not_integrated: explicit artifact `app/hooks/useGame.ts`
is not referenced from selected route `app/page.tsx`.
```

Interpretation:

- The targeted `tool_args_missing_required_field:path` failure was not
  reproduced in the final focused run.
- The failure moved to a later, more specific profile-contract class:
  `profile_verification:nextjs_route_not_integrated`.
- The run is still not an app-quality pass. It stopped because phase output was
  not integrated into the selected route, which is a separate profile/phase
  contract issue from tool-call protocol correction.
- The new behavior remains bounded and provider-independent: it adds one
  schema correction in the responsible repair context and does not add hidden
  retries, dependency setup from schema failure, or automatic original-plan
  resume.

## Focused Eval 005 Implementation

Date: 2026-06-19

This slice targets the later failure from `eval/004`:

```text
profile_verification:nextjs_route_not_integrated
```

Baseline evidence:

- run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-004/20260619T143133`
- selected route:
  `app/page.tsx`
- unintegrated artifact:
  `app/hooks/useGame.ts`
- prior targeted tool-call schema failure was not reproduced

Implemented changes:

- Next.js profile can emit `nextjs_route_integration_required` when a selected
  route is known and an explicit source artifact is in the phase contract.
- Step-plan lint rejects Next.js create/edit/repair source steps that introduce
  an integration candidate but omit the selected route from both instruction
  and `expected_paths`.
- The existing `nextjs_route_not_integrated` verifier remains the final
  read-only guard after execution.
- Profile repair packets now include route integration targets with
  `selected_route`, `unintegrated_artifact`, and expected behavior.

Design interpretation:

- The change is Next.js-only and uses existing profile obligation, plan-lint,
  profile verification, and repair packet channels.
- It does not add a generic artifact graph, hidden retry loop, Anvil
  focused-edit stack, provider-specific prompt branch, or automatic original
  ultra-plan resume.
- Cross-profile rollout is deferred until another observed failure justifies a
  common contract.

## Focused Eval 005 Result

Date: 2026-06-19

Run root:

- `/private/tmp/commandagent-eval-focused-nextjs-runs-005/20260619T152751`

Command shape:

```text
COMMANDAGENT_PLANNER_MODEL=gemini-3.5-flash \
COMMANDAGENT_CONTEXT_BUDGET=65536 \
scripts/eval_agent_slice.sh \
  --cases-dir /private/tmp/commandagent-eval-focused-nextjs-current-004 \
  --out /private/tmp/commandagent-eval-focused-nextjs-runs-005 \
  --runs 1 \
  --binary target/release/commandagent \
  --provider gemini \
  --model gemini-3.1-flash-lite \
  --timeout-secs 900
```

Summary:

```text
case_id               run  rc  success  reason
large-nextjs-app-new  1    1   false    missing:package.json,app/page.tsx
```

Runtime error:

```text
ERROR: plan lint failed: step `create-package-json` has invalid instruction:
profile obligations require package.json work to mention
nextjs_dependencies_required: next, react, and react-dom
```

Invalid plan evidence:

- three invalid phase step plans were saved under `.commandagent/plans/`
- each `create-package-json` instruction mentioned Next.js, React,
  TailwindCSS, PostCSS, Autoprefixer, and Lucide Icons
- none explicitly mentioned `react-dom`
- execution never reached file creation, dependency setup, build verification,
  or route-integration verification

Interpretation:

- The previous `profile_verification:nextjs_route_not_integrated` failure was
  not reproduced, but the run did not reach the route-integration slice.
- The focused eval is therefore blocked by an earlier planning-quality failure
  against an existing package dependency obligation.
- This does not prove app-quality success or route-integration success.
- The result is still bounded and explicit: the invalid phase step plan was
  rejected before execution instead of creating incomplete files.
- The eval summary's `missing:package.json,app/page.tsx` reason is the
  post-run semantic artifact check; the actionable runtime cause is the
  package obligation plan-lint failure above.

## Focused Eval 006 Implementation

Date: 2026-06-19

Targeted failure:

```text
plan lint failed: step `create-package-json` has invalid instruction:
profile obligations require package.json work to mention
nextjs_dependencies_required: next, react, and react-dom
```

Implemented changes:

- Added a step-runner `PlanCorrectionEvidence` payload for deterministic
  correction facts.
- Next.js package/profile obligation lint now reports exact required and
  missing literals, including `react-dom`.
- Next.js route-integration lint now reports the selected route as a required
  and missing path when a source artifact is disconnected from the selected
  route contract.
- Generated step-plan parse failures preserve lint evidence instead of reducing
  it to prose before bounded correction.
- Invalid-plan correction prompts render a `Contract correction evidence`
  block and ask the planner to copy exact required literals and paths into the
  corrected YAML.

Design interpretation:

- This is structured evidence rendering, not a new recovery controller.
- The existing invalid-plan correction budget is unchanged.
- The original plan lint guard reruns unchanged after correction.
- No provider/model-specific prompt branch, sidecar, case memory, artifact
  completion job, or hidden continuation was introduced.
- The implementation initially populates evidence only for the observed Next.js
  profile-obligation failure classes.

Focused eval result:

- run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-006/20260619T160353`
- provider/model:
  `gemini` / `gemini-3.1-flash-lite`
- planner model:
  `gemini-3.5-flash`
- binary:
  `target/release/commandagent`
- commit:
  `507882f65bd4eabf57d1ec9839eada92fe69b07d`
- dirty:
  `true`

Summary:

```text
case_id               run  rc  success  reason
large-nextjs-app-new  1    1   false    profile_verification:nextjs_route_not_integrated
```

Observed improvement:

- The eval moved past the previous `create-package-json` plan-lint failure for
  `nextjs_dependencies_required`.
- The accepted setup plan explicitly instructed the package step to include
  `next`, `react`, `react-dom`, `tailwindcss`, `postcss`, and `autoprefixer`.
- The generated `package.json` includes `react-dom` and preserves
  `scripts.dev: next dev -p 3011` and `scripts.build: next build`.
- Execution reached later project creation, dependency setup/build repair, and
  profile verification.

Remaining failures:

- `npm run build` produced a bounded repair packet with
  `Cannot find module 'tailwindcss'`.
- Final profile verification stopped with
  `nextjs_route_not_integrated` for `app/audio.ts` and `app/types.ts`, because
  the selected route `app/page.tsx` did not reference those explicit artifacts.
- The route-integration repair packet names the selected route and
  unintegrated artifacts.

Interpretation:

- The targeted eval 006 failure class, package-obligation correction ambiguity
  around `react-dom`, was not reproduced.
- The run did not become an app-quality pass.
- The remaining failures are later, independent classes: dependency/build
  environment resolution and route integration for additional generated source
  artifacts.
- The behavior remains bounded and explicit; the runtime saved repair packets
  and did not continue automatically.

## Eval 007 Common Evidence Layer

Date: 2026-06-19

Baseline:

- eval 006 run root:
  `/private/tmp/commandagent-eval-focused-nextjs-runs-006/20260619T160353`
- the previous `react-dom` package-obligation plan-lint failure was not
  reproduced
- later failures remained:
  - `profile_verification:nextjs_route_not_integrated`
  - build repair packet containing `Cannot find module 'tailwindcss'`

Implementation target:

- stabilize structured contract evidence as a common step-runner evidence
  boundary
- keep plan-lint/profile-obligation evidence as the only current producer
- document future verifier/profile/tool/setup adapters without implementing
  them in this slice
- preserve bounded correction and repair budgets

Implemented changes:

- `PlanCorrectionEvidence` is retained as a compatibility alias for the common
  `ContractEvidence` type.
- `ContractEvidence` has assignment-only builder helpers for the fields already
  used by the current plan-lint producer.
- Next.js obligation lint now builds evidence through those helpers while
  preserving the existing rendered prompt content.
- A regression test confirms generic lint errors do not automatically carry
  contract evidence.
- Docs now describe the evidence pipeline as:

```text
producer -> common evidence payload -> consumer renderer -> bounded orchestration
```

Eval decision:

- No focused Gemini eval rerun is required for this slice if prompt rendering
  remains materially unchanged.
- The behavioral evidence remains eval 006. Eval 007 is a boundary/refactor and
  documentation slice, not a new attempt to fix route integration or Tailwind
  build resolution.
