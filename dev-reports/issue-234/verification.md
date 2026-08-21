# Issues 234 and 235 Verification

- Status: `passed`

## Checks

- `cargo test config::tests::`: `passed`
- `cargo test --lib provider_call::tests::`: `passed`
- `cargo test tui::boundary_shell::ambiguity::tests::`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test planner::profiles::python_cli::tests::`: `passed`
- `cargo test --test doc_drift configuration_keys_match_english_reference -- --exact`: `passed`
- `cargo test --test generality_guardrails runner_chokepoints_do_not_grow_past_interim_budget -- --exact`: `passed`
- `git merge-base --is-ancestor 4562b134 HEAD`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `uv run --offline --with PyYAML cargo test`: `passed`
- `off_complete=0; on_complete=0; for arm in python-cli-off python-cli-on nextjs-off nextjs-on data-off data-on ingest-off ingest-on; do events=$(find /private/tmp/commandagent-issue234-uat-20260821/$arm/.anvil/runs -name events.jsonl -type f -print -quit); test -n "$events"; jq -s -e '([.[] | select(.event == "provider_turn_duration" and .caller_scope == "executor")] | length) > 0 and all(.[] | select(.event == "provider_turn_duration" and .caller_scope == "executor"); .think == "omitted")' "$events" >/dev/null; case "$arm" in ingest-*) ;; *-off) jq -s -e '([.[] | select(.event == "provider_turn_duration" and ((.caller_scope // "") | startswith("planner")))] | length) > 0 and all(.[] | select(.event == "provider_turn_duration" and ((.caller_scope // "") | startswith("planner"))); .think == "false")' "$events" >/dev/null ;; *-on) jq -s -e '([.[] | select(.event == "provider_turn_duration" and ((.caller_scope // "") | startswith("planner")))] | length) > 0 and all(.[] | select(.event == "provider_turn_duration" and ((.caller_scope // "") | startswith("planner"))); .think == "true")' "$events" >/dev/null ;; esac; if jq -s -e 'any(.[]; .event == "ultra_plan_complete")' "$events" >/dev/null; then case "$arm" in *-off) off_complete=$((off_complete + 1)) ;; *-on) on_complete=$((on_complete + 1)) ;; esac; fi; done; test "$off_complete" -eq 1; test "$on_complete" -eq 1; test "$off_complete" -ge "$on_complete"`: `passed`

## Paired live UAT evidence

Candidate binary provenance was
`commandagent 0.1.0 6a2f5072+dirty 2026-08-21T23:31:38+09:00`; `dirty` denotes
the candidate implementation and reports verified before the final commit.
Every arm used the same models, provider, prompt, input, and no-retry protocol;
only preset `planner_think` differed.

| Profile | Think off | Think on | Parity |
| --- | --- | --- | --- |
| python-cli | not full, 139.828s, run `01a024c6-3772-7c31-a75b-7a82f7c85241` | not full, 665.643s, run `01a024c8-7bfc-7331-bca3-db7074582333` | `0 = 0` |
| nextjs | full, 226.910s, run `01a024d2-d9b3-7890-abab-3fbe2db4ca22` | full, 759.929s, run `01a024d7-535e-7f82-b6b0-cfa5858b1ac0` | `1 = 1` |
| data | not full, 90.980s, run `01a024e4-4ab8-7083-bbb4-7773f9ee3322` | not full, 415.293s, run `01a024e5-d2e7-74a1-a6c9-22dc0dfc1c9c` | `0 = 0` |
| ingest | not full, 52.138s, run `01a024ec-a08e-72b2-b85e-c186d87ddf02` | not full, 47.424s, run `01a024ed-90e6-7de2-b0d3-788fcfe62d11` | `0 = 0` |
| Band | `1/4` | `1/4` | think off is equal |

Python CLI off failed a malformed verify-command plan; its control failed
required final evidence. Both data arms failed the inspection-schema gate.
Both ingest arms failed N2 source binding. These remain honest failures rather
than inferred passes. Ingest used a manifest-fixed plan and therefore had no
planner turns; all other off/on planner events recorded `false`/`true`
respectively, and all executor events recorded `omitted`.

The live workspaces and raw events are under
`/private/tmp/commandagent-issue234-uat-20260821`; they are intentionally not
part of the repository commit.
