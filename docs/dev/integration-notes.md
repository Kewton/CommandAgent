# Integration Notes

## Repair Pressure Integration (2026-07-11)

The following pre-existing gaps were observed while consolidating repair
pressure. They are recorded here because this integration preserves behavior
and does not change thresholds, events, payloads, or terminal reasons.

### No-Progress Does Not Raise Write Pressure [CLOSED]

The integration originally preserved a gap where the no-progress streak drove
`no_progress_feedback` and `model_stagnation:no_progress_recorded` without
activating `write_required`. T26b2 closes it: the feedback limit now promotes
directly, while carried or short-budget read-only pressure combines with
no-progress turns to reach the same write-required threshold earlier. Compile
repair non-edit turns use the existing target selection, write enforcement,
carryover, and exhaustion paths. The gate #2 #5 shape is fixed in corpus and
loop-level regression tests.

### Dependency Setup Can Stall Network-Restricted Tests [OPEN (low priority)]

`plan_run_nextjs_game_setup_only_fails_inferred_obligation` can enter the real
Node dependency-setup path. Under a network-restricted test environment,
`npm install` can wait for the existing 600-second setup timeout. The test
passes when dependency setup can complete. This integration does not change
the setup decision or timeout.

## Frozen Exception: Template-Owned Implement Steps (2026-07-12) [CLOSED]

UAT `test0712_bs_001` run 3 produced an `ensure-port-scripts` step classified
as `implement`. Its package-script objective was already satisfied, but the
Task 24 preset conversion and short-circuit gates both required `setup`, so the
run exhausted through `model_stagnation:no_progress_recorded` four times.

This bounded exception replaces the kind/name gate only for steps that
explicitly reference template-owned artifacts through `expected_paths`, the
planner-authored instruction, the step id, or deterministic verify commands.
Package scripts/ports and known scaffold configuration files qualify;
ambiguous text such as "configure the project" does not. A route-bound game
implementation and `npm run build` alone remain outside the predicate. The
generic profile contract appended by the runner is also excluded so it cannot
reclassify ordinary implementation steps.

The exception permits kind-independent preset verification conversion,
profile-check prechecks, and the existing verification-bearing no-progress
feedback for this artifact class. It does not change the repair-pressure
transition table, escalation thresholds, event names or payloads, or terminal
reason strings. Converted verification paths are evidence dependencies rather
than mutating ownership claims, so they do not conflict with an implementation
step that owns the same path; duplicate ownership between mutating steps still
fails lint. A failing port check still enters the executor repair path.

## Declarative Knowledge Extraction (2026-07-12) [CLOSED]

The following values moved without byte changes to embedded TOML files:

- `src/planner/profiles/nextjs/knowledge.toml`: preset UltraPlan phase ids and
  prompts; deterministic scaffold, port, build, and implementation matcher
  tokens; setup and template-owned artifact tokens; state-binding and contract
  attribute guidance; canonical package scripts, required hooks, scaffold
  paths, and scaffold file bodies.
- `src/minimal_loop/evidence_knowledge.toml`: visible-surface, input-handler,
  state-update, adversary, motion, score, failure, restart, and persistence
  vocabulary; goal stopwords and Japanese-to-English translations; behavioral
  evidence keys and ordered Next.js repair target candidates.

Both files are embedded with `include_str!`, parsed once through `OnceLock`,
and have no runtime fallback. Migration golden tests fixed the old Rust values
before the final reference switch; retained tests cover the embedded body,
ordering, translation lookup, contract wording, and canonical JSON structure.
No functional issue was discovered during this extraction. The initial
sandboxed full test run could not bind local browser-probe ports; the same test
command passed when run with local-port permission, without a code change.

## Post-gate queue (2026-07-12)

### Panic Boundary [QUEUED]

Gate後タスクT26、最優先。`catch_unwind` でpanicを終端イベントとrecovery
noteへ変換する境界を追加し、fault-injectionテストを完了条件に含む。

### No-Progress to Write-Required Promotion [CLOSED]

T26b2で`repair_pressure.rs`の遷移表を変更し、no-progress上限または
持ち越し・短予算による前倒し圧力との合算で`write_required`へ昇圧するよう
閉鎖した。gate #2の#5（コンパイル修復のno_progress枯渇）をcorpusと
loop-level回帰テストに固定した。バッチ検証では同クラスの件数を継続計測する。

### Undiagnosed package.json Observation [CLOSED]

`test0712_bs_001` #6の観測。ゲート2回＋直近4計測セットで再発なし。
n=1・未再現につきwatch解除。再発時は当該runの`repair.md`で裁定する。

