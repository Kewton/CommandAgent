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

The `.anvil/` config names remain supported as legacy reads. New runs, plans,
repairs, and evidence use the matching `.commandagent/` subdirectories; run
inventory, resume discovery, and evidence consumers retain their applicable
`.anvil/` reads. Default session and workspace-history writes use the platform
`commandagent` state directory and fall back to `anvilminimal` when loading or
copying forward existing state.

## Presets

Select a preset with `--preset <name>`. A preset section accepts all 22 current
keys below. String/enumeration values must be double-quoted; numeric values are
unquoted integers.

| Preset key | Accepted value | Effective fallback when absent everywhere |
| --- | --- | --- |
| `extends` | one parent preset name | no parent |
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
| `openrouter_provider` | one-line inline table with only `only`, `order` (provider slug arrays), and `allow_fallbacks` (boolean), sent as the `provider` object by `openai-compatible` roles; for example with `api = "chat_completions"`, `{ only = ["open-inference/fp4"], allow_fallbacks = false }`; `{}` clears an inherited table (see [Providers](providers.md)) | no routing |

`extends` performs single inheritance. The child wins for every field it
defines, while missing fields come from the parent. Parents may themselves
extend one parent; missing parents and cycles such as `a -> b -> a` are errors.
Inherited field diagnostics retain `preset:<parent>` as their source.

Quoted values may contain one or more `${ENV_NAME}` references. CommandAgent
expands them from the process environment before validating the field. This
works for string/enumeration fields and for quoted numeric fields, for example
`context_budget = "${COMMANDAGENT_CONTEXT_BUDGET}"`. An unset, non-Unicode, or
invalid variable reference fails configuration resolution and appears as `✗`
in `--doctor`; the variable's value is never printed.

```toml
[preset.team_base]
provider = "openai-compatible"
base_url = "${TEAM_LLM_URL}"
model = "${TEAM_EXECUTOR_MODEL}"
planner_model = "${TEAM_PLANNER_MODEL}"

[preset.alice]
extends = "team_base"
model = "${ALICE_EXECUTOR_MODEL}"
```

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
`classifier_model`, `classifier_provider`, `base_url`, `api_key_env`, and
`extends` are
accepted but are **not** part
of that completeness test. If a
higher-priority preset already has the 11 completeness fields but omits
`prompt_layout`, CommandAgent stops searching and does not inherit that preset's
`prompt_layout` from a lower-priority file. Put `prompt_layout` in the same
higher-priority preset, or omit enough completeness fields for the intended
lower layer to be visited. Do not assume the 22 accepted keys are the same as
the 11-key early-stop condition.

## Top-level keys

Only eight keys are valid at the top level of a `config.toml` file:

| Key | Accepted value | Matching CLI override |
| --- | --- | --- |
| `extension_root` | directory path string | `--extension-root` |
| `narration` | `"normal"` or `"quiet"` | `--quiet` forces `quiet` |
| `footer` | `"on"` or `"off"` | `--footer`, `--no-footer` |
| `stream` | `"on"` or `"off"` | `--stream` |
| `prompt_layout` | `"stable"` or `"legacy"` | `--prompt-layout` |
| `plan_preset` | `"none"` or `"profile"` | `--plan-preset` |
| `request_pattern` | `"off"` or `"jev"`; jev classifies a direct `--prompt` request to add request guidance | `--request-pattern` |
| `request_pattern_threshold` | number from `0` to `1` (default `0.8`); a lower jev confidence adds no guidance | none |

```toml
extension_root = "extensions"
narration = "quiet"
footer = "off"
stream = "on"
prompt_layout = "stable"
plan_preset = "none"
```

Keys such as `model` are valid inside a preset but invalid at top level.

> **Compatibility change.** Every existing `config.toml` among the four layers
> (`<workspace>/.commandagent`, `<workspace>/.anvil`, `~/.commandagent`,
> `~/.anvil`) is now parsed at startup, with or without `--preset` or CLI
> overrides. An unknown top-level key, invalid TOML, or a file that cannot be
> read is a startup error; the command exits non-zero and `--doctor` reports a
> failed Configuration check without contacting a provider. Earlier versions
> silently skipped such a file and used a lower-priority file or the default, so
> a typo or a broken higher-priority file could go unnoticed. The diagnostic
> names the file, line and key only, never a value. Startup checks only syntax, unknown top-level keys and
> read failures; `${ENV}` expansion and the contents of a preset are still
> validated only when that preset is selected. Fix or remove the reported
> line to continue.

