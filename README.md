# commandagent

Copyable MVP for a minimal local-first coding loop, YAML step plans, plan run,
ultra plan run, and deterministic verification.

See [SECURITY.md](SECURITY.md) for the trusted-workspace threat model,
`--yes` guidance, environment allowlist, and symlink policy.
See [docs/dev-guardrails.md](docs/dev-guardrails.md) for the runner growth
tripwire and module-boundary guardrails.

## Build

```bash
cargo build
cargo test
cargo run -- --help
```

## Codex Harness

Repository-local Codex skills live under `.agents/skills/` and are invoked as
`$skill-name`. See [docs/codex-harness.md](docs/codex-harness.md) for the
migrated command map, orchestration entry point, safety boundaries, and
validation commands.

## Run

```bash
commandagent --yes --context-budget 65536 --model qwen3.6:27b-coding-nvfp4 --planner-model gemini-3.5-flash --planner-provider gemini --provider ollama
```

From the REPL:

```text
/ultra-plan-run --profile nextjs あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。
```

CLI equivalent:

```bash
commandagent --provider ollama --model qwen3.6:27b-coding-nvfp4 \
  --planner-provider gemini --planner-model gemini-3.5-flash \
  --ultra-plan-run --profile nextjs \
  "あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。"
```

Named presets can live in `.anvil/config.toml` or `~/.anvil/config.toml`.
These are examples only; CommandAgent does not auto-create them:

```toml
# [preset.gemini-tier]
# provider = "gemini"
# model = "gemini-3.5-flash"
# planner_provider = "gemini"
# planner_model = "gemini-3.5-flash"
# context_budget = 65536
# chat_timeout_secs = 180
# profile = "nextjs"
# narration = "normal"
# footer = "on"
#
# [preset.local]
# provider = "ollama"
# model = "qwen3.6:27b-coding-nvfp4"
# planner_provider = "ollama"
# planner_model = "qwen3.6:27b-coding-nvfp4"
# context_budget = 65536
# chat_timeout_secs = 600
# profile = "generic"
# narration = "normal"
# footer = "on"
# plan_preset = "profile" # optional opt-in; omitted defaults to none
#
# [preset.hybrid-a3b]
# provider = "ollama"
# model = "qwen3.6:35b-a3b-coding-nvfp4"
# planner_provider = "gemini"
# planner_model = "gemini-3.5-flash"
# context_budget = 65536
# chat_timeout_secs = 600
# profile = "nextjs"
# narration = "normal"
# footer = "on"
```

```bash
commandagent --preset local --ultra-plan-run "Build a small CLI tool"
```

Hybrid presets use the planner provider/model only for planning calls and the
main provider/model for execution calls. With a local a3b executor, budget RAM
for the Ollama model residency separately from any remote planner; Ollama
requests keep `keep_alive=10m` for the executor model between turns.

Plan presets are explicit opt-ins, not globally enabled. Although
test0711_bs_001 showed a strong qwen27 sample, test0711_bs_004 exposed duplicate
setup-step stagnation in preset implementation phases, so qwen27, gemma-family,
and unmatched planner models currently default to `none`. Planner-tier
resolution remains observable and is independent of whether the model came
from a direct CLI option, the executor-model fallback, or a named config preset.
Set `plan_preset = "profile"` in config or pass `--plan-preset profile` to opt in;
an explicit CLI flag always wins over config and tier defaults.

For `--intent fix --profile data`, the `profile` preset mechanically synthesizes
the four fixed-contract phases from the existing reproducer, contract checks,
and frozen regression bindings. `nextjs` fix runs keep the same path as
`--plan-preset none`; profile-specific synthesis will be generalized only after
a second profile demonstrates the same need. Create-intent preset behavior is
unchanged.

Use `--intent create` or `--intent fix` to select the run intent explicitly:

```bash
commandagent --intent fix --ultra-plan-run "Fix the parser; reproducer: cargo test parser"
```

When `--intent` is omitted, the existing goal-based resolution remains unchanged.
Invalid values are rejected by the CLI before a run starts. Every run records one
`intent_resolved` event with the resolved `value`, its `origin`, and the explicit
flag value in `source` (empty when omitted).

## TUI

Interactive TTY mode uses the same `commandagent>` prompt and slash commands, plus
terminal-only markdown rendering, spinner, Esc/Ctrl-C prompt interrupt, and a fixed
footer. Disable paths:

```bash
ANVIL_NO_SPINNER=1 commandagent --yes
ANVIL_NO_FOOTER=1 commandagent --yes
ANVIL_NO_INTERRUPT=1 commandagent --yes
ANVIL_NO_MARKDOWN=1 commandagent --yes
NO_COLOR=1 commandagent --yes
commandagent --yes --no-footer
commandagent --yes --footer off
```

Release/manual TTY smoke:

```bash
ANVIL_PTY_TESTS=1 cargo test tui_pty_smoke -- --ignored
commandagent --ux-demo
commandagent --help
commandagent --yes --context-budget 65536 --model qwen3.6:27b-coding-nvfp4 --planner-model gemini-3.5-flash --planner-provider gemini --provider ollama
```

`commandagent --ux-demo` is an offline presentation walkthrough for human
review. It exercises the banner, plan card, phase header, activity narration,
live footer interrupt hint, and terminal summary card without contacting a
provider. If a terminal still shows cursor-region artifacts, rerun with
`--footer off`; scrollback breadcrumbs stay enabled without the DECSTBM footer.

## UAT

Before a UAT run, verify the binary provenance:

```bash
commandagent --version
command -v commandagent
```

`commandagent --version` should show the intended commit, dirty marker, and
build timestamp. `command -v commandagent` should resolve to the expected
`target/` binary or install path for the run.

## API Keys

`OPENAI_API_KEY` and `GEMINI_API_KEY` are read from process env first, then
from `.env` in the active workspace. Values are redacted from logs.

Live provider tests are gated:

```bash
ANVIL_LIVE_PROVIDER_TESTS=1 cargo test live_ -- --ignored
```

Smoke model IDs can be overridden with `ANVIL_OPENAI_SMOKE_MODEL` and
`ANVIL_GEMINI_SMOKE_MODEL`.

## Local Symlink

```bash
cargo build --release
ln -sfn "$(pwd)/target/release/commandagent" "$HOME/.local/bin/commandagent"
commandagent --help
```

The symlink is a local convenience and is not part of the copy artifact.

## Copy Validation

```bash
tmp=$(mktemp -d)
git archive --format=tar HEAD | tar -x -C "$tmp"
cd "$tmp"
cargo test
cargo run -- --help
```
