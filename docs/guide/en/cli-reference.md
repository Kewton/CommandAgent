# CLI Reference

[日本語](../ja/cli-reference.md) | [Guide index](../README.md)

`commandagent [OPTIONS] [GOAL]...` starts the interactive TUI when no action is
selected. A trailing goal can contain multiple words without quoting because it
is collected as the final argument list. The installed binary remains the
authority: use `commandagent --help` when its version differs from this checkout.

## Invocation

Use one of the action-selector flags for a direct command, or omit all of them
for the TUI. The action selectors are `--prompt`, `--plan-steps`, `--plan-run`,
`--run-plan`, `--ultra-plan`, `--ultra-plan-run`, `--run-ultra-plan`,
`--validate-plan`, `--setup-interaction-probe`, `--runs`, `--ux-demo`, `--model-probe`, and
`--doctor`. The offline pack actions `--packs`, `--pack-verify`, and
`--pack-pin`, generated-artifact actions `--completions` and `--generate-man`,
config action `--init-config`, and delegated manifest actions
`--validate-manifest` and `--init-profile` are displayed in the same
`Actions (use one)` help group. CommandAgent rejects combinations whose action
contracts are mutually exclusive.

Clap also generates `-h`/`--help` and `-V`/`--version`. They are not part of the
59 application flags below. The hidden `--completion-contract-json <PATH>` is an
internal integration surface and is intentionally not a public user flag.

## Flag reference

