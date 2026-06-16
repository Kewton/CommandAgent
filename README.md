# CommandAgent

CommandAgent is a minimal local-first coding agent for local and API-backed
LLMs.

The MVP focuses on:

- a single minimal execution loop
- an interactive REPL
- built-in file/search/bash tools
- `/ultra-plan-run` style step execution
- thin provider support for Ollama, Gemini, and OpenAI

Legacy engines, sidecar routing, Photon, case memory, PAM, and historical
repair systems are intentionally out of scope.

## Docs

- `docs/philosophy.md`
- `docs/architecture.md`
- `docs/usage.md`
- `docs/adr/0001-minimal-only.md`

## Current Status

This repository is in migration bootstrap. The first milestone is a small Rust
CLI named `commandagent`.

```bash
cargo build
target/debug/commandagent --help
```
