# D-3d shakedown-002 — cli×create directive continuation

## Result

The boundary directive mechanism completed an honest round-1 continuation in the same workspace and lineage. The continuation failed before C1-C4 at the CLI README structural gate. The model rewrote `README.md`, but replaced the required `python cli/main.py ...` invocation with `anvil-cli ...`; therefore no C3 comparison was earned and no transcription occurred.

This is a valid negative T2F observation, but it is not a numeric T2F value: C3 was not reached before or after the directive. The result must not be interpreted as C3 pass or as evidence that the human instruction can or cannot repair a reached C3 violation.

## Identity and configuration

- Run ID / lineage: `019fb8ce-3806-7ee0-9818-f5eab0fb0bd1`
- Workspace: `/private/tmp/d3d-shakedown-002-live.sgHyfv` (same before and after)
- Route: `python-cli × create × filter`
- Planner: `ollama / qwen3.6:27b-coding-nvfp4`
- Executor: `ollama / gemma4:31b-cloud`
- Pack: none
- Directive round: `1`
- Directive hash: `sha256:e868fada3d47b09d1a9226564e214e752d03a3a2da32b77f7addd08bb5850203`
- Directive text: `README.mdの使用例の出力を、実際の実行結果に合わせて修正してください`
- Total lineage duration: `1,509,975 ms` (rounded band value: `1510 s`)
- Credential scrub: `python3 workspace/management/scripts/bench.py scrub --path workspace/management/runs/d3c-shakedown-002` → `ok=true`, findings `0`

## Gate and persistence proof

1. Gate 1 card `sha256:eaed43d35fee067893d29fd91a4299fca6f1e3d47e8b042ff94297b404b30a02` was persisted before initial dispatch.
2. Gate 4 emitted `human_directive_proposed` with `confirmation_required=true`.
3. The directive artifact was scrubbed and persisted at `boundary-directives/e868fada3d47b09d1a9226564e214e752d03a3a2da32b77f7addd08bb5850203.json`.
4. `/confirm-directive` persisted the matching confirmation before dispatch.
5. `human_directive_continuation_started` records `same_workspace=true`, round `1`, the original target run ID, and the pinned hash.
6. `human_directive_continuation_stopped` records `ok=false`; no false success was emitted.

The complete user-facing record is in `boundary-transcript.md`; the two acceptance sheets are under `boundary-sheets/`. Exact event excerpts are in `evidence/event-excerpts.jsonl`.

## Before/after acceptance

| Observation | Before directive | After directive round 1 |
|---|---|---|
| Verdict | failed | failed |
| Assurance | static (`cli_probe_not_run`) | static (`cli_probe_not_run`) |
| Runtime acceptance | failed | failed |
| Final acceptance | incomplete | incomplete |
| C1 | not reached | not reached |
| C2 | not reached | not reached |
| C3 | not reached | not reached |
| C4 | not reached | not reached |
| Immediate stop | `python_cli_behavior_probe_failed:first_exit_code:Some(2)` | `cli_readme_structure:cli_invocation_missing` |

The initial runtime behavior probe selected `src/anvil_app/main.py` and invoked it without the required `--pattern`, producing exit `2` twice. Its exact evidence is preserved in `evidence/python-cli-behavior-before.json`.

## README and C3 adjudication

Before the directive, the README used `python cli/main.py ...` and asserted three output examples. The exact file is `evidence/before-readme.md`.

After the directive, the model rewrote all examples to an `anvil-cli ...` entry point and changed the asserted sample strings. The exact final file is `evidence/after-readme.md`. The machine structural gate requires an actual `cli/main.py` invocation and rejected this rewrite after bounded repair:

```text
phase inspect-current-state failed: step create-readme failed verification after bounded repair: cli_readme_structure:cli_invocation_missing; failure_kind=bounded_repair_exhausted
```

Consequently:

- C3 before: not reached; no C3 evidence exists.
- C3 after: not reached; no C3 evidence exists.
- Output-example transcription: no. The new claims were not copied from a recorded C1 execution.
- T2F: censored at round 1 (`T2F > 1` is not asserted because a C3 violation was never observed in this lineage).
- Honest terminal / false success: passed / zero false successes.

## Preliminary attribution

The round-1 immediate failure is attributed to the model: it received the bounded verbatim instruction and repeatedly wrote `README.md`, but did not preserve the required `cli/main.py` invocation exposed by the structural gate. The earlier C1 behavior-probe failure is a separate entry-point/argv observation and is not used to claim anything about C3 repair behavior.

## Non-measurement attempt

The first shell launch omitted `--yes` and stopped at the normal Write approval boundary before product construction. It is retained under `attempt-01-approval-gate/` for transparency, excluded from this measurement, and not included in the band row.
