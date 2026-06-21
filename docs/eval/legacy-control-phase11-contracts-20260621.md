# Legacy Control Phase 11 Contract Slice

Date: 2026-06-21

Commit at verification start: `429cbbb`

Dirty flag: yes. This report covers the uncommitted Phase 11 working tree.

Binary: `target/release/commandagent`

Provider/model: none for smoke. The eval commands below used dry-run mode.

## Scope

This slice adds typed contract surfaces through Phase 11:

- completion evidence
- evidence binding
- deliverable obligations
- recovery owner
- repair action plan
- semantic failure report
- repair job state
- attempt outcomes
- patch validation
- eval report fields

These are visible contract/eval data. They do not add another execution engine,
hidden continuation, or provider-specific policy.

## Verification

Commands run:

```bash
cargo fmt --check
cargo test
cargo build --release
scripts/eval_smoke.sh
scripts/eval_agent_slice.sh --dry-run --out eval/runs/phase11-smoke-20260621 --runs 1
scripts/eval_report.py eval/runs/phase11-smoke-20260621/20260621T104523
scripts/eval_report.py eval/runs/phase11-smoke-20260621/20260621T104523 --recheck
scripts/eval_large_tasks.sh --dry-run --out eval/runs/phase11-smoke-20260621-large --runs 1
scripts/eval_report.py eval/runs/phase11-smoke-20260621-large/20260621T104523
```

Initial results:

- `cargo fmt --check`: passed
- `cargo test`: passed, 538 unit tests plus integration/doc tests
- `cargo build --release`: passed
- `scripts/eval_smoke.sh`: passed
- focused dry-run root: `eval/runs/phase11-smoke-20260621/20260621T104523`
- large dry-run root: `eval/runs/phase11-smoke-20260621-large/20260621T104523`

Dry-run eval summaries report `0/N` because dry-run does not execute model
work. For this Phase 11 check, the relevant result is that eval wiring,
summary generation, report generation, and recheck generation completed.

Follow-up implementation in the same Phase 11 adoption line added:

- missing expected-path producer data for deliverable obligations, file-layout
  evidence binding, and missing repo-edit completion evidence;
- repair-loop patch validation enforcement for test weakening, with explicit
  stop evidence;
- repeated verifier signature attempt outcomes as `no_progress`;
- eval `summary.tsv` and `meta.json` recovery report fields:
  `active_job`, `recovery_owner`, `target_path`, `target_role`,
  `repair_action`, `tool_policy`, `attempt_outcome`,
  `evidence_binding_status`, `completion_evidence_status`, and
  `explicit_stop_reason`.

Follow-up checks:

```bash
cargo fmt --check
cargo test
bash -n scripts/eval_agent_slice.sh
bash -n scripts/eval_large_tasks.sh
python3 -m py_compile scripts/eval_report.py
scripts/eval_agent_slice.sh --dry-run --out eval/runs/control-completion-dry-run --runs 1
scripts/eval_report.py eval/runs/control-completion-dry-run/20260621T111057
scripts/eval_report.py eval/runs/control-completion-dry-run/20260621T111057 --recheck
```

Follow-up results:

- `cargo fmt --check`: passed
- `cargo test`: passed, 541 unit tests plus integration/doc tests
- script syntax checks: passed
- dry-run eval report includes recovery job counts and the new recovery fields
  in `summary.tsv` and `recheck_summary.tsv`

## Final Focused Eval

Final focused recovery verification is recorded in
`docs/eval/control-recovery-focused-20260621.md`.

Final focused local LLM result:

- root: `eval/runs/control-focused-local-llm-final6/20260621T122253`
- normal report: 8/8
- recheck report: 8/8
- checks: `cargo fmt --check`, `cargo test`, `cargo build --release`,
  `scripts/eval_smoke.sh`, and `scripts/check_branding.sh` passed

## Remaining Broadening Areas

- Focused evidence-binding producers are present for manifest identity, import
  symbol, executable handle, test script, required section, schema column,
  citation, and file layout. Broader profile-specific bindings can still be
  added when new observed failures justify them.
- Completion evidence pass-side producers are present for repo edits,
  verifier pass, command observation, docs section pass, structured data pass,
  and report completeness pass. Additional domain-specific pass evidence should
  stay case-driven.
- Repair job state is rendered but not yet a persistent lifecycle controller.
- Patch validation rejects test weakening in the repair loop. Rollback remains
  gated by an explicit policy requiring verifier-proven worsened outcome and
  safe rollback data.
- No-progress attempt outcomes are recorded for repeated verifier signatures,
  and strategy selection can retry a deterministic operator, switch target role,
  route to evidence binding, choose admitted scaffold rebuild, escalate to
  contract conflict, or explicitly stop.