| Flag | Argument | Default when omitted | Description | Related |
| --- | --- | --- | --- | --- |
| `--yes` | none | off | Allow every tool and skip resume confirmation; recognized Bash writes remain workspace-confined. It never auto-kills a busy-port owner. Use only in a trusted workspace. | [Busy ports](troubleshooting.md#preflight-port-n-is-busy) |
| `--allow` | `<read\|write\|bash:verify>` | legacy read access and per-mutation approval | Allow only the selected tool classes; repeat or comma-separate read, write, and bash:verify. Selected mutations are auto-approved, while omitted classes are blocked. | [Security model](../../../SECURITY.md) |
| `--preset` | `<PRESET>` | none | Select a named `[preset.<name>]` assembled from configuration files. | [Presets](configuration.md#presets) |
| `--pack` | `<ID@VERSION>` | preset `pack`, then none | Activate an exact-version pack. A conflicting preset pack is rejected before the run. | [Pack selection](configuration.md#pack-selection) |
| `--pack-hash` | `<SHA256>` | verified `pack.sha256` | Require the selected pack's exact-byte hash. Requires `--pack`. | [Pack selection](configuration.md#pack-selection) |
| `--extension-root` | `<DIR>` | top-level `extension_root`, then none | Load local packs and `profiles/<id>/manifest.toml` draft profiles. External profiles are forced to draft and pinned by exact-byte hash. | [Pack selection](configuration.md#pack-selection) |
| `--packs` | none | off | List compatible admitted packs and conformant packs found under `--extension-root`, including each source. Requires `--profile` and `--intent`. | [Conflicts](#conflicts-and-combinations) |
| `--pack-verify` | `<DIR>` | none | Run strict conformance for one pack directory and print the same JSON report as `pack_conformance`. | [Conflicts](#conflicts-and-combinations) |
| `--pack-pin` | `<DIR>` | none | Create `pack.sha256` after green conformance, keep an identical pin unchanged, and reject a stale pin. | [Conflicts](#conflicts-and-combinations) |
| `--context-budget` | `<CONTEXT_BUDGET>` integer | `65536` | Set the approximate conversation compaction budget. | [Resolved defaults](#important-resolved-defaults) |
| `--model` | `<MODEL>` | `qwen3.6:27b-coding-nvfp4` | Set the executor model ID. | [Providers](providers.md) |
| `--provider` | `<PROVIDER>`: `ollama`, `lm-studio`, `openai`, or `gemini` | `ollama` | Select the executor provider. | [Providers](providers.md) |
| `--api` | `<chat-completions\|responses>` | `chat-completions` | Explicitly select the OpenAI-compatible API surface; model names never select it implicitly. | [Presets](configuration.md#presets) |
| `--tool-protocol` | `<native\|text>` | provider capability default | Explicitly select native function tools or the established text/XML tool protocol. | [Presets](configuration.md#presets) |
| `--prompt-layout` | `<stable\|legacy>` | `legacy` | Choose prompt section order for A/B measurement. | [Precedence](configuration.md#resolution-precedence) |
| `--plan-preset` | `<profile\|none>` | normally `none`; `profile` is selected for explicit `data` fix/investigate cases | Override planner-tier UltraPlan preset selection. `data/fix` can synthesize F1–F3 steps; `nextjs/fix` remains none-equivalent. | [Precedence](configuration.md#resolution-precedence) |
| `--intent` | `<create\|fix\|investigate>` | inferred from the goal | Force intent instead of goal-based resolution. | [Examples](#examples) |
| `--workflow` | `<PATH>` | none | Run a declarative workflow-circle definition. Mutually exclusive with `--intent`. | [Examples](#examples) |
| `--origin` | `<PATH>` | none | Supply the existing failed origin run workspace for `--workflow`. | [Examples](#examples) |
| `--planner-model` | `<PLANNER_MODEL>` | executor model when providers match | Set the planner model ID. Required when planner and executor providers differ. | [Provider roles](providers.md#provider-matrix) |
| `--planner-provider` | `<PLANNER_PROVIDER>`: `ollama`, `lm-studio`, `openai`, or `gemini` | executor provider | Select the planner provider. | [Provider roles](providers.md#provider-matrix) |
| `--prompt` | `<PROMPT>` | none | Run one minimal-loop prompt instead of entering the TUI. | [Examples](#examples) |
| `--plan-steps` | none | off | Generate and save a step plan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--plan-run` | none | off | Generate and run a step plan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--run-plan` | `<RUN_PLAN>` path | none | Run an existing step-plan YAML file. | [Action exclusivity](#conflicts-and-combinations) |
| `--ultra-plan` | none | off | Generate and save an UltraPlan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--ultra-plan-run` | none | off | Generate and run an UltraPlan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--run-ultra-plan` | `<RUN_ULTRA_PLAN>` path | none | Run an existing UltraPlan YAML file. | [Action exclusivity](#conflicts-and-combinations) |
| `--validate-plan` | `<PATH>` | none | Validate a step-plan or UltraPlan YAML file without executing it; errors include line and column numbers. | [Plan YAML editing](plan-yaml.md) |
| `--setup-interaction-probe` | none | off | Install or validate the managed Playwright interaction probe. | [Probe unavailable](troubleshooting.md#preflight-interaction-probe-unavailable) |
| `--runs` | none | off | List recent runs for the current workspace without creating provider clients. | [Slash `/runs`](slash-commands.md#command-reference) |
| `--ux-demo` | none | off | Run the offline presentation UX demo. | [Action exclusivity](#conflicts-and-combinations) |
| `--model-probe` | none | off | Run the bounded model behavior probe battery. | [Model probe](model-probe.md) |
| `--doctor` | none | off | Diagnose configuration files, provider readiness, interaction probes, and the local environment without making network requests. | [Slash `/doctor`](slash-commands.md#command-reference) |
| `--json` | none | off | Render `--doctor` output as stable machine-readable JSON. Requires `--doctor`. | [Slash `/doctor`](slash-commands.md#command-reference) |
| `--completions` | `<SHELL>`: `bash`, `elvish`, `fish`, `powershell`, `zsh` | none | Generate a completion script from the current Clap definition and write it to stdout. | [Shell completions and man page](#shell-completions-and-man-page) |
| `--generate-man` | none | off | Generate the `commandagent(1)` man page from the current Clap definition and write it to stdout. | [Shell completions and man page](#shell-completions-and-man-page) |
| `--init-config` | none | off | Create `.commandagent/config.toml` from a starter template without overwriting an existing file. | [Config template](#config-template) |
| `--validate-manifest` | `<PATH>` | none | Validate an external profile manifest without running it. | [Manifest v2](../../dev/profile-manifest.md) |
| `--init-profile` | `<ID>` | none | Initialize a draft profile manifest under `--extension-root`. | [Manifest v2](../../dev/profile-manifest.md) |
| `--profile` | `<PROFILE>` | inferred, then `generic` | Set a compiled profile or an external draft ID. An external ID requires the extension root that declares `profiles/<id>/manifest.toml`. | [Profile inference](slash-commands.md#profile-inference) |
| `--style` | `<STYLE>` | `default` | Pass the plan presentation/generation style. | [Inline flags](slash-commands.md#inline-flags) |
| `--resume` | `<RESUME>` | none | Load the named saved minimal-loop session for a direct `--prompt` run. | [Session options](#conflicts-and-combinations) |
| `--offline` | none | off | Block runtime dependency setup and Bash commands containing npm/pnpm/yarn/cargo install, curl, or wget. Provider/API requests and other network-capable commands are unaffected. | [Providers](providers.md) |
| `--quiet` | none | off (`narration = "normal"`) | Suppress presentation narration. | [Top-level keys](configuration.md#top-level-keys) |
| `--summary-json` | none | off | Append one machine-readable terminal run summary as the final stdout line. Omitting it preserves existing stdout bytes. | [Headless execution](../../user/headless.md) |
| `--ollama-host` | `<OLLAMA_HOST>` URL | `http://localhost:11434` | Set the Ollama server base URL used by CommandAgent. | [Ollama host](providers.md#ollama-host-and-models) |
| `--think` | `[=<true\|false\|low\|medium\|high>]` | omitted | Enable Ollama thinking for every Ollama provider role. A bare flag means `true`; explicit values require `=`, for example `--think=high`. | [Ollama thinking](providers.md#ollama-thinking) |
| `--lm-studio-host` | `<LM_STUDIO_HOST>` URL | `http://localhost:1234` | Set the LM Studio base URL; an optional trailing `/v1` is normalized. | [LM Studio server](providers.md#lm-studio-server-and-models) |
| `--num-predict` | `<NUM_PREDICT>` integer | `8192` | Set the maximum provider output-token request. | [Resolved defaults](#important-resolved-defaults) |
| `--max-iterations` | `<MAX_ITERATIONS>` integer | `12` | Set the minimal-loop iteration budget. | [Resolved defaults](#important-resolved-defaults) |
| `--chat-timeout-secs` | `<CHAT_TIMEOUT_SECS>` integer | `600` if either role uses Ollama or LM Studio; otherwise `180` | Set connect and whole-request timeouts for provider calls. | [Resolved defaults](#important-resolved-defaults) |
| `--chat-retries` | `<CHAT_RETRIES>` integer | `1` | Set retries after the initial provider attempt. | [Provider failures](troubleshooting.md#model-id-does-not-exist) |
| `--stream` | `<on\|off>` | on for the TUI, off for direct actions | Control visible executor and repair streaming; planner machine output stays hidden. Streaming still requires an interactive stdin and stdout TTY. | [Top-level keys](configuration.md#top-level-keys) |
| `--state-dir` | `<STATE_DIR>` path | `$XDG_STATE_HOME/anvilminimal`, otherwise `~/.local/state/anvilminimal` | Override saved session and REPL history storage. | [Paths](configuration.md#configuration-search-paths) |
| `--cwd` | `<CWD>` path | current directory | Set and canonicalize the active workspace before config discovery and execution. | [Paths](configuration.md#configuration-search-paths) |
| `--fresh-session` | none | off | Ignore `--resume` and create a session for a direct `--prompt` run. | [Session options](#conflicts-and-combinations) |
| `--footer` | `<on\|off>` | `on` | Control the fixed TUI footer; off keeps scrollback breadcrumbs. | [Footer problems](troubleshooting.md#footer-rendering-problems) |
| `--no-footer` | none | off | Disable the fixed TUI footer. Equivalent in effect to `--footer off`. | [Footer problems](troubleshooting.md#footer-rendering-problems) |

## Interactive REPL controls

Within one TUI session, `/model <id>` and `/provider <name>` change the
executor selection, while `/profile <name>` changes the explicit profile.
These settings apply to new Gate 1 cards and are shown by `/status`; an already
rendered card keeps its frozen identity. Use grouped `/help` or
`/help <command>` for runtime usage and examples. `/status` shows current
execution before the remaining session configuration, `/last` repeats the most
recent result, and `/clear` clears the terminal screen.

`/confirm` always accepts the full hash printed on the latest pending Gate 1
card. By default it also accepts a matching canonical `sha256:` prefix with at
least eight hexadecimal digits, such as `/confirm sha256:77cd5e23`. Set
`COMMANDAGENT_STRICT_CONFIRM=1` before starting CommandAgent to require the
full hash. Prefixes are expanded to the frozen full hash before confirmation is
persisted.

REPL history is isolated beneath `--state-dir` at
`workspace-history/<sha256-of-canonical-workspace>.txt`. Only the active
workspace file is loaded, and history hints require two entered characters and
fit on one terminal line. The former shared `<state-dir>/history.txt` is not
loaded, migrated, modified, or deleted, so existing history is preserved
without exposing it in another workspace.

## Defaults and precedence

Only values declared with a Clap default are fixed before configuration
resolution. Values such as model, provider, context budget, timeout, profile,
footer, and stream receive their effective defaults in `Config::from_cli`.
See [Configuration](configuration.md) for the exact per-field layers.

### Important resolved defaults

| Setting | Effective default |
| --- | --- |
| `num_predict` | `8192` |
| `max_iterations` | `12` |
| `chat_timeout_secs` | `600` seconds when either provider role is Ollama or LM Studio; `180` seconds when both are remote |
| `chat_retries` | `1` retry after the first attempt |
| `context_budget` | `65536` |

### Conflicts and combinations

- `--footer` and `--no-footer` are a Clap-level conflict and cannot be used
  together.
- `--allow` accepts `read`, `write`, and `bash:verify` as repeated or
  comma-separated values. Once supplied, omitted tool classes are blocked;
  `--yes` is the backward-compatible all-tools alias.
- Only one action selector may be used. This is checked after parsing and fails
  with `only one action selector can be used at a time`.
- `--packs`, `--pack-verify`, and `--pack-pin` are Clap-level direct actions.
  They conflict with one another, run action selectors, `--pack`, and
  `--pack-hash`. Listing allows `--extension-root`, while verify and pin take
  their target directory directly.
- `--plan-steps`, `--plan-run`, `--ultra-plan`, and `--ultra-plan-run` require a
  trailing goal.
- `--validate-plan` is an offline, read-only action and conflicts with every
  execution or generated-artifact action. It accepts step-plan, UltraPlan, and
  recovery UltraPlan YAML; see [Plan YAML editing](plan-yaml.md).
- A different `--planner-provider` requires an explicit or preset
  `planner_model`; otherwise startup fails.
- `--think` requires at least one resolved provider role to use Ollama. When
  both roles use another provider, startup fails instead of ignoring the flag.
- For direct minimal-loop prompts, `--fresh-session` takes precedence over
  `--resume`. These session switches are not used by slash-command plan resume.
- `--init-profile` requires an existing `--extension-root`. It creates a
  compact draft v2 manifest and refuses to overwrite an existing file.
- `--validate-manifest` checks a v1/v2 profile manifest or v1 overlay without
  registering or running it. Failures report one file, line, column, and
  reason.

## Config template

`--init-config` creates `.commandagent/config.toml` in the active workspace
(the current directory, or `--cwd`). The starter contains a complete `local`
preset with explicit provider, model, planner, classifier, budget, profile, and
display settings. Review the values before selecting it with `--preset local`.
An existing config is never changed:

```bash
commandagent --init-config
commandagent --preset local
```

## Shell completions and man page

Both interfaces generate from the current Clap command definition, so newly
added flags are included automatically. The completion registration delegates
each completion request to the installed binary. For `--model` and
`--planner-model`, that binary briefly queries the default Ollama `/api/tags`
and LM Studio `/v1/models` endpoints, merges their model IDs, and produces no
warning when either endpoint is unavailable. Generated artifacts write only to
stdout; redirect the output to a user-owned installation path and regenerate
it after updating CommandAgent.

`scripts/setup.sh` offers to install a completion for the detected Bash, Zsh,
or Fish shell. For manual installation, use the appropriate command below.

For Bash with the standard per-user `bash-completion` directory:

```bash
completion_dir="${BASH_COMPLETION_USER_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion}/completions"
mkdir -p "$completion_dir"
commandagent --completions bash > "$completion_dir/commandagent"
```

For Zsh, place the `_commandagent` function in a directory on `fpath`, then
initialize completion:

```zsh
completion_dir="${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"
mkdir -p "$completion_dir"
commandagent --completions zsh > "$completion_dir/_commandagent"
fpath=("$completion_dir" $fpath)
autoload -Uz compinit && compinit
```

Persist the last two lines in `.zshrc`. For Fish:

```fish
set completion_dir (string join / (set -q XDG_CONFIG_HOME; and echo $XDG_CONFIG_HOME; or echo $HOME/.config) fish completions)
mkdir -p $completion_dir
commandagent --completions fish > $completion_dir/commandagent.fish
```

Fish loads that path automatically. For PowerShell, save the generated script
and dot-source it from the current session or from `$PROFILE`:

```powershell
commandagent --completions powershell > "$HOME/.commandagent-completion.ps1"
. "$HOME/.commandagent-completion.ps1"
```

Elvish generation is also available through `commandagent --completions
elvish`; load or store that output according to your Elvish configuration.

To install the generated man page in the common per-user location:

```bash
man_dir="${XDG_DATA_HOME:-$HOME/.local/share}/man/man1"
mkdir -p "$man_dir"
commandagent --generate-man > "$man_dir/commandagent.1"
man -l "$man_dir/commandagent.1"
```

Add the parent `man` directory to `MANPATH` if you want `man commandagent` to
discover it without an explicit path.

## Examples

```bash
# Start the interactive TUI with defaults.
commandagent

# Run one executor prompt against a cloud provider.
commandagent --provider gemini --model gemini-model-id \
  --prompt "Explain the current workspace"

# Generate and run an UltraPlan with explicit roles and profile.
commandagent --provider ollama --model local-executor \
  --planner-provider openai --planner-model openai-model-id \
  --profile nextjs --ultra-plan-run Build a Next.js app on port 3011

# Disable the fixed footer when the terminal renders it badly.
commandagent --footer off
```
