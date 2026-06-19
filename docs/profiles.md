# Profiles

Profiles are small contracts, not hidden applications. They provide a concise
domain hint, optional verifier commands, and optional protected paths. Planning,
execution, verification, and repair remain in the shared step runner.

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
- avoid tsconfig settings that exclude the selected route root
- when an explicit component or source artifact is part of the contract, make
  it reachable from the selected route by import or direct reference

These checks can fail a phase with visible diagnostics. They do not edit files,
score UI quality, or run a hidden Next.js workflow.

Phase step planning also receives a small set of profile obligations derived
from the original goal, required artifacts, and current workspace facts. These
obligations are data-only facts rendered into the phase contract. For Next.js,
they can require package.json work to explicitly preserve `scripts.build` as
`next build`, include `next`, `react`, and `react-dom`, and preserve a requested
dev port such as `3011` in `scripts.dev`. When Tailwind directives or config
are requested, the same obligation path can require `tailwindcss`, `postcss`,
and `autoprefixer` to be mentioned in package.json work. When the selected
route is known and an explicit UI/game source artifact is part of the phase
contract, Next.js can also project a route-integration obligation requiring the
generated step plan to mention the selected route in the source-editing step
instruction or `expected_paths`. Step-plan lint uses these facts only to reject
generated package.json or Next.js source steps that omit the relevant
obligation. If that happens, the existing bounded plan correction path is used;
the profile still does not run a workflow engine or repair files by itself.
This route-integration obligation is intentionally Next.js-specific for now;
common artifact graph behavior should wait for another observed cross-profile
failure class.

During execution, the shared step runner renders an active profile contract
into each step prompt and repair prompt. It combines phase contract facts with
current profile facts collected from disk immediately before the step or repair
turn. This helps preserve contracts such as the selected app root and requested
dev port across later edits, while keeping recovery bounded and visible.

Profile verification failures may later be adapted into the common
contract-evidence payload, but that would still be evidence rendering only.
Profiles must not carry target authority, retry state, semantic confidence, or
workflow decisions.

## Python Contract

The `python` profile is for scripts, libraries, and tests. It prefers local
verification such as `python -m pytest` when a test suite exists or is created.
It should not mutate unrelated virtual environments.

## Rust Contract

The `rust` profile is for Rust CLI/library changes. It prefers `cargo test` as
the deterministic verifier and keeps generated files within the workspace.
For integration tests, references such as `CARGO_BIN_EXE_<name>` must match the
actual Cargo binary name declared in `Cargo.toml`. Tests should reference the
package, binary, module, and public item names that the project actually
defines.

## Investigation And Docs

`investigation` is read-first. It is suitable for diagnosis reports and should
avoid edits unless the user explicitly asks for fixes.

`docs` is for documentation changes. It should preserve source behavior and use
lightweight checks where available.

## Data Contracts

Data profiles protect raw input prefixes:

- `raw/`
- `data/raw/`
- `input/`
- `inputs/`

Derived outputs should be written elsewhere so reruns are reproducible and raw
inputs remain inspectable.

## Profile vs Style

Profiles describe the domain. Styles describe the development method. For
example:

```text
/ultra-plan-run --profile rust --style tdd Add parser coverage
```

This means "use the Rust contract, and prefer test-first steps." It does not
create a separate Rust-specific TDD engine.
