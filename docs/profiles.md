# Profiles

Profiles are structured domain contracts, not hidden applications. They provide
a concise domain hint, profile-specific planning guidance, optional verifier
commands, optional protected paths, deterministic facts, artifact
classification, obligations, profile-specific plan lint, and profile
verification evidence. Planning, execution, verification orchestration, and
repair remain in the shared step runner.

MVP profiles:

| Profile | Purpose | Default verifier |
| --- | --- | --- |
| `generic` | General coding and file tasks | none |
| `nextjs` | Next.js app creation or modification | `npm run build` |
| `python` | Python scripts, libraries, and tests | `python -m pytest` |
| `rust` | Rust CLI/library work | `cargo test` |
| `investigation` | Read-first diagnosis and reports | none |
| `docs` | Documentation updates | none |
| `data-analysis` | Local analysis with immutable raw inputs | none |
| `data-pipeline` | Data extraction/transform/output tasks | none |

## Artifact Classification

Profile path reasoning goes through a shared classified-artifact boundary:

```text
profile + path + provenance
  -> ClassifiedArtifact { path, provenance, kind, eligibility }
  -> profile obligation / verification / recovery evidence
```

Rendered profile text is for prompts, repair packets, and reports. Runtime
decisions must not parse rendered profile text or `workspace.entries` tokens
back into contract artifacts. A workspace observation alone does not create a
route-integration or source-integration obligation.

The common classifier entrypoint is profile-independent. Profile-specific
classifiers may assign kinds such as route entry, UI source, runtime source,
test source, manifest, config, generated declaration, dependency cache, build
output, raw input, derived output, or documentation.

Future profile obligation, verification, and recovery-evidence producers must
consume `ClassifiedArtifact` values. A producer needs observed deterministic
failure evidence, bounded scope, tests for the positive and false-positive
cases, and docs. It must not add workflow authority, retry authority, or
provider/model-specific policy.

When a producer needs relationships rather than a single path, it should
project classified artifacts into the shared ArtifactGraph boundary. The graph
can state lifecycle, role, selected route, setup manifest, source ownership, or
integration edges. It should not infer semantic quality, run tools, or become a
profile-specific workflow engine.

Focused eval cases may assert profile-derived terminal and recovery fields,
such as a Next.js route-integration active job or a Rust Cargo verifier
binding. Those assertions are eval-only checks against observed profile and
step-runner evidence. They do not grant profiles additional workflow authority,
retry authority, or package/setup execution authority.

## Profile Interface

The shared profile boundary keeps generic plan lint from becoming a collection
of framework branches. Core code may resolve a profile id and call common
profile APIs, but it should not embed framework-specific rules directly.

Current profile-owned surfaces are:

- `profile_contract_text`: concise prompt text for the active domain
- `profile_plan_guidance`: plan-generation guidance for profile-specific
  package, verifier, scaffold, or compatibility constraints
- Common profile output: read-only profile id, project root hints, classified
  artifacts, setup artifacts, scaffold artifacts, route/integration artifacts,
  verifier commands, protected paths, behavior obligations, verification
  failures, and recovery candidate hints
- `classify_profile_artifact`: typed path facts used by generic lint,
  verification, setup bootstrap, and recovery targeting
- ArtifactGraph projection: bounded artifact lifecycle and relation facts
  derived from classified artifacts, contract paths, and deterministic
  evidence
- `profile_obligations`: deterministic plan obligations derived from the goal,
  required artifacts, phase facts, and profile facts
- `lint_profile_plan`: profile-specific plan lint that returns common
  contract evidence
- `verify_profile`: read-only profile verification at step or phase boundaries
- `profile_verifier_commands`: profile verifier hints
- `protected_by_profile`: protected path checks for profiles with immutable
  inputs

Profile-specific lint may reject domain drift such as a Next.js plan using
`npx` as a verifier, omitting required package literals, splitting app roots,
or planning disconnected route artifacts. It still returns `PlanLintError` and
`PlanCorrectionEvidence` for the shared bounded correction path. It must not
execute tools, run package managers, choose a workflow, retry, or repair files.

