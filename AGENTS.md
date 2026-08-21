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

## CommandMate CLI

- `commandmatedev` is installed outside this repository and uses state below
  `~/.commandmate`. Its `status` command reads PID and environment files and
  inspects processes. Server-backed commands such as `ls`, `send`, `wait`,
  `respond`, and `capture` resolve the configured endpoint and normally call a
  loopback HTTP server (default `127.0.0.1:3000`); some commands also interact
  with tmux and worktrees. `status` describes the daemon PID file and can report
  `Stopped (no PID file)` while the configured API used by `ls` is healthy, so
  never use `status` alone as the server-reachability verdict.
- Treat `commandmatedev --help` and `--version` as sandbox-safe discovery only.
  Their success does not prove that the CommandMate server or its state is
  reachable. For an authoritative API probe, use `commandmatedev ls --json`
  outside the filesystem/network sandbox and require exit 0 plus valid JSON.
  Run other user-authorized operational commands outside the sandbox with a
  narrowly scoped approval prefix such as
  `commandmatedev status`, `commandmatedev ls`, or `commandmatedev wait`. Do not
  request a blanket approval for every `commandmatedev` subcommand.
- A sandbox result containing `Operation not permitted`, `fetch failed`,
  `ECONNREFUSED`, `couldn't connect`, `Server is not running`, empty output, or
  invalid JSON means **unreachable until verified**, not confirmed stopped.
  Retry `commandmatedev ls --json` outside the sandbox. A valid JSON response
  proves the API is running even if `commandmatedev status --all` says
  `Stopped`. Establish a real stop only from an outside-sandbox API failure
  corroborated by absent listener/process evidence. Never start, stop, restart,
  or kill CommandMate to resolve an ambiguous failure unless the user explicitly
  authorized it.
- Keep authority separate from sandbox approval. Read-only probes may be
  elevated when needed for diagnosis, but `start`, `stop`, `init`, `update`,
  `sync`, `send`, `respond`, `auto-yes`, `verify`, issue mutations, instance
  management, skill installation, and other state-changing operations still
  require explicit user authorization. Invoking the repository `$orchestrate`
  skill with one or more Issues counts as that authorization for every action
  its `SKILL.md` declares authorized for the invoked run. Do not request a
  separate approval between orchestration phases or for `commandmatedev sync`
  needed to register or reuse those worktrees. This standing authorization is
  limited to that run and does not authorize actions the skill excludes, such
  as starting or stopping CommandMate or mutating Issue lifecycle state without
  separate authorization. For authenticated API access, prefer `CM_AUTH_TOKEN`
  over `--token` so the token is not exposed in the process list or shell
  history.
- Run one `commandmatedev wait <worktree-id>` process per worktree when
  practical. Keep it in a resumable execution session and poll that session;
  do not launch repeated wait commands that can briefly disagree during agent
  startup or completion transitions.

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

## GitHub operations

- Prefer `gh` when `gh auth status` succeeds. If `gh` reports an invalid token
  but the user has authorized a push, do not treat that result as proof that
  Git HTTPS authentication is also unavailable. The successful fallback for
  this repository is the existing HTTPS remote and Git credential helper.
- Before committing or pushing to `develop`, fetch the remote branch and verify
  that the local parent matches it:

  ```bash
  git fetch origin develop
  git rev-parse HEAD origin/develop
  ```

- Satisfy the push preflight without `gh` by querying the public GitHub Actions
  API for the full `origin/develop` parent SHA. Record the exact-SHA `CI`
  conclusion, and require `completed` plus `success`; record `acceptance` as
  well when present. Replace `FULL_PARENT_SHA` with the SHA printed above:

  ```bash
  curl -fsSL -H 'Accept: application/vnd.github+json' \
    'https://api.github.com/repos/Kewton/CommandAgent/actions/runs?head_sha=FULL_PARENT_SHA&per_page=20' \
    | jq -r '.workflow_runs[] | [.name, .status, (.conclusion // ""), .head_sha, .html_url] | @tsv'
  ```

- In a mixed worktree, stage only explicit task-owned paths; never use
  `git add -A` to absorb unrelated untracked files. Commit with a short
  imperative English subject, then push through the HTTPS remote:

  ```bash
  git add <explicit-task-paths>
  git commit -m "Describe the coherent change"
  git push -u origin develop
  ```

- After pushing, require `git rev-parse HEAD origin/develop` to print the same
  SHA twice. A Git HTTPS push can succeed even while the separate `gh` token is
  invalid; do not create a PR unless the user separately authorized one.
