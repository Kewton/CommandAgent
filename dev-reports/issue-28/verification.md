# Issue 28 verification

- Status: `passed`
- `python3 -m json.tool .devcontainer/devcontainer.json`: `passed`
- `python3 -m json.tool .devcontainer/devcontainer-lock.json`: `passed`
- `bash -n scripts/bench.sh scripts/build-release.sh`: `passed`
- `npx --yes @devcontainers/cli@latest up --workspace-folder /Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib --remove-existing-container`: `passed`
- `npx --yes @devcontainers/cli@latest exec --workspace-folder /Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib bash -lc 'rustc --version && cargo --version && python3 --version && node --version && just --version && shellcheck --version && just --list'`: `passed`
- `npx --yes @devcontainers/cli@latest exec --workspace-folder /Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib bash -lc 'CARGO_TARGET_DIR=/tmp/commandagent-issue-28-final-target cargo clippy --all-targets -- -D warnings'`: `passed`
- `npx --yes @devcontainers/cli@latest exec --workspace-folder /Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib bash -lc 'CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/commandagent-issue-28-final-target just ci'`: `passed`
- `npx --yes @devcontainers/cli@latest exec --workspace-folder /Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib bash -lc 'CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/commandagent-issue-28-final-target just test-pty'`: `passed`
- `npx --yes @devcontainers/cli@latest exec --workspace-folder /Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib bash -lc 'COMMANDAGENT_BENCH_ROOT=/tmp/commandagent-issue-28-final-bench just bench minimal-loop-expanded --runs 2'`: `passed`
- `npx --yes @devcontainers/cli@latest exec --workspace-folder /Users/maenokota/share/work/github_kewton/CommandAgent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib bash -lc 'cargo fmt --all -- --check'`: `passed`
- `docker image inspect --format '{{.Size}}' vsc-commandagent-issue-28-dev-add-justfile-and-devcontainer-for-reproducib-c408435e9915f9aaafd5d852988f25d86972336e5e3b962f8462a27f768acc43-features:latest`: `passed`
- `git diff --check`: `passed`

## Results

- Final container tools: rustc/cargo 1.94.1, Python 3.12.13, Node.js
  v24.18.0 (LTS), just 1.40.0, and ShellCheck 0.10.0.
- Offline `just ci`: 28 skills validated, Ruff passed, 59 Python harness
  tests passed, ShellCheck passed, 1,533 Rust library tests passed with 15
  ignored, and all integration, corpus, guardrail, conformance, and evaluation
  commands passed.
- `just test-pty` invoked the Issue-specified command exactly. The existing
  `tests/tui_pty.rs` cases retain `#[ignore]`, so Cargo reported three ignored
  tests and a successful exit.
- The benchmark smoke test generated its summary through the variadic
  `just bench` wrapper.
- The final Dev Container image is 1,132,262,419 bytes (about 1.13 GB).

## Diagnostic note

Exploratory Clippy runs with Rust 1.88 and 1.97.1 exposed lint-set drift in
untouched production files. The final Dev Container pins stable Rust 1.94.1,
whose all-target Clippy check passed without changing production code or
weakening warning enforcement.