Profile obligations are also projected into the shared Task Contract as
behavior obligations. This gives plan prompt, plan lint, active step facts, and
eval reports the same vocabulary for dependency setup, manifest contracts,
dev-server port contracts, route integration, test artifacts, docs literals,
and data schema obligations. The projection is data-only. It does not let a
profile execute setup, own recovery policy, or force every ultra phase to
materialize every final artifact.

Common profile output is the same kind of data-only projection. It lets Next.js,
Rust, Python, docs, and data profiles report comparable setup/scaffold/source
facts without making each profile a workflow engine. Recovery candidate hints
are only hints; the shared active-job dispatch gate still owns final owner and
action selection.

The common output schema also exposes profile parity fields for eval and
repair attribution:

- `profile_project_kind`
- `profile_manifest_artifacts`
- `profile_entrypoints`
- `profile_integration_artifacts`
- `profile_completion_evidence`
- `profile_failure_mapping`
- `profile_adapter_families`
- `profile_capability_status`

Every profile should render each capability family as `supported`, `partial`,
or `not_applicable` through `profile.output.capability.<family>`. Missing
profile support should therefore appear as a parity gap, not as an implicit
Next.js-only behavior branch. The profile may name manifest, entrypoint,
integration, completion-evidence, failure-mapping, and adapter-family facts,
but those facts remain common contract inputs. They do not select an active
job, execute setup, or authorize hidden repair continuation.

## Next.js Contract

New Next.js apps need honest dependencies and build scripts. A build script that
uses `next build` must not be changed to fake success. If `node_modules/.bin/next`
is missing, CommandAgent reports `dependency_missing`. With approved online
setup recovery (`--yes` and no `--offline`), the step runner may run one
deterministic npm/pnpm setup command and rerun `npm run build` once. Otherwise
it stops with the setup blocker.

Use:

```text
/ultra-plan-run --profile nextjs Create a Next.js app on port 3011
```

The profile supplies Next.js-specific facts. It does not force a particular
component tree or router layout.

The Next.js classifier distinguishes route entries such as `app/page.tsx`,
route infrastructure such as `app/layout.tsx`, UI source artifacts such as
`components/Game.tsx`, source/style artifacts such as `app/globals.css` and
`src/app/globals.css`, manifests/config files, generated declarations such as
`next-env.d.ts`, dependency caches, and build output. Global CSS files remain
source/style artifacts even when they contain Tailwind directives; they are not
setup/config artifacts and are not route-integration artifacts. Generated
declarations, dependency caches, build output, setup files, and workspace-only
observations are not route-integration artifacts.

During ultra phase execution, the Next.js profile may also provide read-only
fact summaries and phase-boundary verification. The checks are deterministic
and limited to observed project facts:

- do not split the route root between `app/` and `src/app/` unless the plan is
  explicitly migrating the root
- keep `scripts.build` as `next build`
- preserve a requested dev port such as `3011` in `scripts.dev`
- keep `next`, `react`, and `react-dom` dependencies when a Next.js route is
  present
- catch obvious Next.js/React peer dependency conflicts, such as Next.js 14
  with exact React pins below 18.2
- if CSS uses `@tailwind`, keep matching Tailwind/PostCSS config and
  dependencies
- catch the observed Tailwind/PostCSS peer dependency conflict where
  `autoprefixer` 10 is paired with an exact `postcss` pin below 8.0.2
- when generated setup repair fixes dependency compatibility, prefer a stable
  Next.js-compatible dependency family instead of switching the generated app
  to `latest` packages as the repair strategy
- if a generated Next.js app uses `tsconfig.json`, `.ts`, `.tsx`, or
  TypeScript code, keep the package step on a stable TypeScript 5.x range such
  as `^5.4.0` and `@types/react` 18.x toolchain for Next.js 14/React 18;
  TypeScript 6, exact TypeScript pins such as `5.0.0`, and `@types/react` 19
  are treated as generated-app dependency-family drift
- avoid tsconfig settings that exclude the selected route root
- if source imports use `@/*`, define `compilerOptions.paths` for `@/*` in
  `tsconfig.json`; otherwise prefer relative imports
- when an explicit component or source artifact is part of the contract, make
  it reachable from the selected route by import or direct reference

Route integration verification is two-stage. If the explicit component/source
artifact is missing, the profile reports
`nextjs_integration_artifact_missing` and does not also report
`nextjs_route_not_integrated` for that artifact. `nextjs_route_not_integrated`
is reserved for an existing explicit artifact that is not imported or
referenced by the selected route tree.

