# Eval Execution Report 0624_02

## 作成日

2026-06-24

## 対象

CommandAgent の eval 実施状況を、最新の対策実装後 eval root を中心に整理する。

主な参照 root:

- Large runs=3: `eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311`
- Small: `eval/runs/small-local-llm/0624-policy/20260624T115900`
- Focused: `eval/runs/focused-control-recovery/0624-phase3e/20260624T124936`
- Smoke: `eval/runs/manual-all-local-llm/smoke/20260624T104816`
- 前回整理: `workspace/mvp/gptpro_fb/eval/0624_01/implementation_progress.md`

## やったこと

1. 対策計画に沿って runtime / planning / eval reporting を実装した。
   - Phase 1: ultra phase の artifact ownership 分離と conservative owner merge。
   - Phase 2: Create/Edit の target-state-aware tool policy。
   - Phase 3: step complexity lint。
   - Phase 4: manifest preserve gate と `package.json` before/after version diff guard。
   - Phase 5: worker Bash / verifier Bash 境界と blocked Bash evidence field 化。
   - Phase 6: control sign-off と task completion sign-off の分離。
   - Phase 7: small eval layer 追加。
   - Phase 8: progress budget terminal state 追加。
   - Phase 9: eval root の `environment.json` 再現性 metadata 出力。

2. ローカル LLM で eval を実行した。
   - Provider/model: Ollama `qwen3.6:35b-a3b-coding-nvfp4`
   - `OLLAMA_HOST=http://127.0.0.1:11434`
   - Large `runs=3` を実行。
   - Recheck report を実行。
   - Combined sign-off を実行。

3. 実装検証を実施した。
   - `cargo fmt --check`: pass
   - `cargo test`: pass, 781 lib tests + integration tests
   - `cargo build --release`: pass
   - `python3 -m unittest discover -s tests -p 'test_eval_report.py'`: pass, 61 tests
   - `python3 -m unittest discover -s tests -p 'test_eval_signoff.py'`: pass, 25 tests
   - eval schema / dry-run / report recheck: pass

## 発生している事象

Large task completion は改善していない。

### エビデンスファイル

最新 Large `runs=3` root:

- Root directory: `eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311`
- Raw summary: `eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311/summary.tsv`
- Recheck summary: `eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311/recheck_summary.tsv`
- Environment metadata: `eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311/environment.json`

`environment.json` の主要 evidence:

- `provider`: `ollama`
- `model`: `qwen3.6:35b-a3b-coding-nvfp4`
- `ollama_host`: `http://127.0.0.1:11434`
- `ollama_version`: `ollama version is 0.30.10`
- `runs`: `3`
- `timeout_mode`: `bounded`
- `timeout_secs`: `900`
- `git_commit`: `4d3df95b1304d1080af6fcb06646a21ccc33bb77`
- `dirty`: `true`
- `binary_sha256`: `7421f23a019374c8248d2d0fa9b60f1a72351c24119bd21519cd524b8bf3e980`
- `dirty_diff_sha256`: `b2f978a32b50a015ee5f483de0947605f7587a9cd2c197f8e104fa0acf352912`

最新 Large `runs=3` の結果:

- Root: `eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311`
- Result: `success: 0/18`
- Failure categories:
  - planning: 9
  - profile: 3
  - step_policy: 6
- Terminal states:
  - `missing_deliverable`: 7
  - `plan_lint_failed`: 2
  - `profile_contract_failed`: 3
  - `step_policy_failed`: 6
- Diagnostic codes:
  - `blocked_bash_command_policy`: 9
  - `action_required_no_repository_evidence`: 2
  - `nextjs_integration_artifact_missing`: 2
  - `plan_lint:failed`: 2
  - `unknown_verifier_failure`: 2
  - `minimal_loop_max_iterations`: 1
- Unknown/raw diagnostic coverage defects: none

Combined sign-off の結果:

- `root_admission_status: pass`
- `control_contract_signoff: fail`
- `task_completion_signoff: fail`
- `smoke_task_success: 3/3`
- `small_task_success: 4/4`
- `large_task_success: 0/18`
- `focused_assertion_pass: 82/83`

