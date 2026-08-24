# Intent 裁定骨格（D-0a ドラフト）

本書は Phase D の実装前調査である。現行の `create` intent に癒着した裁定面を棚卸しし、D-0b で「intent 非依存の裁定骨格」と「intent / profile ごとの差し込み」へ挙動保存で分離するための境界を定義する。`fix` intent は rule of two の設計検算にだけ使い、本書では実装も有効化もしない。

## 0. 範囲、用語、結論

対象にする「裁定決定点」は、run の次の値を決定、再決定、投影、または上限制限する箇所である。

- evidence の `pass` / `partial` / `failed` / `unverified` と、final acceptance の `full_success` / `partial` / `incomplete`
- earned assurance の `full` / `partial` / `static` / `failed`（および現行互換値 `reduced`）
- terminal / task の `completed` / `partial` / `failed` / `incomplete`
- profile admission による assurance 上限

分類は次の意味で用いる。

- **[骨格]**: intent 非依存。要求集合の充足、実行済み provenance、階層写像、偽装拒否、admission cap、正直終端の投影。
- **[create差し込み]**: create 固有。何を evidence とするか、create の probe / build oracle、実行順序、既存 `CompletionContract` 推論との接続。
- **[profile差し込み]**: すでに profile 境界にある manifest check、profile probe、profile 固有の evidence 解釈。
- **[要裁定]**: D-0a では所有境界または互換方針を確定できないもの。D-0b の実装判断で黙って解消してはならない。

現行経路を要約すると次の順である。

```text
profile/capability から要求を推論
  -> runtime evidence と CompletionContract を照合
  -> profile final verification / external contract
  -> build / browser / interaction qualification
  -> profile behavior probe
  -> final acceptance / release gate / runtime status
  -> earned assurance
  -> profile admission cap
  -> completion event
  -> CompletionSnapshot
  -> terminal projection
  -> data profile の B-2j 再導出
  -> profile admission cap（再適用）
```

重要な結論は三つある。

1. `src/planner/final_acceptance.rs` は要求集合の一般判定と create 固有 probe の順序を同じ関数内に持つ。D-0b では前者だけを leaf module に抽出し、この tripwire には最小 wiring だけを置く。
2. B-2j の data `full` 再導出は `src/eval_events.rs` 内ではなく、同ファイルの投影後に呼ばれる `src/completion_metadata/data.rs` にある。admission cap も `profile_manifest.rs` ではなく `profile_admission.rs` が実体である。
3. 現行 `CompletionContract` は intent、時点、期待極性、同一 oracle の相関を表せない。このままでは `fix` の `before_fails` / `after_passes` / `no_regression` を正直に裁定できない。

## 1. 裁定決定点の棚卸し

行範囲はこのドラフト作成時の `develop`（`9cac7cc`）を基準にする。長い関数は、責務が変わる範囲ごとに分割した。

### 1.1 `src/planner/final_acceptance.rs`

| 分類 | 関数・行範囲 | 決定または投影 |
|---|---|---|
| [骨格] | `contract_origin_for_acceptance` 37–51 | acceptance が参照した契約の provenance を正規化する。単独では verdict を決めないが、未束縛契約から `full` を得させないための入力になる。 |
| [create差し込み] | `bind_completion_contract_for_acceptance` 64、82–98 | 「契約を要求する run」の判定と、未指定時に生成する現行 `CompletionContract` の要求内容を決める。 |
| [骨格] | 同 65–81、99–114 | 明示契約の load、契約の bind / persist / event 投影を行う。契約内容を解釈せず provenance を確定する部分。 |
| [create差し込み] | `ultra_final_acceptance_report_with_deterministic_remedies` 542–587 | dependency reconcile、evidence clear、再実行の順序を決める。これは現行 create acceptance の retry protocol である。 |
| [create差し込み] | `ultra_final_acceptance_report_inner` 594–646 | profile / capability / promotion から capabilities、evidence、obligations を推論し、`CompletionContract` と合成する。profile が返す個々の要求は profile 差し込みだが、この合成順序と現行 contract 接続は create 側。 |
| [要裁定] | 同 647–658 | missing artifact と `verify_runtime_acceptance_with_browser_dirs_and_hints` を接続する。要求集合充足という骨格と、browser / generated-app scanner という create 定義が一つの呼び出しに混在する。 |
| [profile差し込み] | 同 659–675 | invariant check と `DomainProfile::verify_final` を実行し、profile 固有 final verification の成否を取り込む。 |
| [create差し込み] | 同 676–690 | 外部 `CompletionContract` の verify command 結果を `external_ok` に接続する。契約推論と実行順序が create acceptance に埋め込まれている。 |
| [create差し込み] | 同 691–725 | production build failure、browser probe、release gate の順で create の動的 evidence を裁定する。 |
| [create差し込み] | 同 726–735 | FF-1 の interaction qualification を release gate に強制し、heuristic-only evidence の `full` 昇格を拒否する。拒否原則自体は骨格だが、primary/state/restart の定義は create 固有。 |
| [create差し込み] | 同 737–743 | create の gate applicability と実行 telemetry を求める。 |
| [骨格] | 同 744–757 | applicable なのに未実行の gate を検出し、acceptance / release gate を failure にする。未実行ゲートから assurance を獲得させない偽装拒否。 |
| [profile差し込み] | 同 758–764 | `DomainProfile::run_behavior_probe` を実行する。 |
| [骨格] | 同 765–781 | profile probe の `failed` / `partial` / `static` を release gate の上限へ写像する。具体的な probe 意味は profile 側。 |
| [骨格] | 同 782–805 | final acceptance、runtime acceptance、earned assurance、admission cap、release-quality completion と next action を最終確定する。 |
| [骨格] | 同 832–850 | primary reason の優先順位を決め、non-full の正直な理由を選ぶ。 |
| [骨格] | 同 851–883 | acceptance が通っても release / final が non-full なら recovery handoff を要求する。 |
| [骨格] | 同 884–1000 | `ultra_final_acceptance` event に verdict、assurance、gate、reason を投影する。イベント名と schema は凍結対象。 |
| [骨格] | 同 1006–1072 | artifact、profile、runtime、contract、gate、browser の failure を `VerificationReport` に合流し、required failure を acceptance failure にする。個々の evidence 定義は各差し込み側に残る。 |
| [create差し込み] | `final_acceptance_evidence_arbitration` 1089–1104 | browser behavior evidence の create 固有 arbitration に委譲する。 |
| [create差し込み] | `acceptance_dependency_deterministic_reconcile_needed` 2853–2861 | dependency repair を再試行する条件を決める。最終値を直接出さないが、create acceptance の実行順序を変える。 |
| [create差し込み] | `ultra_contract_runtime_acceptance_report` 2863–2919 | profile と capability から現行 create 用の evidence / obligation を構成する。呼び出される requirement hook の実装自体は profile 差し込み。 |
| [要裁定] | 同 2920–2930 | 一般的な要求充足を create 固有 scanner に委譲する混在境界。D-0b では同値な adapter のまま分割する。 |

`final_acceptance_contract_attribute_guidance`（1156–1172）以下の guidance / target-path helper は、失敗後の修復指示を作るだけで verdict や assurance を決定しない。evidence retry の source snapshot や deterministic remedy helper も同様なので、裁定 authority には数えない。ただし D-0b で実行順序を動かす際には create adapter の回帰対象に含める。

### 1.2 `src/planner/final_acceptance_contract.rs`（FF-1b）

