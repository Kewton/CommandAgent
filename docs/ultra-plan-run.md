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
  package scripts, required final artifacts, and profile-projected fact lines

The phase runner stops on the first failed phase. Continuing after a failed
phase would make later phases depend on stale assumptions.

After a phase step plan finishes, profile verification may run for profiles
that define deterministic checks. For example, the Next.js profile can reject
app-root ambiguity, build/dev script drift, missing framework dependencies,
Tailwind config/dependency drift, and route integration drift for explicit
artifact paths. Profile verification is read-only and does not auto-repair.

## Verification And Repair

Verification is deterministic. It runs profile or plan verifier commands through
the Bash offline policy. If verification fails, CommandAgent creates a bounded
repair prompt containing:

- missing expected paths
- verifier commands
- diagnostic lines
- relevant source excerpts when available

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

Repair is capped. If repair is exhausted, CommandAgent writes a short packet to
`.commandagent/repairs/` and prints a suggested `/ultra-plan-run` command.

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