Valid configuration resolves as before: flag, then preset, then config file,
then default, with the same source labels. Sections other than
`[preset.<name>]` are still ignored by the current small parser rather than
treated as configuration, and an extensionless legacy `config` file keeps its
lenient line scan.

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
| `COMMANDAGENT_STEP_WALL_CLOCK_CAP_MS` / `ANVIL_STEP_WALL_CLOCK_CAP_MS` | Sets the operational per-step wall-clock cap in milliseconds. A `RunSessionOptions` value takes precedence over the environment. The value is trimmed and parsed as a `u64`; an unset or unparsable value uses the 15-minute default, and `0` is accepted by the parser. |
| `COMMANDAGENT_PACK_DIRECTORY` | Internal handoff for the selected pack directory. Configure it with `--pack` or a preset instead of setting it by hand. |
| `COMMANDAGENT_PACK_ID` | Internal handoff for the selected pack ID. Configure it with `--pack` or a preset instead of setting it by hand. |
| `COMMANDAGENT_PACK_VERSION` | Internal handoff for the selected pack version. Configure it with `--pack` or a preset instead of setting it by hand. |
| `COMMANDAGENT_PACK_HASH` | Internal handoff for the selected pack exact-byte hash. Configure it with `--pack` or a preset instead of setting it by hand. |
| `COMMANDAGENT_UX_DEMO_FAST` / `ANVIL_UX_DEMO_FAST` | Speeds up the `--ux-demo` presentation waits for development and demos. The variable's presence, not its value, is tested. |
| `COMMANDAGENT_SYSTEM_GUIDANCE` | Adds one common guidance text to the system prompt of every provider request of a direct `--prompt` run. Unset or empty changes nothing. See below. |
| `COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` | Declares a reasoning effort for the roles that use the OpenAI-compatible transport (`openai-compatible` and `lm-studio`). Unset or empty sends nothing. See below. |
| `COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW` | Experimental. `once` asks a generic edit run to review the request's obligations once before its first normal final answer. Unset, empty, or `off` (the default) changes nothing. See below. |
| `COMMANDAGENT_REQUIREMENT_PLAN` | Experimental. `once` asks a new generic run to write a short requirement plan note before its first implementation step. Unset, empty, or `off` (the default) changes nothing. See below. |

For `COMMANDAGENT_*` entries, the current name wins. The matching `ANVIL_*`
name is used only when the current name is absent, and emits a one-time
deprecation warning. The legacy names in the table are nevertheless the exact
currently supported spellings.

The four `COMMANDAGENT_PACK_*` names are the exception: they are read directly
and have no `ANVIL_*` legacy name. They are an internal handoff for the selected
pack, not a user-configured setting; use `--pack` or a preset to select a pack.

`COMMANDAGENT_SYSTEM_GUIDANCE` is an opt-in for comparing a common instruction
text. It is read once, from the process environment only, when a direct
`--prompt` run starts; it has no `ANVIL_*` legacy name, and no config file or
`.env` is read for it. The plan entries, the TUI, and `--model-probe` ignore it.
When it is set, every provider request of the run (each turn, the turns after
tool calls, the empty-response fresh retry, native and XML tool protocols)
carries the text once in its "Request guidance" block; when
`--request-pattern jev` adopts a pattern, the text follows that pattern's
guidance under a "Common guidance:" line. The original request, the profile
guidance, the session history, the `request_pattern_resolved` event, and when
jev is called do not change. The value must be UTF-8, at most 8 KiB (8192
bytes), and not only whitespace; otherwise the run fails with an error that
names the variable and the reason but not the value. The text is ordinary
guidance: it grants or removes no tool permission and changes no verification,
completion, acceptance, or budget rule. The run's secret scrub applies to it,
but do not put secrets in it.

`COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW` is an experimental opt-in for
measuring one completion review; it is off by default and is not a coverage or
verification gate. It is read once, from the process environment only, when a
minimal-loop run starts (no `ANVIL_*` legacy name, no config file, no `.env`,
no CLI flag), and a change during the run has no effect.

- Values: unset, empty, whitespace-only, or `off` disables it, and the run is
  exactly the run without the variable: the same provider requests in the same
  order, the same events, stop reason, and assurance, and no new event. `once`
  (surrounding whitespace trimmed, lowercase) enables it. Any other value,
  including `ONCE` or non-UTF-8 bytes, fails the run before the first request
  with a fixed error that names the variable but not the value.
- Scope: a minimal-loop run (not a plan step) whose effective profile is `generic`, that
  has no completion contract, and in which a Write or Edit succeeded. Only the
  first normal assistant final is considered, after it passed every existing
  check (required paths, the action-prompt check, relative imports), and only
  when at least three loop iterations remain. Plan steps (`PlanRunStep`),
  runs with a completion contract, read-only runs, the automatic post-write
  read completion, the automatic required-artifact completion, and budget
  exhaustion are never deferred; the wall-clock cap, cancellation, and every
  existing failure keep priority.