このモジュールは現在、裁定値を直接発行しない。FF-1 で gate が拒否された後に、どの instrumentation を修復対象にするかを決める create 固有の recovery adapter である。

| 分類 | 関数・行範囲 | 間接的な決定 |
|---|---|---|
| [create差し込み] | `issue_for_hook_status` 10–28 | hook status を primary/state/restart の契約属性 issue に写像する。 |
| [create差し込み] | `guidance_for_hook_status` 30–42 | status ごとの修復 guidance を選ぶ。 |
| [create差し込み] | `target_paths` 44–59 | interaction 契約修復の対象 path を選ぶ。 |
| [create差し込み] | `issues_for_context` 61–97 | report 中の trigger から必要な instrumentation issue を列挙する。 |
| [create差し込み] | `report_contains` 99–108 | report が create interaction failure trigger を含むか判定する。 |

`push_unique`（110–114）は重複排除だけであり裁定点ではない。FF-1b の guidance が成功したこと自体を assurance evidence にしてはならない。

### 1.3 interaction qualification の所在

FF-1 の実体は `src/planner/interaction_qualification.rs` にある。

| 分類 | 関数・行範囲 | 決定または上限 |
|---|---|---|
| [create差し込み] | `qualify_interaction_evidence` 14–72 | contract probe mode、primary interaction、state transition、必要時の restart を検査し、`full_eligible` / `passed` または failed / heuristic-only を決める。 |
| [create差し込み] | `contract_requires_restart` 74–84 | create 契約から restart evidence の必須性を決める。 |
| [create差し込み] | `enforce_release_gate` 86–107 | interaction gate が一度 pass していても qualification が full-eligible でなければ failed へ落とし、理由を加える。 |
| [create差し込み] | `read_qualification` 109–123 | qualification evidence の unreadable / missing を fail-closed で扱う。 |

125–189 行の field extraction helper は値の読み出しだけで、新たな裁定規則を持たない。

### 1.4 `src/planner/assurance.rs`

| 分類 | 関数・行範囲 | 決定または写像 |
|---|---|---|
| [create差し込み] | `production_build_failed_release_gate` 21–30 | build failure を create release gate failure にする。 |
| [create差し込み] | `final_acceptance_release_gate` 32–141 | Next.js / capability から browser gate applicability を決め、probe の pass / partial / failed を release gate に統合する。 |
| [create差し込み] | `acceptance_gate_telemetry` 143–153 | profile / create signal から browser と interaction gate の applicability を決める。 |
| [骨格] | 同 154–164 | applicable gate の observed status を安定した execution telemetry に写像する。 |
| [create差し込み] | `interaction_gate_required` 166–197 | browser interaction を必須にする profile / capability / evidence 条件を定義する。 |
| [骨格] | `gate_execution_status` 199–211 | applicability と observed execution を `performed` / `not_performed` / `not_applicable` に写像する。 |
| [骨格] | `gate_status_disconnected` 213–218 | applicable gate の未実行を検出する。 |
| [骨格] | `acceptance_gates_disconnected_reason` 220–243 | disconnected gate の fail-closed reason を生成する。 |
| [骨格] | `mark_release_gate_profile_behavior_failed` 245–265 | profile probe の上限を release gate に反映する。 |
| [create差し込み] | `runtime_acceptance_unverified_release_reasons` 267–300 | create runtime evidence が unverified である理由を列挙する。 |
| [create差し込み] | `interaction_probe_performed_for_run` 302–321 | create interaction probe の実行済み判定を行う。 |
| [create差し込み] | `runtime_acceptance_has_buildable_nextjs_boundary` 323–337 | Next.js build boundary の有無を判定する。 |
| [create差し込み] | `requires_canvas_surface` 339–355 | create 成果物が canvas surface を要求するか決める。 |
| [骨格] | `append_release_gate_observation_failures` 357–400 | release gate の non-pass を acceptance report の blocking / inconclusive failure に投影する。 |
| [骨格] | `ReleaseEvidenceStatus::as_status` 409–417 | evidence status の安定した文字列表現を与える。具体的 status の意味は create adapter。 |
| [create差し込み] | `browser_release_gate_with_options` 449–597 | browser evidence を、明示 failure は failed、missing / unavailable は partial、全条件 pass は pass に分類する。 |
| [create差し込み] | `nextjs_dev_route_release_evidence` 599–617 | Next.js dev route の evidence を収集する。 |
| [create差し込み] | `interaction_probe_infrastructure_failure_reason` 628–631 | probe 自体の障害を application behavior failure と区別する。親 gate はどちらも failed だが、reason と recovery 分岐が変わる。 |
| [create差し込み] | `release_gate_canvas_surface_missing` 682–695 | canvas surface 欠落を failure にする。 |
| [create差し込み] | `release_interaction_surface_authoritative` 697–708 | canvas evidence について interaction probe を authority にできるか決める。 |
| [create差し込み] | `release_interaction_canvas_marker` 710–732 | authoritative interaction evidence から canvas marker を抽出する。 |
| [create差し込み] | `read_release_evidence` 734–774 | missing を unavailable、unreadable / invalid を failed として fail-closed に読む。 |
| [create差し込み] | `interaction_probe_unavailable_reason` 776–780、`interaction_probe_unavailable_reason_value` 782–784 | probe unavailable を evidence failure と区別し、親 gate の partial 分岐へ渡す。 |
| [create差し込み] | `classify_release_evidence_json` 818–887 | browser JSON を pass / failed / unavailable に分類する。 |
| [create差し込み] | `explicit_release_evidence_failure` 889–961 | evidence 内の明示 failure を検出する。 |
| [create差し込み] | `release_evidence_has_required_detail` 963–1005 | browser evidence の必須 detail を定義する。 |
| [create差し込み] | `is_release_evidence_unavailable_status` 1066–1075 | unavailable と failed の境界を定義する。 |
| [骨格] | `release_gate_final_acceptance_status` 1158–1167 | `pass` / `not_applicable` を `full_success`、`partial` を `partial`、`failed` / unknown を `incomplete` に写像する。 |
| [骨格] | `runtime_acceptance_status` 1169–1180 | report の pass / inconclusive / unverified / not-checked を runtime status に写像する。 |
| [要裁定] | `assurance_for_completion` 1182–1201 | data、generic、その他を hard-code した初期 assurance。profile 所有が漏れており、generic の `reduced` も四階層外だが互換値である。 |
| [要裁定] | `earned_assurance_for_completion` 1203–1235 | data probe / profile seed を hard-code する。profile 所有を骨格に移すことはできないが、移設時の T30 / B-2j 互換を先に固定する必要がある。 |
| [骨格] | 同 1236–1291 | non-full、unknown profile、unbound contract、applicable gate 未実行なら `full` 不可とする獲得規則。 |
| [骨格] | `release_quality_completion_status` 1293–1304 | release / final status を completion status に写像する。 |
| [骨格] | `release_gate_next_action` 1306–1315 | verdict から次 action を選ぶ。 |
| [骨格] | `release_recovery_needed` 1318–1324 | recovery が必要な non-full 状態を判定する。 |
| [骨格] | `release_recovery_acceptance_layer` 1326–1335 | recovery reason の acceptance layer を選ぶ。 |

### 1.5 `src/completion_metadata.rs` と data 後段