The selected route tree is a bounded static graph. The profile starts at the
selected route, follows static relative imports through a small number of
path-confined source files, and treats an explicit artifact as integrated when
it is imported or referenced anywhere in that route tree. This covers normal
shapes such as `app/page.tsx` importing a component that uses a hook or type.
The graph deliberately ignores dependency caches, generated declarations,
build output, manifests, and config files. It does not execute code, run the
TypeScript compiler, infer runtime behavior, or score visual/gameplay quality.

These checks can fail a phase with visible diagnostics. They do not edit files,
score UI quality, or run a hidden Next.js workflow.

Phase step planning also receives a small set of profile obligations and Task
Contract facts derived from the original goal, required artifacts, and current
workspace facts. These obligations are data-only facts rendered into the phase
contract. For Next.js, they can require package.json work to explicitly preserve `scripts.build` as
`next build`, include `next`, `react`, and `react-dom`, and preserve a requested
dev port such as `3011` in `scripts.dev`. When Tailwind directives or config
are requested, the same obligation path can require `tailwindcss`, `postcss`,
and `autoprefixer` compatible versions to be mentioned in package.json work.
When the selected route is known and an explicit UI/game source artifact is
part of the phase contract, Next.js can also project a route-integration
obligation requiring the generated step plan to mention the selected route in
the source-editing step instruction or `expected_paths`. A later step in the
same plan may also satisfy the obligation when it edits the selected route and
names the source artifact by path or file stem, such as creating
`app/components/GameCanvas.tsx` and then editing `app/page.tsx` to render
`GameCanvas`. That obligation is based on classified artifacts, not broad
`*.ts`/`*.js` token scans. Step-plan lint uses these facts only to reject
generated package.json or Next.js source steps that omit the relevant
obligation. If that happens, the existing bounded plan correction path is used;
the profile still does not run a workflow engine or repair files by itself.
If current package.json facts already satisfy a manifest obligation, later
package.json edit steps do not need to restate every required literal just to
pass plan lint. The profile facts and profile verification keep the contract
observable; if the edit drifts the manifest away from those facts, verification
or the next obligation-aware lint reports the deterministic failure.
This route-integration obligation is intentionally Next.js-specific for now;
common artifact graph behavior should wait for another observed cross-profile
failure class.

Plan generation receives Next.js planning guidance from the profile boundary,
not from generic plan-prompt code. The guidance can require a compatible
`next`/`react`/`react-dom` family, a stable TypeScript 5.x range such as
`^5.4.0` and `@types/react` 18.x when TypeScript is planned, matching
`tsconfig.json` path aliases when `@/*` imports are used, Tailwind/PostCSS
package and config literals when Tailwind is planned, and `npm run build` as
the source verifier. These are Profile Contract facts; they do not authorize
dependency installation or hidden workflow execution.

When a generated Next.js plan has one unambiguous `package.json` setup step and
Tailwind is requested from a source/style step, CommandAgent may materialize a
deterministic manifest obligation into that plan step before rerunning plan
lint. The materialized obligation is limited to setup contract facts such as
`next`, `react`, `react-dom`, `typescript 5.x`, `@types/react 18.x`,
`tailwindcss`, `postcss`, `autoprefixer`, `scripts.build=next build`,
`tailwind.config.js`, and `postcss.config.js`. This is plan-level setup
materialization, not package-registry solving, source generation, or hidden
Next.js workflow execution. If the target package step is ambiguous,
CommandAgent should stop or repair with explicit evidence instead of patching
the wrong step.

During execution, the shared step runner renders an active profile contract
into each step prompt and repair prompt. It combines phase contract facts with
current profile facts collected from disk immediately before the step or repair
turn. This helps preserve contracts such as the selected app root and requested
dev port across later edits, while keeping recovery bounded and visible.

