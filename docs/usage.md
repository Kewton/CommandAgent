# Usage

CommandAgent is currently in migration bootstrap.

## Basic Commands

```bash
commandagent --help
commandagent --version
```

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

Future MVP usage will include:

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

Interactive slash commands planned for MVP:

- `/plan-steps`
- `/plan-run`
- `/run-plan`
- `/ultra-plan`
- `/ultra-plan-run`
- `/run-ultra-plan`
- `/exit`
- `/quit`