| 分類 | 関数・行範囲 | 決定または投影 |
|---|---|---|
| [要裁定] | `apply_config_completion_metadata` 11–39 | 12–24 は profile metadata の補完。26–38 は data、generic、その他へ assurance seed を hard-code し、既存値も上書きする。T30 の早期失敗互換を守りつつ profile / intent のどちらが所有するか要裁定。 |
| [profile差し込み] | `apply_config_completion_projection` 41–45 | data 固有の terminal evidence 再導出を dispatch する。 |
| [骨格] | 同 46–51 | 再導出後の assurance に全 profile 共通の admission cap を再適用する。 |
| [profile差し込み] | `src/completion_metadata/data.rs::apply_snapshot` 7–11 | data evidence から snapshot assurance を再構成する。 |
| [profile差し込み] | `src/completion_metadata/data.rs::apply_terminal_projection` 13–22 | B-2j。data かつ final が `full_success` の場合だけ E1–E4 evidence から assurance を再導出する。 |
| [profile差し込み] | `src/completion_metadata/data.rs::completion_assurance` 24–36 | `DataAssurance` と理由文字列を completion assurance に写像する。 |

B-2j は terminal event を信用して `full` をコピーするのではなく、保存済み evidence を再読して獲得値を作る。その後に admission cap が再適用されるため、draft profile が `full` を得る経路はない。

### 1.6 `src/eval_events.rs` の投影系

| 分類 | 関数・行範囲 | 決定または投影 |
|---|---|---|
| [骨格] | `CompletionSnapshot::empty` 271–336 | 欠落 event を full と見なさない保守的 default を定義する。 |
| [骨格] | `latest_completion_snapshot` 439–500 | completion authority となる最新 event を選び、lifecycle profile と reinference を overlay する。 |
| [骨格] | `apply_tui_command_stop_projection` 519–703 | authoritative stop event の status、assurance、gate、reason を terminal projection に上書きする。run-stop 経路ではこの後も admission cap が適用される。 |
| [骨格] | `project_completion` 732–840 | process `ok` と snapshot から runtime / final / release status、terminal / task status、assurance、next action を正直終端へ投影する。 |
| [骨格] | `projected_assurance_from_snapshot` 850–915 | non-full を昇格させず、`full` だけを unknown profile、non-full acceptance、unbound contract、applicable gate 未実行で `partial` に降格する。`earned_assurance_for_completion` と重複する anti-fake authority。 |
| [create差し込み] | `interaction_unverified_probe_unavailable` 942–947 | interaction probe unavailable を create 固有 reason から認識し、表示上の partial / action に影響させる。 |
| [骨格] | `snapshot_from_completion_event` 1997–2252 | `plan_final_contract`、`ultra_final_acceptance`、`tui_command_stop`、`run_stop` の既存 field を `CompletionSnapshot` に取り込む。 |
| [骨格] | `LifecycleProfileFields::apply_to` 2275–2308 | lifecycle で確定した profile、inference source、port、contract origin を snapshot に投影する。profile 名の既知判定は現行 profile dispatch。 |
| [骨格] | `ProfileReinferenceFields::apply_to` 2388–2400 | 後発の profile reinference を snapshot に反映し、admission / assurance の入力を更新する。 |
| [骨格] | `has_completion_fields` 2495–2500 | event を completion authority 候補にできるか判定する。 |
| [骨格] | `runtime_acceptance_status_from_bool` 2502–2513 | 旧 bool field を `pass` / `failed` に読む後方互換投影。 |
| [骨格] | `terminal_status` 2515–2529 | process failure、partial / failed gate、final acceptance から `completed` / `partial` / `incomplete` を決める。 |
| [骨格] | `task_status` 2531–2547 | 同じ authority から task の `completed` / `partial` / `failed` / `incomplete` を決める。 |
| [骨格] | `release_quality_completion` 2549–2558 | gate / final status を release-quality completion に写像する。 |
| [骨格] | `next_action` 2560–2575 | honest terminal の次 action を決める。 |
| [骨格] | `render_tui_command_completion_summary` 2882–2940 | TUI 表示用に non-completed / non-partial の terminal / task / action を整える。表示投影であり、保存 verdict の authority ではない。 |

`command_returned_incomplete`（1921–1928）と incomplete notice / phase breakdown は表示判定だけである。B-2j の `full` 再導出はこのファイルにはなく、1.5 の `completion_metadata/data.rs::apply_terminal_projection` が `project_completion` 後に行う。

呼び出し側にも一つ未解決差がある。direct CLI、run-stop、TUI は completion metadata と admission cap を通る一方、`src/runs.rs` 316–350 の run inventory は `eval_events::project_completion` を直接呼ぶ。inventory が D-0c の byte-parity 境界に含まれるか、また後段 metadata を統一するかは **[要裁定]** とし、D-0b で便乗修正しない。

### 1.7 `src/planner/profiles/data/runtime.rs`

| 分類 | 関数・行範囲 | 決定または写像 |
|---|---|---|
| [骨格] | `DataAssurance::as_str` 24–32、`behavior_status` 34–42 | profile 結果を四階層文字列と behavior status に写像する。型の置き場所は data だが、階層写像の形は骨格候補。 |
| [profile差し込み] | `run_manifest_checks_with_goal` 56–85 | pipeline、内部 check、rerun、classify、summary 保存の data profile 実行順序。 |
| [profile差し込み] | `assurance_from_evidence` 87–108 | pipeline evidence 欠落は failed、run evidence 欠落は static、存在時は E1–E4 を再読して分類する。B-2j の source of truth。 |
| [profile差し込み] | `run_pipeline` 119–143 | manifest-bound pipeline probe の実行結果を記録する。 |
| [profile差し込み] | `run_rerun` 145–164 | reconciliation / rerun probe の実行結果を記録する。 |
| [profile差し込み] | `classify` 166–181 | 未実行は static、pipeline / reconciliation / rerun failure は failed、claims / results failure は partial、全 E1–E4 pass は full とする data 契約。 |

`run_manifest_checks`（52–54）は wrapper、`adapters`（110–117）は catalog 解決、`write_summary` / `read_json` は永続化 helper である。`has_all_manifest_adapters`（203–205）は admission validation の材料で、個々の run verdict は出さない。

### 1.8 profile manifest と admission cap（B-3）

| 分類 | 関数・行範囲 | 決定または上限 |
|---|---|---|
| [profile差し込み] | `src/planner/profile_manifest/schema_v1.rs::ManifestMetadata` 8–13、`ManifestStatus` 29–43 | profile が `draft` / `admitted` を宣言する閉じた schema。 |
| [profile差し込み] | `src/planner/profile_manifest.rs::ManifestV1::from_toml` 95–100、`resolve` 102–126、`validate_structure` 128–131 | manifest の parse / catalog binding / validation を fail-closed に行う。 |
| [profile差し込み] | `src/planner/profile_manifest.rs::nextjs_manifest` 134–140 | admitted status を含む Next.js manifest を提供する。data / Next.js の manifest はどちらも現在 `admitted`。 |
| [profile差し込み] | `src/planner/profile_admission.rs::status` 6–17 | generic は admitted、data / Next.js は manifest、unknown profile は draft とする現行 dispatch。 |
| [骨格] | `src/planner/profile_admission.rs::cap_assurance` 20–22、`cap_assurance_for_status` 24–33 | draft の `full` / `partial` を `static` + `profile_not_admitted` に上限制限し、`static` / `failed` / legacy `reduced` は昇格させない。 |

