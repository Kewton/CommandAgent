# GPT Pro Contract Foundation

Date: 2026-06-20

## Scope

Implemented the first contract-foundation slice from
`workspace/mvp/logic/gptpro`:

- versioned external Job/Event envelope and JSONL observer
- replayable Job State projection
- Command accepted/rejected event helper
- Evidence Envelope and typed Evidence Payload adapter from `ContractEvidence`
- Provider `ModelUsage`, token usage, and cost record shapes
- Budget contract data and deterministic Tool Result truncation
- docs and ADR updates for the new contract boundaries

## Design Reading

This is telemetry and bounded contract data, not a new workflow engine.
CommandAgent still owns one execution engine. CommandMate-facing protocol data
is observable and replayable, while queueing, scheduling, approval UI, and
dashboard behavior remain outside CommandAgent.

## Verification Plan

Expected local checks:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Focused E2E should use one-shot JSONL capture:

```bash
COMMANDAGENT_EVENT_JSONL=<run-root>/events.jsonl target/release/commandagent "<prompt>"
```

Record commit hash, dirty flag, provider/model, projected job status, evidence
payload variant when a failure is intentional, usage availability, and any
budget/truncation event.

## Verification Result

Local checks passed on a dirty worktree:

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo build --release`
- `bash scripts/eval_smoke.sh`

Focused E2E was run with Gemini using one-shot JSONL capture from a temporary
workspace under `/private/tmp`.

- command shape: `COMMANDAGENT_EVENT_JSONL=/private/tmp/commandagent-gptpro-gemini-events.jsonl target/release/commandagent --yes --provider gemini --model gemini-3.1-flash-lite "<file creation prompt>"`
- result: success
- created artifact: `/private/tmp/commandagent-gptpro-e2e/e2e_gptpro_marker.txt`
- artifact content: `ok`
- event evidence: `model_request.started`, `model_response.received`, `tool_call.started`, `tool_call.finished`, and `final_answer.accepted`
- usage boundary: `usage.available=false` with reason `provider_usage_not_attached_to_chat_response`

An earlier local Ollama one-shot attempt failed before model response because
`127.0.0.1:11434` was not reachable. That failure still produced a versioned
JSONL `session.error` event, which validates the observable failure path but not
the successful tool-call path.
