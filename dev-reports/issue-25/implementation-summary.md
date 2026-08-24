# Issue 25 implementation summary

## Outcome

Implemented a built-in, non-repairing environment doctor available as both
`commandagent --doctor` and the TUI `/doctor` command. The CLI supports
`--doctor --json`; failures produce a nonzero exit while warning-only reports
remain successful.

## Runtime changes

- Added the `--doctor` action selector and a `--json` flag that requires it.
- Added `Action::Doctor` to the existing exclusive action-selector contract.
- Routed direct doctor execution before ordinary startup resolution. This lets
  the doctor emit a complete report when config parsing or preset resolution is
  itself the failure.
- Added `src/doctor.rs` with a stable JSON schema (`schema_version = "1"`) and
  aligned human checklist rendering using `✓`, `!`, and `✗`.
- Added `/doctor` to the slash registry, `/help`, and editor completion. The
  command returns the same human report without invoking planner or executor
  model calls.

## Checks implemented

- Effective model, provider, planner model, planner provider, and profile,
  including normalized CLI/preset/config/default source plus the original
  `field_sources` detail.
- Every searched workspace/home `.commandagent/config.toml` and
  `.anvil/config.toml` path, with existence and parse status.
- Selected-preset presence and completeness. If resolution fails, the report
  lists the exact missing keys from the shared `preset_complete` definition.
- Ollama `/api/tags` reachability through the configured host with a two-second
  timeout, followed by executor/planner model membership checks.
- OpenAI and Gemini API-key presence with environment-before-`.env` precedence.
  Only key names, source, and `<redacted>` are rendered; no cloud API request is
  made.
- Existing Playwright availability and exact existing setup remediation.
- State-directory and workspace-root write probes using uniquely created files
  that are removed immediately.
- stdin/stdout/stderr TTY state, `NO_COLOR`, terminal width, and resolved footer
  readiness/disable conditions.
- Workspace `.env` existence and sorted defined key names only.

## Config reuse

`src/config.rs` now exposes a crate-private inspection result while retaining
the established search and parse behavior. Preset merging was extracted into a
shared helper, and `preset_complete` now delegates to the same missing-key list
used by doctor output. Its accepted field set and precedence are unchanged.

## Tests

- Added doctor unit coverage for aggregate severity, human symbols and
  remediation, credential precedence/redaction, write-probe cleanup, and a
  loopback Ollama tags response with present/missing models.
- Added config tests for doctor action exclusivity and shared preset
  completeness inspection.
- Added slash-command coverage proving `/doctor` dispatches without using the
  supplied provider clients.
- Added binary integration coverage for warning-only zero exit, failure
  nonzero exit with valid JSON, secret redaction, CLI source metadata, and an
  unresolvable incomplete preset's missing-key list.
- Updated the editor registry-size assertion to derive from `SLASH_COMMANDS`,
  keeping completion synchronized with the registry.

No corpus/event schema changed, no guarded runner chokepoint changed, and no
live `.anvil/` state or historical evidence was modified.