つまり、eval/reporting の証跡性と分類は改善したが、Large task の完了率は改善していない。

### `recheck_summary.tsv` から見た失敗行 evidence

`eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311/recheck_summary.tsv` から、事象把握に必要な列を抜粋した。

| case | run | terminal_state | diagnostic_code | producer/guard | owner/action | target | disposition |
| --- | --- | --- | --- | --- | --- | --- | --- |
| large-fastapi-app-modify | 1 | `missing_deliverable` | `action_required_no_repository_evidence` | `tool_protocol` / `tool_protocol` | `test` / `correct_tool_protocol` | `tests/test_app.py` | `implementation_blocker`, `inconsistent_tool_protocol_job_owner` |
| large-fastapi-app-modify | 2 | `step_policy_failed` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `explicit_stop` / `stop_with_structured_evidence` | `tests/test_app.py` | `closed_owned_failure` |
| large-fastapi-app-modify | 3 | `missing_deliverable` | `unknown_verifier_failure` | `verifier` / `verifier` | `test` / `repair_source_error` | `app/main.py` | `closed_owned_failure` |
| large-fastapi-app-new | 1 | `step_policy_failed` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `explicit_stop` / `stop_with_structured_evidence` | `app/main.py` | `closed_owned_failure` |
| large-fastapi-app-new | 2 | `step_policy_failed` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `explicit_stop` / `stop_with_structured_evidence` | `app/main.py` | `closed_owned_failure` |
| large-fastapi-app-new | 3 | `step_policy_failed` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `explicit_stop` / `stop_with_structured_evidence` | `app/main.py` | `closed_owned_failure` |
| large-nextjs-app-modify | 1 | `profile_contract_failed` | `nextjs_integration_artifact_missing` | `profile_verification` / `profile_verification` | `route_integration` / `create_missing_integration_artifact` | `components/AnalyticsPanel.tsx` | `closed_owned_failure` |
| large-nextjs-app-modify | 2 | `profile_contract_failed` | `nextjs_integration_artifact_missing` | `profile_verification` / `profile_verification` | `route_integration` / `create_missing_integration_artifact` | `components/AnalyticsPanel.tsx` | `closed_owned_failure` |
| large-nextjs-app-modify | 3 | `profile_contract_failed` | `action_required_no_repository_evidence` | `tool_protocol` / `tool_protocol` | `route_integration` / `correct_tool_protocol` | `components/AnalyticsPanel.tsx` | `implementation_blocker`, `inconsistent_tool_protocol_job_owner` |
| large-nextjs-app-new | 1 | `missing_deliverable` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `scaffold` / `stop_with_structured_evidence` | `src/app/layout.tsx` | `closed_owned_failure` |
| large-nextjs-app-new | 2 | `plan_lint_failed` | `plan_lint:failed` | `plan_lint` / `plan_lint.step_complexity` | `planning` / `split_oversized_mutation_step` | `package.json` | `closed_owned_failure` |
| large-nextjs-app-new | 3 | `plan_lint_failed` | `plan_lint:failed` | `plan_lint` / `plan_lint.step_decomposition` | `verifier_contract` / `add_required_artifact_owner_step` | `next.config.ts` | `closed_owned_failure` |
| large-rust-app-modify | 1 | `missing_deliverable` | `minimal_loop_max_iterations` | `eval_success` / `verifier` | `scaffold` / `create_required_artifact` | `src/lib.rs` | `closed_owned_failure` |
| large-rust-app-modify | 2 | `missing_deliverable` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `scaffold` / `stop_with_structured_evidence` | `src/lib.rs` | `closed_owned_failure` |
| large-rust-app-modify | 3 | `missing_deliverable` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `scaffold` / `stop_with_structured_evidence` | `src/lib.rs` | `closed_owned_failure` |
| large-rust-app-new | 1 | `step_policy_failed` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `explicit_stop` / `stop_with_structured_evidence` | none | `implementation_blocker`, `missing=target` |
| large-rust-app-new | 2 | `missing_deliverable` | `unknown_verifier_failure` | `verifier` / `verifier` | `scaffold` / `repair_source_error` | `src/main.rs` | `closed_owned_failure` |
| large-rust-app-new | 3 | `step_policy_failed` | `blocked_bash_command_policy` | `step_policy` / `step_policy` | `explicit_stop` / `stop_with_structured_evidence` | `src/main.rs` | `closed_owned_failure` |

