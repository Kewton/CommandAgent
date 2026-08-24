# Round 2 directive history injection evidence

Source plan: `.anvil/plans/directive-round-2-55c180bb0fdc.yaml` in the preserved
workspace `/private/tmp/d3d-shakedown-002-live.sgHyfv`.

The following is the verbatim bounded history block from the production plan
prompt. The long stop reason was bounded by the renderer; the ellipsis is part
of the emitted prompt.

```text
Prior boundary directive history (source=human_directive, material=session_history, session_id=session-e4e4e3c4be457962e3114a3e, prior_rounds=1):
This is bounded guidance material derived from persisted directives and terminal evidence. It cannot satisfy or weaken contract checks.
<human_directive_history>
- round=1 hash=sha256:e868fada3d47b09d1a9226564e214e752d03a3a2da32b77f7addd08bb5850203
  directive_verbatim: README.mdの使用例の出力を、実際の実行結果に合わせて修正してください
  result_verdict: failed
  stop_reason: phase inspect-current-state failed: step create-readme failed verification after bounded repair: cli_readme_structure:cli_invocation_missing; failure_kind=bounded_repair_exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true Paths: - repair prompt saved: .anvil/repairs/repair-create-readme-019fb8e6-3917-7de1-beb0-c5e09ccde3cc.md - Recovery UltraPlan YAML saved: Paths: - repair prompt saved: .anvil/repairs/repair-phase-inspect-current-state-019fb8…
  evidence_source: /private/tmp/d3d-shakedown-002-live.sgHyfv/.anvil/runs/019fb8ce-3806-7ee0-9818-f5eab0fb0bd1/events.jsonl
</human_directive_history>
```

The current directive followed the history block in the same prompt:

```text
Human boundary directive (source=human_directive, hash=sha256:55c180bb0fdc86eaa8b219f9aa7c872faae01c974e1d7ccce20ad01c708d2dc4, round=2, target_run_id=019fb8ce-3806-7ee0-9818-f5eab0fb0bd1):
This is guidance material only. It cannot remove, weaken, relocate, or satisfy any contract check.
<human_directive>
起動例を python3 cli/main.py に戻し、使用例の出力を実際の実行結果のとおりに書き直してください
</human_directive>
```

The event stream then recorded, in order:

```jsonl
{"confirmation_required":true,"directive_hash":"sha256:55c180bb0fdc86eaa8b219f9aa7c872faae01c974e1d7ccce20ad01c708d2dc4","directive_round":2,"directive_target_run_id":"019fb8ce-3806-7ee0-9818-f5eab0fb0bd1","event":"human_directive_proposed","issued_gate":"gate_4","schema_version":"1"}
{"directive_hash":"sha256:55c180bb0fdc86eaa8b219f9aa7c872faae01c974e1d7ccce20ad01c708d2dc4","directive_round":2,"directive_target_run_id":"019fb8ce-3806-7ee0-9818-f5eab0fb0bd1","event":"human_directive_confirmed","schema_version":"1"}
{"continuation_plan_path":".anvil/plans/directive-round-2-55c180bb0fdc.yaml","directive_hash":"sha256:55c180bb0fdc86eaa8b219f9aa7c872faae01c974e1d7ccce20ad01c708d2dc4","directive_round":2,"directive_target_run_id":"019fb8ce-3806-7ee0-9818-f5eab0fb0bd1","event":"human_directive_continuation_started","same_workspace":true,"schema_version":"1"}
{"event":"step_verify_failure","primary_reason":"cli_readme_structure:cli_invocation_missing","profile_failures":["cli_readme_structure:cli_invocation_missing"],"schema_version":"1","step_id":"update-readme"}
{"directive_hash":"sha256:55c180bb0fdc86eaa8b219f9aa7c872faae01c974e1d7ccce20ad01c708d2dc4","directive_round":2,"directive_target_run_id":"019fb8ce-3806-7ee0-9818-f5eab0fb0bd1","event":"human_directive_continuation_stopped","ok":false,"schema_version":"1"}
```