- Behavior: that final is deferred once and the model receives one fixed
  request: list up to five obligations the original request states, map what
  was already confirmed, choose one or two important unconfirmed boundary,
  error, or output-format inputs, derive the expected result from the request
  rather than the implementation, run them with the already allowed tools, and
  fix only an observed contradiction. The next normal final passes the
  existing checks again and ends the run with that new answer; the deferred
  answer is never returned. What the model says about the review (for example
  that it covered everything) neither releases nor blocks it. There is no
  second request, no extra iteration, and no reset of an existing counter.
- Host observation: after the request the host counts successful Write/Edit
  calls and non-substituted Bash results, using the existing execution-progress
  classification, not the tool status, and drops successful Bash observations
  older than the latest successful edit. A non-zero exit, a tool error, or a
  policy rejection is not counted as successful, and none of them becomes a new
  blocker. A successful command proves only that it ran: it may be an `echo`,
  unrelated, or a wrong self-written check, and a negative test may correctly
  exit non-zero. The model's self-report is not evidence either.
- Events: the run records `completion_obligation_review` with `phase`
  `triggered`, `closed` (`reason` `next_final`, or
  `scope_exit_before_next_final` when the run ended another way), or `skipped`
  (a fixed reason), plus numbers such as `iteration`, `remaining_iterations`,
  `prompt_version`, and `successful_bash_since_latest_edit`. It never copies
  the prompt, a command, model text, tool output, or a secret, and has no
  coverage or verification-passed field. Existing events, stop reasons, the
  summary's `assurance_level=reduced`, and acceptance and release gates do not
  change.
- Cost: the review normally adds one to three provider calls, but this is not
  guaranteed; the model may use more of the remaining iterations, up to the
  existing limit. Whether the review improves correctness is unmeasured.

`COMMANDAGENT_REQUIREMENT_PLAN` is an experimental opt-in that asks the model
to state the request's obligations and the expected results of a few small
checks before it implements anything. It is off by default. The note is the
model's own unverified memo: the host checks only its shape and size, never
whether it is correct or complete, and it is not a coverage, verification, or
acceptance gate. It is read once, from the process environment only, when a
minimal-loop run starts (no `ANVIL_*` legacy name, no config file, no `.env`,
no CLI flag), and a change during the run has no effect.

- Values: unset, empty, whitespace-only, or `off` disables it, and the run is
  exactly the run without the variable: the same provider requests in the same
  order, the same events, stop reason, and assurance, and no new event. `once`
  (surrounding whitespace trimmed, lowercase) enables it. Any other value,
  including `ONCE` or non-UTF-8 bytes, fails the run before the first request
  with a fixed error that names the variable but not the value.
- Not combinable: `once` together with
  `COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW=once` fails the run before the
  first request with an error that names only the two variables. With this
  variable off, the completion obligation review works exactly as before.
- Scope: a new minimal-loop run (not a plan step) whose effective profile is
  `generic`, that has no completion contract, that does not continue an earlier
  conversation, and in which at least four loop iterations remain after the
  first request. Plan steps (`PlanRunStep`), other profiles, runs with a
  completion contract, resumed or follow-up turns, and runs with less budget are
  skipped. It is never enabled in the middle of a run.
- Request: the first request alone carries one fixed guidance message. It asks
  for a `<requirement_plan>` JSON block at the start of the first reply with up
  to five obligations the request states (`id`, `stated_requirement`,
  `expected_behavior`, `assumption`) and up to two check examples
  (`obligation_id`, `input_outline`, `expected_outcome`,
  `reason_from_request`), with the expected results decided from the request
  before any implementation and ambiguities written as assumptions. The reply
  may continue with normal work and tool calls.
- Limits: the block may hold at most 8 KiB (UTF-8) and the normalized note at
  most 2048 characters, with one to five obligations and zero to two examples.
  Only those fields are accepted, ids must be unique, and every example must
  name an existing obligation. A missing block, a block that does not open the
  reply, malformed or truncated JSON, an unknown field, a duplicate id, a
  dangling reference, or an oversize block discards the whole note (it is never
  cut down to fit); the run continues without a note and the note is never
  asked for again.
- Handling: the host takes the block out of the first reply before any XML tool
  call is extracted or any tool runs. Nothing inside the block, and nothing
  after an unclosed block, is read as a tool call by the loop (native tool calls
  and explicit text mode, `tool_protocol = "text"`). A provider client that
  parses XML tool calls itself before returning the reply (its own XML fallback)
  does so before the host sees the block, so that path is not covered. The kept note passes the
  run's secret scrub, lives only in memory for that run, and is never written to
  the session, a file, or the runtime namespace; a resumed or follow-up turn does
  not get it back. Each later request of the run carries it exactly once, right
  after the original request, as assistant-side context with a fixed header
  saying it is the model's own unverified memo and that the original request and
  later user corrections take precedence. It is never joined to the system
  prompt and never replaces the request, a tool result, or pending feedback;
  history compaction and the empty-response fresh retry keep it, and it adds no
  retry. A later user message makes the note stale and it is no longer sent; the
  run does not plan again.