cap の適用点は `final_acceptance.rs` 798–802 と `completion_metadata.rs` 41–51 の二箇所である。「profile_manifest の admission cap」という機能名に反して、cap 本体は `profile_admission.rs` にある。この二重適用は parity が取れるまで統合しない。

### 1.9 委譲先にある裁定 authority

指定モジュールから verdict が委譲されるため、完全な境界を作るには次も D-0b の対象として認識する必要がある。

| 分類 | 関数・行範囲 | 決定または写像 |
|---|---|---|
| [要裁定] | `src/minimal_loop/evidence.rs::verify_runtime_acceptance_with_browser_dirs_and_hints` 979–1314 | 990–999 と 1247–1291 の要求集合充足 / primary reason は骨格候補。1001–1246 の file、browser、generated-app scanner は create 固有で、現在は同一関数に混在する。 |
| [骨格] | `src/minimal_loop/evidence.rs::refresh_runtime_acceptance_report` 1326–1367 | behavior arbitration 後に blocking / inconclusive / missing evidence から pass を再計算する。 |
| [create差し込み] | `src/minimal_loop/behavior_evidence.rs::arbitrate_final_acceptance` 117–333 | browser probe の未実行、infrastructure failure、behavior pass / fail / unverified / static を裁定する。 |
| [create差し込み] | `src/minimal_loop/behavior_evidence.rs::behavioral_decision` 335–438 | web behavior key と期待値を create evidence に写像する。 |

runner 側の委譲境界は次の通りである。

| 分類 | 関数・行範囲 | 決定または写像 |
|---|---|---|
| [create差し込み] | `src/planner/runner.rs::completion_contract_required` 3486–3514 | profile hook、web/app signal、interactive capability から現行契約の必須性を決める。 |
| [create差し込み] | `run_ultra_final_browser_checks_before_arbitration` 5194–5230、`browser_interaction_probe_options` 5232–5277 | browser / interaction probe の実行順序と persistence / text / echo 条件を決める。 |
| [create差し込み] | `report_has_production_build_failure` 5279–5295、`ultra_browser_probe_required` 5297–5316 | build failure を browser gate より優先し、browser probe applicability を決める。 |
| [要裁定] | `inferred_required_capabilities` 5521–5523、`inferred_required_evidence` 5525–5535、`inferred_required_obligations` 5537–5571 | profile requirement と capability-to-evidence の create 合成が混在する。 |
| [profile差し込み] | `run_profile_behavior_probe` 5573–5603 | profile probe を dispatch し、実行 error を failed observation にする。 |
| [create差し込み] | `external_contract_ok_after_runtime_arbitration` 5627–5635、`external_contract_report_covered_by_runtime_arbitration` 5637–5652、`external_profile_failure_covered_by_runtime` 5654–5676 | 外部契約 failure を runtime evidence で同値に cover できる条件を決める。 |

`src/planner/runner.rs` は growth tripwire なので、D-0b で新しい規則を置かず、既存 adapter から骨格を呼ぶ最小 wiring に留める。

なお direct / TUI の隣接 authority として、`src/lib.rs::effective_direct_status`（488–503）と `src/tui/slash.rs::effective_terminal_status`（655–670）は interaction probe unavailable の completed を partial にする。これらも正直終端 parity に含める。

## 2. IntentContract の要求スロット案

### 2.1 原則

`IntentContract` は manifest 内の自由記述コードではなく、Rust 側で versioned registry に登録する型付き契約とする。profile manifest は catalog 登録済み capability / check / adapter の ID を宣言できるが、任意 shell や assurance 分岐を埋め込めない。

```rust
struct IntentContract {
    id: IntentId,
    version: ContractVersion,
    contract_ref: ContractDocumentRef,
    requirements: Vec<EvidenceRequirement>,
    plan: PlanSkeleton,
    assurance: AssurancePolicy,
    required_profile_hooks: BTreeSet<ProfileHookId>,
}

struct EvidenceRequirement {
    id: RequirementId,
    binding: RequirementBinding, // capability / catalog check / obligation
    stage: EvidenceStage,
    expected: ExpectedOutcome,   // pass / expected-failure / observation
    execution: ExecutionRule,    // must_execute / static_allowed
    impact: RequirementImpact,   // blocking / degradable / full-only
    lineage: Option<LineageKey>, // before/after で同じ oracle を要求できる
}
```

要求は run-local な evidence epoch と source provenance を持つ。単に同名 file や古い JSON が存在するだけでは `must_execute` を満たさない。裁定器は requirement ID ごとに `passed` / `failed` / `not_executed` / `inconclusive` を受け取り、intent の tier predicate、profile admission cap、terminal projection の順で一方向に値を狭める。投影層から evidence 層への昇格は許さない。

現行 `CompletionContract`（`src/minimal_loop/completion.rs` 27–50）の paths、verify commands、profile、goal、capabilities、oracles、evidence、obligations は binding の入力として保持する。ただし intent identity、stage、expected polarity、lineage は持たないため、`IntentContract` 自体の代用にはしない。

### 2.2 スロット

| スロット | 宣言内容 | 既存 capability / checks との関係 |
|---|---|---|
| intent identity | `id`、version、`contract_ref`、`full` の規範文書 | `UltraPlan.intent` を一度だけ registry 解決する。unknown / unsupported intent から `full` を得ない。unknown の既存挙動をいつ fail-closed に変えるかは [要裁定]。 |
| evidence requirements | requirement ID、binding、stage、期待極性、実行要件、lineage | capability は `required_evidence_for_capability` で requirement に展開する。manifest `checks` は catalog ID へ解決し、profile が adapter を供給する。`CompletionContract` の paths / commands / oracles / evidence / obligations は create adapter の入力にする。 |
| plan skeleton | 順序付き `PhaseRole`、各 role の entry / exit requirement、再試行可否 | intent が意味的な順序を所有し、profile が具体 phase、prompt、check scope に展開する。planner が生成した phase も skeleton に対して lint する。 |
| assurance mapping | requirement outcome から `full` / `partial` / `static` / `failed` への単調な predicate | evidence 未実行から full 不可、failed requirement を projection で隠さない、admission cap は earned value の後に適用、terminal projection は値を上げない。 |
| profile hooks | requirement / plan / oracle / probe / repair target / admission の型付き hook | profile は「どう測るか」を実装する。intent は「何を、いつ、どの組み合わせで満たすか」を所有する。 |

四階層の共通写像枠は次の通りとし、どの requirement を blocking / degradable / full-only とするか、および partial に必要な最小動的集合は各 intent contract が宣言する。profile が階層式を差し替えることはできない。

| assurance | 共通条件 |
|---|---|
| `full` | 全 full-required requirement が当該 run で実行済みかつ期待結果を満たし、contract が束縛済みで、全 applicable gate が performed、profile が admitted。 |
| `partial` | intent が定める最小動的集合は実行・充足したが、degradable / full-only requirement に failure、inconclusive、unavailable のいずれかがあり full 条件に届かない。blocking failure は含めない。 |
| `static` | qualifying dynamic evidence が未実行で、静的 artifact / binding だけを確認でき、観測済み blocking failure はない。未実行 gate を pass と扱わない。 |
| `failed` | intent が blocking と宣言した requirement の観測済み failure、または実行可能境界そのものの欠落。具体例は create の production build / data pipeline failure、fix の after / regression failure。 |

この表は既存分類を強めたり緩めたりする新仕様ではなく、現行 create / data の結果を lossless に表現する型枠である。例えば data の claims / results failure が `partial`、pipeline / reconciliation / rerun failure が `failed` である区別は、そのまま requirement impact に写す。

