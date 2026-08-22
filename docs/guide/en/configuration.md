# Configuration

[日本語](../ja/configuration.md) | [Guide index](../README.md)

CommandAgent resolves configuration after it canonicalizes the active workspace
from `--cwd` or the process current directory. It reads files but does not
create a config file or populate presets automatically.

## Resolution precedence

For fields that support every layer, the order is:

```text
CLI flag > selected preset field > top-level config key > built-in default
```

Resolution is field-by-field, and not every setting supports all four layers.

| Fields | Implemented order |
| --- | --- |
| `prompt_layout`, `plan_preset` | CLI > preset > top-level key > computed/built-in default |
| `narration` | `--quiet` > preset > top-level key > `normal` |
| `footer` | `--no-footer` or `--footer` > preset > top-level key > `on` |
| `stream` | `--stream` > preset > top-level key > on for REPL, off for direct actions |
| `model`, `provider`, `context_budget`, `chat_timeout_secs` | CLI > preset > built-in/provider-dependent default |
| `base_url`, `api_key_env` | CLI > preset; used only by `openai-compatible`, whose base URL is required |
| `api` | `--api` > preset > `chat_completions` (OpenAI and LM Studio) |
| `tool_protocol` | CLI > preset > provider capability default |
| `planner_model`, `planner_provider` | CLI > preset > executor role inheritance; a different provider requires a planner model |
| `planner_think` | explicit `--think` > preset > `false`; the CLI flag also remains the executor override |
| `classifier_model`, `classifier_provider` | preset > planner role inheritance; a different provider requires a classifier model |
| `profile` | CLI > preset > goal/workspace inference > `generic` |
| `pack` | CLI `--pack` > selected preset; contradictory explicit values fail |
| `extension_root` | CLI `--extension-root` > top-level key > repository-only search |
| `ollama_host`, `think`, `lm_studio_host`, `num_predict`, `max_iterations`, `chat_retries`, `style`, `state_dir`, and other CLI-only fields | CLI value or CLI-declared/built-in default; config files do not accept them |

The timeout default is `600` seconds if any executor, planner, or classifier
role uses Ollama or LM Studio and `180` seconds when all roles are remote.
`context_budget` defaults to `65536`.
`plan_preset` is normally `none`; explicit `data` plus `fix` or `investigate`
can compute `profile` before the planner-model default is applied.

## Configuration search paths

The active workspace is the canonical `--cwd` path, or the canonical process
current directory when `--cwd` is absent. TOML-style files are inspected in
this order, highest priority first:

1. `<workspace>/.commandagent/config.toml`
2. `<workspace>/.anvil/config.toml`
3. `$HOME/.commandagent/config.toml`
4. `$HOME/.anvil/config.toml`