Profile verification failures are rendered into the common contract-evidence
payload when the profile check has deterministic facts. For Next.js, mixed
`app/` and `src/app/` roots are reported as app-root contract evidence, missing
integration artifacts report the missing artifact as the repair target, and a
route-integration failure reports the selected route, the unintegrated
artifact, and a route-tree repair target when that target is deterministic.
Script drift, dependency drift, Tailwind/PostCSS drift, TypeScript alias/root
drift, and dev-port drift map to their deterministic repair targets such as
`package.json`, `tailwind.config.js`, `postcss.config.js`, or `tsconfig.json`.
Profile evidence may also feed the Recovery Orchestration Contract. For example,
`nextjs_route_not_integrated` becomes `active_job=route_integration_repair`
with `repair_action=connect_artifact_to_selected_route`; an existing artifact
is connected to the selected route graph, while
`nextjs_integration_artifact_missing` becomes
`repair_action=create_missing_integration_artifact`. Manifest and config
drift map to actions such as `add_manifest_dependency`,
`repair_tailwind_contract`, or `repair_tsconfig_alias`. The resulting evidence
can carry `repair_kind`, `repair_action`, `setup_implication`,
`rerun_authority`, `tool_policy_projection`, `target_admission`,
`target_priority`, `artifact_graph_summary`, `recovery_owner`,
`repair_action_plan`, `semantic_failure_report`, `proposed_targets`,
`admitted_targets`, `rejected_targets`, `selected_failure_cluster`,
`repair_brief`, `repair_brief_status`, `action_envelope_status`, and
`eval_report_fields` into the Recovery Task Contract. Evidence-binding failures may become
`active_job=evidence_binding_repair` when a profile can deterministically name
an existing target artifact and the missing proof path. This is still evidence
and policy rendering only. Profiles must not carry retry authority, semantic
confidence, or workflow decisions. Profiles may propose domain facts and route
targets; common target admission owns the final selected target.

Next.js source verification should use `npm run build`, not `npx` compiler
commands. `npx` may perform dependency setup and is blocked by the Bash policy;
using `npm run build` lets verifier-owned setup recovery detect
`dependency_missing`, run one approved setup command when allowed, and rerun
the same verifier.

Dependency compatibility checks are intentionally small. They operate on
observed `package.json` facts and known deterministic failures, such as
`autoprefixer@10.0.0` requiring `postcss` `^8.0.2` while the manifest pins
`postcss@8.0.0`, or a generated Next.js 14/React 18 TypeScript app using
TypeScript 6 or `@types/react` 19. They do not fetch package registry data,
select latest versions, run `npm install`, or solve arbitrary dependency
trees.

## Python Contract

The `python` profile is for scripts, libraries, and tests. It prefers local
verification such as `python -m pytest` when a test suite exists or is created.
It should not mutate unrelated virtual environments.

The Python classifier treats `*.py` runtime files, `tests/**/*.py`,
`pyproject.toml`, requirements files, virtual environments, `__pycache__/`, and
`*.pyc` differently so future producers can avoid treating dependency caches or
build output as source repair targets.

## Rust Contract

The `rust` profile is for Rust CLI/library changes. It prefers `cargo test` as
the deterministic verifier and keeps generated files within the workspace.
For integration tests, references such as `CARGO_BIN_EXE_<name>` must match the
actual Cargo binary name declared in `Cargo.toml`. Tests should reference the
package, binary, module, and public item names that the project actually
defines.

The Rust classifier distinguishes `Cargo.toml`, `src/**/*.rs`,
`tests/**/*.rs`, `benches/**/*.rs`, `examples/**/*.rs`, config files, and
`target/**` build output. No Rust obligation is added by classification alone.

## Investigation And Docs

`investigation` is read-first. It is suitable for diagnosis reports and should
avoid edits unless the user explicitly asks for fixes.

`docs` is for documentation changes. It should preserve source behavior and use
lightweight checks where available.

The docs classifier treats `README.md` and `docs/**/*.md` as documentation
artifacts while treating generated output such as `site/**` or `dist/**` as
build output.

## Data Contracts

Data profiles protect raw input prefixes:

- `raw/`
- `data/raw/`
- `input/`
- `inputs/`

Derived outputs should be written elsewhere so reruns are reproducible and raw
inputs remain inspectable.

The data classifier marks raw input prefixes as protected inputs and derived
locations such as `data/processed/**` and `reports/**` as derived output. It
does not create a data workflow.

## Profile vs Style

Profiles describe the domain. Styles describe the development method. For
example:

```text
/ultra-plan-run --profile rust --style tdd Add parser coverage
```

This means "use the Rust contract, and prefer test-first steps." It does not
create a separate Rust-specific TDD engine.
