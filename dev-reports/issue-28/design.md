# Issue 28 design: reproducible development tasks

## Context

CommandAgent's required development commands are currently split between the
CI workflow, contributor documentation, benchmark scripts, and opt-in test
instructions. The completed predecessor chain is integrated in this worktree,
so the task definitions must cover the current Codex harness, ShellCheck,
Rust, corpus, guardrail, conformance, and Python evaluation steps without
changing their commands or weakening their failure behavior.

## Scope

- Add a root `justfile` with documented recipes for debug and release builds,
  CI-equivalent test groups, the opt-in PTY suite, the full local CI sequence,
  benchmarks, and a representative development run.
- Add a Dev Container configuration based on an official image and reusable
  features. It will provide stable Rust 1.94.1 (newer than the manifest's Rust
  1.88 minimum), Node.js LTS, Python 3.12, `just`, and
  `shellcheck`, plus the Python packages pinned by CI. Retain the generated
  feature lock file so feature implementations resolve by digest.
- Document the optional `just` installation in `CONTRIBUTING.md` and explain
  how a containerized CommandAgent connects to a host Ollama service without
  bundling Ollama in the image.
- Record the developer-facing additions in the changelog.

## Design

Each CI recipe will repeat the command and flags from `.github/workflows/ci.yml`
instead of hiding test behavior in a new script. Rust CI recipes set
`RUSTFLAGS="-D warnings"` explicitly because CI currently supplies that value
at job scope. The `ci` recipe will depend on the harness, ShellCheck, all-target
Rust, corpus, generality-guardrail, conformance, and evaluation recipes in CI
order. The opt-in PTY suite will remain outside `ci`, matching the workflow.

`build-release` will call the existing clean release-build script introduced
by the predecessor work rather than creating a second release path. `bench`
will forward variadic arguments directly to `scripts/bench.sh`. `run` will
launch the local Ollama REPL with the repository defaults while allowing the
provider, model, and Ollama endpoint to be overridden through
`COMMANDAGENT_PROVIDER`, `COMMANDAGENT_MODEL`, and `COMMANDAGENT_OLLAMA_HOST`;
additional CLI arguments will also pass through.

The Dev Container will use the official Python 3.12 Bookworm image to avoid
building Python from source. Official Rust and Node features will add the
minimum Rust profile and Node LTS. Small community features from the Dev
Container registry will install `just` and `shellcheck`, avoiding a custom
Dockerfile. A post-create command will install exactly the Python packages and
versions used by CI. The resulting `just ci` path contains only local tests and
does not invoke model providers or require Ollama at test time.

## Verification

First validate the `justfile` by listing recipes and dry-running commands, then
run focused task recipes and parse the Dev Container JSON. Run the complete
`just ci` sequence locally. Build the Dev Container and run its tool-version
checks and `just ci` inside it when the local Docker/Dev Container tooling is
available. Finish with formatting, Clippy, the full Rust suite, and diff
checks because the task centralizes CI-sensitive shared development commands.

## Risks and mitigations

- CI drift: commands remain visible and literal in the `justfile`, and
  verification compares dry-run output with the current workflow.
- Accidental live calls: PTY and model execution are separate recipes and are
  not dependencies of `ci`.
- Container growth: use a prebuilt official language image, Rust's minimal
  profile, and features instead of a custom Dockerfile.
- Host-provider ambiguity: `.devcontainer/README.md` will state that Ollama is
  absent and show the `host.docker.internal:11434` override explicitly.
