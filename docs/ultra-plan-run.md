# Ultra Plan Run

`/ultra-plan-run` is the large-task path. It splits a user goal into phases,
turns each phase into a step plan, and executes those steps through the minimal
loop.

It is not a second engine. It is an outer planner around the same minimal
execution loop.

## When To Use It

Use `/ultra-plan-run` when the request is too large for one prompt:

- new app creation
- multi-file feature work
- migration or refactor slices
- investigation followed by fixes
- documentation bundles
- data analysis or data pipeline tasks

For a small single-file task, a direct prompt or `/plan-run` is usually enough.

## Basic Form

```text
commandagent> /ultra-plan-run --profile nextjs Create a Next.js app on port 3011
```

Options must appear before the goal:

```text
commandagent> /ultra-plan-run --profile rust --style tdd Add a parser and tests
```

## `/plan-run` vs `/ultra-plan-run`

`/plan-run` creates one step plan and runs it. It is suited to a bounded task
whose steps can be listed at once.

`/ultra-plan-run` creates phase goals first. Each phase gets its own step plan
with a fresh workspace snapshot. It is better for tasks where early results
should shape later steps.

## Phase Contract

Each phase should stay small enough to finish and verify. A phase prompt
includes:

- the original goal
- the current phase goal
- the selected profile contract
- the selected style
- a freshly collected bounded workspace snapshot
- a data-only phase workspace contract with visible entries, lockfiles,
  package scripts, required final artifacts, profile-projected fact lines, and
  profile obligations

The phase runner stops on the first failed phase. Continuing after a failed
phase would make later phases depend on stale assumptions.

Profile obligations are read-only facts derived before phase step planning.
For example, a Next.js goal that mentions port `3011` can project an obligation
that package.json work must explicitly preserve a `scripts.dev` command with
that port. If the generated phase step plan edits package.json but omits that
obligation, normal step-plan lint rejects the plan and the existing bounded
plan correction path asks the planner to fix the plan. This is still common
planning/lint behavior; profiles do not become workflow engines. Next.js also
has a narrow route-integration obligation: if a selected route such as
`app/page.tsx` is known and a classified UI source artifact such as
`app/hooks/useGame.ts` is an explicit phase output, the generated source step
must include the selected route in the instruction or `expected_paths`, or the
same step plan must contain a later route-editing step that touches the
selected route and names the artifact by path or file stem. This prevents a
phase from creating isolated UI/game code while still allowing a clean
create-component step followed by an explicit page-integration step. Workspace
entries, generated declarations such as
`next-env.d.ts`, dependency caches, and build output are context only; they do
not become route-integration artifacts by token matching. The rule is limited
to the Next.js profile until another observed failure justifies a common
contract.

When a deterministic lint/profile obligation rejects a generated phase step
plan, the bounded correction prompt may include a compact contract evidence
block. That block can name the failed step, violated contract, exact missing
literals, or required paths, such as `react-dom` for a Next.js package
obligation. It is not a retry expansion: the original guard reruns unchanged
and the run still stops if the bounded correction fails.

Failure packets also include a compact normalized failure observation when
structured evidence exists. The observation names the terminal state, owning
contract layer, violated contract, producer, guard, diagnostic code, source of
truth, and actionability. It helps the next explicit repair or eval report
identify the failure class, but it does not choose a repair job or increase the
bounded correction budget.

Some high-confidence plan-lint failures can also select an active job before
correction. For example, a Next.js Tailwind source/style step whose package
step omits `tailwindcss`, `postcss`, or `autoprefixer` is a
`manifest_repair` job, not source implementation repair. When exactly one
package step is the target, CommandAgent may materialize the deterministic
manifest obligation into that step and rerun the same plan lint. If the same
missing literals remain after the bounded correction budget, the failure should
include an attempt ledger rather than starting another hidden correction loop.

Generated phase step plans are plan-file contract inputs. Before lint or
execution, CommandAgent parses supported ordinary YAML scalar forms and
normalizes them into the typed step-plan schema. Long phase goals or step
instructions may use YAML block scalar markers `|`, `|-`, `|+`, `>`, `>-`,
and `>+`; CommandAgent normalizes them into typed strings before linting.
Anchors, merge keys, custom tags, and extra nested maps remain outside the
contract. Parse errors, schema errors, and plan-lint errors should be reported
as distinct planning failures.