### 2.3 DomainProfile に要求する差し込み点

既存 method を捨てず、まず adapter として次の型付き面を追加する。名称は D-0b で確定する。

| 追加要求 | 現行の対応物 | 所有境界 |
|---|---|---|
| `admission_status()` | `profile_admission::status`、manifest metadata | profile が status を返し、骨格が共通 cap を適用する。 |
| `intent_requirements(ctx)` | `infer_required_capabilities` / `infer_required_evidence` / `infer_required_obligations`、expected paths | profile が登録済み requirement binding を追加する。tier formula は返さない。 |
| `intent_plan_fragment(ctx)` | manifest plan、preset plan、`deterministic_default_plan` | intent の `PhaseRole` を profile 固有 phase へ展開する。順序制約は intent 側。 |
| `intent_oracle_bindings(ctx)` | `build_oracle`、manifest check、verify command binding | catalog 登録済み oracle と stage を結ぶ。任意 command を assurance rule にしない。 |
| `execute_intent_probe(request)` | `run_behavior_probe`、data `run_manifest_checks_with_goal` | profile 固有 probe を実行し、共通 `EvidenceObservation` を返す。 |
| `intent_repair_targets(ctx)` | `evidence_repair_target_paths`、FF-1b target paths | non-full 後の修復対象だけを返す。修復実行を evidence pass とみなさない。 |

現行 `DomainProfile` の `verify_final`（`src/planner/profile.rs` 117 行）、plan hook（151 行）、`build_oracle`（201 行）、evidence target（244 行）、requirement inference（252–273 行）、behavior probe（276–284 行）は上記 adapter の出発点になる。

### 2.4 共通不変条件

1. `full` は、contract が要求する全 `must_execute` requirement が当該 run / stage で観測され、すべて pass したときだけ獲得できる。
2. applicable gate の `not_performed`、unbound contract、unknown profile は `full` を拒否する。
3. profile probe が返した上限と profile admission cap を越えて昇格しない。
4. terminal / TUI / inventory は一つの adjudication result を投影し、process failure を `completed` と表示しない。
5. event replay、B-2j の evidence 再読、UI projection のいずれも、元の earned assurance より強い値を証拠なしで作らない。
6. failure reason の優先順位と文字列は D-0 の挙動保存対象である。

四階層外の `reduced` は **[要裁定]** である。現行 generic no-contract path と event consumer の byte compatibility のため、D-0b では outcome 表現に legacy passthrough として残し、勝手に `static` / `partial` へ正規化しない。admitted intent の新しい policy は四階層だけを生成する。

## 3. create intent のスロット写像

| IntentContract スロット | 現行実装 | D-0b adapter |
|---|---|---|
| intent identity | `src/planner/intent.rs` 1–10 が create / fix / research を検出し、`src/planner/ultra_plan.rs` 3–16 が保持する。ただし final acceptance は現在 `plan.intent` で契約を選ばない。 | `create@v1` を registry に登録し、現行 acceptance をその実装として束ねる。 |
| `full` の契約参照 | `docs/dev/generality.md` の earned assurance、profile 契約、data では `docs/dev/data-profile-contract.md` の E1–E4。browser interaction / build の意味はコードと ledger に分散。 | create full の規範参照を一箇所から列挙する。D-0 では既存意味を変更しない。 |
| evidence requirements | `final_acceptance.rs` 594–646、runner 5521–5571、`CompletionContract`、`DomainProfile::infer_required_*`。 | 現行 paths / commands / capabilities / oracles / evidence / obligations を `EvidenceRequirement` へ lossless に包む。 |
| requirement satisfaction | `minimal_loop/evidence.rs` 979–1314、`behavior_evidence.rs` 117–438、interaction qualification 14–123、data runtime 56–181。 | 共通集合判定と create scanner を分けるが、status、reason、実行順序は同じ adapter で保存する。 |
| plan skeleton | `UltraPlan`、`deterministic_default_plan` 18–40、profile manifest plan、profile preset。 | `inspect/prepare -> implement -> verify -> adjudicate` の role に現行 profile phase を写像する。role 名は外部 event を変えない内部値。 |
| create 実行順序 | `final_acceptance.rs` 647–805 と retry wrapper 542–587。 | runtime -> profile verify -> external contract -> build/browser/qualification -> profile probe -> adjudicate を `create@v1` が宣言する。 |
| assurance mapping | `assurance.rs` 1158–1291、data classifier、completion metadata、profile admission cap。 | 共通 evaluator に現行 predicate をそのまま移し、create adapter から gate observation を渡す。 |
| profile hook | `verify_final`、`build_oracle`、`infer_required_*`、`run_behavior_probe`、data manifest checks。 | 2.3 の型付き hook へ薄く適合させる。 |
| terminal projection | `eval_events.rs` 439–915、1997–2575、completion metadata。 | intent を見ない共通 `AdjudicationResult -> CompletionProjection` として抽出する。 |

create における browser interaction、pipeline probe、build oracle は「何が evidence か」の定義なので create 差し込みである。一方、それを実行する Next.js / data adapter と manifest check は profile 差し込みである。この二つを同じ trait method に押し込まず、intent requirement と profile binding の二段にする。

## 4. fix intent による rule of two の検算

fix の `full` を仮に「開始時に失敗する再現が、終了時に同じ判定で通り、既存検証に回帰がない」とする。実装や `fix@v1` の registry 登録は行わない。

| IntentContract スロット | fix 仮写像 | 骨格の検算結果 |
|---|---|---|
| identity / contract ref | `fix@v1`、将来の規範文書（例: `docs/fix-intent-contract.md`） | 規範文書は未作成。実装前に full の意味と inconclusive 条件を seal する必要がある。 |
| `before_fails` | stage=`before_change`、expected=`expected_failure`、must_execute、reproducer lineage=`R` | 現行 `CompletionContract` には stage / polarity / lineage がなく、そのままでは載らない。2.1 の field 追加が必要。 |
| `after_passes` | stage=`after_change`、expected=`pass`、must_execute、同じ lineage=`R` | 別の容易な check へのすり替えを lineage 一致で拒否する。after evidence は before より新しい run-local epoch が必要。 |
| `no_regression` | stage=`after_change`、profile が束縛した catalog regression check 集合、全件 must_execute / pass | profile が具体 check を選ぶが、全件充足を full 条件にするのは fix contract。 |
| plan skeleton | `reproduce_before -> repair -> reproduce_after -> regression -> adjudicate` | create の implement-first 順序を骨格に焼き込めないことを確認できる。 |
| full | 三要求がすべて実行済みで pass。`before_fails` と `after_passes` は同一 lineage。 | 共通 requirement evaluator、provenance、correlation があれば表現可能。 |
| partial | before failure と after pass は確認済みだが、regression 集合に inconclusive / unavailable が残る、という仮案。 | full にはしない。baseline 非再現を partial と failed のどちらにするかは [要裁定]。 |
| static | repro / regression が実行されず、静的変更や check binding だけが存在する。 | 未実行から full を得ない共通規則で表現可能。 |
| failed | after が失敗、または実行済み regression が失敗。 | baseline が開始時に pass して「再現しない」場合を failed / inconclusive のどちらに置くかは [要裁定]。 |
| profile hook | reproducer / regression / build adapter と対象 path を profile が返す。 | profile は fix の tier formula を変更できない。 |

この検算で見つかった骨格案の欠陥と修正は次の通りである。