### Sign-off 出力 evidence

Combined sign-off command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/manual-all-local-llm/smoke/20260624T104816 \
  --root small=eval/runs/small-local-llm/0624-policy/20260624T115900 \
  --root focused=eval/runs/focused-control-recovery/0624-phase3e/20260624T124936 \
  --root large=eval/runs/large-local-llm/0624-countermeasures-runs3/20260624T140311
```

Sign-off output excerpt:

```text
root_admission_status: pass
control_contract_signoff: fail
task_completion_signoff: fail
smoke_task_success: 3/3
small_task_success: 4/4
large_task_success: 0/18
focused_assertion_pass: 82/83
```

Sign-off finding excerpt:

```text
focused-python-missing-test-artifact: terminal_state expected ok, observed verifier_command_failed
large-fastapi-app-modify run 1: inconsistent_tool_protocol_job_owner
large-nextjs-app-modify run 3: inconsistent_tool_protocol_job_owner
large-rust-app-new run 1: missing_target
large-fastapi-app-modify run 3: generic_source_fallback
large-rust-app-new run 2: generic_source_fallback
```

## 問題点

1. Large が `0/18` のまま。
   - 6ケースを3回ずつ実行して全て失敗。
   - 以前の baseline `1/6` から task completion は改善していない。

2. worker mutation step が build/test Bash をまだ呼び出している。
   - `blocked_bash_command_policy` が9件発生。
   - policy としては正しく止めているが、planner/model が worker step と verifier step の責務分離を十分に守れていない。

3. final deliverable が作られない。
   - `missing_deliverable` が7件発生。
   - 例: `tests/test_app.py`, `src/lib.rs`, `src/main.rs`, `app/page.tsx` などの必須 artifact が欠落。

4. profile contract failure が残る。
   - `nextjs_integration_artifact_missing` が2件発生。
   - Next.js modify で `components/AnalyticsPanel.tsx` と `app/page.tsx` の integration が満たせていない。

5. sign-off 上の owner/action 不整合が残る。
   - `large-fastapi-app-modify` run 1
   - `large-nextjs-app-modify` run 3
   - `tool_protocol_correction` の owner が `test` や `route_integration` として見えており、tool protocol correction の所有者投影が一貫していない。

6. Focused も完全 pass ではない。
   - `focused_assertion_pass: 82/83`
   - 残件: `focused-python-missing-test-artifact`

## 問題箇所

### 現在のアーキテクチャ整理

CommandAgent の eval 実行時の責務分離は以下の構造になっている。

```text
eval runner
  -> target/release/commandagent
    -> CLI / slash command
      -> /ultra-plan-run
        -> ultra phase planner
          -> phase-local /plan-run step plan
            -> plan lint / profile lint / task contract lint
            -> minimal loop worker
              -> deterministic tools: Read / Write / Edit / Bash / Glob / Grep
            -> deterministic verifier
            -> bounded repair / explicit stop
  -> eval_report.py / eval_signoff.py