Phase planning also carries Task Contract facts. Required artifacts, profile
obligations, and deliverable roles are projected into bounded lines such as
`task.contract.kind`, `task.contract.behavior_obligation.<code>`, and
`task.contract.artifact_roles`. These facts may guide the planner and may let
plan lint reject a dropped required artifact or a missing manifest/setup owner.
For ultra phases, the global final artifacts remain visible as context, but
phase-local step plans are not forced to list every final artifact in their own
`required_artifacts`; final artifacts are still checked at the final boundary.

Step decomposition is also a planning contract. For example, a generated
`setup` step may own `package.json` or `tailwind.config.js`, but it may not own
source artifacts such as `app/globals.css`, `src/app/globals.css`, or
`src/app/page.tsx`. When profile artifact classification can identify that
mismatch, plan lint rejects the step before execution and sends the rejected
path, observed artifact role, allowed setup roles, and required split or
kind-change action through the same bounded correction path. The runtime
setup-source tool policy remains the final guard, not the primary detector.

This common evidence layer is implemented for plan-lint/profile obligations,
provider transport parse failures, tool protocol failures, read-only
step-policy violations, verifier failures, and profile verification failures.
Provider transport evidence is limited to shared response-parser diagnostics;
it does not add provider/model-specific policy. Profile verification also
renders shared contract evidence inside its existing profile repair packet when
the profile check has deterministic facts, such as a selected Next.js route,
missing integration artifact, script/dependency drift, Tailwind/PostCSS drift,
TypeScript alias/root drift, or mixed app roots. Dependency setup is
represented only as diagnostic context on a remaining verifier failure after
one approved setup attempt. These evidence producers render through existing
correction or repair prompts; they must not add hidden continuation or new
retry budgets.

Profile obligation and verification producers consume classified artifacts
rather than rendered profile text. Future producers for Python, Rust, docs, or
data profiles should use the same classified-artifact boundary and must not
scan `workspace.entries` as contract artifacts.

Profile-specific planning guidance and profile-specific plan lint use the same
boundary. Generic plan generation renders guidance returned by the active
profile; generic plan lint validates schema, paths, step kind, verifier safety,
and workspace scope, then consumes profile-specific lint results as common
contract evidence. Framework rules such as Next.js dependency literals,
TypeScript/Tailwind plan contracts, route integration obligations, and `npx`
verifier rejection live behind the profile interface rather than in generic
Plan Lint.

The same phase contract is carried as an active contract during step
execution. Before each executable step, CommandAgent refreshes current profile
facts from disk and renders them with the original phase facts into the step
prompt. If the step later needs verifier repair, the repair prompt receives the
same refreshed active facts. This keeps contracts visible across phase steps
without adding a hidden retry loop or profile-owned workflow.

After a phase step plan finishes, profile verification may run for profiles
that define deterministic checks. If a phase step fails, the same read-only
profile verification may also run at the failed phase boundary so the error can
include profile drift that happened before the step failure. For example, the
Next.js profile can reject app-root ambiguity, build/dev script drift, missing
framework dependencies, Tailwind config/dependency drift, and route integration
drift for explicit artifact paths. Profile verification is read-only and does
not auto-repair.

## Verification And Repair

Verification is deterministic. It runs profile or plan verifier commands through
the Bash offline policy. If verification fails, CommandAgent creates a bounded
repair prompt containing:

- a recovery task section when deterministic evidence can state the repair task
- missing expected paths
- verifier commands
- diagnostic lines
- relevant source excerpts when available
- active profile contract facts for the current step

The recovery task section is the repair instruction. It may name the blocker,
required action, repair target or candidate artifacts, disallowed actions, and
the original verifier/profile/tool-policy check that will judge the repair. It
may also name an execution envelope, tool policy, and evidence requirement for
the next bounded repair turn. For example, read-only step-policy recovery runs
with a read-only tool policy and requires repository read evidence instead of a
file change. Setup/source policy recovery keeps a setup/config-only mutation
policy, so a setup step cannot repair itself by editing source routes or
components. It does not create another execution engine: the minimal loop still
receives one bounded repair turn and the original verifier or guard reruns
unchanged.

