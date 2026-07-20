# Issue 27 implementation summary

## Outcome

CommandAgent now generates shell completion scripts and its man page directly
from the live Clap definition without creating files implicitly or committing
generated artifacts.

## Changes

- Added `clap_complete` and `clap_mangen` as locked runtime dependencies.
- Added `--completions <SHELL>` with Clap-validated Bash, Elvish, Fish,
  PowerShell, and Zsh values. Invalid names use Clap's standard error with the
  accepted values.
- Added `--generate-man` using acceptance option (a). It writes the generated
  `commandagent(1)` source to stdout; this was chosen to keep generation
  available in every binary and independent of release workflow details.
- Added `src/cli_artifacts.rs` as a leaf generator module and short-circuited
  artifact requests before configuration, preflight, providers, and runtime
  state. Completion and man requests conflict at parse time.
- Added process-level coverage for help visibility, all accepted shells,
  invalid input, representative generated content, man output, stderr behavior,
  and no working-directory file writes.
- Extended `scripts/setup.sh` to install a detected Bash, Zsh, or Fish
  completion in a user-owned XDG/default path. Interactive runs confirm first;
  `--yes` installs idempotently; Zsh receives explicit `fpath`/`compinit`
  guidance; unsupported shells receive the manual guide path.
- Updated the English and Japanese CLI references with both public flags and
  Bash, Zsh, Fish, PowerShell, Elvish, and man-page installation guidance.
- Kept all completion scripts and `commandagent.1` generated-only; none are
  tracked in the repository.

## Predecessors

Before implementation, the branch was fast-forwarded to the completed Issue 26
tip, which contains the required Issue 19, 20, 22, 24, 25, and 26 predecessor
chain. This supplied the setup script, bilingual guide, documentation drift
guard, doctor command, and release distribution surfaces used by Issue 27.
