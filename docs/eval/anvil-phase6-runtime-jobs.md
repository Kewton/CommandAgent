# Anvil Loadmap Phase 6 Runtime Jobs

Date: 2026-06-21

Scope:

- setup bootstrap ledger
- stale setup state after manifest/config repair
- setup artifact validation evidence
- Next.js requested-port dev-server smoke
- eval/report fields for setup and dev-server runtime jobs

Runtime-effective changes:

- Setup recovery now renders `setup_job_state`, setup attempt key, manifest
  fingerprint, setup command/result, and verifier rerun result into contract
  evidence and eval fields.
- Manifest or setup-config edits mark setup state stale instead of treating the
  next dependency failure as source implementation evidence.
- Invalid setup manifests produce `manifest_repair` evidence before dependency
  setup is attempted.
- Next.js tasks that request a dev-server port and include an `npm run build`
  verifier now run a bounded `dev_server_smoke` check at plan completion. The
  check validates `scripts.dev`, port availability, endpoint response, and
  process cleanup.
- `port_in_use` is reported as `dev_server_port_contract` instead of source
  repair.

Boundaries:

- No hidden retry budget was added.
- The minimal loop remains the only model execution loop.
- Dev-server smoke does not install dependencies or keep a background server.
- Profiles expose facts and obligations; the step runner owns runtime job
  orchestration.

Verification target:

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo build --release`
- focused eval when local model/tooling permits

Local verification on 2026-06-21:

- `cargo fmt --check`: passed
- `cargo test`: passed
- `python3 tests/test_eval_report.py`: passed
- `cargo clippy --all-targets -- -D warnings`: passed
- `cargo build --release`: passed
- focused eval:
  `scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out eval/runs/current-anvil-phase6-focused --runs 1 --binary target/release/commandagent --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --timeout-secs 900`
  completed, but all 8 cases stopped at `provider_transport_failed` because
  `http://127.0.0.1:11434/api/chat` was not reachable in this run. Runtime
  setup/dev-server behavior was therefore not exercised by this focused eval.