When deterministic failure evidence is specific enough, a Recovery Policy
Contract is applied before the Recovery Task Contract is rendered. The policy
may classify the active job, admit and prioritize repair targets, and select a
single repair action such as `connect_artifact_to_selected_route`,
`create_missing_integration_artifact`, `add_manifest_dependency`, or
`repair_tailwind_contract`. This is not dispatch to another engine. It only
makes the next bounded minimal-loop repair task explicit and keeps the original
guard, verifier, or profile check as the success authority.

The broader Recovery Orchestration Contract renders that decision as
structured evidence. Repair prompts and packets may include
`target_admission`, `target_priority`, `tool_policy_projection`,
`explicit_stop_reason`, `recovery_owner`, `loop_control_action`,
`dispatch_status`, `dispatch_reason`, `candidate_jobs`, `tie_break_reason`,
`repair_action_plan`, `completion_evidence`, `evidence_binding`,
`deliverable_obligations`, `semantic_failure_report`, `repair_job_state`,
`attempt_outcomes`, `exhausted_targets`, `exhausted_roles`,
`exhausted_clusters`, `no_progress_strategy`, `repair_state_status`,
`verifier_diagnostic_payload`, `diagnostic_code`, `observed_expected`,
`affected_cases`, `preferred_repair_role`, `weak_verifier_reason`,
`admitted_cluster_targets`, `patch_validation`, `eval_report_fields`,
`proposed_targets`, `admitted_targets`, `rejected_targets`, `repair_brief`,
`selected_failure_cluster`, `repair_brief_status`, `action_envelope_status`,
and an `artifact_graph_summary` so the standalone repair plan can see why a
target is allowed, why another action is forbidden, which owner/action was
selected, which failure cluster is being repaired, whether a prior bounded
attempt made progress, and which original check must be rerun. These fields
are diagnostics and policy projection; they do not change the retry budget or
continue phases silently.

Verifier diagnostic fields are derived from already-observed verifier output.
They let the repair packet distinguish a Rust compile error, Python import
failure, Next.js type error, route integration failure, dependency/setup
boundary, port conflict, or weak verifier command without asking the model to
infer the class from prose. Unknown verifier failures remain failures, but
they should still carry the original command, exit status, failure signature,
and bounded diagnostic excerpt instead of appearing only as `rc:1`.

If every verifier failure is `dependency_missing` and the step's expected paths
already exist, CommandAgent treats the problem as setup recovery, not source
repair. With `--yes` and without `--offline`, it runs one deterministic setup
command selected from manifest and lockfile evidence (`npm ci`,
`pnpm install`, or `npm install`), stores setup logs under
`.commandagent/setup/`, and reruns the original verifier once. If setup is not
approved, offline, unsupported, ambiguous, fails, times out, or still leaves
`dependency_missing`, CommandAgent stops with a setup blocker and structured
setup evidence. Setup attempts are keyed by verifier step, setup command, and
manifest fingerprint. If a later bounded repair edits a setup manifest or setup
config, setup state is marked stale and a later setup attempt must still pass
the same setup policy and fingerprint guard.

When a Next.js task explicitly requests a dev-server port and a generated plan
has run `npm run build`, CommandAgent treats launchability as a separate
dev-server contract. Passing `npm run build` is not enough to prove that
`npm run dev` can serve the app on the requested port. The bounded
`dev_server_smoke` check validates `scripts.dev`, checks port availability,
starts `npm run dev` with a timeout, requests `/` over localhost, and cleans up
the child process. Occupied ports are reported as `port_in_use` with
`contract_layer=dev_server_port_contract`. The check is verifier-owned
runtime evidence; it does not install dependencies, keep a background server,
or add a hidden repair loop.

When setup fails with a
deterministic package-manager diagnostic such as npm `ERESOLVE` peer dependency
evidence, the blocker may include structured manifest compatibility evidence
that names `package.json`, the dependent package, the required peer range, and
the observed incompatible package. This does not run another setup command or
continue the ultra plan automatically.

