set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments

# List available development tasks.
default:
    @just --list

# Build the debug binary.
build:
    cargo build

# Build and publish a clean release binary under target/release.
build-release:
    ./scripts/build-release.sh

# Run every Rust target with warnings denied, matching CI.
test:
    RUSTFLAGS="-D warnings" cargo test --all-targets

# Run the corpus regression harness, matching CI.
test-corpus:
    RUSTFLAGS="-D warnings" cargo test --test corpus_regression

# Run the generality growth guardrails, matching CI.
test-guardrails:
    RUSTFLAGS="-D warnings" cargo test --test generality_guardrails

# Run the conformance matrix, matching CI.
test-conformance:
    RUSTFLAGS="-D warnings" cargo test --test conformance

# Run the opt-in pseudo-terminal integration suite.
test-pty:
    ANVIL_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored

# Run the three Python evaluation golden-test modules from CI.
test-eval:
    python3 tests/eval/test_acceptance_contract.py
    python3 tests/eval/test_completion_contract_snapshots.py
    python3 tests/eval/test_false_positive_regression.py

# Validate the Codex harness with the commands and pinned tools used in CI.
test-harness:
    python3 scripts/validate_codex_skills.py --tracked-only
    ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/codex_orchestrate.py scripts/validate_codex_skills.py tests/test_codex_orchestrate.py workspace/management/scripts
    python3 -m pytest tests/test_codex_orchestrate.py -q

# Run ShellCheck over repository shell scripts, matching CI.
lint-shell:
    shellcheck scripts/*.sh

# Run the exact non-networked acceptance suite used by both CI workflows.
ci:
    bash scripts/ci.sh

# Run the benchmark harness and pass all arguments through to scripts/bench.sh.
bench *args:
    ./scripts/bench.sh "$@"

# Start CommandAgent; override provider, model, or Ollama host with COMMANDAGENT_* variables.
run *args:
    cargo run -- --provider "${COMMANDAGENT_PROVIDER:-ollama}" --model "${COMMANDAGENT_MODEL:-qwen3.6:27b-coding-nvfp4}" --ollama-host "${COMMANDAGENT_OLLAMA_HOST:-http://localhost:11434}" "$@"
