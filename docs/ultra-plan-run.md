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
`app/page.tsx` is known and a source artifact such as `app/hooks/useGame.ts`
is an explicit phase output, the generated source step must include the
selected route in the instruction or `expected_paths`. This prevents a phase
from creating isolated UI/game code while leaving the selected route unchanged.
The rule is limited to the Next.js profile until another observed failure
justifies a common contract.

When a deterministic lint/profile obligation rejects a generated phase step
plan, the bounded correction prompt may include a compact contract evidence
block. That block can name the failed step, violated contract, exact missing
literals, or required paths, such as `react-dom` for a Next.js package
obligation. It is not a retry expansion: the original guard reruns unchanged
and the run still stops if the bounded correction fails.

This common evidence layer is implemented for plan-lint/profile obligations,
tool protocol failures, read-only step-policy violations, and verifier
failures. Profile verification also renders shared contract evidence inside
its existing profile repair packet when the profile check has deterministic
facts, such as a selected Next.js route or mixed app roots. Dependency setup is
represented only as diagnostic context on a remaining verifier failure after
one approved setup attempt. These evidence producers render through existing
correction or repair prompts; they must not add hidden continuation or new
retry budgets.

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
file change. It does not create another execution engine: the minimal loop
still receives one bounded repair turn and the original verifier or guard
reruns unchanged.

If every verifier failure is `dependency_missing` and the step's expected paths
already exist, CommandAgent treats the problem as setup recovery, not source
repair. With `--yes` and without `--offline`, it runs one deterministic setup
command selected from lockfiles (`npm ci`, `pnpm install`, or `npm install`),
stores setup logs under `.commandagent/setup/`, and reruns the original
verifier once. If setup is not approved, offline, unsupported, ambiguous,
fails, times out, or still leaves `dependency_missing`, CommandAgent stops with
a setup blocker instead of creating a repair prompt.

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
also names the selected route and unintegrated artifact so the standalone
repair plan receives deterministic target evidence instead of only prose.

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
