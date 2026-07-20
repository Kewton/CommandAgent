# Issue 27 design: generated shell completions and man page

## Context

CommandAgent exposes a large flat Clap CLI, but currently offers only
`--help` as an option-discovery mechanism. The completed predecessor chain is
now integrated: it provides the bilingual CLI reference, its documentation
drift guard, and `scripts/setup.sh`, so this change must update those surfaces
along with the CLI.

## Scope

- Add `--completions <SHELL>` for Bash, Zsh, Fish, PowerShell, and the Elvish
  variant exposed by `clap_complete`, generated from the live Clap command
  definition and written only to stdout.
- Add `--generate-man`, generated from the same command definition with
  `clap_mangen` and written only to stdout.
- Add focused process-level tests for help, supported and invalid shell names,
  generated content, stdout/stderr behavior, and the absence of implicit file
  writes.
- Extend the setup script with a user-local completion installation step for
  the detected Bash, Zsh, or Fish shell, plus focused fixture coverage.
- Update the English and Japanese CLI references with both flags and manual
  installation instructions.

## Design

`Cli` remains the single command schema. It gains a typed
`clap_complete::Shell` value and a boolean man-generation flag, with Clap
rejecting an invocation that requests both artifacts. A small leaf module will
build `Cli::command()` and stream either generated artifact to a supplied
writer. `commandagent::run` will dispatch these output-only requests before
configuration loading, preflight checks, provider setup, or runtime-state
creation.

The man-page approach is acceptance option (a), `--generate-man` to stdout.
This was selected because it mirrors the completion interface, is available in
every built binary without coupling to release automation, and leaves the
choice of installation path to the caller. Neither generated completion files
nor `commandagent.1` will be committed.

`scripts/setup.sh` will detect the basename of `$SHELL` and offer one
user-owned target:

- Bash: the XDG/bash-completion user completions directory.
- Zsh: an XDG data `site-functions/_commandagent` file, with the required
  `fpath`/`compinit` snippet printed after installation.
- Fish: the XDG config `fish/completions/commandagent.fish` path.

Interactive mode asks before writing. `--yes` installs the detected
completion as a safe default. Unsupported or unknown shells get manual guide
directions and no file mutation. `--check-only` returns before this step, as it
does for every existing mutation.

## Verification

Run the new CLI-artifact integration test and the setup-script test first,
followed by the documentation drift test. Because this changes shared CLI and
setup contracts, also run Bash syntax checking, ShellCheck, formatting, Clippy,
and the complete Rust test suite.

## Risks and mitigations

- Schema drift: both generators construct output directly from
  `Cli::command()`, and process tests assert representative current flags.
- Accidental runtime side effects: artifact dispatch occurs before
  `Config::from_cli`; a temporary-directory test asserts that generation leaves
  the working directory empty.
- Shell setup portability: use user-owned XDG paths, portable Bash constructs,
  and the existing fake-command setup harness; print the Zsh `fpath` step
  explicitly rather than editing shell startup files.
- Generated-file drift: no generated artifact is checked in; tests generate
  fresh output for every run.
