# Issue 25 design: built-in environment doctor

## Goal

Add a read-only environment diagnosis entry point at `commandagent --doctor`
and `/doctor`. The CLI form also supports `--json`, returns nonzero only when a
check fails, and completes local/network probes within a few seconds.

## Constraints and inspected contracts

- Keep `src/planner/runner.rs` and `src/minimal_loop/loop_run.rs` unchanged.
- Reuse `Config::from_cli` and `ConfigFieldSources` for effective model,
  provider, planner, and profile values instead of creating a second resolver.
- Reuse the config search order, parser, and `preset_complete` definition in
  `src/config.rs`. Diagnosis must still render when ordinary resolution fails.
- Reuse `OllamaClient::list_models`, `load_api_key` semantics, `redact`,
  `playwright_availability`, its existing remediation text, terminal helpers,
  and `FooterEnv` behavior.
- Preserve the live `.anvil/` namespace. The only writes are uniquely named
  temporary probe files that are removed immediately.
- Issue 19 and Issue 20 are committed on their own predecessor worktrees. They
  are documentation-only relative to this issue's runtime surface and are not
  assumed merged into this branch.

## Shape of the change

1. Add `doctor` and `json` flags to `Cli`. `json` requires `doctor`, and doctor
   participates in the existing single-action-selector validation.
2. Route CLI doctor execution before ordinary `Config::from_cli` startup so a
   malformed config or unresolvable preset becomes report data rather than
   suppressing the report. A successful resolution still comes directly from
   `Config::from_cli`.
3. Add a leaf `src/doctor.rs` that owns check collection, severity aggregation,
   aligned text rendering, JSON rendering, and the short Ollama probe.
4. Expose a crate-private config inspection result from `src/config.rs`. It
   reports every searched TOML path and merges a requested preset with the same
   field and completeness rules used by normal resolution. Missing-key names
   are derived from the completeness predicate rather than duplicated in the
   doctor.
5. Register `/doctor` in the slash registry and dispatch it before provider
   work. The REPL form renders the same human report; process exit status is
   relevant only to the direct CLI form.

## Check and output contract

Each check has stable fields:

- `id`: stable dotted identifier;
- `category`: configuration, config file, provider, interaction probe, state,
  terminal, or workspace;
- `label`: concise human label;
- `status`: `pass`, `warn`, or `fail`;
- `message`: redacted diagnostic detail;
- `remediation`: either a non-mutating next step or `null`.

The JSON document uses `schema_version = "1"`, an aggregate `status`, and a
`checks` array. Human output aligns labels and uses `✓`, `!`, and `✗`; a
remediation line is emitted only for warn/fail checks that have remediation.
Any `fail` makes the CLI return nonzero. A report containing only warnings
returns zero.

## Severity decisions

- Invalid config, unresolved configuration, a missing required cloud key,
  unreachable Ollama, a missing configured Ollama model, or an unwritable
  state/workspace directory is a failure.
- Missing optional config files, an absent `.env`, and deliberate `NO_COLOR`
  are informational passes.
- Non-TTY execution, unavailable terminal width/footer, and unavailable
  Playwright are warnings because diagnosis remains usable and the existing
  interaction-probe contract is degradable.

## Safety and timing

- Gemini and OpenAI checks inspect only process environment then workspace
  `.env`; values are passed through `redact` and never printed or serialized.
- Ollama performs one `/api/tags` request through the existing client with a
  two-second connect/total timeout, then validates each configured Ollama role's
  model against the returned tags.
- Directory checks create a unique file with `create_new`, write a short probe,
  close it, and remove it. The doctor never creates missing directories or
  changes configuration.

## Tests and verification

- CLI parsing/help/action tests cover `--doctor`, `--doctor --json`, and action
  exclusivity.
- Doctor unit tests cover severity/formatting, config-file and preset reporting,
  key redaction/source precedence, temporary write probes, and Ollama tags.
- A focused binary integration test validates JSON output and zero/nonzero exit
  behavior without making cloud API requests.
- Run focused doctor/CLI/slash tests first, then `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test` because the CLI,
  config, and slash-command registries are shared contracts.