- **順不同集合の欠陥**: requirements の有無だけでは before / after を表せない。`stage` と plan role の exit 条件を追加する。
- **期待極性の欠陥**: pass だけを成功とするモデルでは「正しく失敗した baseline」を表せない。`ExpectedOutcome::ExpectedFailure` を追加する。
- **すり替えの欠陥**: before と after が別 command でも集合上は満たせる。catalog oracle ID と `lineage` の一致を要求する。
- **stale evidence の欠陥**: 前回 run の after-pass を再利用できる。run ID、evidence epoch、workspace revision / digest の provenance を requirement observation に持たせる。
- **回帰集合の欠陥**: `no_regression` を単一 bool にすると未実行 check を隠せる。profile が束縛した check ID 集合を展開し、各要素を `must_execute` として裁定する。

以上により、骨格は create の「成果物を作って browser / pipeline / build を通す」という前提を持たず、時系列と極性の異なる二つ目の intent を表現できる。

## 5. D-0b 移行方針

### 5.1 抽出順序

1. 現行 event と completion projection の compatibility fixture を先に固定する。event 名、field 名 / 型 / 欠落時 default、status、assurance、reason、配列順を記録する。
2. `src/planner/adjudication/` に leaf module を追加し、typed requirement outcome、四階層 policy、gate execution、admission cap、terminal projection の純粋型を置く。`runner.rs` と `final_acceptance.rs` は呼び出し wiring だけにする。
3. `assurance.rs` と `eval_events.rs` の重複した full downgrade / anti-fake 規則を、まず既存関数を呼ぶ adapter で一箇所に集める。parity 前に reason 優先順位や default を整理しない。
4. `create@v1` の `IntentContract` adapter を作り、現行 requirement inference、probe、build oracle、interaction qualification、external contract、retry 順序をそのまま包む。
5. `plan.intent` から registry を解決する。ただし D-0b で実行可能にするのは既存 create 経路だけとし、fix は設計 fixture に留める。unknown intent の挙動変更は別の明示 migration にする。
6. profile hard-code を 2.3 の `DomainProfile` adapter の後ろへ移す。data E1–E4、Next.js interaction、manifest admission の意味は変えない。
7. D-0c parity gate を全件通してから旧 wrapper の削除可否を判断する。guardrail baseline は上げない。

各段階で narrow unit / corpus check を先に行い、最後に `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` を通す。event / assurance は意味同値ではなく、決定的 fixture 上の byte compatibility を要求する。既存 corpus / conformance は全 green を維持し、失敗を通すために verification、acceptance、evidence、release gate を弱めない。

### 5.2 D-0c parity gate

計 12 run を固定 matrix とする。

| profile | matrix | 本数 |
|---|---|---:|
| Next.js | Space / Breakout / Quiz × qwen35 / gemma31 | 6 |
| data | aggregation: qwen35/profile、qwen35/none、gemma31/profile。timeseries: qwen35/profile、qwen35/none、gemma31/none | 6 |

data の六本は二族、二 executor、profile / none arm を混成し、割付を baseline 取得前に固定する。planner その他の条件も pre/post で固定する。

合格条件は次のすべてである。

- 各 row の現行実装 baseline と抽出後で、`runtime_acceptance_status`、`final_acceptance_status`、`release_gate_status`、`assurance_level` / `assurance_reason`、`task_status`、`terminal_status` が一致する。
- 正直終端が 12/12。process / gate failure を `completed/full` として報告する row がない。
- adjudication 関連 event の名前、field の集合 / 型、status / reason 文字列、必要な event 順序が不変である。
- 固定 event / evidence replay では行単位の byte comparison を hard gate とする。live model run の非決定性とコード差分を混同しないため、live 12-run に加えて同一 artifact replay を必須にする。
- 既存 corpus / conformance が全 green。差分が出た場合は原因を証拠化し、baseline や期待値を上げて吸収しない。

## 6. D-0 凍結宣言

D-0a では `src/` と `tests/` を一切変更しない。D-0b / D-0c が parity を完了するまで、次を凍結する。

- verdict、assurance、gate、terminal status の名称、意味、reason 文字列と優先順位
- completion / acceptance event の名称、schema、field 型、欠落時 default、既存 consumer との互換
- create evidence の定義、requirement inference、probe / build / external contract / retry の実行順序
- FF-1 interaction qualification の primary / state / restart 条件と FF-1b repair semantics
- data E1–E4、B-2j 再導出、profile manifest check、draft admission cap
- generic `reduced` の生成・投影・cap 挙動
- intent detector、既存 plan phase / prompt、manifest schema、profile admitted / draft status
- `.anvil/` runtime namespace、既存 `workspace/management/runs/` と `docs/migration/` の履歴 evidence
- `runner.rs` / `minimal_loop/loop_run.rs` の guardrail baseline
- fix intent の runtime 登録、probe 実装、契約文書の seal、または create 以外への新しい裁定分岐

凍結対象を変える必要が出た場合は、D-0 の「挙動保存」から切り離した schema / contract migration として別途裁定する。

## 7. D-0b 実施結果（2026-07-16）

D-0b は `7f26ad0`（骨格抽出）と `e1095ac`（互換証明）で実施した。新しい intent、knowledge、event field、verdict、assurance 値は追加していない。実行可能な経路は従来の create のみで、fix は本書 §4 の設計 fixture のままである。

### 7.1 抽出境界と依存方向

`src/planner/adjudication/` は次の境界になった。

- `requirements.rs`: 宣言済み capability / evidence / obligation ごとの `Pass` / `Failed` / `Unverified` と、現行の pass / inconclusive / primary-reason 集約。
- `core.rs`: create 固有名を持たない `GateObservation`、contract / gate provenance、未実行 gate の拒否、acceptance / assurance 階層写像、release recovery 判定、admission cap。
- `terminal.rs`: event snapshot から assurance、terminal status、task status、release-quality completion、next action への正直終端投影。
- `create.rs`: 現行 create の contract 接続、probe / build / interaction / external-contract / profile-probe の実行順序と evidence 定義。

Rust の module 境界では `create.rs` を `runner` の private child `adjudication_create` として宣言する。create adapter は `crate::planner::adjudication` の骨格 API を import できるが、private child は sibling の骨格 module から import できない。したがって依存方向は **create差し込み → 骨格** だけで、骨格 → create はコンパイル境界で閉じている。`tests/generality_guardrails.rs::adjudication_dependency_direction_stays_create_to_skeleton` が宣言と import に加え、骨格 production に `browser_readiness` / `interaction_evidence` / `nextjs` / create gate 型名が混入しないことを常時監査する。

### 7.2 旧所在から新所在への対応

行番号の旧側は D-0a 棚卸し時点、新側は D-0b 完了時点である。

