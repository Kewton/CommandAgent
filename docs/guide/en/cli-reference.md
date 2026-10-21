# CLI Reference

[日本語](../ja/cli-reference.md) | [Guide index](../README.md)

`commandagent [OPTIONS] [GOAL]...` starts the interactive TUI when no action is
selected. A trailing goal can contain multiple words without quoting because it
is collected as the final argument list. The installed binary remains the
authority: use `commandagent --help` when its version differs from this checkout.

## Invocation

Use one of the action-selector flags for a direct command, or omit all of them
for the TUI. The action selectors are `--workflow`, `--prompt`, `--plan-steps`,
`--plan-run`, `--run-plan`, `--ultra-plan`, `--ultra-plan-run`,
`--run-ultra-plan`, `--validate-plan`, `--setup-interaction-probe`, `--runs`,
`--ux-demo`, `--model-probe`, `--doctor`, and `--extensions`. The offline pack
actions `--packs`, `--pack-verify`, and `--pack-pin`, generated-artifact actions
`--completions` and `--generate-man`, config action `--init-config`, and
delegated manifest actions `--validate-manifest` and `--init-profile` are
displayed in the same `Actions (use one)` help group. CommandAgent rejects
combinations whose action contracts are mutually exclusive, either in Clap
before parsing finishes or in the resolved configuration after parsing; see
[Conflicts and combinations](#conflicts-and-combinations).

Clap also generates `-h`/`--help` and `-V`/`--version`. They are not part of the
67 application flags below. The hidden `--completion-contract-json <PATH>` is an
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
| `--extensions` | none | off | Inspect profiles, overlays, packs, usability reasons, and the latest journal record under the extension root. | [Conflicts](#conflicts-and-combinations) |
| `--packs` | none | off | List compatible admitted packs and conformant packs found under `--extension-root`, including each source. Requires `--profile` and `--intent`. | [Conflicts](#conflicts-and-combinations) |
| `--pack-verify` | `<DIR>` | none | Run strict conformance for one pack directory and print the same JSON report as `pack_conformance`. | [Conflicts](#conflicts-and-combinations) |
| `--pack-pin` | `<DIR>` | none | Create `pack.sha256` after green conformance, keep an identical pin unchanged, and reject a stale pin. | [Conflicts](#conflicts-and-combinations) |
| `--context-budget` | `<CONTEXT_BUDGET>` integer | `65536` | Set the approximate conversation compaction budget. | [Resolved defaults](#important-resolved-defaults) |
| `--model` | `<MODEL>` | `qwen3.6:27b-coding-nvfp4` | Set the executor model ID. | [Providers](providers.md) |
| `--provider` | `<PROVIDER>`: `ollama`, `lm-studio`, `openai`, `gemini`, or `openai-compatible` | `ollama` | Select the executor provider. | [Providers](providers.md) |
| `--base-url` | `<URL>` | none | Set the base URL for the generic OpenAI-compatible provider; an optional trailing `/v1` is normalized. | [OpenAI-compatible servers](providers.md#generic-openai-compatible-servers) |
| `--api-key-env` | `<NAME>` | none | Read an optional OpenAI-compatible bearer token from this process-environment variable. | [OpenAI-compatible servers](providers.md#generic-openai-compatible-servers) |
| `--api` | `<chat-completions\|responses>` | `chat-completions` | Explicitly select the OpenAI-compatible API surface; model names never select it implicitly. | [Presets](configuration.md#presets) |
| `--tool-protocol` | `<native\|text>` | provider capability default | Explicitly select native function tools or the established text/XML tool protocol. | [Presets](configuration.md#presets) |
| `--prompt-layout` | `<stable\|legacy>` | `legacy` | Choose prompt section order for A/B measurement. | [Precedence](configuration.md#resolution-precedence) |
| `--plan-preset` | `<profile\|none>` | normally `none`; `profile` is selected for explicit `data` fix/investigate cases | Override planner-tier UltraPlan preset selection. `data/fix` can synthesize F1–F3 steps; `nextjs/fix` remains none-equivalent. | [Precedence](configuration.md#resolution-precedence) |
| `--intent` | `<create\|fix\|investigate>` | inferred from the goal | Force intent instead of goal-based resolution. | [Examples](#examples) |
| `--workflow` | `<PATH>` | none | Run a declarative workflow-circle definition. Mutually exclusive with `--intent`. | [Examples](#examples) |
| `--origin` | `<PATH>` | none | Supply the existing failed origin run workspace for `--workflow`. | [Examples](#examples) |
| `--planner-model` | `<PLANNER_MODEL>` | executor model when providers match | Set the planner model ID. Required when planner and executor providers differ. | [Provider roles](providers.md#provider-matrix) |
| `--planner-provider` | `<PLANNER_PROVIDER>`: `ollama`, `lm-studio`, `openai`, `gemini`, or `openai-compatible` | executor provider | Select the planner provider. | [Provider roles](providers.md#provider-matrix) |
| `--prompt` | `<PROMPT>` | none | Run one minimal-loop prompt instead of entering the TUI. | [Examples](#examples) |
| `--plan-steps` | none | off | Generate and save a step plan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--plan-run` | none | off | Generate and run a step plan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--run-plan` | `<RUN_PLAN>` path | none | Run an existing step-plan YAML file. | [Action exclusivity](#conflicts-and-combinations) |
| `--ultra-plan` | none | off | Generate and save an UltraPlan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--ultra-plan-run` | none | off | Generate and run an UltraPlan for the trailing goal. | [Action exclusivity](#conflicts-and-combinations) |
| `--run-ultra-plan` | `<RUN_ULTRA_PLAN>` path | none | Run an existing UltraPlan YAML file. | [Action exclusivity](#conflicts-and-combinations) |
| `--validate-plan` | `<PATH>` | none | Validate a step-plan or UltraPlan YAML file without executing it; errors include line and column numbers. | [Plan YAML editing](plan-yaml.md) |
| `--setup-interaction-probe` | none | off | Install or validate the managed Playwright interaction probe. | [Probe unavailable](troubleshooting.md#preflight-interaction-probe-unavailable) |
| `--runs` | optional `<ID>` | off | List recent runs, or show one run by ID, without creating provider clients. | [Slash `/runs`](slash-commands.md#command-reference) |
| `--events` | none | off | Show the selected run's events in chronological order. Requires `--runs <ID>`. | [Troubleshooting](troubleshooting.md) |
| `--filter` | `<phase\|tool\|provider>` | none | Filter a selected run's events by phase, tool, or provider. | [Troubleshooting](troubleshooting.md) |
| `--ux-demo` | none | off | Run the offline presentation UX demo. | [Action exclusivity](#conflicts-and-combinations) |
| `--model-probe` | none | off | Run the bounded model behavior probe battery. | [Model probe](model-probe.md) |
| `--doctor` | none | off | Diagnose configuration files, provider readiness, interaction probes, and the local environment. Depending on the configured provider, reachability checks connect to that host (for example OpenAI /v1/models or Responses, Ollama, or LM Studio); a check is skipped when its API key is absent. | [Slash `/doctor`](slash-commands.md#command-reference) |
| `--json` | none | off | Render --doctor, --extensions, or --runs output as stable machine-readable JSON. | [Slash `/doctor`](slash-commands.md#command-reference) |
| `--completions` | `<SHELL>`: `bash`, `elvish`, `fish`, `powershell`, `zsh` | none | Generate a completion script from the current Clap definition and write it to stdout. | [Shell completions and man page](#shell-completions-and-man-page) |
| `--generate-man` | none | off | Generate the `commandagent(1)` man page from the current Clap definition and write it to stdout. | [Shell completions and man page](#shell-completions-and-man-page) |
| `--init-config` | none | off | Create `.commandagent/config.toml` from a starter template without overwriting an existing file. | [Config template](#config-template) |
| `--validate-manifest` | `<PATH>` | none | Validate an external profile manifest without running it. | [Manifest v2](../../dev/profile-manifest.md) |
| `--init-profile` | `<ID>` | none | Initialize a draft profile manifest under `--extension-root`. | [Manifest v2](../../dev/profile-manifest.md) |
| `--profile` | `<PROFILE>` | inferred, then `generic` | Set a compiled profile or an external draft ID. An external ID requires the extension root that declares `profiles/<id>/manifest.toml`. | [Profile inference](slash-commands.md#profile-inference) |
| `--style` | `<STYLE>` | `default` | Pass the plan presentation/generation style. | [Inline flags](slash-commands.md#inline-flags) |
| `--resume` | `<RESUME>` | none | Load the named saved minimal-loop session for a direct `--prompt` run. | [Session options](#conflicts-and-combinations) |
| `--offline` | none | off | Refuse runtime dependency setup and Bash commands containing npm/pnpm/yarn/cargo install, curl, or wget (lexical check, always). When --sandbox applies an OS backend, the OS also refuses outbound network connections from sandboxed commands: macOS allows only loopback (TCP and UDP) and unix sockets; Linux needs Landlock ABI4 (kernel 6.7) or later and then allows TCP only to the dev-server port(s) without restricting UDP, while below ABI4 the network is not confined and only the lexical check applies. Without a backend only the lexical check applies. Provider/API requests are unaffected. | [OS write, read, and network sandbox](#os-write-read-and-network-sandbox) |
| `--sandbox` | `<auto\|required\|off>` | `auto` | Select the OS write sandbox for Bash and verifier commands. `auto` applies it when the backend is available and otherwise continues with lexical checks only; `required` fails those commands when the backend is unavailable; `off` keeps the previous behavior. | [OS write, read, and network sandbox](#os-write-read-and-network-sandbox) |
| `--quiet` | none | off (`narration = "normal"`) | Suppress presentation narration. | [Top-level keys](configuration.md#top-level-keys) |
| `--summary-json` | none | off | Append one machine-readable terminal run summary as the final stdout line. Omitting it preserves existing stdout bytes. | [Headless execution](../../user/headless.md) |
| `--trace` | none | off | Opt in to scrubbed provider request and response traces under the active run directory. | [Troubleshooting](troubleshooting.md) |
| `--ollama-host` | `<OLLAMA_HOST>` URL | `http://localhost:11434` | Set the Ollama server base URL used by CommandAgent. | [Ollama host](providers.md#ollama-host-and-models) |
| `--think` | `[=<true\|false\|low\|medium\|high>]` | omitted | Enable Ollama thinking for every Ollama provider role. A bare flag means `true`; explicit values require `=`, for example `--think=high`. | [Ollama thinking](providers.md#ollama-thinking) |
| `--lm-studio-host` | `<LM_STUDIO_HOST>` URL | `http://localhost:1234` | Set the LM Studio base URL; an optional trailing `/v1` is normalized. | [LM Studio server](providers.md#lm-studio-server-and-models) |
| `--num-predict` | `<NUM_PREDICT>` integer | `8192` | Set the maximum provider output-token request. | [Resolved defaults](#important-resolved-defaults) |
| `--max-iterations` | `<MAX_ITERATIONS>` integer | `12` | Set the minimal-loop iteration budget. | [Resolved defaults](#important-resolved-defaults) |
| `--recovery-plan-auto-runs` | `<0..20>` integer | `0` | Automatically execute at most this many validated Recovery Plans after a failed UltraPlan execution, including direct actions, matching REPL commands, and resume (0 disables; total plan executions are at most 1 + this value). | [Plan YAML](plan-yaml.md) |
| `--chat-timeout-secs` | `<CHAT_TIMEOUT_SECS>` integer | `600` if either role uses Ollama or LM Studio; otherwise `180` | Set connect and whole-request timeouts for provider calls. | [Resolved defaults](#important-resolved-defaults) |
| `--chat-retries` | `<CHAT_RETRIES>` integer | `1` | Set retries after the initial provider attempt. | [Provider failures](troubleshooting.md#model-id-does-not-exist) |
| `--stream` | `<on\|off>` | on for the TUI, off for direct actions | Control visible executor and repair streaming; planner machine output stays hidden. Streaming still requires an interactive stdin and stdout TTY. | [Top-level keys](configuration.md#top-level-keys) |
| `--state-dir` | `<STATE_DIR>` path | `$XDG_STATE_HOME/commandagent`, otherwise `~/.local/state/commandagent` | Override saved session and REPL history storage; the default loader retains the legacy `anvilminimal` fallback. | [Paths](configuration.md#configuration-search-paths) |
| `--cwd` | `<CWD>` path | current directory | Set and canonicalize the active workspace before config discovery and execution. | [Paths](configuration.md#configuration-search-paths) |
| `--fresh-session` | none | off | Ignore `--resume` and create a session for a direct `--prompt` run. | [Session options](#conflicts-and-combinations) |
| `--footer` | `<on\|off>` | `on` | Control the fixed TUI footer; off keeps scrollback breadcrumbs. | [Footer problems](troubleshooting.md#footer-rendering-problems) |
| `--no-footer` | none | off | Disable the fixed TUI footer. Equivalent in effect to `--footer off`. | [Footer problems](troubleshooting.md#footer-rendering-problems) |

## OS write, read, and network sandbox

The `--sandbox auto|required|off` flag (default `auto`) controls an OS-level
write sandbox, a home-credential read sandbox, and (for `--offline`) a network
sandbox for the model Bash tool, verification/acceptance commands, and the
runtime dependency setup (the Python `sh -c` and the `npm install`). The lexical
checks still run first; the sandbox adds confinement for the writes, reads, and
(with `--offline`) network the lexical guard cannot see.

- **What the OS closes (when the backend is available):** writes performed inside
  a command or script — `sh -c 'cp ...'`, `eval`, `xargs`, `find -exec`, an
  interpreter's `open(..., 'w')`, `dd of=`, `sed -i`, and an install-time script
  such as `setup.py` or an npm lifecycle script — and hardlinks to an inode
  outside the workspace. Only the workspace, the run's temp directory, the known
  dependency caches, and device files such as `/dev/null` are writable.
- **Read confinement:** reading the home secret locations (`.ssh`, `.aws`,
  `.gnupg`, `.netrc`, `.git-credentials`, `.npmrc`, `.pypirc`,
  `.docker/config.json`, `.kube`, `.config/gh`, `.config/gcloud`) is denied, and
  on macOS a workspace credential named by the #501 rule is denied too; the three
  strict dotenv templates stay readable. This closes the reads the lexical guard
  cannot see: a credential through a non-secret symlink, a file named by an
  environment variable such as `BASH_ENV`, and a name an interpreter assembles at
  run time when that assembled name is itself a denied name. On Linux only the
  home secret locations are denied, so a non-secret symlink to a workspace
  credential is not closed by the read layer. Denying the home npm config means
  an `npm` that reads a private-registry `~/.npmrc` fails inside Bash. On Linux
  the deny list becomes a read-allow list rebuilt at each command's start, so a
  location created after a command starts is not readable inside that command;
  `ls /`, `ls $HOME`, and `echo $HOME/*` fail by design. When a writable location
  (for example the workspace) is the home or an ancestor of a home secret the
  read layer cannot deny those secrets, and the status says the read isolation is
  not effective instead of claiming it is applied; an empty `HOME` is reported
  the same way.
- **Network (`--offline` only):** when the backend is applied, an `--offline`
  run has its outbound connections refused by the OS, so an arbitrary program the
  lexical guard cannot name (`python -c "urllib..."`, `node -e "fetch(...)"`)
  cannot reach the network. macOS denies every network operation except loopback
  (TCP and UDP) and unix sockets. Linux (Landlock ABI4, kernel 6.7) denies every TCP
  connect except the run's dev-server port(s): Landlock matches the port only, so
  the same port on an external host still connects and UDP is not restricted. When
  the dev-server port can be taken neither from the goal nor from the profile,
  Linux refuses every TCP connect, localhost included. Whether Landlock's TCP
  rule also covers a Multipath TCP (MPTCP, `IPPROTO_MPTCP`) socket's connect
  depends on the kernel; on a kernel where it does not, an MPTCP connect goes
  through, a limit of the same kind as UDP (CI records the observed result as the
  `issue672/mptcp` probe verdict). Below ABI4 the network is not confined — the
  `bash_sandbox_status` reason says so. A run that is not `--offline` is
  unchanged.
- **What stays lexical-only:** every command run when the backend is unavailable.
  `--yes`, `--allow bash:verify`, and `--offline` do not close an indirect write
  or a home-secret read by themselves; only the sandbox, when applied, does.
  `--allow bash:verify` admits a command by its classification only, so the
  contents of any script it invokes are limited by the sandbox alone. With `auto`
  and an unavailable backend the dependency setup runs outside the sandbox as
  before and the status records it as not applied; with `required` it fails
  honestly.
- **Modes:** `auto` uses the backend when available and otherwise warns once and
  records `bash_sandbox_status` with `applied=false` and a reason, continuing
  with lexical checks only; `required` fails Bash and verifier commands with
  `bash_sandbox_unavailable`; `off` keeps the previous behavior. The read
  isolation is reported through the same event's `reason` (it names the applied
  read isolation, or is empty for a fully confined sandbox). On macOS the
  backend is Seatbelt and is unavailable under a nested sandbox (`rc=71`); on
  Linux it is Landlock and is unavailable below ABI1 (kernel 5.13), when
  disabled, or when a seccomp filter denies it.
- Dependency caches (cargo, npm, pip, Playwright browsers) stay writable so
  builds and tests work; they remain a contamination path outside the workspace.

macOS `mktemp` ignores `TMPDIR`; use `mktemp -p "$TMPDIR"` or a template under
`$TMPDIR`. The applied write-probe record is under
`tests/corpus/apps/issue502-indirect-write/fixtures/macos-probe-results.md`; the
applied read-probe record is under
`tests/corpus/apps/issue502-indirect-read/fixtures/macos-probe-results.md`.

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

`/provider` switches only among the four built-in providers `ollama`,
`lm-studio`, `openai`, and `gemini`. The generic `openai-compatible` provider,
including its base URL and key variable, is configured before startup with
`--provider`, `--base-url`, and `--api-key-env`, or a preset; change it there
and restart CommandAgent instead of using `/provider`. When the executor
provider is OpenAI, `/model <id>` and a `/provider openai` switch reject the
ambiguous `gpt-5.6` alias with the same check the CLI applies at startup.

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
- Clap rejects some combinations while parsing: `--workflow` conflicts with
  `--intent`; `--origin` requires `--workflow`; the `--doctor`/`--extensions`/
  `--runs` JSON group and the pack direct-action group accept only one member
  each. For example, `--doctor --runs` is a Clap-level conflict.
- After parsing, the resolved configuration accepts exactly one action selector
  and fails with `only one action selector can be used at a time`. For example,
  `--prompt x --ux-demo` reaches that check because each flag is a valid
  selector on its own. `--workflow` without `--origin` also fails here, with
  `--workflow requires --origin`.
- `--completions` and `--generate-man` are Clap-level conflicts with every other
  action selector and with a trailing goal, so a combined invocation is rejected
  before generation begins.
- Pre-run argument and configuration rejections exit `2` before a run starts and
  produce no run summary; see [headless execution](../../user/headless.md).
  `--doctor` is the exception: it follows its diagnostic path, records the
  failed Configuration check in its report (including JSON), and exits `1`
  instead of `2`. For example, `--ux-demo --doctor --json` exits `1` with the
  JSON report rather than the usual exit `2`.
- `--packs`, `--pack-verify`, and `--pack-pin` are Clap-level direct actions.
  They conflict with one another, run action selectors, `--pack`, and
  `--pack-hash`. Listing allows `--extension-root`, while verify and pin take
  their target directory directly.
- `--extensions` is read-only and accepts `--extension-root`; when omitted,
  it resolves the top-level `extension_root` setting. `--json` applies to
  `--doctor`, `--extensions`, and `--runs`; using `--json` without one of those
  targets is rejected.
- `--plan-steps`, `--plan-run`, `--ultra-plan`, and `--ultra-plan-run` require a
  trailing goal.
- `--validate-plan` is an offline, read-only action and conflicts with every
  execution or generated-artifact action. It accepts step-plan, UltraPlan, and
  recovery UltraPlan YAML; see [Plan YAML editing](plan-yaml.md).
- A different `--planner-provider` requires an explicit or preset
  `planner_model`; otherwise startup fails.
- A role that resolves to the generic provider needs its base URL: pass
  `--base-url` or set preset `base_url` when the executor, planner, or
  classifier is `openai-compatible`, including a preset that sets only the
  classifier to the generic provider. With no `openai-compatible` role,
  `--base-url` and `--api-key-env` are rejected by the resolved configuration.
  These are not Clap `requires` relationships.
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