For Next.js phases, verifier plans should use `npm run build` rather than
`npx tsc --noEmit` or other `npx` commands. `npx` is blocked as a possible
network/dependency setup command and cannot be connected to bounded dependency
setup recovery. `npm run build` keeps the check under the project build script
and lets the verifier classify `dependency_missing` before setup.

If a later bounded repair changes package-manager manifests such as
`package.json`, `package-lock.json`, or `pnpm-lock.yaml`, the setup attempt
state is keyed by the new manifest fingerprint. Approved online setup may run
once for that changed manifest state and then rerun the same verifier. This is
still verifier-owned setup, not repair-turn authority.

If `package-lock.json` exists but no longer reflects dependencies declared by
`package.json`, setup recovery may select `npm install` instead of `npm ci` so
the lockfile and installed packages can be refreshed under the same bounded
setup policy. This decision comes from deterministic manifest/lock evidence,
not from a model-issued Bash command.

For npm setup recovery, CommandAgent includes dev dependencies because Next.js
builds commonly require local TypeScript, Tailwind, PostCSS, and type packages.
This still belongs to verifier-owned setup recovery and does not authorize
ordinary model-issued package-manager commands.

Normal model-issued `Bash(npm install)` remains blocked. Dependency setup is
runtime-owned and is triggered only by verifier evidence.

If a step stops before a verifier can run because the model emitted an invalid
tool call, CommandAgent reports a `tool_args_*` execution-contract failure.
For example, `Write` without `path` becomes
`tool_args_missing_required_field:path`. For eligible file-changing initial
steps, the runtime can issue one strict current-step protocol correction that
names the failed tool, missing field, required fields, and deterministic target
path when known from missing expected paths or a single step `expected_paths`
entry. Repair turns may also correct malformed `Write` or `Edit` calls while
fixing a failed verifier, because the repair turn is an explicit
mutation-allowed session. It then reruns the same expected-path checks and
verifier commands. Repeated malformed tool calls stop explicitly and do not
count as original ultra-plan completion.

If a read-only step such as `inspect` or `report` attempts mutation,
CommandAgent records a `step_policy` evidence entry with the failed tool and
the `read_only_step_mutation` contract. The normal repair/replan path receives
that evidence. The selected recovery envelope keeps the repair turn read-only,
so `Read`, `Glob`, `Grep`, or read-only `Bash` can provide repository evidence,
while `Write`, `Edit`, mutating `Bash`, and prose-only repair remain failures.
The runtime does not silently move the mutation into a different step.

Repair is capped. If repair is exhausted, CommandAgent writes a short packet to
`.commandagent/repairs/` and prints a suggested `/ultra-plan-run` command.
Profile verification failures use the same explicit continuation model: the
runtime writes a bounded profile repair packet with failure codes, phase
contract facts, profile facts, and expected paths, then stops. Running the
suggested command starts a standalone repair plan; it is not hidden continuation
of the original ultra plan. For Next.js route integration failures, the packet
also names the selected route, unintegrated artifact, and route-tree repair
target when known so the standalone repair plan receives deterministic target
evidence and `repair_action=connect_artifact_to_selected_route` instead of only
prose.
For Next.js missing integration artifacts, the packet names the missing
artifact itself as the repair target and requires creating it before route
integration is checked, with
`repair_action=create_missing_integration_artifact`.

## Repair Replan Example

```text
commandagent> /ultra-plan-run --profile nextjs "$(cat .commandagent/repairs/repair-verify-build-1234567890.md)"
```

This starts a new explicit task using the compact repair packet. It is
deliberately user-visible so CommandAgent does not hide unbounded retries. It
is a standalone repair plan; the original ultra plan remains incomplete until
the user explicitly resumes or replans it.

## Current MVP Limit

The parser, schemas, verifier, repair artifacts, profile contracts, ultra
execution core, and REPL dispatch are present. Live behavior still depends on
the selected model and local toolchain, so complex workflows should be verified
with the current binary before publishing them as supported.