| 旧: ファイル・行 / 決定点 | 新: module・関数 | 挙動保存上の扱い |
|---|---|---|
| `final_acceptance.rs` 37–51 `contract_origin_for_acceptance` | `adjudication/core.rs` 20–38 同名関数 | event replay 由来の contract provenance 文字列をそのまま移動。 |
| `minimal_loop/evidence.rs` 990–999、1247–1291、1326–1367 の要求集合集約 | `adjudication/requirements.rs` 1–109 `RequirementStatus` / `RequirementOutcome` / `evaluate_requirements`; wiring は `evidence.rs` 1274–1309、1320–1364 | missing / weak / inconclusive の pass 条件と primary-reason 優先順位を保存し、既存 report field は増やしていない。 |
| `assurance.rs` 154–164、199–243 の gate execution / disconnected 判定 | `adjudication/core.rs` 11–18、45–83 `GateObservation` / `execution_status_from_observed` / `gate_status_disconnected` / `disconnected_gate_observations_reason`; create field adapter は `create.rs` 29–54 | applicable だが未実行の gate から獲得できない規則を移動。create 固有 gate 名は adapter が入力し、status / reason は同一。 |
| `assurance.rs` 245–265、357–400 の profile 上限・release observation 投影 | `adjudication/core.rs` 85–119 `profile_behavior_failure_reasons` / `append_gate_observation`; create adapter は `adjudication/create.rs` 56–64、1393–1424 | 共通の failure 合流順を保存し、browser evidence と compile error の読み方は create 側に残した。 |
| `assurance.rs` 1158–1180 の final / runtime status 写像 | `adjudication/core.rs` 121–141 `final_acceptance_status_from_release_gate` / `runtime_acceptance_status`; create wrapper は `create.rs` 66–74 | `full_success` / `partial` / `incomplete` と runtime status の文字列をそのまま移動。 |
| `assurance.rs` 1236–1291 の earned-full downgrade | `adjudication/core.rs` 144–200 `earned_assurance_from_base`; create field adapter は `create.rs` 109–136 | unknown profile、unbound contract、non-full acceptance、未実行 gate の full 拒否を移動。profile seed と legacy `reduced` は `assurance.rs` 3–61 に据え置いた。 |
| `assurance.rs` 1293–1335 の release-quality / next-action / recovery 写像 | `adjudication/core.rs` 202–244; create wrappers は `create.rs` 76–107 | status と reason の選択順をそのまま移動。 |
| `profile_admission.rs` 24–33 の draft cap | `adjudication/core.rs` 246–254 `cap_assurance_for_status`; profile dispatch wrapper は `profile_admission.rs` 8–32 | 適用点は create final acceptance (`create.rs` 737–741) と completion metadata (`completion_metadata.rs` 50) の二箇所を維持し、順序を統合していない。 |
| `eval_events.rs` 850–915 の full 再投影拒否 | `adjudication/terminal.rs` 4–62 `projected_assurance`; create-shaped event field adapter は `eval_events.rs` 855–888 | B-2j を含む既存 earned 値を昇格せず、legacy `reduced` を passthrough する。 |
| `eval_events.rs` 2515–2575 の terminal / task / release / action 写像 | `adjudication/terminal.rs` 64–122; wiring は `eval_events.rs` 736–773 | process failure を completed にせず、既存の表示文字列を byte 保存。 |
| `final_acceptance.rs` 64–114、542–1072、1089–1104、2853–2930 の create orchestration | `adjudication/create.rs` 419–1123 | contract bind、retry、runtime→profile→external→build/browser→qualification→profile probe→adjudicate、event / report 投影を関数単位で移動。骨格判定は上記 core API を呼ぶ。 |
| `runner.rs` 3486–3514、5194–5316、5627–5703 の create 判定補助 | `adjudication/create.rs` 138–417 | contract 必須性、browser probe、build 優先、external-contract coverage を移動。`runner.rs` には private module 宣言と呼び出し wiring を残した。 |
| 旧 `assurance.rs` の browser / release evidence 定義 21–1156 | `adjudication/create.rs` 1125–2179 | create 固有 evidence の意味をまとめて移動。profile probe 実装や manifest check は profile 境界のまま。 |

`final_acceptance.rs` の 744–1072 にあった frozen event schema と failure-report 合流は、create-shaped field を骨格型へ逆流させないため `ultra_final_acceptance_report_inner` の byte-compatible projection adapter として移動した。status、assurance、provenance、cap、terminal の決定 authority は上表の骨格関数である。

### 7.3 残した create 差し込み

| create 差し込み | D-0b 後の所在 |
|---|---|
| `CompletionContract` の要求判定、生成、bind と外部 contract coverage | `adjudication/create.rs` 138–167、340–479 |
| deterministic remedy、dependency reconcile、evidence clear、再試行順序 | `adjudication/create.rs` 481–526、1014–1123 |
| runtime scanner、profile verify、external verify、build、browser、interaction qualification、profile probe の順序 | `adjudication/create.rs` 528–1012 |
| browser / interaction release evidence の定義と fail-closed 読み出し | `adjudication/create.rs` 1125–2179 |
| browser behavior arbitration | `minimal_loop/behavior_evidence.rs` 117–438。create adapter から既存関数を呼ぶ。 |
| FF-1 interaction full qualification | `planner/interaction_qualification.rs` 14–123。条件も reason も変更していない。 |
| FF-1b contract attribute guidance / target | `planner/final_acceptance_contract.rs` 10–108。裁定後の修復差し込みとして据え置き。 |
| profile probe / manifest check 実装 | `DomainProfile` と `planner/profiles/{nextjs,data}/`。profile 差し込みのまま。 |

### 7.4 D-1 予約と未裁定事項

D-0b の `RequirementOutcome` は `kind`、requirement ID、現行互換 status だけを持つ。次の field は **予約のみ** で、型、event、contract、runtime namespace のいずれにも追加していない。

- `stage`: `before_change` / `after_change` などの時系列 slot
- `expected polarity`: pass と expected failure の区別
- `lineage`: before / after が同一 reproducer / oracle であることの相関

これらと fix 契約文書、fix runtime 登録、baseline 非再現の tier は D-1 に委譲する。D-0a の [要裁定] である profile hard-code seed、generic `reduced`、requirement inference と scanner の混在 adapter、completion metadata dispatch、run inventory の投影差、unknown intent の扱いには触れていない。

### 7.5 機械的な挙動保存

`tests/adjudication_compat.rs` は次の6 fixtureを、completion event の全81-key shape、persisted JSONL bytes、runtime / final / release verdict、assurance level / reason、terminal / task / release-quality / next-action の決定的 bytes で固定する。

1. Next.js full
2. Next.js production-build failed
3. Next.js interaction partial / probe unavailable
4. data full
5. data static
6. data failed

実 emitter の81-key集合は `ultra_final_acceptance_event_carries_generic_static_assurance` でも固定した。`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` は green で、全体実行には unit、6 byte fixtures、conformance、corpus regression、data profile conformance、guardrail、doc tests が含まれる。D-0c の live 12-run matrix は §5.2 の後続 gate であり、D-0b の fixture replay と混同しない。

### 7.6 行数予算

物理行数と `generality_guardrails` の production / `cfg(test)` 分類を D-0b 完了値で固定した。

| 新 module | total | production | test / cfg(test) |
|---|---:|---:|---:|
| `adjudication/mod.rs` | 7 | 7 | 0 |
| `adjudication/core.rs` | 255 | 255 | 0 |
| `adjudication/create.rs` | 2,179 | 2,165 | 14 |
| `adjudication/requirements.rs` | 184 | 110 | 74 |
| `adjudication/terminal.rs` | 124 | 124 | 0 |
| **合計** | **2,749** | **2,661** | **88** |

`src/planner/final_acceptance.rs` は D-0b 開始時の 2,931 行から 2,209 行へ **722 行削減**した。guardrail baseline も 2,942 / 2,937 production から 2,209 / 2,204 production へ引き下げた。`assurance.rs` の baseline も 1,311 行から、[要裁定] seed だけを残す 63 行へ引き下げた。既存 baseline を増やした箇所はない。

