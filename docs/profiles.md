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
is missing, CommandAgent should install dependencies when allowed or stop with
`dependency_missing`.

Use:

```text
/ultra-plan-run --profile nextjs Create a Next.js app on port 3011
```

The profile supplies Next.js-specific facts. It does not force a particular
component tree or router layout.

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
