# Issue 217 implementation summary

Implemented the Epic #260 Lane D combined CLI change for Issues #217, #219,
and #225, plus the delegated argument surfaces from #248.

## Help and documentation contract

- Replaced the obsolete MVP command summary with the package's local-first,
  verified-workflow description.
- Added help text to every public application flag and grouped the rendered
  help into Actions, Models and Providers, Planning and Verification,
  Workspace and State, and Display sections.
- Kept the Issue #206 `--yes` workspace-confinement warning while synchronizing
  every English Description cell exactly with its Clap help string.
- Updated the English and Japanese CLI references and the guide index for 57
  public flags.
- Extended `tests/doc_drift.rs` to reject missing help, description drift,
  bilingual flag-set drift, and advertised-count drift.

## Validation and discoverability

- Added Clap-level positive-integer validation for `--max-iterations` and
  `--chat-timeout-secs`; zero now exits as a parse error before configuration,
  provider calls, or recovery artifacts.
- Enabled clap_complete's dynamic engine and attached bounded local-model
  completion to `--model` and `--planner-model`.
- Dynamic completion merges, sorts, deduplicates, and prefix-filters Ollama
  `/api/tags` and LM Studio `/v1/models` IDs. Unreachable endpoints, bad HTTP
  statuses, and invalid responses silently produce no candidates.
- Preserved `--completions <SHELL>` for Bash, Elvish, Fish, PowerShell, and Zsh;
  it now emits a registration that delegates to the exact generating binary.
  `--generate-man` remains generated from the same Clap definition.

## Config and delegated manifest surfaces

- Added `--init-config`, which creates a complete starter
  `.commandagent/config.toml` in the current or `--cwd` workspace. Creation is
  private on Unix and uses create-new semantics, so an existing config is never
  overwritten.
- Added parsing, help, conflicts, and docs for
  `--validate-manifest <PATH>` and
  `--init-profile <ID> --extension-root <DIR>`.
- Did not implement manifest validation/profile initialization and did not edit
  Lane I loader, schema, manifest, or corpus files.

## Compatibility and predecessor

- The required #234 predecessor was already present at `f848084e` before the
  design note or implementation edits; its `Config` initializer remains
  unchanged.
- Existing public flag spellings/defaults, hidden integration arguments,
  action implementations, event schemas, historical evidence, and `.anvil/`
  runtime state are unchanged.
- No corpus fixture changed because this work changes only Clap parsing,
  generated CLI artifacts, documentation, and a new pre-runtime config action;
  no event, recovery, or corpus contract changed.