T27 status: REOPENED -> CLOSED (root-caused, fixed by T27)。
`test0712_gab_001` #10でinteraction失敗単独時に診断済みのroute-bound sourceが
修復ターゲットへ接続されず、`required_path`の先頭`package.json`へ落ちる同属事象を
再確認した。確定`state_binding_diagnosis`のpathを最優先ターゲットへ接続し、
interaction failureのevidence mappingとcontract hook guidanceも接続して閉鎖した。

### Probe Input Game Assumption [WITHDRAWN]

WITHDRAWN (attempted 2026-07-13, reverted on parity gate failure, see ledger
T29)。過適応監査項目: プローブ入力のゲーム前提、具体的には
`interaction_probe` の ArrowLeft/ArrowRight/Space dispatch 直書き。
契約駆動dispatch化を試行したが、パリティゲートが2回とも5/6一致で止まり、
Space/qwen35 と Breakout/qwen35 が偽陰性化した。現行のゲーム入力判定は
dispatch直後のlistener/rAFタイミングと結合しているため、再訪時は
listener登録待ち・rAF同期・リトライ付きdispatchを含むタイミングモデル再設計を
前提にする。

### Dependency Setup Network Stall [OPEN (low priority)]

依存セットアップのネットワーク停滞は低優先度のままOPENとする。既存の
600秒setup timeoutとネットワーク制限下の待機挙動はこの棚卸しでは変更しない。

### Data Goal Feature Stopword Noise [OPEN (low priority)]

DATA-5。dataゴールの特徴抽出で英語ストップワードが60個超のノイズとして
生成される事象を記録する。B-2bではdata manifestの知識注入・検証正準化・
dependency拒否フィードバックを優先し、この特徴抽出ロジックは変更しない。

## Phase B profile relocation settlement (2026-07-16)

B-4 audited the core-resident Next.js knowledge recorded by the frozen
template-owned-step exception, the declarative-knowledge extraction, the
profile-manifest format-gap ledger, and the production `"nextjs"` literal
guardrail. The audit distinguishes movable profile policy from execution-order
code that still belongs at a shared chokepoint.

| Area | B-4 disposition | Reason |
| --- | --- | --- |
| `NextjsProfile` implementation, aliases, manifest admission lookup | **Moved** from `planner/profile.rs` and `profile_admission.rs` to `planner/profiles/nextjs/domain.rs`. | The adapter, identity aliases, dependency policy, capability inference, and evidence requirements are used only by Next.js. Core now retains only `DomainProfile` dispatch. |
| Profile-invariant repair excerpt path discovery | **Moved** from `planner/final_acceptance.rs` to `planner/profiles/nextjs/repair_excerpts.rs`. | Tailwind/scaffold excerpt selection is Next.js repair policy; final acceptance only formats the returned paths. |
| Browser probe lifecycle (`minimal_loop/browser_probe.rs`) | **Retained in core**: 2 audited literals. | Spawn/timeout/build-readiness sequencing and evidence writes are shared minimal-loop lifecycle boundaries. Moving only the profile branch would split one lifecycle and risk event-order changes. |
| Route-bound evidence and import closure (`minimal_loop/evidence.rs`, `import_scan.rs`) | **Retained in core**: 4 + 1 audited literals. | Route closure feeds the shared evidence graph and source-role classifier. Its Next.js default is coupled to cross-profile evidence collection; relocation requires an explicit evidence-provider interface, not a file move. |
| Dependency reconciliation (`minimal_loop/loop_run.rs`, `planner/verify.rs`) | **Retained in core**: 2 + 3 audited literals. | The branches select Next.js dependency setup inside shared verify execution and authority tracking. Their ordering is part of the minimal-loop/setup state machine. |
| Plan lint and runner lifecycle (`planner/lint.rs`, `runner.rs`) | **Retained in core**: 2 + 14 audited literals. | The lint terms participate in common plan-quality ordering; runner occurrences label browser/dev-server lifecycle telemetry at the runner tripwire. Moving them without a profile lifecycle interface would move orchestration rather than policy. |

The literal guardrail decreased from 34 production occurrences across 11 core
files to 28 across 7 files. No baseline was raised: removed entries disappear
from the exact expected map. The remaining list above is the Phase B
relocation list; it is intentionally non-zero and each retained group requires
a new shared interface before movement.

## Phase B post-seal queue (2026-07-16)

### Scenario-bound E4 assertions [QUEUED]

Add scenario-bound semantic assertions such as `non_negative(amount)` without
weakening or bypassing the current E4 schema gate. The measured motivation is
present in both data families: aggregation UAT #7 produced a full run whose
April value included the `-500` amount, and the timeseries UAT #8 / #9 reports
also recorded negative-value adoption. This is a purchase of additional
meaning verification; it does not retroactively change the mechanical meaning
of existing full results.

### E2 percent-claim exercise [WATCH]

