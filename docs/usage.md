# Usage

CommandAgent is currently in migration bootstrap.

## Basic Commands

```bash
commandagent --help
commandagent --version
commandagent --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4
commandagent --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 "Create README.md"
```

When started from a terminal without a prompt argument, CommandAgent opens the
minimal interactive REPL:

```text
commandagent>
```

Blank lines are ignored. `/exit` and `/quit` end the REPL. Each successful turn
runs the minimal loop and saves a session under `.commandagent/sessions`.

## Configuration

CommandAgent merges configuration in this order:

1. built-in defaults
2. `.commandagent/config`
3. environment variables
4. CLI flags

Supported bootstrap keys include:

- `provider`
- `planner_provider`
- `model`
- `planner_model`
- `context_budget`
- `max_iterations`
- `timeout_secs`
- `retries`
- `yes`
- `offline`
- `resume`
- `state_dir`

The formal environment prefix is `COMMANDAGENT_*`, for example:

```bash
COMMANDAGENT_PROVIDER=ollama
COMMANDAGENT_MODEL=qwen3.6:35b-a3b-coding-nvfp4
COMMANDAGENT_PLANNER_PROVIDER=gemini
COMMANDAGENT_PLANNER_MODEL=gemini-3.5-flash
```

API keys use provider-standard names:

```bash
GEMINI_API_KEY=...
OPENAI_API_KEY=...
```

Provider examples:

```bash
commandagent --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4
commandagent --provider gemini --model gemini-3.1-flash-lite
commandagent --provider openai --model gpt-5.4-mini
```

Planner and executor models can differ:

```bash
commandagent \
  --provider ollama \
  --model qwen3.6:35b-a3b-coding-nvfp4 \
  --planner-provider gemini \
  --planner-model gemini-3.5-flash
```

Interactive slash commands use the same parser as the future step runner. The
parser recognizes:

- `/plan-steps`
- `/plan-run`
- `/run-plan`
- `/ultra-plan`
- `/ultra-plan-run`
- `/run-ultra-plan`

The parser also accepts leading `--profile` and `--style` options for plan
commands, plus bounded file references such as:

```text
/ultra-plan-run --profile nextjs "$(cat .commandagent/repairs/repair.md)"
```

File references are resolved inside the current workspace and cannot escape it.
The REPL itself handles `/exit` and `/quit`.

Step plans are stored under `.commandagent/plans` as CommandAgent-owned YAML.
The current schema is:

```yaml
goal: "..."
profile: "generic"
style: "default"
steps:
  - id: "short-slug"
    instruction: "one concrete action"
    expected_paths:
      - "relative/file/path"
    verify:
      - "local verification command"
```
