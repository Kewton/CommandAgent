# Design: Issues #255, #229, and #232

## Scope

This combined row freezes three related CLI contracts:

- named presets support one `extends` parent plus `${ENV_NAME}` interpolation;
- new runtime and session writes use the CommandAgent namespace while reads retain legacy Anvil locations;
- `--runs` supports list/detail/events/JSON views and `--trace` records scrubbed provider exchanges.

The implementation keeps `src/config.rs`, `src/runs.rs`, and `src/state.rs` as the public contract surfaces. Path selection and trace persistence live in new leaf modules. CLI changes are limited to argument parsing and dispatch data. Provider tracing is wired once at the shared provider-call boundary; no provider-specific behavior is added.

## Decisions

1. Preset inheritance is single-parent and recursive. Child fields win, inherited fields keep the parent preset as their source, missing parents fail, and a cycle reports the complete inheritance chain. Existing cross-file priority and early-stop behavior remain intact for each named preset.
2. `${ENV_NAME}` references are expanded only from the process environment while parsing quoted config values. Missing or non-Unicode variables are configuration errors, so `--doctor` projects them as a failed configuration check without exposing secret values.
3. Dynamic `--preset` completion reads preset table names from the existing four config search paths, then filters, sorts, and deduplicates without provider/network access.
4. `.commandagent` is the canonical workspace write root and `commandagent` is the canonical platform state directory. Read inventories search canonical paths first and `.anvil`/`anvilminimal` second. Explicit `--state-dir` remains exact and does not gain an implicit fallback.
5. `--runs` with no ID keeps the concise table. An ID selects one run; `--events` projects its JSONL chronologically; `--filter` accepts `phase`, `tool`, or `provider`; and `--json` uses a versioned object for list, detail, or event output. Run IDs are validated before path lookup.
6. `--trace` is opt-in. Each provider exchange is written as a separate versioned JSON file below the active run's `trace/` directory. Prompts, tool declarations, replies, and errors pass through the existing event redaction before persistence. Trace-write failures warn but do not rewrite provider success/failure semantics.

## Compatibility and verification

- Existing event names and fields remain valid; `prompt_body_saved` changes only for explicitly traced runs.
- Legacy config, run, and session reads are covered by focused migration tests.
- Focused tests cover inheritance, cycles, environment resolution, completion discovery, list/detail/events/JSON rendering, filter behavior, path precedence, and trace redaction.
- Because shared Rust and CLI contracts change, verification will include formatting, Clippy with warnings denied, and the full Rust test suite after focused checks.
