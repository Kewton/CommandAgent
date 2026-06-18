# CommandAgent

CommandAgent is a minimal local-first coding agent for local and API-backed
LLMs. It keeps one small execution loop, built-in file/search/bash tools, and a
bounded step-runner architecture for larger tasks.

Legacy engines, sidecar routing, Photon, case memory, PAM, and historical repair
systems are intentionally out of scope.

## Install

Install the latest release:

```bash
curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | bash
commandagent --help
```

Install a specific tag:

```bash
curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh \
  | COMMANDAGENT_VERSION=v0.1.0 bash
```

Source builds use the current stable Rust toolchain until MSRV is fixed. See
`docs/install.md` for manual install, provider setup, and platform details.

## Quickstart

Build from source:

```bash
cargo build --release
target/release/commandagent --help
```

Start an interactive REPL with Ollama:

```bash
ollama serve
ollama pull qwen3.6:35b-a3b-coding-nvfp4
target/release/commandagent \
  --provider ollama \
  --model qwen3.6:35b-a3b-coding-nvfp4
```

Run a one-shot prompt:

```bash
target/release/commandagent \
  --provider ollama \
  --model qwen3.6:35b-a3b-coding-nvfp4 \
  "Create README.md with a short usage note."
```

Inside the REPL:

```text
commandagent> Create docs/notes.md with a short note.
commandagent> /exit
```

Blank lines are ignored. `/exit` and `/quit` end the REPL.

## API Providers

Gemini and OpenAI use XML fallback tool calls. Put API keys in the environment
or a local `.env` loader of your choice:

```bash
export GEMINI_API_KEY=...
export OPENAI_API_KEY=...
```

Examples:

```bash
target/release/commandagent --provider gemini --model gemini-3.1-flash-lite
target/release/commandagent --provider openai --model gpt-5.4-mini
```

Planner and executor targets can differ:

```bash
target/release/commandagent \
  --provider ollama \
  --model qwen3.6:35b-a3b-coding-nvfp4 \
  --planner-provider gemini \
  --planner-model gemini-3.5-flash
```

## Step Runner

CommandAgent includes the migration core for:

- `/plan-steps`
- `/plan-run`
- `/run-plan`
- `/ultra-plan`
- `/ultra-plan-run`
- `/run-ultra-plan`

The current repository contains the parser, plan schemas, profile contracts,
verifier, bounded repair artifacts, and ultra-plan execution core. Full REPL
slash-command execution is still being wired during the MVP migration.

Repair prompts are saved as bounded packets and can be re-entered with:

```text
/ultra-plan-run --profile nextjs "$(cat .commandagent/repairs/<file>.md)"
```

## Evaluation

Offline smoke:

```bash
scripts/eval_smoke.sh
```

Large task wiring dry-run:

```bash
scripts/eval_large_tasks.sh --dry-run
```

MVP sign-off uses `runs=1` for large cases. Release-quality stability checks use
`scripts/eval_large_tasks.sh --release-quality` for `runs=3`.

## Release

Releases are created from `v*` tags pushed from commits already on `main`.
`.github/workflows/release.yml` builds Linux and macOS assets and publishes
SHA-256 checksum files. See `docs/release.md` for the release operation.

## Docs

- `docs/install.md`
- `docs/release.md`
- `docs/philosophy.md`
- `docs/architecture.md`
- `docs/usage.md`
- `docs/providers.md`
- `docs/profiles.md`
- `docs/evaluation.md`
- `docs/known-limitations.md`
- `docs/adr/0001-minimal-only.md`
