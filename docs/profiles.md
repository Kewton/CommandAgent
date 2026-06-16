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

## Data Contracts

Data profiles protect raw input prefixes:

- `raw/`
- `data/raw/`
- `input/`
- `inputs/`

Derived outputs should be written elsewhere so reruns are reproducible and raw
inputs remain inspectable.