For a top-level key, the first file containing that key wins. For a selected
preset, the first value for each field wins while missing fields can be filled
from later files. This permits a workspace preset to override only part of a
user preset, subject to the [completeness trap](#the-preset-completeness-trap).

The `.anvil/` names remain supported. Do not rename existing `.anvil/` runtime
or guide paths based only on the newer `.commandagent/` config namespace.

## Presets

Select a preset with `--preset <name>`. A preset section accepts all 20 current
keys below. String/enumeration values must be double-quoted; numeric values are
unquoted integers.

| Preset key | Accepted value | Effective fallback when absent everywhere |
| --- | --- | --- |
| `pack` | exact `"id@MAJOR.MINOR.PATCH"` selector | no pack |
| `model` | model ID string | `qwen3.6:27b-coding-nvfp4` |
| `provider` | `"ollama"`, `"lm-studio"`, `"openai"`, `"openai-compatible"`, or `"gemini"` | `"ollama"` |
| `base_url` | HTTP(S) URL without credentials, query, or fragment | required for `openai-compatible` |
| `api_key_env` | process-environment variable name | no bearer authentication |
| `api` | `"chat_completions"` or `"responses"` | `"chat_completions"` |
| `tool_protocol` | `"native"` or `"text"` | provider capability default |
| `planner_model` | model ID string | executor model when providers match; otherwise required |
| `planner_provider` | `"ollama"`, `"lm-studio"`, `"openai"`, `"openai-compatible"`, or `"gemini"` | executor provider |
| `planner_think` | `"true"`, `"false"`, `"low"`, `"medium"`, or `"high"` | `"false"` |
| `classifier_model` | model ID string | planner model when providers match; otherwise required |
| `classifier_provider` | `"ollama"`, `"lm-studio"`, `"openai"`, `"openai-compatible"`, or `"gemini"` | planner provider |
| `context_budget` | non-negative platform-sized integer | `65536` |
| `chat_timeout_secs` | non-negative 64-bit integer | provider-dependent `600` or `180` |
| `profile` | profile string | inferred, then `"generic"` |
| `narration` | `"normal"` or `"quiet"` | `"normal"` |
| `footer` | `"on"` or `"off"` | `"on"` |
| `stream` | `"on"` or `"off"` | on for REPL, off otherwise |
| `prompt_layout` | `"stable"` or `"legacy"` | top-level value, then `"legacy"` |
| `plan_preset` | `"none"` or `"profile"` | top-level/computed planner value |

The following complete preset shows the current measured local role split. It
is an explicit example, not a built-in default; read the [role-pair evidence
and scope](../model-probe-results/2026-08-22-local-role-pairs.md) before use.

```toml
[preset.local_role_split]
model = "qwen3.8:27b-mlx"
provider = "ollama"
api = "chat_completions"
tool_protocol = "native"
planner_model = "qwen3.8:27b-mlx"
planner_provider = "ollama"
planner_think = "false"
classifier_model = "qwen3.5:4b"
classifier_provider = "ollama"
context_budget = 65536
chat_timeout_secs = 600
profile = "generic"
narration = "normal"
footer = "on"
stream = "on"
prompt_layout = "legacy"
plan_preset = "none"
```

This exact local probe supports the smaller classifier only. It does not
support replacing the planner with the measured 9B or 4B candidates. Re-run
the [model probe](model-probe.md#role-pair-procedure) and scenario admission
checks when a model digest, host, context, provider, or build changes.

An unknown key or invalid value in a parsed preset is an error naming the file,
line, and `preset.<name>.<key>`. Selecting a name that does not occur in any
search path is also an error; CommandAgent does not silently use defaults for a
missing named preset.

### The preset completeness trap

Preset merging stops early once these 11 fields are present: `model`,
`provider`, `planner_model`, `planner_provider`, `context_budget`,
`chat_timeout_secs`, `plan_preset`, `profile`, `narration`, `footer`, and
`stream`.

`prompt_layout`, `api`, `tool_protocol`, `pack`, `planner_think`,
`classifier_model`, `classifier_provider`, `base_url`, and `api_key_env` are
accepted but are **not** part
of that completeness test. If a
higher-priority preset already has the 11 completeness fields but omits
`prompt_layout`, CommandAgent stops searching and does not inherit that preset's
`prompt_layout` from a lower-priority file. Put `prompt_layout` in the same
higher-priority preset, or omit enough completeness fields for the intended
lower layer to be visited. Do not assume the 20 accepted keys are the same as
the 11-key early-stop condition.

## Top-level keys

Only six keys are valid at the top level of a `config.toml` file:

| Key | Accepted value | Matching CLI override |
| --- | --- | --- |
| `extension_root` | directory path string | `--extension-root` |
| `narration` | `"normal"` or `"quiet"` | `--quiet` forces `quiet` |
| `footer` | `"on"` or `"off"` | `--footer`, `--no-footer` |
| `stream` | `"on"` or `"off"` | `--stream` |
| `prompt_layout` | `"stable"` or `"legacy"` | `--prompt-layout` |
| `plan_preset` | `"none"` or `"profile"` | `--plan-preset` |

```toml
extension_root = "extensions"
narration = "quiet"
footer = "off"
stream = "on"
prompt_layout = "stable"
plan_preset = "none"
```

Keys such as `model` are valid inside a preset but invalid at top level.
Unknown top-level keys make that file fail parsing. Named-preset loading
surfaces the parse error; top-level field lookup skips a file that did not parse
and continues to lower-priority files. Sections other than `[preset.<name>]`
are ignored by the current small parser rather than treated as configuration.

## Pack selection

Pack selectors always include an exact version, for example
`nextjs-acme@1.0.0`. CommandAgent searches
`<extension_root>/<id>/<version>` (and the compatible
`<extension_root>/packs/<id>/<version>` layout) before
`<workspace>/packs/<id>/<version>`. Every selected directory must contain a
`pack.sha256` equal to the loaded assist/eval exact-byte hash. A missing pin,
stale pin, `--pack-hash` mismatch, or profile/intent mismatch exits with code 2
before run-start evidence is emitted.

```toml
extension_root = "extensions"

[preset.nextjs_acme_cagentpack]
pack = "nextjs-acme@1.0.0"
profile = "nextjs"
```

## Legacy extensionless config

If no matching value is found in the four `config.toml` paths, top-level lookup
also checks these extensionless files in order:

1. `<workspace>/.commandagent/config`
2. `<workspace>/.anvil/config`

The legacy `.anvil/config` format remains supported exactly as named. It is a
line-oriented `key = value` fallback only for `narration`, `footer`, `stream`,
`prompt_layout`, and `plan_preset`; it does not support presets. Values may be
quoted or unquoted, and `#` starts a comment. Prefer `config.toml` for new
configuration.

## Environment variables

The OpenAI key and optional `LM_STUDIO_API_TOKEN` are read only from the process
environment. The Gemini key checks the process environment first and then a
workspace `.env`; see [Providers](providers.md). The following environment
variables also affect normal user-visible behavior:

| Variable | Effect |
| --- | --- |
| `LM_STUDIO_API_TOKEN` | Supplies an optional Bearer token when LM Studio server authentication is enabled. |
| `NO_COLOR` | Disables ANSI color. It has no `ANVIL_*` alias. |
| `COMMANDAGENT_NO_FOOTER` / `ANVIL_NO_FOOTER` | A non-empty value disables the fixed footer. |
| `COMMANDAGENT_NO_SPINNER` / `ANVIL_NO_SPINNER` | A non-empty value disables the progress spinner. |
| `COMMANDAGENT_NO_MARKDOWN` / `ANVIL_NO_MARKDOWN` | A non-empty value disables terminal Markdown rendering. |
| `COMMANDAGENT_NO_INTERRUPT` / `ANVIL_NO_INTERRUPT` | A non-empty value disables the raw-mode interrupt monitor. |
| `COMMANDAGENT_NO_TERMINAL_TITLE` / `ANVIL_NO_TERMINAL_TITLE` | A non-empty value disables Ultra phase progress in the terminal title. |
| `COMMANDAGENT_NO_BELL` / `ANVIL_NO_BELL` | A non-empty value disables the completion bell for commands lasting at least 10 seconds. |
| `COMMANDAGENT_EVAL_EVENTS` / `ANVIL_EVAL_EVENTS` | Overrides the event JSONL path. |
| `COMMANDAGENT_PLAYWRIGHT_DIR` / `ANVIL_PLAYWRIGHT_DIR` | Adds an explicit Playwright module search directory for the interaction probe. |
| `COMMANDAGENT_COMPLETION_CONTRACT` / `ANVIL_COMPLETION_CONTRACT` | Supplies an external completion-contract path. |
| `COMMANDAGENT_DEV_SERVER_PROBE` / `ANVIL_DEV_SERVER_PROBE` | A false value disables the planner dev-server probe. |
| `XDG_STATE_HOME` | Changes the base used by the default state directory. |
| `HOME` | Supplies user config paths and the fallback state-directory home. |
| `LC_ALL`, then `LANG` | Determines whether the spinner uses UTF-8 frames. |

For `COMMANDAGENT_*` entries, the current name wins. The matching `ANVIL_*`
name is used only when the current name is absent, and emits a one-time
deprecation warning. The legacy names in the table are nevertheless the exact
currently supported spellings.

## Inspect the effective configuration

Start the TUI and run `/status`. The startup banner and status card show key
resolved values and their sources, including `flag`, `preset:<name>`,
`config:<file>`, inferred/default sources, timeout source, footer, stream, and
prompt layout.

Use this view before diagnosing an unexpected model, profile, layout, timeout,
or display mode. The view does not display API keys; never print key values to
diagnose configuration.