The timeseries family produced percent-bearing reports but reached no
`claims-binding.json` evidence in 12 runs. Keep percent normalization and
matching on WATCH until the first timeseries completion exercises E2. Do not
infer a pass or failure from report text alone.

### DATA-10 inspection write non-follow-through [ACCEPTED; CLOUD FOLLOW-UP]

The residual inspection write non-follow-through class recurred six times
across UAT #7, #8, and #9 (2 + 2 + 2), including gemma31 as well as qwen35.
Literal JSON examples, missing-key feedback, phase scoping, and verified
short-circuiting have exhausted the machine-side countermeasures; the remaining
distribution is recorded as model-originated. Keep the local-tier failures in
the band denominator and use the emitted recovery YAML for cloud follow-up.

## Phase D pre-D-2 queue (2026-07-17)

### FIX-5 Profile-invariant repair target precedence [CLOSED]

`uat-test0717-fix-004`の`fix4_hook_qwen35_002`では、FIX-4bの対象である
hook predicate文脈は`src/app/page.tsx` / `contract_attribute`へ正しく解決
された。その後、独立したmissing-relative-import profile invariantのbounded
repairへ移った際に、診断済みsourceよりgeneric `required_path`が優先され、
`package.json`が筆頭targetになった。宣言分母22 run中1 runで非支配的だが、
bandから除外せずFIX-5として分離する。

これはT27のinteraction診断source喪失、FIX-4bのpredicate診断source喪失と
同属である「具体的な修復sourceを汎用required pathへ落とす」系の文書化4例目。
profile-invariant repair文脈でも診断・invariant由来fileをgeneric
`required_path`より先に解決することを完了条件とし、D-2着手前に消化する。
fix契約、F1〜F3、既存bandの裁定値を変更するタスクではない。

`6decdce`でfix配下の修復target解決を
`diagnosis_mapped → contract_attribute → evidence_mapped → required_path`へ
統一した。missing-export invariantはimport scan既存の`definition_path`を
`diagnosis_mapped`として再利用し、実測fixtureでは`src/app/game-engine.ts`が
`package.json`より先に解決される。create byte fixture 6/6と既存event schemaは
不変である。

| QUEUED | D-2c生成経路のchokepoint化 or fix計画合成の適用拡大（fix×nextjsが同病を発症した時の処方箋） |
# bench v0.2 queued requirements (2026-07-21)

## D-3c transfer and elevated single-intent arm [QUEUED] (2026-07-23)

D-3c (PM router plus boundary dialogue shell) is transferred behind Phase C
completion. An elevated single-intent investigate measurement outside the
workflow circle is queued as a separate arm and does not alter the circle
Queued: add `epoch` fields to `workflow_started` and `workflow_adjudicated` from the next circle measurement. Update the event baseline and verify that new sheets derive circle-wide duration from the tracked audit body.
denominator.

The following are queued from the dfix-006 live measurement; this section is
only a requirements record and does not change the v0 harness:

- `--report-root` separation: default to repository `workspace/management/runs/`
  so audit assets become Git-tracked automatically.
- Bench-native exceptional rerun support for `interrupted(environment)`, with
  both the interrupted and one-time new-directory records saved automatically.
- Long-run handling: two consecutive session-lifetime × gemma long-run
  interruptions were observed; add an execution-order option or an explicit
  human terminal procedure.
- Skeleton improvements confirmed by dfix-006: show scrub status in the run
  matrix, transfer self-output allowance counts/prefixes, and list resume
  interruption text and final state.

## Acceptance sheet integration [QUEUED]

Benchフェーズ(e)へ`acceptance_sheet`生成を統合し、シート自給率を手動生成0%から改善する。既存のevidence・判定・分類の裁定は変更しない。

## IntentSchema migration [QUEUED]

- fix（段階2）：合成計画snapshot＋conformance 9本でbyte互換を証明してから移行。
- create（段階3）：manifest preset snapshot＋byte互換6/6を証明してから移行。

## Known flaky test determinization [QUEUED]

`final_acceptance_budget_exhaustion_uses_last_cycle_reason`の低頻度・並列干渉候補を、dev-server flakyと同じ棚で共有状態除去として決定化する。発現時は単独3回反復で再診断する。

80msキャンセル猶予テストのタイミングflake 1回発現 (E-3b full初回・単独1/1 pass+full再走green)—— budget-exhaustion系と同棚・決定化候補

## Next.js band input restoration (2026-07-22) [QUEUED]

Restore the 12 pre-migration Next.js measurement sets as auditable inputs by
analyzing their archive format and adapting it to the current profile-aware
scanner. Until that separate task is complete, keep the tracked Next.js band
frozen and treat regeneration from the current repository as unsupported; see
[`analysis.md`](../../workspace/management/runs/band-f821-diff/analysis.md).
