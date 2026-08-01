# D-3d v1.1 shakedown-002 — directive round 2

## Result

Round 2 ran in the original workspace and lineage with the persisted round 1
directive and its evidence-derived result injected into the continuation plan.
The model edited `README.md` repeatedly, but retained
`python3 src/anvil_app/main.py ...` rather than the explicitly requested
`python3 cli/main.py ...`. The machine structural gate rejected all four repair
attempts with `cli_readme_structure:cli_invocation_missing`. The run therefore
ended honestly before C1-C4; C3 was not reached and no output transcription was
earned.

## Identity and configuration

- Run ID / lineage: `019fb8ce-3806-7ee0-9818-f5eab0fb0bd1`
- Session: `session-e4e4e3c4be457962e3114a3e`
- Workspace: `/private/tmp/d3d-shakedown-002-live.sgHyfv` (same for initial, round 1, and round 2)
- Route: `python-cli × create × filter`
- Planner: `ollama / qwen3.6:27b-coding-nvfp4`
- Executor: `ollama / gemma4:31b-cloud`
- Pack: none
- Directive round: `2`
- Directive hash: `sha256:55c180bb0fdc86eaa8b219f9aa7c872faae01c974e1d7ccce20ad01c708d2dc4`
- Directive: `起動例を python3 cli/main.py に戻し、使用例の出力を実際の実行結果のとおりに書き直してください`
- Round 2 terminal duration: `1,670,960 ms` (rounded band value: `1671 s`)
- Credential scrub: `bench.py scrub` returned `ok=true`, findings `0`

## Session and history proof

`boundary-sessions/session-e4e4e3c4be457962e3114a3e/session.json` contains
two contiguous rounds. Round 1 is represented by reference to the immutable v0
artifact; its artifact bytes still hash to
`e868fada3d47b09d1a9226564e214e752d03a3a2da32b77f7addd08bb5850203`.
Round 2 has its own scrubbed artifact and persisted confirmation with the same
`55c180…` hash.

The production plan contains `material=session_history`, the complete round 1
directive, `result_verdict: failed`, and the evidence-derived structural stop
reason before the round 2 directive. The exact bounded block is preserved in
`evidence/directive-history-round-2.md`. The user-facing proposal,
confirmation, terminal sheet, and failure actions are appended to
`boundary-transcript.md`.

## Acceptance progression

| Observation | Initial | Round 1 | Round 2 |
|---|---|---|---|
| Verdict | failed | failed | failed |
| Assurance | static (`cli_probe_not_run`) | static (`cli_probe_not_run`) | static (`cli_probe_not_run`) |
| Final acceptance | incomplete | incomplete | incomplete |
| C1 | not reached | not reached | not reached |
| C2 | not reached | not reached | not reached |
| C3 | not reached | not reached | not reached |
| C4 | not reached | not reached | not reached |
| Immediate stop | Python behavior probe exit 2 | README invocation missing | README invocation missing |

## README and C3 adjudication

The exact final README is `evidence/after-readme-round-2.md`. Its command lines
are all rooted at `src/anvil_app/main.py`; the required `cli/main.py` command is
absent. The primary event is:

```text
step update-readme failed verification after bounded repair: cli_readme_structure:cli_invocation_missing; failure_kind=bounded_repair_exhausted
```

- Structural gate: failed after four bounded README edits.
- C3 reached: no.
- Output transcription: no; no C1 observation was available to bind a claim.
- False success: zero.
- T2F: still right-censored after round 2. `T2F=2` is not earned because C3
  was never reached in either directive round.

The result shows that persisted multi-round history reached the model and that
the model continued to write the intended artifact. It does not show that the
model can repair the C3 testimony wall, because the preceding structural gate
remains the observed blocker.
