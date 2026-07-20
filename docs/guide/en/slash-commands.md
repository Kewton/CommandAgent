# Slash Commands

[日本語](../ja/slash-commands.md) | [Guide index](../README.md)

Enter slash commands at the interactive `commandagent>` prompt. `/help` is
rendered from the same command registry used for dispatch, so it is the runtime
source of truth for the installed binary.

## Command reference

The registry contains 15 primary entries. `/quit` is a separately accepted
command name and an alias of `/exit`, giving 16 accepted names in total.

| Command name | Usage shown by `/help` | Behavior |
| --- | --- | --- |
| `/help` | `/help` | Show the command list, footer hint, queued-input limits, and interrupt behavior. |
| `/status` | `/status` | Show effective configuration and provider readiness. |
| `/doctor` | `/doctor` | Diagnose configuration files, provider readiness, interaction probes, and the local environment without making network requests. |
| `/runs` | `/runs` | List recent workspace runs and recovery availability. |
| `/resume` | `/resume [run-id\|yaml-path]` | Prepare and, after confirmation, resume a recovery UltraPlan. An empty argument selects the latest recoverable run when available. |
| `/plan` | `/plan` | Show the active plan and current activity. |
| `/plan-steps` | `/plan-steps <goal>` | Generate and save a step plan; prints its path. |
| `/plan-run` | `/plan-run <goal>` | Generate and run a step plan. |
| `/run-plan` | `/run-plan <path>` | Run an existing step-plan YAML file. |
| `/ultra-plan` | `/ultra-plan <goal>` | Generate and save an UltraPlan; prints its path. |
| `/ultra-plan-run` | `/ultra-plan-run <goal>` | Generate and run an UltraPlan. |
| `/run-ultra-plan` | `/run-ultra-plan <path>` | Run an existing UltraPlan YAML file. |
| `/setup-interaction-probe` | `/setup-interaction-probe` | Install or validate the managed Playwright interaction readiness probe. |
| `/model-probe` | `/model-probe` | Run the bounded model behavior probe battery. See [Model Behavior Probe](../../model-probe.md). |
| `/exit` | `/exit or /quit` | Leave the TUI. |
| `/quit` | `/exit or /quit` | Alias of `/exit`; leave the TUI. |

Unknown commands fail with `unknown slash command: <name>; type /help for
commands`. Planning command failures are reported without leaving the REPL.

## Inline flags

Slash input recognizes these overrides before it joins the remaining words into
the goal:

| Inline flag | Values | Effect |
| --- | --- | --- |
| `--profile <PROFILE>` | a profile name | Overrides the configured profile for this command and disables automatic profile inference. |
| `--style <STYLE>` | a style name | Overrides the configured style for this command. |
| `--prompt-layout <stable\|legacy>` | `stable` or `legacy` | Overrides prompt section order for this command. |

They are useful on commands that execute or generate plans. They are parsed for
all slash input, but discovery commands such as `/help` and `/status` return
before applying them.

```text
/plan-run --profile nextjs --style compact "Build an app on port 3011"
/ultra-plan-run --prompt-layout stable Improve the current project
```

## Goal text and quoting

The parser splits on spaces and tabs except inside double quotes, removes the
double-quote delimiters, and joins the remaining words with a single space.
Single quotes and shell escape syntax have no special meaning. This is a small
command parser, not a shell.

For action commands, put inline flags before or among goal words. A flag token
without a following value is left in the goal rather than treated as an
override. Quote a phrase when preserving its internal grouping makes the input
clearer.

## `$(cat <path>)` expansion

After parsing inline flags, CommandAgent repeatedly replaces literal
`$(cat <path>)` forms in the goal with the UTF-8 contents of the referenced
file. The path is resolved by the workspace path guard: relative paths are
rooted at the active workspace, and paths that escape it are rejected. This is
an internal expansion; no shell is started and no other command substitution is
supported.

```text
/ultra-plan-run --profile nextjs "$(cat prompts/site-goal.txt)"
```

Multiple forms are expanded from left to right. If an opening `$(cat ` has no
closing `)`, it remains ordinary goal text. Missing, disallowed, or non-UTF-8
files cause the slash command to fail.

## Profile inference

When neither the CLI nor a selected preset made the profile explicit, each
slash command can infer a profile from its expanded goal and workspace:

1. A Next.js goal token selects `nextjs`.
2. Otherwise, a Python CLI goal token selects `python-cli`.
3. Otherwise, a `package.json` dependency or devDependency named `next` selects
   `nextjs`.
4. Otherwise, the presence of `pyproject.toml` selects `python-cli`.
5. Otherwise, the configured fallback (normally `generic`) remains.

Goal evidence takes precedence over workspace evidence. An inline
`--profile`, CLI `--profile`, or preset `profile` disables inference, including
when the explicit value is `generic`. The `data` profile is not inferred by
this routine and must be selected explicitly.

## TUI notes

- Enter queues input typed while a command is running: at most 10 lines, each at
  most 4096 bytes. Backspace edits pending input.
- Ctrl-C interrupts. Esc clears non-empty pending input; otherwise it
  interrupts. Repeating an interrupt force-finalizes.
- `/plan-run` and `/ultra-plan-run` run the Next.js busy-port and interaction-
  probe preflight when the effective profile is `nextjs`.
- `/resume` checks workspace drift and asks for confirmation unless `--yes` was
  used to start the TUI.
- `/exit` and `/quit` are recognized only as the complete trimmed input.