- Planning turn: when the first reply is only a valid block (no tool call and no
  other text), that reply counts as one existing iteration and provider call and
  the run moves on. No iteration is added or returned, and no existing counter
  is reset. Failures, timeouts, cancellation, and budget exhaustion stay what
  they are.
- Events: the run records `requirement_plan` with `phase` `requested`,
  `captured`, `skipped`, or `released`, a fixed `reason`, `prompt_version`,
  `iteration`, `remaining_iterations`, `obligation_count`, `example_count`,
  `note_chars`, and `plan_only_turn`. `captured` means the shape was valid, not
  that the note is right. The event never copies the note, the request, example
  values, tool output, a secret, an unknown key name, or a parser message.
  Existing events, stop reasons, assurance, and acceptance and release gates do
  not change; a successful self-written example or the model saying everything
  was checked raises nothing. Whether the note improves correctness is
  unmeasured.

`COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` declares how hard an
OpenAI-compatible server should reason. It is read from the process environment
only (no `ANVIL_*` legacy name, no config file, no `.env`) when a provider
client is built, and every clone, worker, and transport retry of that client
sends the same value. It has no CLI flag.

- Scope: every role whose provider resolves to the OpenAI-compatible transport,
  that is the generic `openai-compatible` provider and `lm-studio`, for the
  executor, the planner, and the classifier alike. Ollama native, OpenAI, and
  Gemini roles never send it, and its value never stops them from starting.
- Values: one of `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, or `max`,
  in lowercase, after surrounding whitespace is trimmed. Case and aliases are
  not converted. Unset, empty, or whitespace-only sends no field, so the
  request is exactly the one sent without the variable. Any other value
  (`LOW`, `true`, an unknown word, non-UTF-8 bytes) fails before the first
  request with an error that names the variable and the accepted words but
  not the value.
- Wire: Chat Completions (`--api chat-completions`) adds a top-level
  `"reasoning_effort": "<value>"`; Responses (`--api responses`) adds
  `"reasoning": {"effort": "<value>"}`. Nothing else in the request changes:
  `max_tokens` and `max_output_tokens` stay, and no OpenAI-only field or
  Ollama `think` field is added.
- Unsupported values: CommandAgent checks its own vocabulary only. It does not
  ask the server which values the model supports and does not special-case
  model or host names. When the server rejects the value (for example HTTP 400
  for a word the model does not offer), the turn fails as an ordinary
  non-retryable provider error; CommandAgent does not resend without the field,
  with a different effort, through another API, or as `think`.
- Other settings: `--think` still controls only Ollama native roles, and
  `COMMANDAGENT_OPENAI_REASONING_EFFORT` still applies only to the `openai`
  provider; neither falls back to this variable, and this variable does not
  fall back to them.
- Evidence: `--doctor` reports `config.compatible_reasoning_effort` per role as
  `declared`, `omitted`, `not_applicable`, or `invalid` (an invalid value used
  by a compatible role fails the check), and the headless summary records the
  same declaration under `model_metadata.compatible_reasoning_effort`. Both
  describe what CommandAgent sends, not what the server applied
  (`server_support_verified` stays `false`). When a value is sent, the
  `provider_request` event adds `requested_reasoning_effort`,
  `reasoning_effort_source` (`env`), and `provider_role`.

For example, with an OpenAI-compatible executor and a native Ollama planner:

```bash
COMMANDAGENT_COMPATIBLE_REASONING_EFFORT=low commandagent \
  --provider openai-compatible --base-url http://127.0.0.1:11434/v1 \
  --model glm-5.3-flash:cloud \
  --planner-provider ollama --planner-model <planner-model> \
  --prompt '<request>'
```

The executor sends `reasoning_effort: "low"`; the Ollama planner keeps its own
`--think` setting and receives no effort field. If the planner also used
`--planner-provider openai-compatible`, it would send the same `low`.

## Inspect the effective configuration

Start the TUI and run `/status`. The startup banner and status card show key
resolved values and their sources, including `flag`, `preset:<name>`,
`config:<file>`, inferred/default sources, timeout source, footer, stream, and
prompt layout.

Use this view before diagnosing an unexpected model, profile, layout, timeout,
or display mode. The view does not display API keys; never print key values to
diagnose configuration.