## 8. D-1 fix intent v0 実施結果（2026-07-16）

### 8.1 契約の封緘と registry

規範文書を [`fix-intent-contract.md`](fix-intent-contract.md) v0 として実装前に
fixed とした。`adjudication/contract.rs` の registry は、従来 create 経路を
`create/current` adapter として保持したまま `fix/v0` を追加し、次の3要求と
4つの plan role を宣言する。

| fix requirement | stage | expected polarity | execution / correlation | plan role |
|---|---|---|---|---|
| `before_fails` | `before` | `failure` | `must_execute`、lineage必須 | `reproducer_before` |
| `after_passes` | `after` | `success` | `must_execute`、`before_fails`と同一lineage、より新しいepoch | `reproducer_after` |
| `no_regression` | `after` | `success` | profileがrun開始時に束縛した全件を`must_execute` | `regression` |

`repair` role は `before_fails` 成立後だけに置く。`FixRuntime::for_plan` は文字列の
独自分岐ではなくこの registry を解決し、unknown intent の従来挙動は変更しない。

### 8.2 実行順序と evidence provenance

実装は `planner/fix_runtime.rs` に閉じ、`ultra_plan_flow.rs` には初期化、before
差し込み、fix終端へのdispatchだけを置いた。実行順は次で固定した。

1. run開始時にprofile regression binding集合を凍結し、run IDを発行する。
2. 第1 phaseのStepPlanは、expected_result=`fail`のverify step 1件、verify
   command 1件、expected paths 0件だけを受理する。profile scaffold、presetの
   deterministic template、quality retry、executorを通さず、正規化したRを
   bounded verifierで直接実行する。
3. Rが開始時から成功した場合は修正phaseへ進まず、
   `failed(baseline_not_reproduced)`で停止する。blocked / timeout / unavailableも
   未実行成功へ読み替えない。
4. repair phase群の終了後、保存した同一bindingのRを再実行する。commandの
   正規化済み本文と安定hashをbinding / lineageとし、after epochがbeforeより
  新しいことを共通evaluatorで検証する。
5. F2成功時だけ、凍結済みprofile regression集合を全件実行する。集合の欠落、
   重複、差し替えは`regression_set_mismatch`等でfailedにする。途中のprofile
   promotionはfixでは無効にし、run開始時の集合を後から弱いprofileへ替えない。

各観測は `evidence/fix-<run-id>-*.json` に `intent`、contract version/ref、
requirement ID、binding ID、`stage`、`expected`、`lineage`、run-local `epoch`、
run ID、`executed`、outcome、reasonを保存する。completion eventにも同じ
correlation値とevidence path集合を投影するが、create eventの既存key / 値 /
byte fixtureは変更していない。

### 8.3 profile差し込み

`DomainProfile` に `before_fix_phase`、`fix_regression_bindings`、
`run_fix_regressions` を追加した。defaultのbefore hookは何も生成しないため、
baseline前にcreate scaffoldが障害を消すことはない。dataだけは入力保護snapshotを
読み取り、書き込みはしない。

| profile | 凍結するF3集合 | adapter |
|---|---|---|
| Next.js | `profile_contract`、`profile_verify_1` (`npm run build`) | 既存final profile verifier＋bounded verify command |
| data | `pipeline_probe`、`data_reconciliation`、`data_claims_binding`、`data_rerun_consistency`、`data_results_schema` | manifest解決済みruntime checks。inspection checkは現行契約どおりfinal-boundではない |
| Python CLI | `profile_contract`、`profile_verify_1` (`python3 -m compileall -q src`) | 既存final profile verifier＋bounded verify command。現行はmanifest未admissionのためraw fullもterminal assuranceはstatic上限 |
| generic / 未登録 | `profile_contract` | no-op passとして数えず`unavailable`。F1 / F2成立時はpartial、未登録profileはさらにadmission capでstatic |

profileはcheckの具体実装とbindingを供給するが、F1〜F3のtier式、lineage、epoch、
集合一致条件は変更できない。

### 8.4 assuranceと正直終端

`adjudication/fix.rs::evaluate_fix_evidence` が唯一のfix tier authorityである。
F1〜F3全成立をfull、F1 / F2成立かつF3にinconclusive / unavailableが残る場合を
partial、修正済みだがF系実行ゼロをstatic、F2失敗・実行済み回帰失敗・
baseline非再現をfailedへ写像する。修正の設計品質を示すfieldやtierは持たない。

fixの `ultra_final_acceptance` は `contract_origin=fix_intent_v0` とし、
`completion_metadata` のcreate/profile seedがfixで獲得したtierを再解釈しない。
ただし共通のprofile admission capは従来と同じfinal acceptance eventとterminal
projectionの2箇所で適用する。rawのfix契約tierはeventの`verdict`に保持し、cap後の
値を`assurance_level`へ投影する。
data create evidenceからfix assuranceを再導出する経路もfix originでは停止する。
repair phaseのplanner / executor / profile checkが早期returnした場合も、runtimeの
終端guardが保存済みF1から`failed(after_not_executed)`を投影する。baseline前の
中断は`failed(before_not_executed)`とし、途中失敗をcreate由来のtierへ落とさない。

### 8.5 機械的検証と互換境界

- `tests/fix_intent_conformance.rs` は開始時成功、Rのlineageすり替え、回帰集合縮小、
  回帰binding改変、stale after epoch、未実行provenanceの6ネガティブに、F2失敗・
  実行済み回帰失敗のfailed写像とevidence schemaを加えた9ケースを固定する。
- runtime unit / UltraPlan flow testは、beforeでexecutorが呼ばれず、失敗観測後の
  repairだけが実行され、同じRのafterとprofile regression後にのみfullになること、
  およびbaseline非再現でrepair前停止することを実行で確認する。
- `test0716_d1_fix_intent_contract` corpusはfullと
  `baseline_not_reproduced`のevent / evidence語彙を固定する。
- D-0bのNext.js 3件＋data 3件の`adjudication_compat` byte fixtureは6/6一致を維持し、
  create promptへのfix guidance混入もunit testで拒否する。
- `generality_guardrails` はcontract / fix evaluator / fix runtimeの行数予算と、
  skeletonがcreate private adapterへ逆依存しない境界を追加で固定する。

D-0aで保留したgeneric `reduced`のcreate意味論、requirement inferenceとscannerの
混在、run inventory、unknown intentは変更していない。completion metadata dispatchは
fix originの上書き禁止だけをD-1の契約要件として追加し、create側の既存dispatch値は
byte互換のまま残した。v0の意味を変える将来変更は、台帳に明示した契約改訂としてのみ
行う。

### 8.6 行数予算

新しいleaf moduleはD-1完了時の物理行数とproduction / `cfg(test)`分類を
`generality_guardrails`に固定した。

| module | total | production | test / cfg(test) |
|---|---:|---:|---:|
| `adjudication/contract.rs` | 279 | 246 | 33 |
| `adjudication/fix.rs` | 521 | 351 | 170 |
| `planner/fix_runtime.rs` | 919 | 650 | 269 |
| **合計** | **1,719** | **1,247** | **472** |

既存chokepoint baselineは引き上げていない。`ultra_plan_flow.rs`は1,601行で既存
baseline 1,570の2%許容上限1,602以内、`runner.rs`も既存baseline未満に留めた。
`ultra_plan_flow.rs`の差分はruntime生成、before dispatch、fix終端dispatchの最小
wiringであり、tier式とprobe実装はleaf側に置いた。