```

責務境界:

- Provider: LLM transport のみ。
- Ultra planner: phase 分解と phase-local step plan 生成。
- Plan lint: 実行前に plan contract 違反を止める。
- Minimal loop: 1 step の tool-call 実行。
- Tool executor: tool policy と path / Bash policy enforcement。
- Verifier: build/test/check コマンドの実行責任。
- Repair loop: bounded repair prompt / explicit stop / evidence routing。
- Eval/reporting: 既に観測された evidence の分類と sign-off。runtime を変更しない。

### Runtime / tool policy

- `src/agent/minimal_loop/tool_executor.rs`
  - Create/Edit/Repair の tool policy enforcement。
  - Bash を read-only inspection に制限する箇所。
  - `ToolExecutor::execute` (`src/agent/minimal_loop/tool_executor.rs:74`)
    - LLM tool call を deterministic tool 実行へ橋渡しする。
  - `tool_policy_violation` 系処理 (`src/agent/minimal_loop/tool_executor.rs:271`)
    - `Bash command is not read-only for this step: class=...; reason=...; command=...` を生成する。
    - Large root の `blocked_bash_command_policy` 9件の直接発生箇所。

- `src/agent/minimal_loop/loop_run.rs`
  - progress budget と final answer / missing artifact guard。
  - max iterations まで進捗がないケースの終端分類。
  - `run_session_with_observer` (`src/agent/minimal_loop/loop_run.rs:43`)
    - 1 worker session の model request、tool execution、guard feedback、final answer acceptance を管理する。
  - `progress_budget_error` (`src/agent/minimal_loop/loop_run.rs:318`)
    - max iterations 到達時に required mutation / repository evidence / artifact progress がゼロの場合の terminal state を作る。

- `src/tools/bash.rs`
- `src/tools/mod.rs`
  - Bash command classification と policy violation の evidence 化。
  - `ToolError::BashBlocked` (`src/tools/mod.rs:30`)
    - blocked command, command class, reason を保持する。
  - Bash classification (`src/tools/bash.rs:173`)
    - command を read-only / build-test / setup / unknown などに分類する。

### Planning / phase decomposition

- `src/agent/step_runner/ultra_plan.rs`
- `src/agent/step_runner/ultra_run.rs`
- `src/agent/step_runner/runtime/phase_contract.rs`
  - phase-owned artifacts と global final artifacts の分離。
  - explicit `owned_artifacts` と goal 由来 inference の merge。
  - `run_ultra_plan` (`src/agent/step_runner/ultra_run.rs:35`)
    - phase ごとに workspace snapshot と phase contract を作り、step plan 実行へ渡す。
  - `phase_step_plan_prompt` (`src/agent/step_runner/ultra_run.rs:106`)
    - current phase owned artifacts / preserve artifacts / verify-only artifacts / global final artifacts を planner prompt に出す。
  - `phase_owned_artifacts` (`src/agent/step_runner/ultra_run.rs:149`)
    - explicit `owned_artifacts` と phase goal 由来の required artifact を merge する。
  - `PhaseWorkspaceContract` (`src/agent/step_runner/runtime/phase_contract.rs:17`)
    - phase の required/global/preserve/verify-only artifact facts を render する。
  - `PhaseWorkspaceContract::collect_with_scope` (`src/agent/step_runner/runtime/phase_contract.rs:114`)
  - `PhaseWorkspaceContract::render` (`src/agent/step_runner/runtime/phase_contract.rs:206`)

- `src/agent/step_runner/plan_lint/mod.rs`
  - step complexity lint。
  - oversized mutation step、manifest/source 混在、required artifact owner 欠落の検出。
  - `lint_step_plan_with_workspace` (`src/agent/step_runner/plan_lint/mod.rs:33`)
    - workspace-aware lint の入口。
  - `lint_step_plan_generic` (`src/agent/step_runner/plan_lint/mod.rs:40`)
    - path/verifier/instruction/ownership/complexity/manifest gate を順に検査する。
  - `lint_required_artifact_owners` (`src/agent/step_runner/plan_lint/mod.rs:95`)
    - final artifact owner がない plan を検出する。
  - `lint_step_complexity` (`src/agent/step_runner/plan_lint/mod.rs:82`)
    - mutation step の target 数、missing artifact 数、manifest/source 混在を検査する。
  - `step_complexity_violation` (`src/agent/step_runner/plan_lint/mod.rs:327`)
    - `plan_lint.step_complexity` evidence を作る。
  - `lint_manifest_preserve_gate` (`src/agent/step_runner/plan_lint/mod.rs:365`)
    - modify task で既存 manifest を理由なく mutation target にする plan を拒否する。

- `src/agent/step_runner/plan_prompt.rs`
- `src/agent/step_runner/runtime/prompts.rs`
  - planner / worker に提示する責務分離指示。

### Repair / evidence routing

- `src/agent/step_runner/runtime/repair_loop.rs`
  - repair owner/action projection。
  - patch validation。
  - tool protocol correction evidence。
  - Large sign-off で残った owner/action 不整合の主な確認対象。
  - `repair_step_with_state` (`src/agent/step_runner/runtime/repair_loop.rs:681`)
    - bounded repair turn の中心。repair prompt 生成、minimal loop 再実行、patch validation、verifier rerun を束ねる。
  - `turn_error_reason_and_diagnostic` (`src/agent/step_runner/runtime/repair_loop.rs:109`)
    - minimal loop error を repair/eval 用 reason code へ変換する。
  - `tool_protocol_contract_evidence` (`src/agent/step_runner/runtime/repair_loop.rs:973`)
    - invalid tool call / prose-only / missing repository evidence を tool protocol evidence にする。
  - `step_policy_contract_evidence` (`src/agent/step_runner/runtime/repair_loop.rs:1648`)
    - `blocked_bash_command_policy` evidence を作る。Large root の step policy rows の主要 evidence producer。
  - `blocked_bash_eval_fields` (`src/agent/step_runner/runtime/repair_loop.rs:1701`)
    - `failed_tool`, `blocked_command`, `command_class`, `command_authority` を eval field に出す。
  - `patch_validation_report_for_changed_files` (`src/agent/step_runner/runtime/repair_loop.rs:1368`)
    - repair 後の changed files に patch validation を適用する。
  - `content_patch_validations_for_changed_files` (`src/agent/step_runner/runtime/repair_loop.rs:1399`)
    - test weakening / Next.js manifest family / manifest before-after diff を検査する。
  - `manifest_contents` (`src/agent/step_runner/runtime/repair_loop.rs:1437`)
    - repair turn 前の `package.json` snapshot を取る。

- `src/agent/step_runner/integrity_guard.rs`
  - manifest mutation authority。
  - `manifest_version_family_conflict`
  - `manifest_unexpected_version_change`
  - `validate_patch_proposal` (`src/agent/step_runner/integrity_guard.rs:324`)
    - patch の path / manifest authority を検査する。
  - `validate_manifest_patch_authority` (`src/agent/step_runner/integrity_guard.rs:380`)
    - manifest repair authority なしの package/dependency manifest mutation を拒否する。
  - `detect_nextjs_manifest_version_family_conflict` (`src/agent/step_runner/integrity_guard.rs:517`)
    - Next.js / React / TypeScript の既知不整合を拒否する。
  - `detect_manifest_unexpected_version_change` (`src/agent/step_runner/integrity_guard.rs:613`)
    - dependency 追加 repair が既存 dependency version を変えた場合に拒否する。

- `src/agent/step_runner/failure_observation.rs`
  - terminal observation の Rust 側 taxonomy。
  - `TerminalState` (`src/agent/step_runner/failure_observation.rs:10`)
  - `terminal_state_from_contract_evidence` (`src/agent/step_runner/failure_observation.rs:542`)
    - contract evidence から terminal state を決める。

### Profile / recovery orchestration

- `src/agent/step_runner/profiles.rs`
  - Next.js profile facts / profile verification / route integration obligation。
  - `lint_nextjs_route_integration_obligations` (`src/agent/step_runner/profiles.rs:1897`)
    - step plan が selected route integration を落としていないか lint する。
  - Next.js profile verification around `nextjs_integration_artifact_missing` (`src/agent/step_runner/profiles.rs:2292`)
    - missing integration artifact を profile failure として報告する。
  - `nextjs_route_integration_candidate` (`src/agent/step_runner/profiles.rs:2847`)
    - route integration 対象 artifact を判定する。

- `src/agent/step_runner/recovery_orchestration.rs`
  - `nextjs_integration_artifact_missing` などの evidence を active job / owner / action に変換する。
  - `nextjs_integration_artifact_missing` branch (`src/agent/step_runner/recovery_orchestration.rs:974`)
    - `integration_artifact_creation` / `route_integration` 系の候補を作る。

- `src/agent/step_runner/recovery_policy.rs`
  - Profile failure から bounded repair policy を組む。
  - `missing_integration_artifact_policy` (`src/agent/step_runner/recovery_policy.rs:434`)
    - `create_missing_integration_artifact` を作る。

### Eval/reporting

- `scripts/eval_report.py`
- `scripts/eval_signoff.py`
- `scripts/eval_failure_observation.py`
- `scripts/eval_runtime_job_report.py`
- `scripts/failure_observation_taxonomy.tsv`
  - failure category / terminal state / sign-off finding の分類。
  - owner/action consistency の評価。
  - `normalize_observation` (`scripts/eval_failure_observation.py:92`)
    - raw row / evidence text から terminal observation を作る。
  - `terminal_state_from_reason` (`scripts/eval_failure_observation.py:260`)
  - `diagnostic_code_from_evidence` (`scripts/eval_failure_observation.py:451`)
  - `large_disposition_projection` (`scripts/eval_runtime_job_report.py:379`)
    - failed large row の disposition を投影する。
  - `owner_action_status` (`scripts/eval_runtime_job_report.py:493`)
    - `inconsistent_tool_protocol_job_owner` を判定する。
  - `generic_source_fallback_findings` (`scripts/eval_signoff.py:566`)
    - `missing_deliverable` が generic source fallback に落ちている行を sign-off finding にする。
  - `large_disposition_findings` (`scripts/eval_signoff.py:624`)
    - missing target / implementation blocker などを sign-off finding にする。

## 想定直接原因

1. `blocked_bash_command_policy` 多発の直接原因
   - worker mutation step で model が `cargo check`, `cargo test`, `npm run build`, `python -m pytest` 相当の build/test Bash を発行している。
   - 現行 policy では create/edit/repair worker における Bash は read-only inspection のみ許可されるため、正しく拒否される。

2. `missing_deliverable` の直接原因
   - 必須 artifact の owner step があっても、実際の minimal loop が該当 file を作成しきれていない。
   - または plan lint / explicit stop / tool policy stop により、artifact 作成前に実行が停止している。

3. `nextjs_integration_artifact_missing` の直接原因
   - Next.js modify で analytics panel artifact または selected route integration が作成・接続されていない。
   - profile verification が required integration artifact の欠落を検出している。

4. owner/action 不整合の直接原因
   - tool protocol correction の evidence が、元の failed target owner を引き継いだまま sign-off projection されている。
   - `tool_protocol_correction` 自体の owner と、domain target owner が report 上で分離されていない。

5. `minimal_loop_max_iterations` の直接原因
   - minimal loop が要求された mutation / artifact creation に到達できず、既定 iteration 上限まで進んだ。
   - 今回追加した `progress_budget_exhausted` は最新 Large root では発火しておらず、既存 max-iteration diagnostic として残っている。

## 根本原因

1. Large task に対して phase / step decomposition がまだ不十分。
   - 対策で oversized step は一部止められるようになったが、model は依然として worker step に verifier 的行動や複数責務を混ぜる。
   - Large task を完了可能な粒度へ安定分解する contract がまだ足りない。
   - 根拠:
     - Latest Large root で `plan_lint_failed` が2件残っている。
     - `plan_lint.step_complexity` / `plan_lint.step_decomposition` が発生している。
     - `blocked_bash_command_policy` が9件あり、worker step が verifier 的 Bash を発行している。
     - `src/agent/step_runner/plan_lint/mod.rs:82` の step complexity lint は検出できているが、Large completion へ戻す correction は成功していない。

2. runtime guard は失敗を正しく止める方向へ強くなったが、成功に導く planner correction は不足している。
   - Bash policy, manifest guard, step complexity lint は failure attribution を改善した。
   - しかし、拒否後に task completion へ戻すための bounded correction / replan が弱い。
   - 根拠:
     - `Unknown/raw diagnostic coverage defects: none` であり、分類はできている。
     - 一方で `large_task_success: 0/18` のため、分類改善が task completion に転化していない。
     - `step_policy_failed` 6件と `missing_deliverable` 7件が残る。
     - `src/agent/step_runner/runtime/repair_loop.rs:681` の bounded repair は実行単位として存在するが、`attempt_outcome=not_attempted` が18件で、Large sign-off 上は修復実行成功まで到達していない。

3. tool protocol / domain owner の report model が混線している。
   - tool-call shape の失敗は tool protocol owner が持つべきだが、report では domain target owner と組み合わさり、owner/action inconsistency として残っている。
   - control sign-off 上の失敗であり、task completion とは別に修正が必要。
   - 根拠:
     - Sign-off finding に `inconsistent_tool_protocol_job_owner` が2件ある。
     - `large-fastapi-app-modify` run 1 は `producer=tool_protocol`, `selected_action=correct_tool_protocol`, しかし `active_owner=test`。
     - `large-nextjs-app-modify` run 3 は `producer=tool_protocol`, `selected_action=correct_tool_protocol`, しかし `active_owner=route_integration`。
     - `scripts/eval_runtime_job_report.py:493` の `owner_action_status` がこの組み合わせを不整合として扱っている。
     - `src/agent/step_runner/runtime/repair_loop.rs:973` は tool protocol evidence を作るが、report projection では domain target owner と同じ行に出ている。

4. Large success criteria に対して local LLM の実装品質が不足している。
   - small は `4/4` で通っているため、単純な tool policy や eval harness の破損ではない。
   - Large では複数 artifact、profile constraints、verifier boundary、integration obligations を同時に満たす必要があり、現在の minimal loop + bounded correction では安定完了できていない。
   - 根拠:
     - Small root は `small_task_success: 4/4`。
     - Smoke root は `smoke_task_success: 3/3`。
     - Large root だけ `large_task_success: 0/18`。
     - Large rows は FastAPI / Next.js / Rust の新規・modify 全ケースで失敗しており、単一 profile 固有ではない。
     - `environment.json` に provider/model/binary SHA が保存されており、同一 local LLM / binary の実行として比較可能。

5. profile verification と artifact ownership は可視化されたが、実装 completion へ閉じる仕組みがまだ弱い。
   - `nextjs_integration_artifact_missing` のような profile failure は検出できる。
   - しかし missing integration artifact を作り、route に接続し、verifier/profile を通す一連の bounded repair はまだ十分に成功していない。
   - 根拠:
     - `large-nextjs-app-modify` run 1/2 で `nextjs_integration_artifact_missing` が発生。
     - `active_owner=route_integration`, `selected_action=create_missing_integration_artifact`, `target=components/AnalyticsPanel.tsx` まで特定できている。
     - それでも terminal state は `profile_contract_failed` のまま。
     - `src/agent/step_runner/profiles.rs:2292` 付近で profile verification は missing integration artifact を検出できる。
     - `src/agent/step_runner/recovery_policy.rs:434` の missing integration artifact policy は存在するが、Latest Large root では completion まで到達していない。

6. missing target / generic source fallback は、target admission と repair ownership の粒度不足を示している。
   - 根拠:
     - Sign-off finding に `large-rust-app-new run 1: missing_target` がある。
     - Sign-off finding に `large-fastapi-app-modify run 3` と `large-rust-app-new run 2` の `generic_source_fallback` がある。
     - `scripts/eval_signoff.py:566` の `generic_source_fallback_findings` が、repairable target へ十分に狭まっていない行を検出している。
     - `scripts/eval_signoff.py:624` の `large_disposition_findings` が missing target を sign-off blocker として扱っている。

## 影響

- Eval root admission は通るため、証跡・分類・再現性 metadata は利用可能。
- Control sign-off は fail のため、まだ sign-off 可能な状態ではない。
- Task completion sign-off も fail のため、Large task 対応力は未達。
- Small eval は pass しているため、小規模 task regression は現時点では確認されていない。

## 現時点の結論

今回の eval 実施では、対策実装により failure attribution と eval observability は改善した。

一方で、Large completion は `0/18` で改善していない。現在の失敗は、主に以下へ収束している。

- worker が verifier Bash を呼ぶ step policy failure
- 必須 artifact 未作成
- Next.js integration artifact 欠落
- tool protocol correction の owner/action 投影不整合

次の対応は、広い retry や provider/model 固有 patch ではなく、以下の順で絞るべき。

1. tool protocol correction owner を domain owner と分離して sign-off inconsistency を解消する。
2. worker が verifier Bash を呼ぶ前に plan lint / prompt contract で verify step へ分離させる。
3. missing deliverable に対する bounded correction を、artifact owner step 作成までではなく実ファイル作成成功まで追跡する。
4. Next.js integration artifact repair を focused case で固定し、Large に戻す。
