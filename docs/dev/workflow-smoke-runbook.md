# Workflow live smoke runbook

This template is the required preflight and capture protocol for workflow live
smokes. Replace angle-bracketed values, preserve the command text in the run
record, and use a fresh origin copy for every consumed attempt.

## Binary provenance preflight

Build and install the intended repository revision before the smoke. The PATH
lookup and version checks below are mandatory; record their complete output.

```sh
cargo build --release
cargo install --path . --locked
which commandagent
commandagent --version
git rev-parse --short HEAD
```

The resolved binary must be the newly installed `commandagent`; its reported
commit must match `git rev-parse --short HEAD`, and the version must not contain
`+dirty`. Stop before execution if any check differs. This prevents a stale
PATH binary from silently changing the CLI contract.

For schema v0.2, validate the exact workflow and extension root before consuming
an origin attempt. The draft profile must resolve from that root, executor and
planner overrides must each be complete provider/model pairs, and the workflow
must not contain any condition outside the contract's closed vocabulary.

```sh
commandagent --extension-root <extension-root> --validate-profile <extension-root>/profiles/<draft-id>/manifest.toml
grep -nE '^(version:|[[:space:]]+(planner_)?(model|provider):|[[:space:]]+when:)' <workflow-yaml>
```

## Origin and execution

Confirm the real failed-run layout before starting:

```sh
grep -l '"event":"run_stop"' <origin>/.anvil/runs/*/events.jsonl
find <origin>/.anvil/plans -name 'recovery-*.yaml' -print
```

Run in a normal terminal and preserve both epoch values and the exit code:

```sh
start_epoch=$(date +%s)
commandagent --workflow <workflow-yaml> --origin <origin>
exit_code=$?
end_epoch=$(date +%s)
printf 'start_epoch %s\nend_epoch %s\nexit %s\n' "$start_epoch" "$end_epoch" "$exit_code"
```

Pass the same configured `extension_root` (or `--extension-root`) used during
preflight to the workflow run. For any v0.2 workflow containing a draft node,
capture the terminal event and `evidence/workflow-circle.json` and require that
neither reports `circle_full`; a verified closure is capped as
`circle_failed` with reason `profile_not_admitted`.

Do not monitor or interrupt an active smoke unless the task explicitly changes
the protocol. After completion, archive the workflow events, circle evidence,
node-run events, confinement path, epochs, and exit code, then run the bench
scrub before committing.

## Calibrated non-change

The v8 investigation rejected relaxing the model-stagnation thresholds (C2).
Those limits remain unchanged: the honest stagnation terminal is preferable to
weakening the bounded-run contract. The early diagnosis-file instruction is
the selected first-line mitigation.
