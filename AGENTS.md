# CommandAgent repository instructions

All paths in these instructions are relative to the repository root. The Rust
crate and the `commandagent` binary both live at the repository root. Read
`docs/dev/dev-guardrails.md` before changing production code.

## Engineering guardrails

- Treat `src/planner/runner.rs` and `src/minimal_loop/loop_run.rs` as growth
  tripwires. Put new behavior in a new module or an existing leaf module and
  keep changes at these chokepoints to minimal wiring. Do not raise guardrail
  baselines to admit growth.
- Preserve honest-failure semantics. Do not weaken verification, acceptance,
  evidence, or release gates merely to make a scenario pass. Any verification
  rewrite must be semantically equivalent or stricter.
- Add focused tests for behavior changes. When an event, recovery, or corpus
  contract changes, add or update the relevant fixture under
  `tests/corpus/apps/` as well.
- Keep event names and schemas backward compatible unless the task explicitly
  authorizes a schema migration.
- Do not rewrite historical evidence under `workspace/management/runs/` or
  `docs/migration/`. A task may add a new run directory, but existing records
  remain immutable.
- Keep the live `.anvil/` runtime namespace unchanged unless a dedicated state
  migration is explicitly requested.

## Codex harness

- Store repository skills under `.agents/skills/<skill-name>/SKILL.md` and
  invoke them as `$skill-name`.
- Keep reusable details in a skill's `references/` directory instead of a
  repository-level `.codex/prompts` directory.
- `commandagent` is this repository's product binary. `commandagentdev` is the
  local development launcher. `commandmatedev` is a separate CommandMate
  orchestration CLI; do not substitute one for another.
- Do not start or stop CommandMate, dispatch workers, create or merge pull
  requests, or publish releases unless the user has authorized that external
  action.

## OpenAI API credentials

- For an authorized OpenAI live probe, load `OPENAI_API_KEY` from the
  repository-root `.env` into the same shell that launches `commandagent`:

  ```bash
  set -a
  source ./.env
  set +a
  commandagent <authorized arguments>
  ```

  `commandagent` reads the process environment and does not load `.env`
  implicitly. Confirm availability with `commandagent --doctor --json`; never
  print the key, pass it as a command-line argument, or copy it into evidence.
- Keep `.env` ignored, unstaged, and owner-readable only (`chmod 600 .env`).
  Never commit it or include it in logs, run reports, or scrub output.
- Create or rotate a key in the OpenAI Platform API key dashboard, then expose
  it as `OPENAI_API_KEY` as described in the official OpenAI developer
  quickstart: <https://developers.openai.com/api/docs/quickstart>.

## Verification

Run the narrowest relevant check first, then broaden in proportion to risk.
Before handing off a production-code change, normally run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For Python harness changes, also run Ruff and the focused Python test module.
For release-sensitive changes, build `target/release/commandagent` and verify
its `--version` output.

Use short imperative English commit subjects and keep one coherent task per
commit. Never commit `.env`, `workspace/temp/`, runtime state, caches, or raw
logs.
