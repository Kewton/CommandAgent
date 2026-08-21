# Mechanism Ledger

M6 generality status is declared in [generality.md](generality.md). This ledger
keeps runtime mechanism entries separate from that declaration; M6 introduced
documentation and test guardrails only, with no behavior changes.

## M6 Roadmap Cross-Reference

| milestone | status | date | reference |
|---|---|---|---|
| M0 | complete | 2026-07-04 | [generality.md#roadmap-completion](generality.md#roadmap-completion) |
| M1 | complete | 2026-07-04 | [generality.md#roadmap-completion](generality.md#roadmap-completion) |
| M2 | complete | 2026-07-04 | [generality.md#clause-evidence](generality.md#clause-evidence) |
| M3 | complete | 2026-07-04 | [generality.md#clause-evidence](generality.md#clause-evidence) |
| M4 | complete | 2026-07-04 | [generality.md#clause-evidence](generality.md#clause-evidence) |
| M5 | complete | 2026-07-04 | [generality.md#clause-evidence](generality.md#clause-evidence) |
| M6 | complete | 2026-07-04 | [generality.md](generality.md) |

## Generic Assurance Track Cross-Reference

Status: complete on 2026-07-05.

| milestone | status | date | reference |
|---|---|---|---|
| G0 | complete | 2026-07-04 | Generic assurance scope and named guarantees established in [generality.md#scope-s](generality.md#scope-s). |
| G1 | complete | 2026-07-04 | Generic contract binding guarded by `generic_contract_bound` and runner tests. |
| G2 | complete | 2026-07-04 | Known-manifest promotion covered by `generic_ultra_promotes_to_nextjs_after_workspace_manifest` and `generic_ultra_promotes_to_python_cli_after_pyproject_manifest`. |
| G3 | complete | 2026-07-05 | Ambiguous no-profile evidence includes static-tier fallback `test0704-4030444542434647484814950515354_001` and final promotion run `test0704-403044454243464748481495051535455565758_000`. |
| G4 | complete | 2026-07-05 | Default-port policy, AMBIGUOUS runbook hardening, final G3 corpus harvest, and this closure ledger. |

## Quality Track Cross-Reference

Status: Q1 concluded on 2026-07-05.

| milestone | status | date | reference |
|---|---|---|---|
| Q1 model-tier baseline | complete | 2026-07-05 | [generality.md#recommended-model-tier](generality.md#recommended-model-tier) |
| Q1 final round | complete | 2026-07-05 | [generality.md#q1-final-quality-baseline](generality.md#q1-final-quality-baseline) |
| Q1 residual corpus harvest | complete | 2026-07-05 | [generality.md#clause-evidence](generality.md#clause-evidence) |
| Q1 boundedness closure | complete | 2026-07-05 | [boundedness-guarantees](#boundedness-guarantees) |

## Local-Model Compatibility Track Cross-Reference

Status: complete on 2026-07-06.

| milestone | status | date | reference |
|---|---|---|---|
| Instructions 64-72 | complete | 2026-07-06 | Final local-tier verdict in [generality.md#recommended-model-tier](generality.md#recommended-model-tier); final distribution `test0704-999-Q1-62646566676869707172_001`. |
| Corpus closure | complete | 2026-07-06 | Local residual and golden fixtures in [generality.md#clause-evidence](generality.md#clause-evidence). |
| Runbook closure | complete | 2026-07-06 | Local-provider timeout and co-residency notes in [uat/scenarios.md#command-shape](uat/scenarios.md#command-shape). |

## Multi-Model Generalization Track Cross-Reference

Status: complete on 2026-07-08.

| milestone | status | date | reference |
|---|---|---|---|
| Instructions 89-102 | complete | 2026-07-08 | Model-family adoption moved from repeated dialect-specific fixes to permanent assets: model probe, boundary audits, deterministic reconciliation, repair ladders, boundedness, and corpus-backed tier vocabulary. |
| Tier vocabulary | complete | 2026-07-08 | `model_stagnation:read_only_loop` and repair follow-through classes record model limits for tier decisions; they are not system defects when the terminal state is honest and bounded. |

## Local Single-Model GAME Track Cross-Reference

Status: complete on 2026-07-07.

| milestone | status | date | reference |
|---|---|---|---|
| Instructions 81-87 | complete | 2026-07-07 | Single-model GAME verdict in [generality.md#recommended-model-tier](generality.md#recommended-model-tier). |
| Corpus closure | complete | 2026-07-07 | Golden local single-model GAME full-pass fixture in [local-single-qwen36-game-full-pass](../../tests/corpus/apps/local-single-qwen36-game-full-pass/expectations.toml). |
| Contract-design closure | complete | 2026-07-07 | Invisible-only observability contract principle in [generality.md#contract-design-principle](generality.md#contract-design-principle). |

## Promotion-Path Incident Chain

| incident | committed evidence | fix | permanent guard |
|---|---|---|---|
| False-full promoted run: final acceptance reported `full_success` / `assurance_level=full` while browser readiness and interaction evidence were `not_applicable`. | `workspace/management/runs/uat-test0704-40304445424346474848149505153545556-000/uat-report.md` | Instruction 57 introduced the earned-assurance invariant: full assurance is derived from executed gate statuses, and disconnected promoted-web gates fail loudly. | `acceptance_gates_disconnected` is added to release-gate reasons; `earned_assurance_for_completion` reduces assurance when gate telemetry and release-gate status disagree. |
| UTF-8 boundary panic during promoted evidence scanning: substring extraction sliced inside a Japanese character and aborted before final acceptance. | `workspace/management/runs/uat-test0704-403044454243464748481495051535455-000/uat-report.md` | Instruction 56 routed truncation through char-boundary helpers, including `floor_char_boundary` / `truncate_at_char_boundary`, and wrapped TUI slash-command execution with a terminal panic guard. | Multibyte truncation tests cover the helper path; `panic_caught` events distinguish a Rust panic from user abort or model failure. |
| Dependency setup authority required after promotion: package state declared `autoprefixer`, but later verify/build lacked authority to reconcile missing installed dependencies. | `workspace/management/runs/uat-test0704-4030444542434647484814950515354555657-000/uat-report.md` | Instruction 58 added run-level setup authority after promotion/manifest repair and boundary reconciliation for declared Node dependencies. | Later verify/contract steps inherit sanctioned setup authority when the run created the dependency need; absent authority still ends as `dependency_setup_authority_required`. |

## Local-Model Compatibility Incident Chain

| instruction | incident | fix | permanent guard |
|---|---|---|---|
| 64 | Local model tool calls emitted absolute workspace paths and corrupted path prefixes that were semantically workspace-relative but failed tool validation. | Tool-argument path normalization and corrupted-prefix salvage admit the mechanical dialect while preserving path confinement. | `tool_args_path_normalized` telemetry plus path-guard tests ensure repaired paths remain workspace-relative. |
| 65 | Local planner verifier commands used shell-control variants and cwd wrappers that obscured a deterministic base check. | Plan-time verifier normalization splits safe shell-control forms and rewrites absolute workspace `cd` wrappers to cwd-relative verifier commands. | Planner sanitizer tests require normalized verifier commands to pass the same deterministic policy as authored commands. |
| 66 | Scaffold/setup completion could strand after the model created only framework support files or omitted deterministic setup artifacts. | Deterministic scaffold completion materializes known setup members and reports remaining missing paths explicitly. | Setup-scaffold unit coverage and loop telemetry distinguish scaffold members from task artifacts. |
| 67 | Runtime verifier execution did not fully match planner normalization, and local models added `2>&1 | tail/head -N` output pipes that masked base-command exit status. | The runtime boundary applies the same verifier normalization and strips output-limiting head/tail pipes while preserving the base command. | Runtime tests assert parity with plan-time normalization and emit `output_pipe_stripped` / normalization telemetry. |
| 68 | Local provider turns and planner-call chokepoints were tuned for remote latency, causing honest local work to hit provider deadlines too early. | Per-provider defaults set local/Ollama provider turns to 600 seconds and remote API turns to 180 seconds, with explicit `--chat-timeout-secs` override. | Provider-call chokepoint telemetry records effective timeout source; conformance keeps bounded provider turns mandatory. |
| 69 | Direct CLI runs could exit before completion finalization, and release/final fields could ignore a failed Python behavior probe. | The direct CLI entry now finalizes before exit, and earned-assurance fields consume `python-cli` behavior-probe results. | Python-CLI conformance rejects failed behavior probes that leave `full_success`, release pass, or runtime pass fields. |
| 70 | Repeated verifier policy errors caused local-model phase death even when an expected-path deterministic substitute existed. | On the second identical verifier policy rejection within a step, the runner substitutes `test -f <expected_path>`, emits `verify_command_substituted`, and marks the oracle tier degraded. | Substitution tests assert continuation without hiding degradation; final acceptance gates remain unchanged. |
| 71 | Iteration exhaustion could be reported as a bare budget label instead of the concrete artifact blocker. | Exhaustion now classifies non-scaffold missing artifacts as `artifact_follow_through_exhausted` with missing paths and stagnation-feedback count. | Conformance rejects terminal `loop_stop` / `tui_command_stop` reasons that strand at `max_iterations` or bare budget labels. |
| 72 | Deep local web runs exposed two probe accounting gaps: `probe_script_error` was app-labeled, and observed state mutation could still leave persistence `not_evaluated:no_mutation_observed`. | Probe script errors use infrastructure taxonomy with setup remediation, and persistence evaluation reconciles observed state dimensions as the pre-reload mutation. | Interaction-probe tests cover infrastructure taxonomy, error propagation, remediation, and observed-mutation persistence evaluation. |

Reviewer-process lessons:

- Gate-status fields outrank labels. Reviews must quote the executed gate
  fields that earned a status; `full_success`, `pass`, or `full` labels alone
  are not evidence.
- Landing conditions must be pre-committed before final rounds. The expected
  terminal distribution, corpus harvest duty, and no-false-full criteria must
  be written down before calling a measurement round final.

## Local Single-Model GAME Incident Chain

Instructions 81-87 closed the local single-model GAME quality track. The
mechanisms below are permanent because each fix maps to an observed failure
mode rather than a single prompt string.

| instruction | incident | fix | permanent guard |
|---|---|---|---|
| 81 | Local GAME runs stalled on broad evidence failures whose repair prompts did not name the root-cause implementation target. | Root-cause-mapped repair guidance now anchors missing gameplay evidence to route-bound implementation files and concrete source obligations. | Repair prompts must name the missing capability/evidence, implementation target, and bounded change surface instead of asking for generic evidence. |
| 82A' | Restart evidence risked becoming a visible-design mandate rather than an honesty boundary. | Reachability-honest classification treats in-play restart/recovery as behavioral evidence, while unreachable or overlay-only restart degrades to unverified/partial. | Observability contracts may require invisible hooks only; visible restart placement remains a preference. |
| 83A | Compile repair for class/member mismatches lacked the receiver definition context needed for local models to repair both sides consistently. | Class-receiver definition context and public API extraction are included in compile repair prompts. | Corpus fixtures assert imported class context and a remedy menu for keeping caller/callee contracts consistent. |
| 84A' | Side effects in expected paths and generated artifacts could pollute dependency/setup lifecycles or verifier targeting. | Two-tier side-effect sanitization separates task artifacts from setup/generated paths before lifecycle setup and repair targeting. | Runner tests keep side-effect paths out of dependency lifecycle setup and verifier false-positive repairs. |
| 85 | Repeated no-edit compile repair attempts consumed wall clock without re-anchoring source context. | Compact repair sessions escalate after zero-edit/no-progress compile repair turns and carry anchored compile frames forward. | Compile repair tests require compact-session prompts to preserve frame excerpts and tool-schema reminders. |
| 86A | Final acceptance composite failures could target the outer readiness label and miss inner build verifier compile errors. | Composite unwrapping targets implementation compile repair when readiness/acceptance fails because build verifier produced parseable compile errors. | `browser_readiness_failed:build_verifier_failed` with compile frames routes to the shared compile ladder. |
| 86C | Probe evidence was labeled missing or unavailable when the probe was provisioned but never exercised because an upstream gate failed. | `not_exercised:<upstream_reason>` vocabulary distinguishes unreached probes from missing probe infrastructure. | Summary/gate-table and evidence-tier tests require `not_exercised` wording for upstream build/readiness failures. |
| 86D' | Styling invariants forced Tailwind into plain-CSS apps and created incoherent stacks. | Presence-conditional styling accepts plain CSS with zero Tailwind artifacts and repairs only partially present Tailwind stacks. | Plain-CSS and partial-Tailwind fixtures guard the coherence boundary. |
| 87A | Lifecycle-captured SWC source frames were present but not parsed, causing build failures to terminate as dependency setup lifecycle failures. | Lifecycle-frame routing parses SWC `Error:` frames without a `Failed to compile` banner and classifies them as compile errors. | Parser and verifier tests require lifecycle source frames to enter implementation compile repair. |
| 87B | Recovery UltraPlan YAML could fail render/parse roundtrip and be discarded. | Recovery roundtrip hardening uses JSON-compatible YAML quoting and writes loadable `needs_review` artifacts on validation failure. | Roundtrip tests cover quotes, backslashes, multiline, Japanese, and the exact recovery-prompt shape. |
| 94 diagnosis datum | Early-death profile projection remained under diagnosis after `test0708_005`. | `test0708_007` records `Effective profile: nextjs` on a post-binding death path (`recovery_prompt_saved.recovery_profile=nextjs`, `tui_command_stop.effective_profile=nextjs`), supporting the hypothesis that the projection defect is limited to earlier death paths rather than runtime profile resolution. | Corpus fixture preserves the post-binding nextjs projection datum while exhaustion honesty is repaired separately. |
| 95 step-1 diagnosis | `test0708_009` exposed a cross-file hook contract mismatch where page code destructured `gameState`/`movePlayer`/`shoot` from `useGameEngine`, but the repair prompt only showed the failing destructuring line. Attempt 2 was appended and attempt 3 compact; rollback was invoked but skipped with `snapshot_missing:src/app/page.tsx` because only setup/config had completed and no passing page snapshot existed. | Compile repair definition context now also follows missing properties in object destructuring to the imported hook/function and, for long exported functions, extracts the top-level returned object API when the bounded definition head would miss it. Compact escalation and rollback behavior were diagnostic, not changed. | Compile guidance tests require destructured hook missing-property failures to include the imported hook API surface and caller/callee consistency remedy. |
| 95B | Four model families had produced zero-edit compile repair turns even after successful generation writes, leaving single-file compile failures to exhaust or rollback. | After compact zero-edit compile repair, a one-shot compact regeneration prompt requires a full-file Write for the single failing source file and accepts it only when rebuild parsed compile errors strictly decrease; rejected regeneration restores the file snapshot and falls through to rollback/classification. | Runner tests cover successful regeneration, rejected regeneration restore, multi-file skip, telemetry, and rollback fallback after rejected regeneration. |
| post-95/96 A | `test0708_010` showed an ESM `postcss.config.js` in a non-module package, but the failure surfaced as a generic PostCSS `plugins` build error. | Tailwind/PostCSS coherence now treats `.js`/extensionless config files with `export default` as incoherent unless `package.json` is `type=module`, leaves `.mjs` alone, and deterministically rewrites owned config to CommonJS. | Profile tests cover the exact PostCSS ESM shape, `.mjs` pass-through, and compile repair guidance for the `plugins` export error. |
| post-95/96 B | `test0708_011` exhausted on repeated `edit_anchor_not_found` even though the model was attempting localized edits. | Edit now reports a deterministic best-match excerpt, salvages only a unique whitespace-normalized anchor region with `edit_anchor_salvaged` telemetry, and escalates same-file repeated anchor failures to full-file Write guidance. | Edit/registry/session tests cover salvage, ambiguity rejection, best-match feedback, telemetry, and second-failure Write escalation. |
| post-95/96 C | `test0708_012` reached final acceptance with browser readiness/probe evidence, then exhausted as `loop_progress_exhausted: no concrete blocker recorded` while restart/input evidence remained pending. | Final-acceptance repair exhaustion now classifies as `capability_evidence_unresolved:<keys>`, emits pending evidence/remedies, and recovery artifacts lead with per-key remedy guidance and restart attachment points. | Runner and conformance tests require final-acceptance exhaustion with pending keys to name `capability_evidence_unresolved` instead of a bare exhaustion/no-blocker reason. |
| post-97 A | `test0708_016` exposed a Bash-tool runtime policy boundary that rejected `cmd1 && cmd2` before the shared verifier normalizer could split it. | Runtime Bash policy now enters the shared normalization pipeline, executes normalized `&&` and `;` segments through bounded commands, preserves `&&` short-circuit behavior, and still rejects semantic pipes with policy feedback. | Runtime tests cover the verbatim Bash shape, `false && echo x` short-circuiting, and the protection-coverage audit now treats runtime Bash policy as a normalizer boundary. |
| post-97 B | `test0708_013` showed root-anchor path salvage did not fire for an absolute `/Users/.../commandagent_mvp/01/test0708_013/package.json` path whose last two workspace components were present and unique. | Path normalization now tries root-anchor salvage for raw absolute paths that canonicalize outside the active workspace, returning a distinct salvage kind and telemetry. | Path-guard and registry tests cover the verbatim absolute path and an ambiguous duplicate-anchor rejection. |
| post-a3b stale path | The a3b five-run campaign still produced stale absolute paths where root-anchor salvage was insufficient or unobservable, and rejection feedback did not name the active workspace root or nearest declared artifact. | Tool path normalization now emits fallback-evaluation telemetry, then accepts only a unique suffix match against step expected paths plus current-plan artifact paths. Rejected stale absolute paths remain unwritten, recover with feedback naming the current root and nearest relative expected path, and do not guess across workspaces. | Registry and loop-run tests cover accepted required-path fallback, rejected stale absolute feedback, model adaptation after feedback, and current-plan artifact candidates; corpus fixtures record probe correlation. |
| post-97 C | The gemma4-cloud round needed instrumentation for suspected provider context truncation without turning it into a gate. | Provider turn telemetry records estimated prompt tokens sent, provider prompt/eval counts, and finish reason, and emits a warning-only `context_truncation_suspected` event after persistent undercut. | Provider and summary tests require token fields, hysteresis warning emission, and a one-line summary/card note when warnings fire. |
| post-106 A | `test -f X && echo "EXISTS" || echo "MISSING"` was still being diagnosed as `shell_control_syntax` in a later run even though the runtime Bash boundary already knew how to strip summary echoes. | The shared runtime verifier normalizer now strips any plain echo-only success/failure branch pair, so the exact `EXISTS`/`MISSING` form reduces to the base `test -f` check and emits normalization telemetry before policy diagnosis. | Runtime tests cover the verbatim `EXISTS`/`MISSING` fixture, and the verify-boundary audit is keyed to command-capability sites rather than a fixed path list. |
| Q1 boundedness A/B | `content_b` repeatedly issued `ls -R && cat package.json`; each command hit the 180s Bash cap because broad recursive listing traversed the whole workspace including generated dependency trees. The old repeated-tool-error key used exact error text, so timeout variants were not aggregated reliably. | Step sessions now have a wall-clock cap, command/provider time sinks are recorded, similar command timeouts share a normalized prefix key, the second timeout gives strategy-change feedback, and the third terminates as `command_timeout_loop`. | Loop-run tests cover similarity grouping, strategy feedback, and step wall-clock expiry; conformance requires terminal timeout repetition to stop as `command_timeout_loop`. |
| Q1 boundedness C | `tool_a` readiness found port 3011 occupied by a stale Next dev server from the same run rather than an unrelated external service. The earlier lifecycle cleanup was per call site, so the child survived into a later readiness cycle and terminal exhaustion could still say no blocker. | Bounded spawn now owns a run-scoped server child registry. Phase transitions, terminal guards, SIGINT/drop paths, and readiness `port_in_use` retry reap registered children with `server_reaped { origin_phase }`; external owners remain honest failures with pid/command. | Bounded-process and runner tests cover registry reaping telemetry and owner parsing; conformance rejects empty-handed no-blocker exhaustion vocabulary. |
| Post-101 Q1 | `content_b` placed dependency installs in verify commands and later hit invalid npm semver, while `game_a` had camelCase/PascalCase collision logic that static evidence missed. | Verify install segments are substituted into runtime-owned dependency reconciliation before remaining verify commands run; invalid semver output names the manifest entry and corrected example; gameplay identifier matching is case-insensitive across source/browser evidence helpers. | Verify, runtime Bash, sanitizer/lint, semver remedy, evidence property, and corpus tests pin the install-substitution, invalid-semver, and camelCase/PascalCase evidence shapes. |
| Final multi-model A | TOOL/GAME fixtures showed `grep X && grep Y` rejected as unsplit shell control at verifier lint/contract entry points even though plan/runtime/Bash boundaries had split machinery. | StepPlan lint and completion contract validation now enter the shared planner verify normalizer before policy diagnosis; unsupported split fragments fail with allowed verifier categories instead of a generic shell-control label. The 92C protection audit now treats StepPlan verify-policy lint as a normalizer boundary. | Lint, completion, and protection-coverage tests pin the multi-grep shape and prevent new direct lint bypasses. |
| Final multi-model B | CLI fixtures showed setup StepPlan lint exhaustion under `python-cli` did not substitute the 94C setup fallback because the fallback was effectively Next.js-only. | Setup fallback now uses the profile scaffold boundary for known profiles; `python-cli` produces `pyproject.toml`, `src/<package>/main.py`, and setup-safe `test -f` verification rather than failing as phase scaffold exhaustion. Python compilation remains a later deterministic gate. | Runner tests cover lint-exhausted python-cli setup fallback; corpus fixture records the CLI scaffold shape. |

## Review Process Rule

Any instruction touching contracts, invariants, or vocabularies requires a
one-paragraph overfitting review at issue time. The review must say what the
change fossilizes, whose design freedom it narrows, and what degradation path
keeps declined preferences honest. This rule is mandatory because 82A' caught
restart reachability drifting toward visible UX control, 84A' caught artifact
and setup side effects that could narrow legitimate implementation shapes, and
86D' caught Tailwind stack injection fossilizing one styling choice over plain
CSS.

## Boundedness Guarantees

Permanent invariant: no pathway may require human interruption.

| mechanism | instruction | guarantee | permanent guard |
|---|---|---|---|
| Provider-turn wall-clock bound | 62 B | Every chat/provider turn emits duration telemetry, is capped by configuration, retries once on `provider_turn_timeout`, then terminates with an honest terminal handoff instead of requiring manual interruption. | `tests/conformance` named assertion `bounded_provider_turns`; unit coverage for `provider_turn_timeout` terminal records and recovery handoff. |
| Verify-command wall-clock bound | 62 C | Deterministic verify commands are capped per command/profile, the process group is killed on expiry, and timeout is classified as `OracleError` / `verify_command_timeout:<command>` with bounded-check repair guidance, never as an implementation-edit target. | `tests/conformance` named assertion `bounded_verify_commands`; verifier tests for process-group kill, OracleError classification, and python-cli timeout substitution. |
| Step-session wall-clock bound | Q1 boundedness | Any single step session has a generous wall-clock cap. Expiry terminates itself as `step_wall_clock_exhausted` and names the dominant command/provider sink instead of waiting for manual SIGINT. | Loop-run test with a short cap and conformance terminal checks. |
| Similar command timeout aggregation | Q1 boundedness | Bash command timeouts count toward repeated-tool-error handling by normalized similarity, not exact error identity. Repeated broad recursive listings get strategy-change feedback and then terminate as `command_timeout_loop`. | Loop-run timeout-similarity test and conformance `command_timeout_loop` assertion. |
| Run-scoped server reaping | Q1 boundedness | Every launched readiness/dev server registered through bounded spawn is reaped at phase and terminal boundaries. Mid-run `port_in_use` from a registered child is deterministically reaped and retried once; external owners are reported with pid/command. | Bounded-process registry test, readiness owner parser test, and `server_reaped` corpus fixture. |

## Required Fields

- mechanism id
- default state / off flag
- target scenarios
- admission benchmark
- before/after result
- final audit date
- rollback condition

## M001

| field | value |
|---|---|
| mechanism id | M001 |
| default state / off flag | off until admitted |
| target scenarios | TBD |
| admission benchmark | minimal-loop-expanded |
| before/after result | TBD |
| final audit date | TBD |
| rollback condition | regression in target or adjacent scenarios |

## M002

| field | value |
|---|---|
| mechanism id | M002 |
| default state / off flag | off until admitted |
| target scenarios | TBD |
| admission benchmark | minimal-loop-expanded |
| before/after result | TBD |
| final audit date | TBD |
| rollback condition | regression in target or adjacent scenarios |

## Task-track backfill (2026-07-10 – 2026-07-11)

The entries below backfill the task-track mechanisms that were implemented and
measured during the 2026-07-10 to 2026-07-11 campaign. Commit hashes were
checked against `git log --oneline`; corpus fixture names were taken from
`git show --stat` under `tests/corpus/apps/`. The original 2026-07-10/11 UAT
report files are not present in this checkout, so their run IDs remain as
historical labels below without dead links; the committed corpus fixtures stay
the local evidence pointers.

| ID | mechanism | commit | motivation (discovery UAT) | corpus fixture | verification (result UAT) | state |
|---|---|---|---|---|---|---|
| T1 | Port-grep semantic rewrite | `923b623` | `test0710_bs_002` combo2-002 (report unavailable in this checkout) | `test0710_stage0_pivot` / `package-port-only-grep-normalization.jsonl` | Non-recurrence in `test0710_bs_003` (report unavailable in this checkout) | admitted |
| T2 | Import/JSX boundary static guard | `5991fba` | `test0710_bs_002` combo1-001/003 (report unavailable in this checkout) | `test0710_stage0_pivot` / `static-import-guards.jsonl` | Non-recurrence from `test0710_bs_003` onward (report unavailable in this checkout) | admitted |
| T3 | `write_required` rung | `8949be9` | `test0710_bs_002` read-only exhaustion group (report unavailable in this checkout) | `test0710_stage0_pivot` / `read-only-write-required.jsonl` | Fired in `test0710_bs_003`; wrong target exposed and handled by T4 (report unavailable in this checkout) | admitted |
| T4 | Evidence-to-target mapping | `19c1d10` | `test0710_bs_003` combo1 wrong `package.json` target (report unavailable in this checkout) | `test0710_stage0_pivot` / `read-only-write-required-evidence-targets.jsonl` | Removed in `test0710_bs_004`; related route later recurred and was unified by T20 (report unavailable in this checkout) | superseded_by_T20 |
| T5 | Hook snapshot and restoration rung | `f1af34e` | `test0710_bs_003` combo3 primary hook loss (report unavailable in this checkout) | `test0710_stage0_pivot` / `hook-snapshot-primary-restore.jsonl` | Class stopped dominating later runs | admitted |
| T6 | Deterministic step template | `3ae842b` | `test0710_bs_003` planner-time dominance (report unavailable in this checkout) | `test0710_stage0_pivot` / `deterministic-step-plan-used.jsonl` | `test0710_bs_004` observed event counts 2/2/1 (report unavailable in this checkout) | admitted |
| T7 | Deterministic restart repair (rung C) | `-` | `test0710_bs_004` (report unavailable in this checkout) | `-` | Not built by design; decomposed into T9/T16/T19 | not_built |
| T8 | Hook grep quote-independent normalization | `fb1689d` | `test0710_bs_004` combo3 quote false negative (report unavailable in this checkout) | `test0710_bs_004_command_normalization` / `combo3-hook-grep-normalized.jsonl` | Same pair full in `test0710_bs_005` (report unavailable in this checkout) | admitted |
| T9 | State-binding diagnosis | `4e6c4c8` | `test0710_bs_004` combo1-family interaction failures (report unavailable in this checkout) | `test0710_stage0_pivot` / `state-binding-diagnosis.jsonl` | `bs_005`/`bs_006` returned undeterminable, then T16 calibrated/prevented the class (`test0710_bs_005`, `test0710_bs_006`) | superseded_by_T16 |
| T10 | Inspect-command normalization | `fb1689d` | `test0710_bs_004` combo2 timeout (report unavailable in this checkout) | `test0710_bs_004_command_normalization` / `combo2-inspect-command-normalized.jsonl` | Fired in later runs | admitted |
| T11 | Preset UltraPlan | `c268c53` | `test0710_bs_004` plan-isomorphism evidence (report unavailable in this checkout) | `test0710_stage0_pivot` / `preset-ultra-plan-used.jsonl` | `test0710_bs_006` fired 6/6; default later rolled back by T24 while opt-in remained (report unavailable in this checkout) | admitted_optin |
| T12 | Adversary vocabulary, translation, and probe-dimension scan | `0be0f74` | `test0710_bs_006` breakout combo1 arbitration false negative (report unavailable in this checkout) | `test0710_bs_006_breakout_combo1` | `test0711_bs_001` #7 full (report unavailable in this checkout) | admitted |
| T13 | Edit-anchor recovery ladder | `8aa3c2c` | `test0710_bs_005` combo2 anchor x8 (report unavailable in this checkout) | `test0710_bs_005_anchor_recovery_combo2` | Old terminal state disappeared in `test0711_bs_001`; T18 completed the interlock (report unavailable in this checkout) | admitted |
| T14 | Route-unbound wiring guidance | `f1bfb82` | `test0710_bs_005` combo1 (report unavailable in this checkout) | `test0710_bs_005_route_unbound_combo1` | `test0711_bs_002` #7 recovered in live run and reached full (report unavailable in this checkout) | admitted |
| T15 | Probe preflight | `49325f6` | `test0710_bs_006` space combo3 infrastructure failure (report unavailable in this checkout) | `test0710_bs_006_probe_preflight_space_combo3` | Recorded in every later run | admitted |
| T16 | Input-coupled diagnosis and contract wording | `764094b`, `926d3cd` | Calibration from `bs_005`/`bs_006` real artifacts (`test0710_bs_005`, `test0710_bs_006`) | `test0710_bs_005_006_state_binding_input_coupled` | Contract wording acted preventively; class disappeared, while the diagnosis path remained unexercised | admitted_prevention |
| T17 | Tier-coupled preset default | `b08e92f` | `test0711_bs_001`/`test0711_bs_002` A/B (`test0711_bs_001`, `test0711_bs_002`) | `test0711_bs_001_plan_preset_tier` | Did not fire in `test0711_bs_003`, fixed by T23, then `test0711_bs_004` measured distribution degradation and T24 rolled it back (`test0711_bs_003`, `test0711_bs_004`) | rolled_back |
| T18 | Anchor x stagnation interlock | `8cba4ef` | `test0711_bs_002` #2 (report unavailable in this checkout) | `test0711_bs_007_anchor_stagnation_interlock` | `test0711_bs_003` showed carryover firing (report unavailable in this checkout) | admitted |
| T19 | Contract-attribute-missing guidance | `53a967d` | `test0711_bs_001` #6 (report unavailable in this checkout) | `test0711_bs_001_6_contract_attribute_missing` | Fired in `test0711_bs_003` #1 (report unavailable in this checkout) | admitted |
| T20/T20b | Target-resolution unification and escalation carryover | `f8388cf6`, `7e920fb0` | `test0711_bs_002` #1 `package.json` recurrence (report unavailable in this checkout) | `test0710_stage0_pivot` / `final-acceptance-target-resolution.jsonl`; `test0711_bs_008_escalation_carryover` | `test0711_bs_003` showed `page.tsx` target resolution and carryover telemetry (report unavailable in this checkout) | admitted |
| T21 | Compile diagnostic extraction | `99807bc` | `test0711_bs_002` #8 webpack-internal location (report unavailable in this checkout) | `test0711_bs_008_compile_diagnostic_extraction` | `test0711_bs_003`/`test0711_bs_004` recorded real source locations (`test0711_bs_003`, `test0711_bs_004`) | admitted |
| T22 | Implementation-detail grep demotion | `1188842` | `test0711_bs_002` #3 `addEventListener` grep (report unavailable in this checkout) | `test0711_bs_002_source_detail_grep_advisory` | Not exercised later; zero demotions also confirmed the gate was not broadly relaxed | admitted_unexercised |
| T23 | Tier decision from resolved model | `fe690b1` | `test0711_bs_003` default non-firing (report unavailable in this checkout) | `test0711_bs_003_resolved_planner_tier` | `test0711_bs_004` fired 6/6 (report unavailable in this checkout) | admitted(対象機構はrolled_back) |
| T24 | Satisfied-setup short-circuit, preset-step conversion, and default-none rollback | `042880f` | `test0711_bs_004` #1/#7 (report unavailable in this checkout) | `test0711_bs_004_preset_setup_no_progress` | Verification UAT passed P0/P1 (report unavailable in this checkout) | admitted |

Summary:

- Across 24 tasks: one rollback (T17), one deliberately unbuilt item (T7), and two replacements (T4→T20, T9→T16).
- Every task has a motivating measured failure run; no speculative mechanism was admitted.
- T17 is the concrete example of the "off until admitted" operating rule working end to end: adopt, measure, detect degradation, and roll back, with each step recorded.

## Integration phase closure (2026-07-11 – 2026-07-12)

Commit ranges and counts below were checked against `git log --oneline`.
Reported file sizes are measured at the closing commit; this records
`runner.rs` at 18,242 lines rather than the previously quoted 18,243.

| 項目 | コミット | 結果 |
|---|---|---|
| 台帳バックフィル T1-T24 | `c28e6af` | 完了（T17の`rolled_back`履歴を含む） |
| runner分解 | `b567dc0..f938b22`（5コミット、両端を含む） | `runner.rs` 29,286→18,242行（-38%）、`repair_targeting` / `final_acceptance` / `ultra_plan_flow` / `assurance` の4モジュール新設、各モジュールに成長予算 |
| エスカレーション状態機械化 | `22ddb9a..233a192`（6コミット、両端を含む） | 5系統を`repair_pressure.rs`（純粋関数遷移表746行）へ統合。既知の穴（no_progress非昇圧）は明示エントリとして保存 |
| 凍結例外 T24b | `76cb059` | 成果物鍵の述語でsetupステップ変種を無害化（ゲート基準を塞ぐ有界修正として例外承認） |
| 欠陥修正 T25/T25b | `c226b9b` | UTF-8文字境界panic（約90run潜伏）の修正・重複統合・掃引・実クラッシュfixture固定 |
| 知識外部化 Stage 1 | `460afe0..986daff`（4コミット、両端を含む） | Next.js `knowledge.toml`（252行）+ `evidence_knowledge.toml`（340行）、golden 8本（各知識群4本）、値バイト保存 |

### Gate record

Both report directories below were present when this closure was recorded.

| 試行 | レポート | 結果 |
|---|---|---|
| gate #1 | `workspace/management/runs/uat-test0712-g-001/` | FAIL — #3 が `evidence.rs:4044` のUTF-8境界panicで不正直終端（exit=101、recovery artifacts無し）。ゲートが約90run潜伏の欠陥を捕捉 |
| gate #2 | `workspace/management/runs/uat-test0712-g-002/` | PASS — G1〜G5全通過。8/8正直終端、偽成功ゼロ、既知クラス再発ゼロ、full 2/8（Quiz 2/2）、#3は同条件で正直終端に転化。`selection_reason=evidence_mapped` の実戦初確認 |

### Summary

(a) 挙動退行ゼロで構造統合を完了（凍結例外1件は例外プロセス内で処理）。

(b) ゲート自体が不正直終端の欠陥を発見・根絶させた——ゲート運用が
機能した実例として記録する。

(c) 本フェーズ以降、runner / loop_run / 新4モジュール / repair_pressure /
知識のRust所有境界（nextjs.rs / evidence.rs）に個別成長予算が存在する。
知識TOML自体に個別行数予算はないが、知識の変更は `knowledge.toml` 編集＋
シナリオ行列回帰が正規手順となる。

## Post-closure task ledger (2026-07-13)

| ID | 機構 | コミット | 動機 | 検証 | 状態 |
|---|---|---|---|---|---|
| T29 | プローブdispatch契約駆動化 | なし（試行→全revert） | 過適応監査: `interaction_probe` のゲーム操作直書き（ArrowLeft/ArrowRight/Space）の宣言化 | パリティ計測5/6×2回で退行（Space/qwen35、Breakout/qwen35 が偽陰性化）。ゲーム入力判定がdispatch直後のlistener/rAFタイミングと癒着しており、宣言化にはプローブのタイミングモデル再設計が必要と判明 | **withdrawn** |
| T30 | assurance投影のprofile dispatch | 本コミット | data早期失敗が汎用full seedによりpartialへインフレした契約違反を、[investigation-01.md](../../workspace/management/runs/uat-test0713-data-001/investigation-01.md) に基づき厳格化方向へ修正 | data 4-run判定表、E1/E3未達negative conformance、Next.js早期失敗互換 | **fixed** |

- 据え置きの根拠: 実害ゼロ（非ゲームは候補要素クリック経路が補完、バンド窓78runで本件起因の偽陰性/偽陽性ゼロ）に対し、修正には裁定層のタイミング再設計を要するため、コストが見合わない。
- 再訪条件: (a) 第4のシナリオ族でdispatch不足起因の偽陰性が実測されたとき、(b) 近縁profile（Vue等）がプローブ拡張を要求したとき。再訪時はタイミングモデル再設計（listener登録待ち・rAF同期・リトライ付きdispatch）を含む適正スコープで行う。

## Repository migration (2026-07-14)

- 移送元: `Kewton/Anvil@anvilminimal-migration-base`。filter前の
  `develop` HEADは`ec1519958c2210e3bcadcd19d7c23e51146a82ce`。
- 方式: `git filter-repo`で旧クレートsubtreeと`workspace/management`の
  2系統を`--path`選択し、旧クレートsubtreeをリポジトリrootへrenameした。
  抽出履歴はsquashせずCommandAgentの履歴へmergeした。
- 旧新SHA対応表: [`docs/migration/anvil-commit-map.txt`](../migration/anvil-commit-map.txt)。

本文書内の移行以前のコミットハッシュは旧Anvil上のSHAであり、
`docs/migration/anvil-commit-map.txt`およびAnvilリポジトリの凍結タグ
`anvilminimal-migration-base`で解決する。

- Rename: anvilminimal → commandagent（crate/binary: `d05a410`、生きた参照: `835c04f`、本コミット）。以後のUATレポートのversion表記は commandagent。旧名は歴史的記録内で有効。機械出力はversion/CLI、banner、remediation/再現コマンド、truncation marker、probe User-Agent、およびevalの`engine_label`/`binary_kind`/`subject`/レポート見出しのみ新名へ更新し、イベント名・JSONキー・スキーマは不変。
- M-4移行ゲート（`test0714_m4_001`、2026-07-14）: G1/G2 **PASS**（新buildから6/6を各1回実行・正直終端・収集完了、data失敗5/5がassurance契約準拠、partialインフレ0）につきRepository migration完了を正式宣言し、Phase Bを再開。Evidence: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_001/uat-report.md` / `aggregate.json`。
- B-2d（DATA-7/8/9、2026-07-14）: verify lint拒否原文テレメトリ（`82fff89`）、`.anvil`一貫私有化＋有界フィードバック（`bdca35c`）、Python traceback抽出・決定的修復注入・`traceback_mapped`ターゲット解決（本コミット）を、固定data契約を変更せず導入。一次資料: [`investigation-b2d.md`](../../workspace/management/runs/uat-test0714-m4-001/investigation-b2d.md)。
- DATA-10 / DATA-7段2 / FF-1b（2026-07-15）: dataチェックと正準成果物のフェーズスコープ化（`10d0143`）、`data_inspection_schema`（`4d9a4da`）、verify書き換え拡張＋runtimeテレメトリ（`0ba612f`）、contract instrumentation欠落ガイダンス配線（本コミット）を導入。一次資料: [`investigation-data10.md`](../../workspace/management/runs/uat-test0714-m4-004/investigation-data10.md)。
- B-2f（2026-07-15）: inspectionの5キー字義例と実測値拘束をmanifest・修復ガイダンスへ追補（`bc9ec91`）し、data契約をassertする実測 `python/python3 -c` verifyを対応カタログチェックへ正準化（本コミット）。既存13件の正準化とNext.jsバイト列は維持。一次資料: [`uat-report.md`](../../workspace/management/runs/uat-test0715-ff1-002/uat-report.md)。
- B-2g E2較正（2026-07-15）: 偽陽性49件（日付分割36＋照合域13）を除去。モデル起因の違反は0件だった（[`investigation-e2.md`](../../workspace/management/runs/uat-test0715-ff1-002/investigation-e2.md)）。契約§6ネガティブ維持＋照合域の新設ネガティブで非緩和を担保。
- B-2h DATA-11／inspection行数照合／nearest_miss修復注入（2026-07-15）: 動的・正準最終フェーズから他フェーズ明示束縛チェックを除外してE1〜E4のみをfullゲート化（`2d42ae4`）、inspection報告行数を実CSV/TSV論理行数と照合（`4ddffcf`）、claims-bindingの違反claim・最近傍キー／値・差分をstep／最終受け入れ修復へ注入（本コミット）。根拠: [`uat-report.md`](../../workspace/management/runs/uat-test0715-data-005/uat-report.md)。
- B-2i DATA-12（2026-07-15）: data stepを全expected_paths実在＋実verify全pass時だけモデルターン前に短絡し、動的phaseで正準化後に空となったverify stepへphase別の既定checkを束縛する。根拠: [`uat-test0715-data-006`](../../workspace/management/runs/uat-test0715-data-006/uat-report.md)。
- B-2k DATA-7b（2026-07-16）: verify lintのシェル制御構文判定をshクォート対応とし、引用payload内の`; | & || &&`の偽陽性を除去。E2較正と同属の検証器精度修正であり、Next.js方向の影響は偽陽性減少のみ。既存のクォート外制御構文・ファイル書き込みredirectネガティブ維持で非緩和を担保。根拠: [`uat-test0716-data-008`](../../workspace/management/runs/uat-test0716-data-008/uat-report.md) Run 6。

## Incident: semantic false-full (2026-07-14)

| ID | 機構 | コミット | 動機 | 検証 | 状態 |
|---|---|---|---|---|---|
| FF-1 | heuristic合格のfull資格剥奪 | `7dd98ad` | uat-test0714-m4-003 Run 6: 初の意味論的false-full。クイズのゴール（3問・スコア・リトライ）に対しシューティングゲームが生成され、契約フックゼロ（contract_hook_status=primary_missing、action_hooks=[]）のままheuristicプローブのmarker差分合格（element_count変化）によりfull_successを獲得した。根因: browser-interaction.json のprobe_mode / contract_hook_status がレポート整形専用で、release gateの判定に接続されていなかった（潜在欠陥、回帰ではない）。機械的偽装（evidence捏造・検証迂回・プローブ不実行のfull投影）はゼロ継続 | 回帰fixture tests/corpus/apps/test0714_m4_003_run6_false_full（full_eligible=false）、interaction_qualification 単体テスト、パリティ6本PASS（uat-test0715-ff1-001、full 3本すべて probe_mode=contract、heuristic-full 0） | fixed |

- 発見経緯: 機械はfullと判定したが、人間の監査（成果物の目視）がgoal種別との不一致を検出した。UAT報告者がG1 FAILと判定し台帳更新を保留した対応は、本プロジェクトの検収規律の実演である。
- 残存する境界（本修正の対象外）: 契約フックを正しく備えた「goal種別と異なる成果物」は依然fullを獲得しうる。goal種別と要求surfaceの契約束縛は未実装であり、検収可能性階層の実測された境界として記録する（対処は契約設計の将来課題。安易なgoal語彙マッチは偽陰性を量産するため急がない）。
- 過去バンドへの影響監査: 実施済み（[audit-report.md](../../workspace/management/runs/ff1-band-audit/audit-report.md)参照）。ウィンドウ内full 31件はすべてcontract-mode（非空state dimensions）で、heuristic-only / unverifiableは0件。既存バンドは有効。

## Data profile first fulls (2026-07-15)

[`uat-test0715-data-007`](../../workspace/management/runs/uat-test0715-data-007/uat-report.md) は、dataプロファイルで初めてのfullを同一固定コード上の2 runで記録した。

| run | preset / phase | 獲得evidence | 工程・短絡 |
|---|---|---|---|
| Run 3 `data7_gemma31_profile_001` | gemma31 / profile、5/5 phase | E1 `60 = 57 + 3`、E2 `26/26`、E3 passを含むE1〜E4全PASS | inspection行数照合を1回修復し、`step_short_circuited` 16件 |
| Run 4 `data7_qwen35_none_001` | qwen35 / none、3/3 phase | E2 `46/46`、E3 passを含むE1〜E4全PASS | 動的計画から最終受け入れまで完走 |

これは、実行プローブ、勘定照合、レポート数値束縛、再現性からなる非ブラウザ裁定チェーンがend-to-endでfullを獲得した初の実証である。到達までの計測はdata UAT #1〜#7の48 run。機械起因クラスDATA-1〜12は全て根治済みで、修正経路は上記B-2d（DATA-7/8/9）、DATA-10、B-2f、B-2g、B-2h（DATA-11）、B-2i（DATA-12）の各台帳行と一次資料を参照する。

### Preset A/B裁定

- 固定コードA/Bはprofileが1完走・1full、noneも1完走・1fullで同率だった。noneの死因はplanner scaffold起因優勢ではない。
- 事前宣言した打ち切り条項により、dataのデフォルト`plan-preset`は`none`維持、presetはopt-inで確定する。この問いへの追加計測は行わない。
- UAT #4〜#6のnone 0/12完走は機械欠陥期の正直な記録である。欠陥根治後の固定コードではnoneアームからもfullが出たため、過去値でA/B裁定を上書きしない。

### Inspection書き込み非追従の再分類

UAT #7ではqwen35 profileでinspection書き込み非追従が2件、gemma31 profileでは0件で完走となり、executor横断で発生率が異なるモデル分散クラスと確定した。機械側は字義JSON例、欠落キー列挙、`nearest_miss`まで導入済みであり、以後は[Spaceの4%分散](../../workspace/management/runs/band_summary.md)と同じバンド特性として受容・文書化する。DATA-10の機械的なフェーズ束縛欠陥は根治済みであり、この残存クラスとは区別する。

### 集計注記

バンド集計は`final_acceptance_status`と`evidence/data-assurance.json`を正とする。B-2j以前の完走runでは、獲得済みfullがterminal projectionの`completion_contract_not_bound`によりpartialへデフレした値を含む。B-2j（`13b994f`）は`full_success`時にE1〜E4の実在evidenceからassuranceを再導出して投影し、evidence不在・不整合時の保守側投影と早期失敗のT30判定を維持する。歴史的イベントは改変しない。

- data × create バンド宣言（2026-07-15）: 機構安定後窓は2/6 full、全期間窓は2/38 full（観測48 run中、操作誤り・preflight未達の10 runを理由付きで分母外）。再計測は集計スクリプトのみとし、原表は[`band_summary_data.md`](../../workspace/management/runs/band_summary_data.md)を参照する。
- B-3 admission gate（2026-07-16）: `draft` profileのassurance宣言を`static / profile_not_admitted`に上限制限し、dataを[バンド宣言](generality.md#measured-capability-bands-data--create)に基づく初の`admitted` profileへ昇格した。以後、新profileはdraft起点でfull宣言不可。

## Phase B settlement: data profile cost (2026-07-15)

data契約のfixedから初バンド宣言までを、次の再現可能な境界で集計した。
移行前SHA `2c982fc` は
[`docs/migration/anvil-commit-map.txt`](../migration/anvil-commit-map.txt) 上の
現リポジトリSHA `4f57714`（2026-07-13 21:23:57 JST）に対応する。終点は
`68fdaf0`（2026-07-16 00:19:41 JST、7月15日キャンペーンのバンド宣言）
で、実時間は50時間55分44秒、キャンペーン日では3日である。移行mergeが
別履歴を含むため単純な `rev-list` は使わず、data契約・実装パス、上記の
B-2台帳行、調査/UAT記録、バンド生成物からfull SHAを集めて重複排除した。

### タスク、コミット、計測

| 集計項目 | 実測 | 集計規律 |
|---|---:|---|
| 台帳上のタスクID | 18 | B-0〜B-3の4 ID、B-2a〜B-2jの10 ID、下記4調査。B-2は配下a〜jのumbrellaなので、重複しない実行タスク数は17 |
| 一次資料調査 | 4 | [`investigation-01.md`](../../workspace/management/runs/uat-test0713-data-001/investigation-01.md)、[`investigation-b2d.md`](../../workspace/management/runs/uat-test0714-m4-001/investigation-b2d.md)、[`investigation-data10.md`](../../workspace/management/runs/uat-test0714-m4-004/investigation-data10.md)、[`investigation-e2.md`](../../workspace/management/runs/uat-test0715-ff1-002/investigation-e2.md) を `rg --files workspace/management/runs` で列挙 |
| fixed→初バンドのscoped commits | 38 | `4f57714`〜`68fdaf0`からdata/B系の契約・実装・調査・UAT・バンドcommitを対象パスと台帳で選び、full SHAで重複排除 |
| B-0〜B-3の全ライフサイクルcommit | 42 | 上記38に、fixed直前のB-1 schema/doc 2件（`bb510b7`、`62a3320`）と、バンド後のB-3 gate/admission 2件（`2c2d154`、`8930784`）を加算。B-4清算commitは含めない |
| 観測キャンペーン | 9 set | 正式data UAT #1〜#7の7 setに、無効計測 `uat-test0714-m4-002` / `m4-004` の2 setを加えた「7 set + α」 |
| 観測run | 48 | [`band_summary_data.md`](../../workspace/management/runs/band_summary_data.md) の走査行数。正式分母38、操作上のmodel-ID誤り5とpreflight未達・未完了5の計10は理由付き分母外 |
| full | 2/38 | E1〜E4とdata-assuranceの実在を集計器が横断確認。evidence欠口を持つfalse-fullは0 |

38 commitは、`git show -s --format='%H %s'` で各対象SHAの存在とsubjectを
確認した。42という値は期間を曖昧に広げた値ではなく、B-1がfixedの直前、
B-3が初バンドの直後という工程境界を別枠で明示した全ライフサイクル値で
ある。

対象38 SHA（表示は短縮形）: `4f57714 ac336a8 728dc5c 81a94dd baf008b
16af7c2 ae76537 23b18dc 1ee67e3 97f9b94 18e07ea 82fff89 bdca35c e6a6697
cbe5fe2 476d132 10d0143 4d9a4da 0ba612f f049af7 dc701aa bc9ec91 b8d405b
a708a22 c418d4d 0103ae5 cc829bc 2d42ae4 4ddffcf 859cd08 8b97959 3b99c64
7b177fe 88e0a69 13b994f 4b4426e 88d0bff 68fdaf0`。

### コード量

行数は終点 `68fdaf0` のblobを `git ls-tree` / `git show` / `wc -l` で
数え、変更量は `git diff --numstat 4f57714 68fdaf0` で集計した。生成物や
空白を推定換算せず、Rust内の同居テストを含む物理行数である。

| 境界 | ファイル / 行 | 注記 |
|---|---:|---|
| data profile一式 | 25 files / 4,638 lines | `src/planner/profiles/data.rs` と `src/planner/profiles/data/**`。期間差分は +4,492/-3 |
| manifest | 145 lines | `src/planner/profiles/data/manifest.toml`。上の4,638行に内包 |
| E系チェック群 | 10 files / 1,635 lines | `checks.rs`、`results_schema.rs`、`runtime*.rs`、`claims_binding.rs` と同submodules。上の4,638行に内包 |
| 層2のcatalog/probe | 2 files / 665 lines | `capability_catalog/data.rs` 205行、`minimal_loop/pipeline_probe.rs` 460行 |
| 指定された層2範囲の合計 | 12 files / 2,300 lines | E系1,635行とcatalog/probe 665行の非重複和 |
| profile境界外の `src/` 差分 | 71 paths / +4,980/-604 | 新規leaf 16 pathsは +2,934/-0、既存core 55 pathsは +2,046/-604。profile外コストを新規leafだけに見せない |

### 共通機構の再利用監査

共通機構は置換せず再利用したが、「全て変更ゼロ」という一括記述はGit差分
では裏付けられなかったため、機構本体とdata配線を分けて記録する。

| 共通機構 | 実測された再利用状態 |
|---|---|
| minimal loop | 実行・修復ループ本体を再利用。data配線と共通境界修正により `loop_run.rs` は +89/-79で、byte-zeroではない |
| 修復圧力状態機械 | `repair_pressure.rs` は両端ともblob `1cb04168a98bd992c165ccad4ee30335acd0ffab`。変更ゼロで流用 |
| ターゲット解決 | 既存解決経路を流用しつつtraceback/選択leafを追加。`repair_targeting.rs` は型のleaf移設により +4/-33で、変更ゼロではない |
| `eval_events` | productionのevent emitter/schemaは変更ゼロ。ファイル差分 +1/-1 はbinary renameに伴うtest期待文字列だけ |
| 計測規律 | preflight、1 run 1回、artifact退避、理由付き分母除外を変更せず流用。集計器自体にはdata対応 +554/-8を追加 |

したがって、ゼロ変更で再利用できたと機械的に言えるのは修復圧力の遷移
kernel、productionの`eval_events`契約、およびUAT実行規律である。minimal
loopとターゲット解決は共通機構を再利用したが、dataで観測した一般欠陥の
配線修正を含む。この区別もdataプロファイル追加コストに算入する。

歴史的な縦穴との丸め比較は、nextjsが約10計測set・約200 run・2日、
dataが7正式set + α（観測9 set）・48 run・3日である。nextjs側はPhase A
台帳の概数、data側は上記集計器の実数であり、同精度の比率としては扱わない。

### 第2シナリオ族の追加コスト（2026-07-16）

時系列族UAT #8 / #9とB-2k後の再計測による追加コストの境界は、B-4
settlement `fcb9ac8`の次から族別band生成`207dd33`までである。

| 第2族の追加コスト | 実測 | 集計方法・内訳 |
|---|---:|---|
| B-2kタスク | 2 | DATA-13 goal参照入力優先（`271deeb`）とDATA-7bクォート考慮lint（`2028eb4`） |
| 一次資料調査 | 0 | 独立したinvestigation task / reportは追加せず、UAT #8一次資料を直接fixture化 |
| UAT | 2 set / 12 run | [`uat-test0716-data-008`](../../workspace/management/runs/uat-test0716-data-008/uat-report.md) と [`uat-test0716-data-009`](../../workspace/management/runs/uat-test0716-data-009/uat-report.md)、各6 run・再試行なし |
| 追加commit | 5 | `git rev-list --count fcb9ac8..207dd33`。UAT #8（`46d9e34`）、B-2k 2件、UAT #9（`c4d5727`）、族別band生成（`207dd33`）。本封緘docs commitは自己参照を避けて境界外 |
| 族別分布 | aggregation 2/38、timeseries 0/12 | Window Bはaggregation 2/6、timeseries 0/6。機械生成原表は[`band_summary_data.md`](../../workspace/management/runs/band_summary_data.md) |

## Phase B seal (2026-07-16)

| 出口条件 | 照合結果 | 根拠 |
|---|---|---|
| 正直終端 | 達成 | 機構安定後UAT #7は6/6が理由付き終端。全期間の無効10 runも破棄・中断理由を保存し、成功へ投影していない |
| 偽成功ゼロ | 達成 | 初full 2件はE1〜E4実在を横断確認し、false-full evidence gap 0。契約§6ネガティブとB-3 admission gateを維持 |
| 2実例からの汎用化 | 達成 | nextjs/dataの2 manifestからschema v1とprofile admissionを確定 |
| 2シナリオ分布 | 限定達成・継続 | executor/presetの分布は測ったが、dataバンドは同一fixtureの月次×地域集計という単一シナリオ族だけ。複数dataシナリオ族への外的妥当性は主張しない |
| コスト実測 | 達成 | 本節のタスク、commit、run、行数、core差分をGitと生成バンドから集計 |
| schema v1 | 達成 | `6986cc8`。named guidance variants、artifact either/exactly-one、phase bindingを確定し、repository manifestはv1のみ受理 |
| relocation | 達成 | `472b3f3`。移設可能なNext.js policyをprofile leafへ移し、productionのNext.js literal guardを34件/11 filesから28件/7 filesへ減少。残置理由はintegration notesに保存 |
| admission gate | 達成 | `2c2d154` / `8930784`。draftはevidence full相当でもstatic上限、data/nextjsは根拠付きadmitted |

Phase Bは上記の限定を含む現在スコープで封緘する。残存事項は次の3点で、
既存バンドやfullの意味を拡張解釈しない。

- inspection成果物への書き込み非追従はexecutorごとのモデル分散として受容済みであり、DATA-10の機械的束縛欠陥とは分離してバンドに残す。
- dataの計測シナリオ族は1本である。シナリオ族の拡張はPhase B後の継続項目とする。
- E4のアサーション範囲拡張は候補として残す。現行E4を実行せずfullを与える変更は行わない。

### 第2シナリオ族の清算と最終封緘（2026-07-16）

上の2026-07-15時点の「単一シナリオ族」という限定を履歴として残した上で、
UAT #8で観測したDATA-13 / DATA-7bの機械偽陽性は、B-2k後のUAT #9で
いずれも再発0だった。時系列族の0/12は機械不良による無効計測として捨てず、
現行ローカルティアの能力バンドとして分母に残す。E2のpercent claim照合は
時系列族が未完走のため未実戦であり、初完走待ちのWATCHへ移す。

**Phase B sealed (2026-07-16): 出口条件——2族分布・偽成功ゼロ・E1〜E4実証・schema v1・admission gate・コスト表——全達成。timeseries 0/12 は受容済み能力バンドとして宣言（nextjs Spaceと同位置づけ）。**

## Phase D task ledger (2026-07-16)

| ID | 機構 | コミット | 動機 | 検証 | 状態 |
|---|---|---|---|---|---|
| D-0b | intent非依存裁定骨格とcreate差し込みの挙動保存抽出 | `7f26ad0` / `e1095ac` | createに癒着した要求集約、provenance、assurance写像、admission cap、正直終端をleafへ分離し、create→骨格の一方向境界を確立 | Next.js full/build-failed/interaction-partial＋data full/static/failedの6本をevent 81-key・JSONL・verdict・assurance・terminal projectionのbyte fixtureで固定。骨格productionのcreate固有語ゼロをguardrail化し、`cargo test`（unit/conformance/corpus/data conformance/guardrail/doc）全green、fmt・clippy `-D warnings` green | **complete** |
| D-0c | intent裁定骨格のlive非悪化ゲート | 本コミット | D-0b抽出後の両profile live runでverdict・assurance・event形・正直終端の非悪化を確認しD-0を閉じる | `uat-test0716-d0c-001` 12/12正直終端、false-full 0、Quiz 1/2 full、assuranceインフレ/デフレ0、source event名148/148追加削除0、裁定event 81/43/54-key signature一致、byte fixture 6/6・`cargo test`全green | **complete** |
| D-1 | fix intent v0契約、時系列evidence、runtime裁定 | 本コミット | fix fullを「開始時に失敗した同一Rが修正後に成功し、凍結済みprofile回帰集合が全件成功」に固定し、baseline非再現・Rすり替え・回帰集合縮小/改変・stale epoch・未実行獲得を拒否する | `fix_intent_conformance` 9/9（6 negative＋F2/回帰failure写像＋schema）、runtime/UltraPlan focused tests、fix event corpus、D-0 create byte fixture 6/6、guardrail、fmt、clippy `-D warnings`、full `cargo test` | **complete** |
| D-1b | CLI intent明示選択と解決記録 | 本コミット | goal文言に依存せず`--intent create\|fix`で計画・runtime経路を固定し、省略時は既存検出をbyte互換で維持する | CLI create/fix/省略/不正値、明示fix 4段計画、明示createのfix風goal上書き、`intent_resolved` 1回発行、標準event fixtureは同event 1行追加のみ、create裁定byte 6/6・fix conformance 9/9・全test・fmt・clippyを検証。D-0c create比較のsource event名基準は148→149（`intent_resolved`追加）へ更新する。D-1で追加済みの`fix_evidence_recorded`も含む全intent source集合は149→150であり、以後の全intentゲートは150を比較基準とする | **complete** |
| FIX-1 | bounded child processの継承`NODE_ENV`正規化 | 本コミット | `uat-test0717-fix-001` 6/6の`NODE_ENV=production`汚染と2本のdev依存不足を受け、tool・verify・probe共通境界で親由来値を除去する。子コマンドが明示した値は保持し、profile固有モードを強制しない | productionホスト下のbounded childおよびnpm setup、`host_env_normalized { variables:[NODE_ENV], strategy:unset_inherited }` 1回発行、create裁定byte 6/6・fix conformance 9/9・全test・fmt・clippyを検証。全intent source event名基準は150→151へ更新する | **complete** |
| FIX-2 | fix契約§8由来のreproducer候補誘導 | 本コミット | `uat-test0717-fix-002`のhook goalに対するbuild選択・全域grep・別欠陥束縛を受け、goalの契約属性名またはbuild/compile失敗種別をprofile catalogのroute-bound checkへ決定的に写像する | hook Run 3/4型→`hook_attribute_present`（`src/app/page.tsx`束縛）、build型→`next_build_verify`、無言及/create→無変更をunit/corpusで固定。`fix_reproducer_suggested { basis, suggestion }`追加により全intent source event名基準は151→152へ更新し、F1 baseline gateの裁定権は不変 | **complete** |
| FIX-3 | F1失敗診断のcause/repair prompt・write圧力配線 | 本コミット | `uat-test0717-fix-002` Run 1/2/5のPhase 2停滞を受け、F1の生出力が短縮evidence化の際に後続promptへ渡らない断線を解消する | build compile診断（T21系）とPython traceback（B-2d系）をF1生出力へ適用し、file/line/error kind/excerptを`isolate-cause`→`repair`へ注入。診断時は汎用build templateを迂回し、Run 1 `initGame` fixtureで`src/app/page.tsx`・`diagnosis_mapped`、Pythonで`traceback_mapped`を固定。新event名なし（基準152据置）、create裁定byte 6/6・fix conformance 9/9・全test・fmt・clippyを検証 | **complete** |
| FIX-4a | fix診断blockの変更step限定 | 本コミット | `uat-test0717-fix-003` compile 2本で、生成時lint後にverify instructionへ付加された`write-pressure target`が実行直前planner lintに拒否された | runtime-derived診断blockは正規化後のimplement/repair系stepだけへ付加し、verify/inspectへは付加しない。Run 1 `initGame`実測形のrepair＋verify fixtureでimplementへの注入、verifyからのwrite-target語彙消失、plan lint通過を固定。新event名なし（基準152据置） | **complete** |
| FIX-4b | fix F1契約predicateのroute-bound修復target配線 | 本コミット | `uat-test0717-fix-003` hook 3本が診断なしでgeneric `required_path`の`package.json`筆頭へ落ちた。これはT27と同属の「契約診断済みsourceを汎用required pathへ失う」再発として相互参照する | 実行済みF1 failure commandをnextjs manifestの`hook_attribute_present`へ照合し、既存`contract_attribute_repair_guidance`（欠落属性・位置・例）をPhase 2へ接続。inspect/implementのwrite圧力を`src/app/page.tsx`・`contract_attribute`へ解決し、verifyは不変、`package.json`は最後のfallbackとする。実測commandの空白差、runtime F1→Phase 2、generic required-path優先順位、corpusを固定。新event名なし（基準152据置） | **complete** |
| FIX-5 | fix修復target優先順位の恒久統一 | `6decdce` | `uat-test0717-fix-004` hook/B・qwen35のmissing-export profile invariantが`package.json` / `required_path`へ落ちた。T27のinteraction診断source喪失、FIX-4bのpredicate診断source喪失と同属4例目として相互参照する | fix全修復文脈を既存resolverの`diagnosis_mapped → contract_attribute → evidence_mapped → required_path`へ集約。実測非export fixtureで`src/app/game-engine.ts`が筆頭かつ`package.json`非選択、create裁定byte 6/6、fix conformance 9/9、corpus、full `cargo test`、fmt、clippy `-D warnings`を検証。新event名なし（基準152据置） | **complete** |
| D-2a | fix × data profile配線 | 本コミット | FIX-2のR候補供給を`DomainProfile`境界へ一般化し、data goalを`pipeline_probe`、results/reconciliation/claims、inspectionのmanifest catalog checksへ決定的に写像。タイプA仮説との差分は、internal catalog Rをfix runtimeがshell扱いする未配線1点で、既存profile executorへのdispatchを追加した。F3の`pipeline_probe + final-bound E1〜E4`凍結とB-2d traceback→FIX-5 target resolverは既配線のため変更せず検算のみ | data R写像3種、internal catalog Rの実check fail→pass、F3集合snapshot、`python3 -B pipeline/main.py` traceback→Phase 2→`traceback_mapped`、data fix計画corpus、nextjs R/4段plan snapshot、fix conformance 9/9、create裁定byte 6/6、full `cargo test`（browser probe権限あり）、fmt、clippy `-D warnings`。新event名なし（基準152据置）。作業時間概算: 約30分 | **complete** |
| FIX-6a | data reproducer写像のlive語彙・正準path較正 | 本コミット | `uat-test0717-dfix-001`のpipe goal原文`実行がエラーで失敗`が連続token `実行エラー`に一致せず、`fix_reproducer_suggested`が0/3だった決定的欠口を閉じる | 実測goal原文と`実行がエラー` / `エラーで失敗` / `失敗します`を`pipeline_probe`へ写像し、語彙なしでもmanifestの正準`pipeline/main.py`字義言及を`basis=goal_path_mention`で提示。既存data traceback写像、data plan corpus、nextjs R/4段plan snapshotを維持。新event名なし（基準152据置）。作業時間概算: 約10分 | **complete** |
| FIX-6b | F1 reproducer自壊の分類・確定前再構築 | 本コミット | `uat-test0717-dfix-001` Run 6でinline Python Rのescaped newlineが対象読込前の`SyntaxError`となり、退化Rの失敗がF1候補になった効率損失を閉じる。full偽装耐性のlineage・極性裁定は変更しない | F1 failureを論理上`subject_failure`（schema-v1既定値・通常bytesでは省略）/ `reproducer_defect`（明示field）に分類。defect attemptは`before-attempt-{epoch}` evidenceへ保存してF1/lineageを未確定のまま決定的feedbackを返し、1回だけR再生成を許可する。F1確定後の再束縛はruntime guardと既存lineage conformanceで拒否。Run 6実測fixture、分類・feedback・再構築・確定後拒否、fix conformance、create裁定byte、corpus、全test、fmt、clippyを検証。`fix_evidence_recorded`にdefect時のみoptional `failure_classification`を追加したが、新event名はなく基準152据置。作業時間概算: 約25分 | **complete** |
| FIX-7a | data fix cause-isolationの存在条件つき成果物参照 | 本コミット | `uat-test0717-dfix-002` Run 4/6で、不在の`output/inspection.json`をisolate-causeが無条件にread要求してF1後に停滞した | data fixのisolate-causeだけにruntime-bound artifact policyを付与し、F1 evidenceと実在ファイルを一次材料に固定。不在のcreate正準成果物を読むread-only stepは生成後にも除外し、実在時は従来どおり保持する。Run 4実測fixture、存在/不在、planner lint、nextjs fix非適用、create裁定byte 6/6、fix conformance 9/9、nextjs 4段snapshot、corpus、guardrail、fmtを検証。新event名なし（基準152据置）。作業時間概算: 約15分 | **complete** |
| FIX-7b | data fix cause-isolationの役割漏出正規化 | 本コミット | `uat-test0717-dfix-002` Run 2/3で、`isolate-cause`内にimplement作業が混入し、read-only executorの権限と指示が不整合になった | data fixだけにphase-role policyを設け、write要求は後続`repair`へ移送して`implement`権限で実行する。選択規則は(a)移送を優先し、後続repairがない非標準planだけ(b)isolate内implement再分類とする。targetはFIX-5共通resolverの`diagnosis_mapped/traceback_mapped → contract_attribute → evidence_mapped → required_path`順を再利用。Run 2実測fixture、移送・fallback再分類・planner lint、nextjs fix snapshot、create裁定byte 6/6、fix conformance 9/9、corpus、guardrail、full `cargo test`、fmt、clippy `-D warnings`を検証。新event名なし（基準152据置）。作業時間概算: 約15分 | **complete** |
| FIX-8 | data fix計画の成果物所有権重複解消 | `6a06b9e` | dfix-003 `planner_error` 原文「duplicate expected path ownership: pipeline/main.py in fix-pipeline and run-pipeline」。FIX-7bの移送後に同一成果物を複数stepが所有し得た副作用を恒久化 | implement系stepに所有権を残し、verify/run系stepは参照へ降格する既存正規化をFIX-8規則として明文化し、実測計画fixtureと回帰テストで固定。作業時間概算: 約20分 | **complete** |
| FIX-9a | fix計画の空verify既定束縛 | `db35d97` | dfix-003のpipe gemma B・schema gemma B・schema qwen A2の3runで、変換後のverify配列が0本となり`verify step requires at least one verify command`でscaffold停止。実装コメント記載どおり、FIX-7bのstep移送・phase filter後に空verifyを検出する経路を対象化 | reproduce/verify-afterはF系R・回帰、isolate-causeはread-only検証、repairは`test -f`＋関連catalog checkを束縛し、束縛不能は従来どおりerror。作業時間概算: 約35分 | **complete** |
| FIX-9b | 不在成果物参照の再計画経路修正 | `81fe12f` | dfix-003 schema qwen Aの一次資料「phase isolate-cause failed: path does not exist: output/inspection.json」。FIX-7aのisolate prompt適用後、repair/recovery再計画のstep bindingが同じ不在参照を再導入していた経路を特定 | dataのrepair/recovery bindingにも存在条件フィルタを適用し、write-capable stepは保持、read-only参照のみ除去。nextjs/createには適用しない。作業時間概算: 約25分 | **complete** |
| FIX-9c | pipeline probe出力のlossy UTF-8化 | `27f9022` | dfix-003 pipe qwen Bの`phase isolate-cause failed: stream did not contain valid UTF-8`を受け、probe stdout/stderr境界の厳密UTF-8変換がphase停止を起こす経路を特定 | 不正バイトを置換して内容つき継続し、置換発生は既存capture warningへ記録。新event名は追加せず、イベント基準152への影響なし。作業時間概算: 約20分 | **complete** |
| FIX-9d | FIX-9予算guardrail適合分離 | 本コミット | FIX-9バッチ（`db35d97`/`27f9022`）は権限制約下の検証により予算違反4件（data_isolate 160・pipeline_probe 382・setup_step_policy 435/935・verify_default 164/191/355）を非検出のまま受理された。preflight gateが1件目を捕捉後、全75予算のread-only監査で残余を全数特定し、純移動のサブモジュール分離で解消（baseline引上げなし）。以後、実装バッチの受理条件に権限付きfull suite greenを必須とする | 存在フィルタ・stream capture・preset変換・空verify束縛を各leaf moduleへテスト込みで純移動。全予算監査、fix conformance 9/9、create byte互換6/6、nextjs fix snapshot、fmt、clippy `-D warnings`、権限付きfull suiteをgreen確認 | **complete** |
| D-2b | data fix計画の機械合成 | 本コミット | dfix-001〜004 v5の24 runはmachine-only 15・model-only 4・複合5で、D-2a後もF1写像ではなくplanner生成StepPlanの所有権重複・空verify・不在成果物参照がキャンペーンごとに新形で支配した。これにより「profile配線不足」を主因とするタイプA仮説は部分反証され、個別rewriteの追加ではなくfixed契約F1〜F3から4段構造を合成する判断へ移行 | `--intent fix --plan-preset profile --profile data`だけを対象に、既存`fix_reproducer`・contract checks・FIX-5 target resolver・凍結F3集合から`reproduce-before → isolate-cause → implement-fix → verify-after`を既存StepPlanへ合成し、既存lint・実行・裁定へ通す。Rをgoalから解決できない場合だけモデルへ構築を委ね、step構造は機械側で再束縛。manifest schemaは不変、nextjs fixはnone同経路、create全経路は不変。pipe/schema snapshot、実F1実行fixture、再発3原文negative、fix conformance 9/9、create byte互換6/6、nextjs fix snapshot、corpus、予算、fmt、clippy `-D warnings`、権限付きfull suiteで検証。`fix_plan_synthesized { profile, phase_count, r_basis }`追加により全intent source event名基準は152→153。作業時間概算: 約60分 | **complete** |
| D-2c | 計画確定チョークポイントの一本化 | 本コミット | 複数扉一門番病（時間軸版・4例目）の恒久修正。適用範囲は合成経路（fix×data）。planner生成経路は挙動保存のため従来形（repair chain→template lint、変異後再正規化なし）を維持し、第1層の穴は生成経路に残存する。生成経路の恒久解は計画合成の適用拡大であり、もぐら叩き（正規化追加）では行わない | 合成・実行計画のfinalize chokepointと、生成経路3箇所の`lint_template_contract`を字句guardrailで固定。既存fixtureでイベント形・verdict不変を確認。作業時間概算: 約30分 | **complete** |
| D-3b | investigation intent契約v0 | 本コミット | D-3b開始: investigation intent契約v0をfixed（本コミット）。fix契約の実装済み機構（stage/極性/reproducer_defect/baseline_not_reproduced）を再利用する前提で設計。I2 diagnosis_bound はE2 claims-bindingの診断版であり、虚偽診断をfailedとする（partialではない） | **fixed** |
| D-3b工程2 | investigation intent実装 | 本コミット | fixed契約I1/I2、diagnosis binding、data profile 3段計画合成、CLI・earned assurance conformanceを実装。create/fix既存経路は差し込み前fixtureで維持 | `investigation_plan_synthesized`・`investigation_adjudicated`追加により全intent source event名基準153→155。作業時間概算: 約120分 | **complete** |
| INV-1a | investigation完了投影のintent dispatch | 本コミット | `uat-test0718-inv-001`で全6runがdata create由来の`static(data_profile_probe_not_run)`を表示した投影dispatch欠落を修正。T30(B-2c)・B-2jに続く投影層の同属3例目として、profile×intentの両軸dispatchへ移行 | `investigation_adjudicated`を最優先し、I1実行済み未裁定を`failed(investigation_incomplete)`、R未実行を`static(investigation_probe_not_run)`へ固定。inv-001 run 2/run 1/未実行の3形fixtureとcreate/fix既存byte fixtureで非影響を検証 | **complete** |
| INV-1b | investigation diagnose契約ガイダンス | 本コミット | inv-001 run 6の例示code block 5件全違反とrun 4の不在`output/inspection.json`参照に対し、DATA-1の生成側字義例・存在束縛処方をdiagnosis版として適用 | 実観測で置換するerror/path:line/実在code引用の字義例、修正案code block禁止、hidden/runtime/heavy directory除外済み・辞書順最大64件の実在file一覧を合成diagnoseへ注入。run 6型unbound fixtureをstrictに拒否したまま、ガイダンス準拠対fixtureがviolations 0になることを固定 | **complete** |

## D-1 close: fix × nextjs (2026-07-17)

| 行程 | コミット / 計測 | 封緘根拠 |
|---|---|---|
| 契約fixed | `74ce3e4` | fix fullをF1 before failure、同一lineageのF2 after success、凍結回帰集合F3全passに固定 |
| 裁定・runtime実装 | `643e814` | fix adjudication、時系列evidence、baseline/lineage/regression/epoch gateを実装 |
| CLI明示選択 | `d955032` | `--intent fix`と`intent_resolved`を文書・台帳まで封緘（実装commitは`4342e8c`） |
| FIX-1 | `32e14d0` | 親由来`NODE_ENV`をbounded child境界で正規化 |
| FIX-2 / FIX-3 | `e24f542` / `e0f3f67` | 契約由来R誘導とF1診断のPhase 2配線 |
| FIX-4a / FIX-4b | `63532c6` / `b99b624` | 診断注入を変更stepへ限定し、predicate failureをroute-bound修復targetへ接続 |
| live計測 | `8754592` / `61ab3f5` / `0fedc86` / `2f45863` | `uat-test0717-fix-001`〜`004`、4 set・24 run・各run再試行なし。raw 24、FIX-1前の環境留保2本を除く宣言分母22 |
| intent軸集計・band宣言 | `e4aee4a` / `c9d5ae1` | intent列、fix 2族、F1〜F3 false-full abortを機械化し、[`band_summary_fix.md`](../../workspace/management/runs/band_summary_fix.md)を生成。既存nextjs/data bandはbyte不変 |

初fullは2026-07-17の`uat-test0717-fix-001` / `fix1_compile_gemma31_001`。
F1 `npm run build` failure@epoch 1から、同一lineageのF2 success@epoch 2、
凍結回帰`profile_contract` / `profile_verify_1` success@epoch 3/4までの実物連鎖が
存在し、full以外の設計品質は主張しない。

偽装耐性のlive実績は、#2の`baseline_not_reproduced`拒否2件、偽full 0件。
FIX-2後の#3/#4ではR関連性逸脱0を維持した。lineage不一致、回帰集合縮小、
epoch逆転の拒否はconformanceで固定済みだが、この24 runでは未行使であり、
live実績として推定しない。

intent軸の学習ループは4計測setで初バンドへ到達した。比較用の履歴値は
nextjs × createが10 set、data × createが9 setであり、同一精度の能力率比較
ではなく、intent追加時の較正set数としてのみ記録する。fix × nextjsの正式分布は
compile_error_fixがgemma31 1/4・qwen35 0/5、contract_hook_fixがgemma31
0/7・qwen35 0/6である。

B-3のadmission cap / manifest admissionはprofile単位であり、intent単位の
admission機械化は未実装である。fixの本宣言は測定bandの封緘であって新しい
admission gateではない。intent単位の規律の機械化はE-0へ委譲する。

**D-1 closed (2026-07-17): fix契約v0、明示CLI、F1〜F3裁定、4セット24run、
初full、偽full 0、intent軸の機械生成bandを封緘。FIX-5はD-2前queueとして分離後、
`6decdce`で閉鎖した。**

## D-2 close: fix × data (2026-07-18)

| 行程 | 内容 |
|---|---|
| 配線・較正 | D-2a 30分、dfix-001〜003でFIX-6〜9を較正 |
| 分離・診断 | 予算違反4件を分離、dfix-004 v1〜v5の空振り4発行、収束不能を3層診断 |
| 構造投資 | D-2b計画合成、D-2cチョークポイントと適用境界、診断B判定 |
| 最終計測 | dfix-005 v2、合成6 run、機械クラス0/6、repair read-only停滞6/6 |

タイプA仮説は部分的に反証された。配線は30分（予測どおり）だが、
intent×profileの真のコストはplanner生成計画の脆弱性にあり、恒久解は
正規化の追撃ではなく計画合成という構造投資だった。この教訓を第3 intent以降の
見積り基準とする。UAT教訓は、自己完結指示、発行者確認済みrepoパス、完全一致
コマンド、分節実行、不在主張の検索方法提示を恒久規則とする。

Phase D出口条件「fixが2profile×2族以上で分布計測済み」を達成。残件はD-3（連結run）。

## D-3b close: investigation intent (2026-07-18)

| 行程 | コミット / 計測 | 清算内容 |
|---|---|---|
| 契約fixed | `87d432a` | I1 reproducer failureとI2 diagnosis binding、およびfull/partial/static/failedの境界を実装前に固定 |
| 実装 | `3922668` / `ed8c201` / `badbc81` / `1f50e4f` | adjudication、I2照合、3段計画合成、CLI・conformanceを4コミットで実装 |
| 初計測 | `df1afdb` / `uat-test0718-inv-001` | 6 runでI1 6/6、I2到達2/6。投影dispatch欠落とdiagnosis形式逸脱を検出 |
| INV-1 | `3dea4e8` / `3302dd9` | intent別完了投影dispatchと契約由来diagnoseガイダンスを追加 |
| 第2計測 | `ea00fa7` / `uat-test0718-inv-002` | 6 runでI1 6/6、投影不整合0。I2到達2/6、形式逸脱は減少したが残存 |
| バンド | `00672c4` / `9485163` | 12 runをI1/I2/adjudication不変条件つきで機械集計し、generalityへ転記 |

### Intent追加コスト2点目の実測

コード量は `git diff --numstat 87d432a..1f50e4f -- src` で実装4コミットを
測った。production sourceは +1,201/-44（net +1,157）。主要な新規leafは
`adjudication/investigate.rs` +211、`investigation_plan_synthesis.rs` +208、
I2照合器 `investigation_binding.rs` +249、investigation runtime +190である。
INV-1を含む `87d432a..3302dd9 -- src` は +1,580/-49
（net +1,531）で、投影dispatchの `completion_metadata/intent.rs` +188を
含む。実装時のsrc+tests総差分は同じ範囲の `git diff --stat` で
+1,418/-49だった。

| 会計項目 | 実数 / 算出方法 |
|---|---|
| タスク数 | 6（契約、実装、inv-001、INV-1、inv-002、band/settlement） |
| コミット数 | 11。`git rev-list --count 87d432a^..9485163`。本settlementコミットは自己参照なので除外 |
| 計測 | 2 set、12 formal run、非消費0 |
| 実装時間 | 台帳に記録された工程2概算120分。契約・INV-1・band行には時間概算がないため推測加算しない |
| UAT純工程時間 | inv-001: 92+437+382=911秒、inv-002: 271+917+485=1,673秒、合計2,584秒（43分04秒） |
| UAT実行区間wall採用時 | inv-001: 92+640+382=1,114秒、inv-002: 271+1,137+485=1,893秒、合計3,007秒（50分07秒） |
| 記録済み総時間の下限 | 実装120分＋UAT純工程43分04秒 = 163分04秒。時間未記録の3工程は隠さず未算入 |

D-2で得た見積り基準「配線＋合成器＋計測2〜3セット」は**当たった**。
合成ファーストによりD-2型のplanner較正ループは発生せず、機械一次死因は
0/12、計測は予測範囲内の2セットで閉じた。所要の点推定はD-2で固定して
いなかったため、上表の163分04秒は比較可能な記録済み下限として扱う。

create/fix/investigateの3実例がD-0骨格上に成立し、骨格のrule of twoを
越えた。`IntentSchema`への宣言化抽出条件は整ったが、着手はPhase D-3a/Cの
後とし、E-0域で判断する。

Phase Dの分布条件は5セルまで到達した。調査→fixの連結runはD-3a未達であり、
これを残件とする。

## Brand migration Phase 2 (2026-07-20)

| 対象 | 導入内容 | 互換境界 | 検証 |
|---|---|---|---|
| 環境変数 | 外部名を `COMMANDAGENT_*` へ統一 | canonical優先・旧名fallback・旧名単独時は変数ごとにprocess内1回だけ警告 | 新のみ／旧のみ／両方／なしのmatrixとwarn-once test |
| 設定 | workspace/homeの `.commandagent/config.toml` とworkspaceの `.commandagent/config` をprimary化 | 各scopeで旧namespaceをread-only fallbackとして維持。live `.anvil/` runtime stateは非変更 | 新のみ／旧のみ／両方の優先順位test |

本Phaseは環境変数と設定発見だけを移行し、event schema、runtime state、履歴evidence
には変更を加えない。

## Brand migration Phase 3 decision (2026-07-20)

決定は **Option A（内部プロトコル識別子を維持）** とする。

| 境界 | 維持する識別子・契約 |
|---|---|
| 生成成果物の属性 | `data-anvil-*` |
| LLM tool-call fallback | `<anvil_tool_call>` |
| 内部app識別子 | `anvil_app` |
| live runtime state | `.anvil/` |
| 機械可読データ | JSON keys、event names、schemasをすべて現行どおり維持 |

これらはユーザー向け製品名ではなく、LLMとの動作契約、corpus、既存runtime
state、および機械消費側との互換境界である。Phase 1の可視名変更やPhase 2の
外部設定入口移行の対象には含めず、本決定に伴うproduction code・fixture・
schema変更は行わない。

過適応監査: 本決定は内部の旧綴りを固定し、保守者による一括renameの自由を
狭める一方、ユーザー向け命名や将来のversioned protocolは制約しない。維持
コストが実害として観測された場合の再訪経路は、専用migration Issueでversioning
またはdual-read、fixture/corpus、`.anvil/` state migrationを同時に設計・検証
することであり、互換境界を暗黙に書き換えてはならない。

## bench v0 (2026-07-21)

bench v0——dfix-004型プロトコル事故クラス（参照解決不能/採取元不在/wrapper/preflight飛ばし）の構造的排除。判断の自動化はしない線を文書化。

## bench実弾試験の収穫#1 (2026-07-21)

bench実弾試験の収穫#1——自己出力によるpreflight自己封鎖の解消。clean要求の趣旨（他作業混入の防止）を保存したまま適用範囲を精密化。

## 受理CI (2026-07-21)

受理CI——FIX-9型（非green受理）と手動scrubの構造的排除。CIは必要条件。

## 二重トラック合流点検#1 (2026-07-21)

二重トラック合流点検#1——UXウェーブのclippy非green合流をCIが初回走行で捕捉。清掃と、Issue/PRトラックへのCI必須化（本CIがその強制装置）を記録。

## bench v0.1実弾受理 (2026-07-21)

bench v0.1実弾受理（H-1〜H-5）。合成アーム機械ゼロが計11runに。missing_arg反復＝モデル起因tool call不正形として分類。

D-3a開始: workflow円環契約＋schema v0をfixed（本コミット）。YAMLは構成のみ（固定語彙・未知語彙拒否）、辺発火はearned（evidence実在＋lineage連続＋epoch順序）、洗浄禁止（fix full≠circle_full、閉門=起点束縛の再検証）。workflow宣言駆動（時間軸①）の初実装対象
D-3a-2: workflow円環層の実装コスト実測（9コミット相当: schema strict parse、earned edge、node実行シーム、verify_origin導出・終端、CLI→orchestrator→executor配線、conformance corpus）。2度の部分着地（conformance骨格→シーム→閉線）を経て完成。D-3a-2cスモークはcreate→investigate辺のrun_stop証拠不足でcircle_failed終端。
D-3a-2d: 実測fixture主義違反を是正し、起点E-Bを実レイアウト（.anvil/runs/*/events.jsonl＋.anvil/plans/recovery-*.yaml）へ統一。ノード観測イベント基準を追加。
D-3a-2e: D-2c同型の教訓を円環へ適用。ノード集合走査をrouteグラフ駆動へ置換し、E-A〜E-D辺ゲート後のみ起動、実UUID run_idを配管する。
D-3a-2収穫#5: workflow子実行の再入デッドロック。スモーク4回の無音ハングの根因。6時間ハング実プロセスのsample採取（`__psynch_mutexwait`）で確定。修正=子実行は`run_config`直呼び・panic捕捉は外側バウンダリに一元化。検証: 採取スタックは`run_resolved_config_for_workflow → run_resolved_config → catch_cli_run → PanicHookGuard::install → Mutex::lock → __psynch_mutexwait`、構造回帰テストは子経路のpanic hook導入0回を固定。C2の停滞しきい値緩和は却下し不変。
バンド再生成可能性の破れをCI→調査で検出。集計器の静かな0化を禁止。nextjs入力セットの復元はQUEUED（アーカイブ形式の解析→現行走査への適合、別タスク）。
D-3a-3a: workflow schema v0→v0.1。nodesにexecutor用model/providerの任意ペアを追加し、省略時はグローバル構成を継承。改訂対象は§7のみで§1〜§6の裁定意味論は不変。バンドはモデル構成に付く原則により、ノード構成の異なるworkflowは別計測列とする。
circle-001の収穫——伝播欠落の系（workspace→model→profile/goal/イベント先）を全数監査で恒久終了。監査表を実装コメントに常設。計測3runはP0成立・モデル分布としては無効（profile不適用）と記帳。
circle-002の収穫——全数監査の境界はConfig構造体ではなく呼び出し面。ultra_plan_run欠落により合成不発・prompt経路縮退。監査表に呼び出し面の節を常設。

## D-3a settlement（中間、2026-07-22）

| 行程 | 確定点 | settlement上の意味 |
|---|---|---|
| 契約 v0 | `18d9668` | §1〜§6の裁定意味論、earned edge、lineage、verify_origin、洗浄禁止を実装前にfixed化 |
| 初期実装 | `f73dd18`〜`308efff` | strict schema、辺検査、node実行シーム、verify_origin、CLI→orchestrator→executorを閉線 |
| スモーク v1〜v8 | `8b530b1`〜`863c8e3` | 合成fixtureから実レイアウトへ移し、route駆動・封じ込め・実executorを順次実在確認。v8で初の完全一周と正直終端 |
| schema v0.1 | `5fffda9` | node単位model/providerを追加。§1〜§6不変、異なるnodeモデル構成は別計測列 |
| D-3a-3c 全数監査 | `972107a`〜`ec768ec` | Config全フィールドの由来を固定し、profile/goal/event出力/run_idを一元化 |
| D-3a-3d 呼び出し面監査 | `a983725` | Config外の`ultra_plan_run`を固定し、合成計画経路への縮退を解消 |
| circle-001〜003 | `e977ce6` / `5e03899` / `1aec3a2` | 001・002を機械欠陥により除外し、003をlocalアームの正式値札0/3として確定 |

剥がした欠陥の総目録は、発明レイアウト、workflow子実行の再入
デッドロック、辺ゲート不在、workspace伝播欠落、Config誤12行＋潜在3、
呼び出し面の`ultra_plan_run`欠落、PATH旧バイナリ×2である。いずれも
モデル分布へ洗浄せず、fixture・配線・監査境界または実行手順の欠陥として
切り離した。

実装コスト実測（git履歴）: 契約封緘`18d9668`からcircle-003確定
`1aec3a2`まで17時間28分41秒、連続43コミット。このうちD-3a本線は
40コミットで、同区間に挟まったband整合性専用3コミットを除く。
`src/`・`tests/`・`workflows/`のいずれかに接触した実装・conformanceは
24コミット、run証跡を含むコミットは16、docsを含むコミットは11
（複数カテゴリの重複あり）。初期見積り9コミット相当に対し、実測fixtureと
live smokeが配線・伝播境界の欠陥を段階的に露出させた差分をこの実数に残す。

D-3a settlementは中間であり、D-3aクローズはelevatedアームの完了時と
する。Phase D出口条件は、schema v0.1のnode別上位モデル経路で
調査→fix連結を実測1本成立させることである。local 0/3は正式値札だが、
この出口条件の代替にはしない。

D-3a-3f / elev-001の収穫——R解決の語彙依存が円環導出goalで不発。
起点実態からの機械導出＋事前検証済み供給へ（語彙もぐら叩きの回避・
D-2b合成と同じ構造判断）。これによりR構築のplanner依存が円環経路から
消える。schema v0.1の固定carry語彙に`reproducer_suggestion`を追加する
§7改訂であり、§1〜§6の裁定意味論は不変。
`workflow_reproducer_prevalidated`と`workflow_reproducer_bound`を追加し、
単intentの全intent source event名基準155は不変。

D-3a-3g / E-A過投影の修正——`run_stop.status=completed`とノード裁定の
`assurance_level=full`を混同していた参照を裁定eventへ統一。elev-002では
E-Bが契約違反遷移を阻止しており、E-A/E-B二重防壁の実効を確認した。

D-3a-3g / `claims_absent`の構造手当——reproduce-candidate段のR失敗出力を
redact済み有界tail・決定的抜粋・既存B-2d traceback mappingとしてdiagnose段へ
機械注入するFIX-3/T21系を再利用。I2照合器とfull閾値は不変。

D-3a-3h / I2較正#1——認識錨をキーワードから出力実在へ。E2較正
（date_label）と同型の検証器精密化。D-3bクローズ時のWATCH
（違反原文の蓄積待ち）の履行。circle-elev-003の実測3runが較正コーパス。
D-3a-3i: 円環の設計完成——検証済み診断が修正の照準になる（調査労働の成果物化）。
D-3a-3i: カタログR形の contract 写像は未観測ギャップとして単発fixにも適用。
D-3a-3j: elev-005の収穫——照準被覆を完成（R字義パス規則＋manifest駆動producer写像）。列挙でなく規則化（FIX-6a/D-2b型）。
D-3a-3k: 存在前提の3周を全数監査で終了。修復ターゲットは指定であり事実ではない。存在分岐はwrite段の責務。
D-3a settlement（最終、2026-07-23）: 契約v0→schema v0.1→スモークv1〜v8→circle-001〜003→elev-001〜008。計測21run、スモーク8本。剥がした機械欠陥14項目（発明レイアウト／再入デッドロック／辺ゲート不在／workspace伝播／Config誤12行／潜在Config3行／呼び出し面ultra_plan_run欠落／PATH旧バイナリ／R提示不発／E-A過投影／I2認識錨／照準被覆／存在前提3周／HTTP500環境中断）。存在前提は全廃し、前提追加は監査表で検問、binary乖離はversion hash確認で防止。

Phase D complete (2026-07-22): 出口条件①fix intentの2profile×2族以上の分布計測（D-2、2026-07-18）②調査→fix連結runのend-to-end成立（D-3a、2026-07-22、circle_full実測1本・全証拠連鎖）。両達成し、行列は2profile×3intent＋workflow層。「検収済みAI労働のワークフロー基盤」が実測つきで成立。D-3c（PMルーター＋境界対話シェル）はPhase C依存としてC完遂後のキューへ移管。
D-3a-3k: 存在前提の3周を全数監査で終了。修復ターゲットは指定であり事実ではない。存在分岐はwrite段の責務。
C-1a開始: evidenceの人間翻訳層。acceptance_sheet.pyは転記主義・発明禁止で開始。
C-1a完遂: 差し戻しを受け、実evidenceの観測値（I2/F/E2/verify_origin）を転記するテストと3枚の実演を追加。テストgreenと翻訳の実在は別であることをPhase C版として記録。
C-2完了: デモは実在証跡の再生であり、演出物・モック・合成データを使用しない。C-1aシートの実測値と一次資料パスを台本・手順書へ固定。
Phase C complete (2026-07-23): ①検収シート3形の自動生成（full/failed/circle——内容検収基準つき）②10分デモの再演可能性（リハーサル実測8分40秒）③KPI定義と初期値記録（未計測項目は未計測と明記）④D-3c引継メモ。翻訳債務（場面3）の返済完了。演出物ゼロ——デモは全て実在evidenceの再生。
Phase E開始（2026-07-24）: 失敗クラス登録簿とレポート自動分類。人間が見るのは未知だけ、への転換初弾。分類は表示のみで裁定を自動化しない。
E-0-2: 較正が常に材料つきになった。抽出器の形被覆はE-0-1自己実演UNKNOWNの収穫——可視化設計の初勝利。
E-0-2是正: 終端形（failure_kind）と帰属（死因）の混同を訂正。保存形を全走査し、較正材料を実run由来で蓄積した。
E-0-3: 計測すれば必ず検収シートが付いてくる体制。E-0三部作完了（分類自動化・較正自動蓄積・翻訳自動生成）。
E本体開始: E-0で規律を機械に写した。E本体は増殖性の実証（scaffold→IntentSchema→第3profile判定戦）。
E-1: scaffoldジェネレータを追加。契約・manifest・conformance・corpus・admissionをoff状態で先行生成し、判断を空欄のままレビューへ渡す。
E-2a: IntentSchema草案を既存create/fix/investigateのrule of two実例から抽出。設計先行で、実装判断はレビュー後。
E-2b段階1: investigate IntentSchemaを構成専用として導入。合成実体・照合・裁定はRustに残し、byte互換証明を受理基準とする。
E-2b段階1検証: schema unit 1件はgreen。full suiteは外部probe/Ollama等22件失敗のため互換証明宣言を保留し、実体修正なしで停止報告。
E-2b段階1再検証: schema 1/1、investigation synthesis 5/5、conformance 8/8 green。イベントは既存fixture照合で差分なし。権限付きfull greenと基線行列は環境未復元のため証明保留。
E-2b最終確認: CI API接続不能で経路Cの確定値を取得できず、経路Bも同一環境条件未復元のため互換証明を保留。
E-2b基線試行: HEAD失敗集合は33件と再集計。基線checkout→develop復帰は確認したが、逐次exact行列は未完遂のためB判定・証明は保留。
E-2b受理条件整理: 同一HEADの失敗集合が22→33件に変動し、本環境で厳密B行列は成立不能。次の健全セッションでのHEAD full suite greenを確定条件とし、fix移行は保留。
E-2b段階1証明済み（条件付き追認）: 環境非依存3点＋反復行列6/6 pass。低頻度flakeの共有状態除去はQUEUED、発現時は3+3行列で再診断する。
E-2b予約: fix段階2は合成snapshot＋conformance 9、create段階3はmanifest preset＋byte互換6/6を各々先行証明する。
E-2b段階2診断: 中間コミット583c1a4はテストヘルパー未importで非ビルド可。以後の多段実装はコミットごとにcargo checkを受理条件とする。
E-2b段階2証明済み: fix snapshot 5/5、conformance 9/9、イベント互換、A-B-A環境行列、full suite 1758/30/0（passed/ignored/failed）。
E-2b段階3証明: create schema配線（profile manifestが形、intent schemaが意味）、manifest/create 6/6、fix snapshot 2/2、corpus 1/1、guardrail 9/9、full suite 1759/30/0。E-2b全段完了（クローズ宣言はレビュー確認後）。
E-2 complete: IntentSchema——3 intentのbyte互換移行完了。検証器と証明値（段階1: snapshot5/5+conformance8/8+反復行列6/6、段階2: snapshot5/5+conformance9/9+A-B-A交絡解消+full 1,758、段階3: 互換6/6+full 1,759）。盲点3件（予算枯渇flake・環境残留交絡・ローカル/CI検査乖離）を記録。「profileが形を、intentが意味を宣言する」境界を確立。scaffold(E-1)へのintents/*.yamlテンプレ追加をQUEUED。残るはE-3（第3profile判定戦）。
E-3開始: 第3profile(cli)のscaffold実測・draft契約・カタログ束縛計画。判定戦の主メトリクスは新規Rust行数（現時点0）。
cli契約v0 fixed（2026-07-24）: 実行観測主義・ケース束縛凍結・6負例。E-3判定戦の裁定基準を確定。
受理CI乖離の根治: 真因はRuffバージョン差（ローカル0.14.9／CI0.16.0）。0.16.0へピン固定し、ツールチェーン固定を受理信頼性の教訓として記録。
所要ラベル裁定: 円環全体18秒とノード実行6秒は両実測。ラベル曖昧が真因で記録訂正不要。acceptance sheetを2行ラベルへ精密化。
所要是正: シートは監査本体の転記装置。本体に無い数字は「記録なし」が正しい出力、人手計測は出自明示の別枠でのみ表示可。
E-3b実測: cli argv probe／help binding／draft manifest／C1〜C4 runtimeを組上げ、合成conformanceはfull 1/1・負例6/6、実CLI full 1/1。新規production Rustはコミット累計257→474→936行（見込み180行の5.20倍）、test Rustは71→127→304行。bounded process・pipeline stream capture・data E3等価判定・Manifest v1/catalog・E2型nearest_miss受け皿を流用したが、型付きevidence／freeze／assurance／adapter費を見込みから欠落していた。admissionはoffのまま。
bench v0.3: benchの計測可能領域がfix/investigate世界に閉じていた欠落を、create用empty workspaceの新規作成・無垢性検証で閉鎖し、全intentセルをbench圏内にした。
E-3c admission（2026-07-25）: cli×create localアームは機械クラス0/6（`uat-test0724-cli-001-v3`）、conformance 6負例＋実fixture、正直終端6/6であり、`off until admitted`の昇格条件が成立した。full 0/6（0%）の値札を隠さず`admitted`へ昇格する。timeseries 0/12を受容済みバンドとして宣言したPhase B封緘と同型であり、到達runだけにC系evidence実在を要求する。
CLI-1: 投影dispatch病の3例目（T30/B-2c→INV-1→CLI-1）。admitted CLIのC1未実行が汎用seedから`partial(acceptance_not_full_success)`へ投影された欠落を、契約§4のC1〜C4写像で個別配線した。加えてE-1 scaffoldのadmission checklistへ「completion assurance投影写像の実装＋実測fixture」を必須化し、profile追加の定形装備へ昇格した。
CLI-2: DATA-1族第3属——goal語彙のverify字義束縛。CLI createではモデル生成UltraPlanのREADME phaseに元goalが入り、そこから汎用plannerが生成したStepPlan verifyを正準化する防壁がなかった。README実在・見出し配下の`cli/main.py`起動例という言語中立の構造検証へ置換し、内容忠実性をC3管轄へ分離して根治した。同属監査ではdataは`step_policy::canonicalize_step_plan`のcatalog-checkチョークポイント、nextjsはmanifest由来のdeterministic StepPlan templateで生成verifyを固定しており、goal語彙字義が実行assertへ直通する経路は現存しない。CLIだけが`profile=cli`の汎用planner経路に留まり、両防壁を通らなかったことが固有機序だった。
CLI-3: 配線の実在病の反復（D-3a-2円環→CLI-3）。conformanceはC1〜C4部品の単体成立を証明したが、`profile=cli`のfinal acceptanceはgeneric behavior probeへ落ち、productionから部品を起動しなかった。共通final acceptance境界からCLI manifest runtimeへ配線し、実subprocessと4 evidenceを確認するproduction経路テストで起動実在を固定した。scaffold admission checklistにも「各検証部品のproduction acceptance経路からの起動テスト」を必須化し、conformanceは部品を、経路テストは起動を検証する二層を定形装備にした。
CLI-4: C部品初陣の較正一括——E2（49偽陽性）/I2（認識錨）と同じ通過儀礼。elev-003実測原文だけをfixtureに、C1 optional/placeholder記法をsample実在値へ正規化、C2方向2を凍結正常argvへの未知option追加投与へ精密化、C3のlabel付き隣接出力blockを実行照合した。C4とC2方向1は初陣成立のまま厳格性不変。併せてCLI final repairからNext.js fallbackを遮断し、C2/C3 nearest_miss形を較正collectorへ接続した。

## E-3 settlement（最終、2026-07-26）

### 判定戦の主文

問い「カタログ部品だけで新profileが立つか」への答えは、
**部分的に否・ただし方法論は成立**である。

共通機構の流用自体は成立した。新規実装なしで使えた実測リストは次の
とおり。

- `bounded_process`の環境allowlist、process group終了、timeout outcome。
- `verifier_env::normalized_command_at_root`のworkspace境界interpreter解決。
- pipeline probeの有界stdout/stderr capture、truncation metadata。
- data E3の再実行等価判定leaf。
- Manifest v1のclosed schema、phase scope、catalog解決、admission cap。
- E2型のclaim / observation / verdict / `nearest_miss`受け皿。
- E-1 scaffoldの契約・manifest・conformance・corpus・admission先行生成。
- E-0のrun自動分類、検収シート自給、較正corpus、scrub、bench preflight。

しかし新しい検証面そのものはカタログから無料では出なかった。CLI C1〜C4
では照合器本体に加え、当初見積りから漏れた配管5点、すなわち
①型付きevidence schema、②実行前case freeze、③4 check→2 componentの
catalog dispatch、④honest assurance classification、⑤manifest adapterが
必要だった。新規production Rustは257→474→936行、test Rustは
71→127→304行。production見込み180行に対し実測936行、756行超過、
5.20倍である。

さらにproduction化には構造的な通過儀礼が付随した。

1. 投影配線欠落（CLI-1）: C1未実行をpartialへ漏らした。
2. 起動配線欠落（CLI-3）: conformanceで部品が存在してもfinal acceptance
   がC runtimeを呼ばなかった。
3. repair境界欠落（CLI-4）: CLI failure後にNext.js artifactへfallbackした。
4. 初陣較正1周（CLI-4）: C1 optional/placeholder、C2方向2の必須引数遮蔽、
   C3具体出力block未抽出という3欠口を実測原文から精密化した。

これは新種の失敗ではない。投影はT30/B-2c→INV-1、起動はD-3a-2円環、
較正はE2/I2、repair境界は既存profile境界の反復である。方法論が成立した
根拠は、欠陥をモデル分布へ混ぜず、実測fixtureで切り分け、E-1 scaffold
checklistへ「completion assurance投影写像」と「production acceptance
経路からの部品起動テスト」を恒久化できた点にある。

### 行程とコミット

E-3 scoped commitは25。途中のRuff parity 2件と円環所要裁定3件は同じ
ancestryにあるがE-3件数から除外した。E-3開始
`e122d25`（2026-07-24 23:41:16 JST）からelev-004確定
`4fc9c6d`（2026-07-26 18:24:05 JST）までのwallは42時間42分49秒。
fixed契約`0b1b5d5`からは41時間53分14秒である。

| 行程 | 全commit | 確定点 |
|---|---|---|
| scaffold・契約・賭け | `e122d25`, `309ae54`, `a0fe5bf`, `0b1b5d5` | scaffold実測、draft、catalog見込み180行、契約v0 fixed |
| E-3b C部品 | `227e9ce`, `46e4f1a`, `27d787b` | C1/C4、C2、manifest/C3/runtime/conformance。production Rust 936行 |
| bench create対応・local計測 | `e1916a1`, `23ce002`, `0bc35fb`, `7087bab` | sources無しblockerを収穫、empty無垢性、dry-run、local 0/6 |
| admission・elev-001 | `704d7f9`, `afcb919` | admittedへ昇格、cloud初値札0/6、投影欠落を採取 |
| CLI-1 | `723af96`, `e3b191b` | §4投影配線、local/elevated死因解剖 |
| CLI-2 | `8d4691a`, `fca064b`, `3c063fd` | README構造verify、帰属訂正、elev-002でC未配線を採取 |
| CLI-3 | `917acef`, `9d89c50` | production C runtime起動、elev-003で1run初陣 |
| CLI-4 | `4305cd6`, `4133078`, `52f9a97`, `d854c05`, `4fc9c6d` | C1/C2/C3較正、repair境界、collector、formal elev-004 |

### 計測と時間

時間は各immutable UAT reportの`date +%s`基準。campaign間のレビュー待ちを
run costへ混ぜず、比較可能なrun列合計を使う。

| 計測 | 実run | run列 / accepted dry-run | 位置づけ |
|---|---:|---:|---|
| `uat-test0724-cli-001` | 0 | 0秒 | sources無しsuiteをbenchが拒否。機械blocker |
| `uat-test0724-cli-001-v2` | 0 | 159秒 | full-green preflight後の6/6 dry-run |
| `uat-test0724-cli-001-v3` | 6 | 5,838秒 | local reference、full 0/6、C到達0 |
| `uat-test0725-cli-elev-001` | 6 | 3,309秒 | 投影欠落期。Window Bから除外 |
| `uat-test0725-cli-elev-002` | 6 | 4,185秒 | C runtime未配線期。Window Bから除外 |
| `uat-test0725-cli-elev-003` | 6 | 3,176秒 | 配線後・較正前。C到達1 |
| `uat-test0725-cli-elev-004` | 6 | 3,739秒 | formal Window B。C到達2、C3捏造拒否6/6 |

実runは30本、run列合計20,247秒（5時間37分27秒）。accepted dry-runを
含む計測実時間は20,406秒（5時間40分06秒）。formal Window Bは正直終端
6/6、full 0/6、C evidence 2/2到達runで4点完備。local/cloudともfull 0%を
値札として保持する。

### 第4profile見積り式

次のprofile見積りは「既存部品2個×90行」の単純和を廃止し、次式を
事前宣言する。

```text
第4profile =
  共通流用面 0行（ゼロ円）
  + 新検証面の照合器 500〜1,000 production Rust行
  + 配管定形5点
  + 実測原文による較正 1〜2周
  + local/elevated等の計測 2〜3 campaign
```

配管定形5点は、typed evidence、freeze、catalog dispatch、assurance投影、
manifest/production acceptance adapterを意味する。scaffold checklistが
投影写像と起動実在テストを先に要求するため、CLIで後発した2事件は第4
profileでは実装前見積りへ入る。新検証面が既存照合器と完全同型なら
500行未満もあり得るが、実測なしにゼロ円とは裁定しない。

### クローズ

E-3はクローズする。第3profileの契約、部品、production配線、admission、
local/cloud band、E-0装備、較正corpus、generality宣言まで実測で閉じた。
第2ラウンドを行うか、ここで増殖性実証を十分とするかの進退は
**レビュー裁定**とし、本settlementから自動着手しない。

E-4開始（2026-07-26）: 第4profile `ingest` の第1段（取得済みsnapshot→抽出・整形）をscaffold駆動・draft/offで開始し、E-3見積り式（N2/N3照合器＋配管500〜1,000 production Rust行、較正1〜2周、計測2〜3campaign）を初適用する。自治体イベント整形を実ユースケースのドッグフーディング枠とし、ネットワーク取得・鮮度のfetch probeは第2段QUEUEDに残す。
E-4a契約fixed（2026-07-25）: N2正準化を値保存・決定的な事前宣言・フィールド別evidence記録の三条件で精密化し、宣言フォーマットへの表記変換を許容しつつ値改変の拒否を維持する。E2較正の教訓を実装後の偽陽性洪水より前に契約段階で先取りした。

INGEST-7契約v0.1（2026-07-28）: N2へ第3束縛域
`document-level shared context`を追加した。候補内の部分値と候補外の
title/見出し断片を、値保存・決定的宣言・両断片の出典位置記録という
三条件でのみ合成する。elev-006の`8/3(月)`＋page title
`2026年`→`2026-08-03`を偽陽性から救う一方、日付ずらし、別候補の値の
流用、文書内にない値は従来どおり拒否する非緩和の意味論精密化である。

INGEST-7 violation/資産裁定（2026-07-28）: elev-006 list 003のN2 36件は
9 record×4 fieldの個別捏造ではなく、modelが凍結candidate IDから
`data/snapshots/` prefixを全件落とした単一lineage違反の派生である。N3の
unknown 10＋unaccounted 10と同根であり、照合器は緩和しない。また
`会場未定`はsource字義値として忠実なため採用を正当と裁定し、silent drop
実弾は日付欠落の理由付き除外5/6と数える。scaffoldへ「意図した不備資産は
意味的曖昧さでなく機械抽出不能にする」をmeasurement asset design項目として
追加した。

## E-4b ingest検証部品（2026-07-27）

E-3の第4profile見積り式（production Rust 500〜1,000行）を初めて実測
した。追加行はコミット単位で、testは`#[cfg(test)]`以下とintegration
testを別計上する。

| コミット | 部品 | production Rust | test Rust | production累計 |
|---|---|---:|---:|---:|
| 1 | N3 selector宣言読取り・実行前freeze・候補列挙・勘定 | 391 | 140 | 391 |
| 2 | N2同一候補block束縛・三条件正規化・nearest_miss | 478 | 90 | 869 |
| 3 | manifest/catalog、N1/N4/N5、runtime、投影、起動実在、conformance | 929 | 434 | 1,798 |

production合計は1,798行で、予測上限1,000行を798行超過（上限比
1.798倍、下限500行比3.596倍）。testは664行、Rust総追加は2,462行。
部品別にはN3 391、N2 478、commit 3のcatalog+manifest 242、
N1/N4/N5 runtime+assurance 463、domain/final acceptance・凍結lineage
配線150、投影74。
N2/N3だけで869行に達し、予測帯が配管5点を数値へ十分織り込めて
いなかったことを第4profileでも確認した。

流用実測は、N1=`pipeline_probe`の隔離・有界実行、N5=data rerun adapter
と共通等価判定leaf、N4=data E4のclosed key/type assertion境界
（goal宣言field用adapterは新規）、Manifest v1/catalog/draft admission cap、
E2/I2型`nearest_miss`、E1型候補勘定式である。

scaffold checklist自己評価は、fixed契約束縛・実profile/intent・N1〜N5
manifest・assurance投影fixture・production final acceptanceからのN1〜N5
起動実在・conformance 6負例+full正例まで完了。実run archiveとreviewer
admissionは未完了なので`admission=off`/production manifest `draft`を維持
する。合成fixtureは部品conformanceを閉じるが、初陣較正を代替しない。
pre-push受理はconformance負例6/6・full正例1/1、production起動実在1/1、
権限付き`cargo test --all-targets` 1,814 passed / 0 failed / 30 ignored。

INGEST-1フェーズ正準化（2026-07-27）: D-2b教訓の第2変奏として、
検証成果物をモデルに書かせる生成StepPlan設計が「壊れた検問所」を量産し、
ingest初計測では4/6 runを直接停止させた。manifest/presetは検証scriptを
要求しておらず、近因はmodel生成物、設計根因は生成計画をprofile境界で
再束縛しなかったmachine側にある。ingest createのphase gateを
`pipeline/main.py`、JSON出力、正準selector宣言、reportの構造だけへ固定し、
意味検証はN1〜N5 acceptanceへ一元化した。

INGEST-2進捗意味論（2026-07-27）: 「書く=進捗」の一義が実行型phaseで
過制約化し、成功実行を差分なしとして4/6 runで停滞誤殺した。相異なる
exit 0 commandを実行型進捗へ加える精密化を行い、同一command反復、
非零終了、純読取りtool（Read/Glob/Grep）は非進捗のまま維持した。
investigate read-onlyからC2較正を経た停滞検知系譜の精密化であり、
検知閾値やread-only loop終端は変更していない。

INGEST-3正準形字義例（2026-07-28）: DATA-1処方の第4適用
（data字義例→INV-1診断形→cli主張形→ingest正準形）。elev-002では
`candidate_selector`の`kind/value`正準形を散文だけで要求し、6/6のモデルが
同じ文字列形へ逸脱したため、近因model／設計根因machine（knowledge）へ
帰属を精密化した。structure gateの全要求形、許容selector語彙、
inspection/accounting/record format、records配列の字義例と
「値は例・実snapshot観測で全置換」のDATA-1定型を合成/preset/repair
ガイダンスへ先行配布し、実測負例→正準正例の対fixtureで固定した。
E-1 scaffold checklistには「構造gate要求形の字義例ガイダンス＋fixture」を
投影写像・production起動実在に続く第3の定形装備として必須化した。

INGEST-4 ingest create preset（2026-07-28）: D-2b構造転回の第3適用
（fix合成→investigation合成→ingest preset）。elev-003で残った
verify内変更要求と2/6の検証script expected_pathは、開いたplanner生成分布を
事後正準化する構造の症状だった。ingest createの既定計画を
implement（4納品物の所有権を一本化・字義例同梱）→
`python3 -B pipeline/main.py`実行→機械structural gate→N1〜N5 final
acceptanceへ固定し、UltraPlanと各StepPlanのplanner自由作文をproduction
経路から除いた。全StepPlanは既存`finalize_step_plan_for_execution`を通る。
「1計測1欠陥」を閉じるため、E-1 scaffold checklistには「計画は
machine synthesis/profile preset、planner自由作文禁止」を投影写像・
起動実在・構造gate字義例に続く第4の定形装備として必須化した。

INGEST-5 preset段分解（2026-07-28）: elev-004のmodel帰属6/6を
machineへ訂正した。INGEST-4のレビュー発行指示自体が、pipeline実行で生まれる
`output/records.json` / `output/report.md`をimplement段のexpectedへ置き、
run前に6/6を16〜24秒で停止させた。modelが直接書く
`pipeline/main.py`は6/6、inspectionは4/6、実行成果物は0/6という分布を
一次根拠とする。fix合成で確立した所有権原則を「段の期待成果物はその段の
生成能力に限る」へ再確認し、implementはmodel-authored 2件、runは
`python3 -B pipeline/main.py`実行後のrecords/report機械postconditionへ
分離した。検証は削除せず実行後へ移し、欠落をcommand failureとして固定する。
裁定者の指示もmachine floorの入力として監査対象である。

INGEST-6入力実構造注入（2026-07-28）: DATA-1系の第5適用
（data字義例→INV-1診断形→cli主張形→ingest正準形→入力実構造）。
elev-005のfinal acceptance到達4runは、`data/snapshots/*.html`のGlobと
`ls -R`だけを反復して本文をReadせず、実構造と一致しないselectorを4/4で
宣言して候補0件となった。近因はmodel、設計根因は読むべきpathだけを指し、
材料をgeneration promptへ置かなかったmachine knowledge gapと二層裁定する。
ingest implement presetへ、ファイル名を併記した先頭12行と反復候補要素周辺
2 windowを、ファイル数・探索entry/depth・bytes・行長の全境界つきで機械注入
する。「セレクタは実在構造から導出し、構造一致時以外は例示を写さない」を
明記した。E-1 scaffold checklistには「読むべき入力は目の前に置く」を一般化し、
bounded machine-injected source material＋実測fixtureを第5の定形装備として
追加した。

INGEST-8機械語彙配布（2026-07-29）: elev-007では全6runが、machineが
freeze evidenceで発行した`data/snapshots/events-*.html#N`からprefixを
落とし、N2 216 fieldとN3 6/6が同じlineage不一致で停止した。近因model、
設計根因machine（発行語彙をgeneration promptへ返さなかったknowledge
gap）と二層裁定する。ingest implement phaseを、実構造つき暫定
selector/record format宣言→candidate freeze→pipeline/final inspection実装へ
分け、後段implement promptへ凍結ID全件を字義注入する。照合側はexactまたは
`/`境界の一意suffixだけをcanonical IDへ決定的解決し、provided/matches/
resolved/statusをN2/N3 evidenceへ残す。曖昧suffixと偽IDは従来どおり
violationであり、語彙配布と解決責任をmachine側で閉じるDATA-1系の完結形。

E-4d admission（2026-07-29）: `uat-test0726-ingest-elev-008`は機械クラス
0/6、conformance 6負例＋full正例、localからelev-008まで全54run正直終端、
偽成功0であり、`off until admitted`の昇格条件が成立した。local full 0/6、
elevated full相当4/6（66.7%）の値札を隠さず`admitted`へ昇格し、
runtime-shaped投影fixtureでearned fullが`profile_not_admitted`へcapされず
fullを表示することを固定した。

## E-4 settlement（第1段、2026-07-29）

### 問いと答え

問い「E-3で宣言した第4profile見積り式は当たったか」への答えは、
**照合器は当たり、配管は等倍、較正は大幅な過少見積り**である。

- N2/N3照合器は869 production Rust行で、予測500〜1,000行の帯内だった。
- manifest/catalog、N1/N4/N5 adapter、typed evidence、freeze、投影、
  production起動を含む配管は929行だった。照合器比1.07倍であり、
  「新検証面とほぼ等量の配管を買う」という等倍則を確認した。
- 較正は予測1〜2周に対し、実測で機械床8枚＋契約改訂1回を要した。
  local 1＋elevated 8の9campaign、54runのformal run列合計は
  23,004秒（6時間23分24秒）だった。

### 較正乖離の内訳

8枚はcampaign数を都合よく周へ畳まず、再発防止可能な欠陥クラスとして数える。

| 系統 | 枚 | 機械床 | 一次観測 |
|---|---:|---|---|
| 伝達 | 1 | verification成果物をmodelへ自作させた責務伝達 | local |
| 伝達 | 1 | workspace Python実行をdependency setupへ誤分類したauthority伝達 | local |
| 意味論 | 1 | 相異なる成功commandを進捗と認識しない実行意味論 | elev-001 |
| 伝達 | 1 | structure gateのkind/value正準形を字義配布しないknowledge | elev-002 |
| 伝達 | 1 | planner自由作文・実入力材料未注入を残した計画源／材料伝達 | elev-003、elev-005 |
| 段設計 | 1 | run生成物をimplement期待へ置いた段×生成主体の不整合 | elev-004 |
| 意味論 | 1 | 正当な複合CSSを列挙できないselector engine被覆 | elev-006 |
| 伝達 | 1 | freeze済み正準candidate IDを後段implementへ返さない語彙配布 | elev-007 |

内訳は**伝達5・意味論2・段設計1**。これとは別に、候補内の部分日付と
文書見出し年を値保存で合成する`document-level shared context`を定義するため、
fixed v0からfixed v0.1へ契約を1回改訂した。契約改訂を機械床へ混ぜず、
予測外コストとして独立表示する。

第5profileでの再発防止見込みは、E-1 scaffoldへ恒久化した次の5点を
実装前検問として使うことにある。

1. completion assurance投影写像とruntime-shaped fixture。
2. production acceptance経路からの部品起動実在テスト。
3. structure gate要求形の字義例ガイダンス。
4. machine-synthesized plan/presetによるplanner自由作文の禁止。
5. boundedな実入力材料のprompt注入。

これは再発ゼロの保証ではない。既知の伝達・段設計gapを実装前に落とし、
残った未知の意味論gapだけを較正campaignへ送る見込み値である。

### 改訂見積り式

```text
新検証面 =
  照合器 500〜1,000 production Rust行
  + 配管 等倍
  + 較正（機械床N枚:
      伝達・意味論・段設計の3系統をchecklistで事前検問した残り）
  + 計測 5〜10 campaign
```

流用面は既存実装を再利用する限り新規行ゼロだが、adapter・typed evidence・
dispatch・projection・production activationを配管として必ず別見積りする。
「較正1〜2周」は廃止し、床の分類と除去を数える。計測5〜10campaignは
ingest実測9campaignを根拠にした帯であり、local/elevated、機械欠陥除去後の
正式窓を含む。

### クローズ

E-4第1段をクローズする。保存済みHTML/text snapshotからの自治体イベント
整形について、N1実行、N2出典束縛と値保存正規化、N3候補勘定、
N4宣言format、N5再実行一致、admission、local/elevated band、
generality宣言まで実測で閉じた。値札はlocal full 0/6、elevated
full相当4/6（66.7%）であり、壁は空の必須dateを採用したモデル2/6である。

第2段のnetwork取得・鮮度を検証するfetch probeは**QUEUED**のまま維持する。
Excel/JSON等の入力拡張も第1段fullへ遡及して含めず、
`docs/dev/integration-notes.md`の事業queue 5件として分離する。

## E-5a — 文字列プロトコルの型付け（2026-07-29）

`src`内のID生成51箇所を種別enum＋`Display`へ中央化し、発話文字列を
byte互換のまま固定した。Rust enumとPython producerの全ID→`classes.toml`、
全`match_stop_class`→Rust/Python実在producer、の双方向整合guardにより、
ドリフト事故2件（抽出UNKNOWN・ID接頭辞）の族を構造的に封鎖した。

初回guardは未登録6 familyと越境producer 1件を実際に検出した。6 familyは
`violation_family`として形状既定modelの仮置き帰属つきで登録し、
`interrupted(environment)`はPython benchの正式producer語彙として宣言した。
これは可視化設計の3勝目（E-0-1 UNKNOWN、ingest床監査に続く）であり、
不整合をgreenへ隠さず、登録簿の種別不足とcross-language境界を先に露出させた。
最終登録簿は36件（terminal 30、violation_family 6）。

## E-5b — ProfileRuntimeレジストリ（2026-07-29）

`runner.rs`の監査済みprofile dispatchを**110→3**へ縮約し、107地点を
typed `ProfileId`から解決した`ProfileRuntime`へ移した。残存3地点は
workspace推論境界1件と、typed IDを描画・比較するtelemetry 2件だけである。
runtime解決は`ProfileRuntimeRegistry::resolve`の**1点**、production
`runner.rs`のprofile字義behavior分岐は0件となった。

再散逸guardは残存3形を明示allowlistし、4件目、旧string dispatch helper、
profile字義比較を拒否する。同時に監査台帳の110 unique site・消化107・
残存3を機械照合する。第5profile追加の同一定義による接触ファイル実測は
**26→20**。completion projection、acceptance起動、preset、repair、
guidance/material、probeのdispatch配線をレジストリが肩代わりし、
profile固有の照合意味論・字義ガイダンス・runtime-shaped fixtureは
引き続きchecklistで検問する。

## E-5c — evidence共通エンベロープ（2026-07-29）

E/F/I/C/N/circle/workflowの**7 family**へ、既存フィールドを改名・削除
せず、`evidence_envelope`を加法追加した。必須形は
`envelope_version/family/kind/epoch/claims/nearest_miss/source_refs`。
歴史evidenceは無改変で、3消費者は「新形を統一読取り／旧形を従来読取り」
の明示fallbackを持つ。

family追従guardは登録7 × 横断3消費者（較正collector・検収sheet・classify）
= **21/21**を初回green。未adapterの新familyと死んだadapterを双方向で
列挙失敗させ、ingest nearest_miss流出事件の族を構造的に封鎖した。
Rust enumも同じ機械可読登録簿7/7へ突合する。

同時に`workflow_started`/`workflow_adjudicated`へUnix秒`epoch`を追加し、
共通エンベロープのepochと単一値で束縛した。新しい検収sheetは円環全体
所要を自動導出し、epochを持たない歴史fixtureは従来どおり
「記録なし」と表示する。E-5a/bのbyte互換とは異なり、E-5cでは
**加法互換**を移行規律として確立した。

## E-5d — runner責務3分割（2026-07-29）

`runner.rs`を案Aの3責務へ純移設した。物理18,087行
（production 9,655 / inline test 8,432）から、facade 144行
（production 131 / test 13）へ縮小し、driver 3,291行、phase 3,931行
（配下flow 1,656行）、acceptance 2,523行へ責務を固定した。
production側の親globは0件で、test名前空間の4件だけを意図的に残す。

最初に22-eventのordered lifecycle fixtureを現挙動から採取し、その後の
全バッチでprompt bytes、adjudication 6形、conformance、corpus、event順序を
無変更greenにした。既存snapshot・fixture・event・evidence・stop-class文字列は
1バイトも変更していない。最終の権限付き`cargo test --all-targets`は
1,873 passed / 30 ignored / 0 failed、fmt・全target clippy `-D warnings`もgreen。

growth guardのrunner-family物理被覆は、移行前19,688行
（runner 18,087 + 旧flow 1,601）から移行後26,751行
（production module群11,545 + runner tests 15,206）へ拡大した。差分7,063行は、
従来guard外だった既存外出しtest 6,677行と機械的なmodule/import wiringを
被覆へ入れた結果であり、空いたrunner予算は全て引き下げた。分割先全fileと
test aggregate/per-file、production/test別の上限をguardし、分割をgrowth
guard回避路にしない。

E-5bと同一定義の第5profile接触面は**26→20のまま**で、分割による再増加は0。
型/registryがcompletion投影、production acceptance起動、preset選択、
repair policy、guidance/material注入、probe選択の6 checklist項目を肩代わりする。
profile固有意味論・字義例・runtime-shaped fixtureのレビューは残す。

16状態のE-5f設計は
[`e5f-phase-state-machine.md`](e5f-phase-state-machine.md)へ固定した。
状態機械化は**QUEUED**であり、案A完了後の地形に対するレビュー裁定まで
production control flowへ刃を入れない。

## E-5e — 残債の決定化とpanic境界（2026-07-30）

既知flake 2件を検証意図を弱めず決定化した。
`final_acceptance_budget_exhaustion_uses_last_cycle_reason`は共有
`AtomicUsize`の予測可能port帯とbind/check/dropのTOCTOUをやめ、
test専用ephemeral leaseからproduction dev-serverへ所有権を渡す。
キャンセル猶予テストはTERM trapのready同期点と仮想clockで100msの
猶予全量を検証し、壁時計80ms assertだけを除いた。両者はそれぞれ
20/20連続greenである。

dev-server port監査は`TcpListener::bind` 24地点を、listener保持15、
subprocess用ephemeral選択4、unreachable負例3、availability観測2へ分類した。
共有Atomic allocatorと二重bindを除去して24→22地点、Atomic allocator 0とし、
未guardだったrunnerの3境界を既存lifecycle mutexへ収容した。これは小差分で
決定化可能な枠(a)の処置で、productionの`port_in_use`正直失敗は変更していない。

productionの裁定・投影・照合器・registryにある`unwrap/expect`は初回33件。
内訳は裁定1、投影0、照合器/leaf 26、registry/embedded manifest 6だった。
run上位の裁定1件を同値な`Option` matchへ変換し、残り32件を
repository-owned静的不変条件24と局所証明つきleaf 8へ裁定した。panic許容は
静的定義・testだけとし、外部/run入力境界ではtyped failureを必須にした。
leaf 8件の全型変換はbounded QUEUEDとして値札を残す。

## E-5 settlement（2026-07-30）

E-5の数値主文は次のとおり。

| 工程 | 清算値 | 構造的な封鎖 |
|---|---|---|
| E-5a 語彙 | 登録36 class（terminal 30 / violation family 6）、Rust/Python producer双方向guard | ID発行と登録簿のdriftをCIで拒否 |
| E-5b dispatch | profile分岐110→3、runtime resolve 1点、第5profile接触26→20 | `ProfileRuntime` registryと再散逸guard |
| E-5c evidence | 共通envelope 7 family、横断3消費者21/21、workflow epoch債務返済 | 新旧fallbackとfamily追従guard |
| E-5d runner | `runner.rs` 18,087→144行＋driver/phase(+flow)/acceptanceの4 module、guard被覆+7,063行 | ordered lifecycle fixtureと全移設先growth guard |
| E-5e residual | flake 2件各20/20、port bind 24→22・Atomic allocator 0、unwrap 33→上位0＋裁定済み32 | 仮想clock・ephemeral allocation・panic layer policy |

E-5a〜eを通じ、profile追加時に人が読むだけだったチェックリストと監査表を、
typed vocabulary、registry、evidence envelope、追従/再散逸/growth guard、
責務module、panic境界へ降ろした。プロセス資産を型と実行時/CIの検問へ
変換したことがE-5の清算結果である。

## Phase E出口宣言（2026-07-30）

Phase Eは次の4成果を満たしたため、ここで出口を宣言する。

1. **規律の機械化**: E-0の自動分類、較正collector、検収sheet自給が、
   CLI/ingestの新セルでもUNKNOWN収穫・較正・sheet生成として複利稼働した。
2. **intent宣言化**: E-2でcreate/fix/investigateの3実例を宣言へ移し、
   snapshot/conformance/event文字列のbyte互換を維持した。
3. **profile追加方法論**: E-3/E-4で見積り式v2、scaffold 5装備、
   CLI/ingestの実戦較正を得た。自治体イベント整形は出典束縛・候補勘定・
   正規化・捏造拒否の実弾つきでelevated full相当66.7%を記録した。
4. **構造債務の返済**: E-5a〜eで語彙、dispatch、evidence、runner責務、
   flake/port/panic境界を型・registry・guard・moduleへ移した。

未達は隠さず次段へ送る。第三者による1セル追加の実証はG/BP1、
第4 intentの宣言追加実測はQUEUED、E-5f状態機械化は16状態の設計済み
QUEUEDである。ingest第2段fetch probeを含む事業queue 5件と、
局所証明つきunwrap/expect 8件のtyped変換も現行fullへ含めない。
現在地の正準indexは`docs/dev/integration-notes.md`の
「Phase E exit: canonical next-stage queue」に集約した。

## P-0b — パック契約v0封緘（2026-07-30）

P-0の4裁定を反映し、exact-byte hash、C1後の`cli-validation`所有、
v0構造schemaの充足と改訂儀式、イン・リポジトリ`packs/`を固定した。
署名つき外部供給はPhase GへQUEUEDとし、未知キーから将来schemaを
推測しない。パックはRust登録済み部品の構成であり、契約フロア以下へ
検証を弱体化できない。

実装では、棚卸し済み**注入器17経路中4族**（ingest入力構造、
ingest凍結candidate ID、investigateのR出力、fixの失敗出力・診断carry）
をtyped builtin pack registry経由へ移した。既存rendererとevent producerは
変更せず、4族それぞれの実測snapshot/fixtureとcorpusを無変更greenにして
prompt/event bytesのbyte互換を証明した。

strict decoderは未知・重複key、YAML拡張、発明ID、不正なsource/point、
非正規pathをRust登録簿との突合前後で拒否する。contract-floor mergeは
profile manifest/intent contractを底として、check除去・境界移設・timeout
拡大・extractor/normalizer除去を拒否し、登録済みnarrowingだけを許可する。
初回のreviewed `ingest-default@1.0.0` conformanceはfloor 5/5・schema 1、
exact-byte hash
`sha256:becb151410f52276c066aed0f80772b39babf683e44b8ea49b2a275af5492c2b`
でgreen。bench metaには任意の`pack.id × pack.hash`欄を加え、未指定suiteの
既存metadata bytesは維持した。

## CLI-5 — 証言修復の照準と材料（2026-07-31）

pack-002のlive 2runはC1材料を注入できた一方、汎用fallbackが
`cli/main.py`をwrite pressureの先頭へ置き、C3主張の抽出元`README.md`を
書けない照準交絡を起こしていた。C3 evidenceの出典を
`testimony_artifact_mapped`で照準し、全C3主張について
「README記載 / 実出力」の有界対照を渡す`c3_binding` sourceを登録した。

これは「照準＋材料の対原則」である。材料は正しい行動チャネル
（修復照準とwrite pressure anchor）と揃って初めて援助になる。
full率を構成する`モデル × 試験 × 援助`のうち、援助は単一テキストではなく
`(材料, 照準)`の対である、と精密化した。

## P-1 CLI arm settlement — 援助の天井（2026-07-31）

pack-001〜003の実験系列は`n=18`、C系到達かつrenderer liveが3件だった。
証言成果物への照準とC3対照材料の被覆という2枚の交絡を順に除去した後も、
援助飽和状態でREADMEへの転記行動は0件だった。live 3件はいずれも同じ
C3証言違反署名で終端したため、`cli × C3`の残余の壁をモデルの
証言修復行動と確定する。

これは「援助の天井」の初実測である。援助で動く壁と動かない壁は、
`材料 × 照準 × 圧力`の三点を完備してから残余をモデルへ帰属する手順で
判別する。材料不足や誤照準を残したまま効果ゼロをモデルへ帰属しては
ならない。

## P-1b — Next.js契約の遡及封緘（2026-07-31）

Phase A産の`nextjs × create`について、現行のbuild・route・browser・
state検問を遡及文書化し、T1証言束縛の語彙錨抽出、型付き語彙、
段階assurance、bandラベル透明化を裁定して契約をfixedへ封緘した。
これにより、稼働中profileのうち契約文書を持たない最後のセルを解消した。

## Phase P settlement — パック制度の稼働宣言（2026-07-31）

| 工程 | 清算値 | 獲得した制度 |
|---|---|---|
| P-0 | 制度契約fixed、注入器17経路・評価binding 20（19 distinct ID）・抽出器候補8・normalizer 5を棚卸し | packは実装済みRust語彙の構成であり、検証実体や裁定演算をYAMLへ移さない境界を固定 |
| P-0b | 17経路中4族をbuiltin pack経由へ移し、prompt/event bytesを4/4 byte互換で維持 | strict decoder、exact-byte hash、契約floor guard、1コマンドconformance、benchのpack pin受け口 |
| P-1 | `cli-assist`と`data-assist`の2本を実装。CLI armはpack-001〜003の`n=18`、renderer live 3件 | `材料 × 照準 × 圧力`を完備してから残余をモデルへ帰属する手順と、援助飽和でも動かない壁の天井を実測 |
| P-1 testimony floor | 仮のeval IDをpackへ発明せず、Rust T1→固定Next.js契約→production acceptanceの順で実装 | 閉じた語彙の裁定どおり、証言フロアはまず契約・Rust側でearnedにし、その後だけpackが参照できる |
| P-2 | bench metaへpack ID × exact-byte hashをpinし、全6 profile bandへprofile別`Full meaning`と証言試験状態を表示 | 能力値をモデル名だけでなくpack構成と試験の意味へ結び、同じ`full`という語の非対称を可視化 |

Phase Pはここで清算する。パック制度は単に外部ファイルを読めるだけでなく、
**効く場所、効かない場所、それを測る手順**を獲得して稼働した。
援助の効果がゼロだった場合も、露出前の未検証、照準交絡、材料不足、
三点完備後のモデル残余を分離して記録できる。P-3の利用者ループ実証は、
この制度を人間へ提示するD-3c境界対話シェルの後段で行う。

## D-3c contract seal — 境界対話の実装裁定（2026-07-31）

D-3c設計をfixedへ封緘した。正式band由来のtyped task-family catalog、
決定的一意でない場合だけの閉語彙LLM分類、決定的一意を含む全routeでの
永続human confirmation、4境界gate、既存REPLへのleaf配線を実装方式とする。
見積りはproduction Rust **1,320〜2,280行**、較正**5〜10 campaign**を
確定値とし、確認前dispatch・REPL bypass・偽の決定的一意を必須guardで封鎖する。

## D-3c provider-call boundary correction（2026-07-31）

D-3c実装で保護監査が曖昧分類器の直接provider呼びを初日捕捉した。
分類呼び出しを既存`provider_call`のclone worker・timeout・cancel polling・
turn event経路へ移し、短応答要件も同じchokepoint内の512-byte上限として
適用した。allowlist例外はゼロで是正した。chokepoint網は既存部品だけでなく、
新規コンポーネントにも即日有効であることの実証となった。

## D-3c settlement — 境界対話の清算（2026-07-31）

D-3cは、設計draft、封緘、B0〜B5、provider-call境界是正1件の
**8コミット**で完了した。production Rustは**2,322行**で、事前見積り
1,320〜2,280行に対して上限比**+1.8%**（+42行）だった。是正コミットの
増分67行は、曖昧分類を既存`provider_call` chokepointへ通し、有界応答・
timeout・cancel・event記録を継承する代金に帰属する。確認前dispatch、
REPL bypass、偽の決定的一意を塞ぐguard 3種に加え、保護監査が直接provider
呼び出し1件を実装初日に捕捉し、allowlist例外ゼロで是正した。

`d3c-shakedown-001`は4ゲートを通し、N1〜N5全pass、`assurance=full`へ
到達した。Gate 1の永続確認記録は
`sha256:564ec8f762ef42048d0f4e22ae088ba201865490de2e1c2d3ef10df103b9f62c`
であり、会話表示ではなく記録hashからdispatchの正当性を再監査できる。

見積り方法論の第3実測として、CLIは見込みの5.2倍、ingestは照合器が
予測帯内かつ配管が等倍、D-3cは上限比+1.8%だった。差の大小ではなく、
**事前宣言→実測→差分の機序帰属**を毎回主文に置く運用が定着した。

## D-3d v1.1 — 指示セッションとT2F記録の開始（2026-08-01）

指示はWriteを解錠した。パック材料は`材料 × 照準 × 圧力`を飽和させても
live 3/3でREADME編集を起こさなかった一方、明示的な人間指示はround 1で
READMEの実編集を起こした。この対照を、援助の階層
`材料 < 照準 < 明示指示`として記録する。ただしWriteの発火と編集品質は
別である。round 1は新しい起動例欠陥を導入し、構造ゲートが拒否した。

v1.1は同一lineageを`boundary-sessions/<session-id>/`へ束ね、round 2以降へ
過去の指示原文とevidence由来の結果を有界注入する。さらにGate 3からの
full後改修は、直前fullを獲得したcheck集合の回帰凍結なしには起動できない。
v0単発指示のartifact・prompt・event bytesは維持した。

`d3c-shakedown-002`のround 2ではround 1履歴の注入とREADMEへの反復Writeが
実在したが、`cli/main.py`起動例を復元できず構造ゲートで正直終端し、C3は
未到達だった。T2F記録を開始するが、数値`2`は未獲得であり、現時点では
round 2までの右打ち切りとして扱う。

## CLI証言基準線の完成（2026-08-01）

`d3c-shakedown-002` round 2は「起動例を`python3 cli/main.py`に戻す」という
字義指示とround 1履歴を受けても、別pathを保持したまま構造gateで終端した。
レビュー裁定により、次の成功可能roundは3以降なので基準線を**T2F≥3**とする。
これはC3 passを獲得したという意味ではなく、字義指示への不服従まで含む
F-2 Luna比較の下限値である。

| 援助段 | 実測した材料 | cli証言壁での結果 |
|---|---|---|
| 材料 | C1実出力、C3対照、nearest miss | 編集行動ゼロ |
| 照準・圧力 | README照準、write anchor、修復圧力 | live 3/3で同一違反 |
| 明示指示 | 永続化した人間指示と履歴 | Writeは発火したが字義指示に従わず、round 2でもfull未達 |

援助階層の全段を通した同一セルの基準線が揃った。F-2はこの基準線に対し、
モデル階級だけをLunaへ替えてT2FとC3到達・転記を比較する。

## F-0 — OpenAI providerとドリフト探針（2026-08-01）

OpenAIの厳密ID `gpt-5.6-luna`をChat Completions互換経路へ追加し、既存
`provider_call` chokepointのclone worker・timeout・cancel・turn eventを
そのまま継承した。既存OpenAI Responses/Ollama/Gemini経路は変更せず、
新経路だけが返却model ID、response ID、`system_fingerprint`、service tier、
created epochを中央turn eventへ加える。これがband構成同一性をendpointの
実体へ錨付けする最初のドリフト探針である。

資格情報境界は`OPENAI_API_KEY`のprocess environment専用とし、clientの
Debug、HTTP反射error、events、summary evidenceの全てで同一keyが不在となる
負例を固定した。live hello probeは`gpt-5.6-luna`を返し、fingerprintは
provider返却どおり`null`、11 input / 4 output token、1.871秒、公式単価に
基づく推定USD 0.000035だった。実物は
`workspace/management/runs/f0-openai-smoke/`に保存した。

## F-2a-3 — tool protocol能力の明示宣言（2026-08-01）

暗黙前提の第2連弾（parameter注入に続く能力仮定）を受け、モデル構成へ
`tool_protocol=native|text`を追加した。能力は宣言し、モデル名から推測しない。
明示`text`は新しいprotocolを増やさず、ローカルモデルで歴戦済みの有界tools
非送信・XML/text指示・typed解析・通常repair経路をOpenAIにも流用する。
未宣言経路は既存provider capability判定を保存し、既存request bytesを変えない。

## F-2a-5 — tool parse失敗の自己記録化（2026-08-01）

Luna text実測の個別解剖で、parse失敗を生んだ応答本文を機械が破棄し、
error文字列から3/6を裁定できない観測gapが判明した。text parse失敗は今後、
failure kind、parse error、失敗点周辺のscrub済み512-byte原文、model、protocol、
phaseを加法eventとenvelope準拠evidenceへ自己記録する。`tool_parse`を横断family
guardと較正collectorへ接続し、モデル族ごとの方言較正を他の照合器と同じ
コーパス駆動へ降ろした。これは可視化設計の4例目である。

text橋の最小能力は成立している。Lunaは003窓の4runで5件のWriteを既存
text/XML protocol経由で実行した。一方、安定完走にはモデル族ごとの方言較正が
必要である。レビューの「唯一の橋」評価には、textは作動するが普遍的に透明な
橋ではなく、Responses API/native tools（F-0b）との比較裁定が要る、と注記する。

## F-2a-6 — text tool-call方言較正・第1周（2026-08-02）

`tool_parse`較正コーパス6件を入力に、プロトコル普遍の最終fallbackを2規則だけ
追加した。先頭の完結JSON値はobject・tool名・allowlist・必須arguments型の全検証を
通る場合に限って採用し、後続を捨てる。開きtool-call tagと完結JSON bodyがあり、
閉じtagだけが欠ける場合に限って閉じtagを補う。いずれも適用前に通常parserと
ToolSpecを再通過させ、適用時はscrub済み256-byte上限の変更片を`repair_applied`
eventとenvelope evidenceへ必ず自己申告する。モデル名による分岐はない。

裁定者予測の外れも保存する。事前の「おしゃべり文」推論に対し、F-2a-5の実物は
余分な`}` 1件・閉じtag欠落3件・根本的な散文不遵守2件だった。推論から修復器を
発明せず、実物がコード推論に勝ち、較正コーパスから限定規則を導いた実例である。

## F-0b — OpenAI Responses正門とLuna native実測（2026-08-02）

壁5枚目はendpoint作法不一致だった。推論系はChat Completionsでfunction toolsを
拒否し、text橋では散文・空応答へ退行した。`api=responses`を明示構成として追加し、
reasoning/message/function_call output itemと推論状態をprovider契約で保持・再送する
正門へ移した。既定は`chat_completions`のまま、モデル名sniffはなく、全呼び出しを
既存`provider_call` chokepointへ通した。

`uat-test0801-cli-luna-006`はResponses nativeで112/112 provider turns、115 function
calls、endpoint rejection 0を記録し、C系へ5/6到達した。C3はpass 2 / violation 1 /
claims_absent 2で、LunaがREADMEの実出力を正直に転記した初実物をstats/filter各1件で
得た。fullは0/6で、残る壁はC1 placeholder束縛、生成Python互換性、C3不一致、
assurance投影へ分解された。

これによりtext橋の判定を確定する。観測と方言較正を配当した上で、reasoning系
フロンティアにはtext橋は不適であり、Responsesが正門である。一方、text較正で得た
自己記録化と限定repairの制度的配当は残る。Luna 006費用はcached inputを分離して
USD 0.312987、reasoning tokensは8,399だった。

F-2a-7はCLI assurance投影gapの4例目を確定した。ただし今回は、未実行や違反を
高く見せる偽成功側ではなく、C1/C2/C4 pass・C3 `claims_absent`という契約§4の
partial形をfailedへ落とす初の過小評価側だった。Luna 006の実evidenceをfixtureにし、
C2 passをpartial条件へ加えた一方、violation→failed、C1未実行→static、C4不成立→
failedを固定した。偽成功ゼロは昇格側だけでなく降格側も契約字義へ較正して守る。

## F-2a-7 — CLI投影整合とLuna比較系列の第一清算（2026-08-02）

F-2a系列は壁6枚（parameter注入・endpoint境界・観測gap・方言被覆・endpoint
作法・投影gap）を推測でなく実測へ変えた末、モデル因子の単独観測に到達した。
Luna 006のC3はpass 2 / violation 1 / claims_absent 2であり、Gemma正式Window Bの
README捏造拒否6件との比較から、証言壁は階級で動くことを確認した。この完全基準線×
Luna全窓の対照表をF-1スコア設計の第一検算材料に指定する。

契約§4へ投影を戻したLuna 007は、C到達2/6のC3を2/2 passとし、
`filter_luna_001`でCLI史上初のfullを得た。Luna合算n=42はfull 1、C到達7、
観測費用USD 0.981943である。残余は裸・日本語placeholderの伝達床とcanonical
verifyの機械床へ分解され、full率をモデル単独の値と誤読しない材料も同時に残した。

## F-2a-8 — 機械床返済とモデル因子系列のsettlement（2026-08-02）

モデル因子の単独観測に到達した系列を8窓・n=48・総費用USD 1.200131
（計画時の「$1.3前後」に対する確定値）で清算する。F-2aで踏んだ壁7枚、すなわち
parameter注入、endpoint境界、観測gap、方言被覆、endpoint作法、投影gap、
C1字義例欠落は、いずれもfixture・typed構成・event/evidence・限定repair・
投影guard・manifest guidanceの恒久装備へ転化した。併せてcanonical verifyの必須
positional保持と、full/complete終端をfailure分類へ流さない`success`区分を床として
固定した。既存provider request/event bytesと非CLI経路のgoldenは変更していない。

Luna 008は床返済後もfull 1/6、C3 pass 2を再現し、007→008の2窓で同じ
full率とC3 pass数を得た。008のC到達は3/6、C3はpass 2 / violation 0 /
claims_absent 1、UNKNOWN 0。`stats_luna_003`のverifyは必須positional
`data/sample.csv`を保持し、脱落形を再発させなかったが、sampleにない列名
`amount`を正直に拒否した。したがって床返済は失敗を成功へ読み替えたのではなく、
機械交絡だけを除いて残余のモデル出力差を露出した。

| model / arm | window | protocol | n | C到達 | C3 pass / violation / absent | full | 費用 |
|---|---|---|---:|---:|---|---:|---:|
| Gemma | formal Window B (`elev-004`) | native | 6 | 2 | 0 / 2 run（6 claims）/ 0 | 0 | 未記録 |
| Gemma | assist v1.0 segment 1 (`pack-001`) | native | 6 | 0 | 0 / 0 / 0 | 0 | 未記録 |
| Gemma | assist v1.0 segment 2 (`pack-002`) | native | 6 | 2 | 0 / 2 / 0 | 0 | 未記録 |
| Gemma | assist v1.1 ceiling (`pack-003`) | native | 6 | 1 | 0 / 1 / 0 | 0 | 未記録 |
| Gemma | human directive round 1–2 | native | 2 | 0 | 0 / 0 / 0 | 0 | 未記録 |
| Luna | 001 | Chat/native | 6 | 0 | 0 / 0 / 0 | 0 | $0.000000 |
| Luna | 002 | Chat/native | 6 | 0 | 0 / 0 / 0 | 0 | $0.000000 |
| Luna | 003 | Chat/text | 6 | 0 | 0 / 0 / 0 | 0 | $0.038459 |
| Luna | 004 | Chat/text | 6 | 0 | 0 / 0 / 0 | 0 | $0.118284 |
| Luna | 005 | Chat/text | 6 | 0 | 0 / 0 / 0 | 0 | $0.202160 |
| Luna | 006 | Responses/native | 6 | 5 | 2 / 1 / 2 | 0 | $0.312987 |
| Luna | 007 | Responses/native | 6 | 2 | 2 / 0 / 0 | 1 | $0.310053 |
| Luna | 008 | Responses/native | 6 | 3 | 2 / 0 / 1 | 1 | $0.218188 |

窓区分は001〜002を**machine BLOCKED**、003〜005を**text bridge**、006〜008を
**Responses/native**とする。確定事実は、Gemmaのformal・pack・人間directiveという
全援助段でC3 pass 0だったのに対し、LunaはC3 pass累計6（要求下限4+）、
violation 1を観測し、さらに`filter_luna_001`で2026-08-02にCLI史上初のfullを
得たことである。これは証言壁がモデル階級で動く直接観測であり、Gemma×Lunaの
全窓対照表をF-1スコアとT2F設計の第一検算材料に指定する。到達率、C3条件付き分布、
full率は別軸のまま保持し、BLOCKED/text窓をnative能力分母へ混ぜない。

F-1b seal（2026-08-02）: score schema v0、3 parameter family、`fail / violation = -w/2`の誠実性フロア、timestampなしはfinal-only、profile × model階級相関の`n < 5`非表示、scripted directive最大3 round、band五数要約を固定し、全履歴read-only遡及走査の完走をruntime実装の先行条件とした。

## BoN-0 — run末選別の最小形と初計測（2026-08-03）

BoN-0——第4因子（試行数）の初の制度計測。選別はearnedのみ・予測ゼロ・
`ρ=0.063`によりBoN-2はゲート維持。単一filter goalの同一構成6runを別workspaceで
完走し、full 1/6の`filter_bon0_005`を採用した。binary・入力pin・model meta・pack
pinは6/6一致し、5敗者のevidenceは削除せず保持した。途中刈り込みと修復接続は0件。

F-C-1b seal（2026-08-03）: E-4第2段を、exact-domain/URLの閉契約、bounded child
単一fetch境界、robots fail-closed、同URL×UTC日cache、取得時刻基準のN6として
固定した。実装前見積りはcomparator/checkers 650〜1,050 production Rust行、
plumbing 700〜1,200行、合計1,350〜2,250行、残存機械床4〜8、初回live較正5〜10
campaignのまま封緘し、公開サイトの初計測は別指示まで行わない。

裁定者誤り系譜・事件1a（2026-08-03）: `p=0.17, N=6`の4窓で`>=1 full`期待を`3.4/4`とした依頼者算術を、実行前に`4 × (1 - 0.83^6) ≈ 2.69/4`へ訂正した。

裁定者誤り系譜・事件1b（2026-08-04）: 上の丸め値`2.69/4`は正しかったが、中間値を`2.688076...`と転記した裁定者算術も誤りだった。正確には`1 - 0.83^6 = 0.673059626631...`、4窓期待は`2.692238506524...`。実行済みpredeclarationはSHA証跡のため不変とし、別 correction recordで訂正した。

裁定者誤り系譜・事件2（2026-08-04）: `2/12`の点推定`p̂=16.7%`を既知の母比率として二項期待へ代入し、`3.5〜4.5`を予測帯と呼んだ設計が誤りだった。Wilson CIと基準率の分母を宣言せず、推定不確実性を予測へ伝播していない。Lunaは追い計測せず`4/42=9.52%`へ清算し、以後のBoN系列は分母・CI・Beta-binomial予測帯なしの宣言をlint拒否する。

## F-BoN-V settlement — 分散の価値と適用限界（2026-08-04）

4検証を追い計測なしでCLOSEし、statusを
`機構実証済み・統計検証で問題検出(n=4)`とした。Lunaはpool
`4/42=9.52%`（Wilson 95% CI 3.77–22.07%）、N=6の点代入増幅45.15%。pinned
4窓のfull分布`[1,0,0,0]`は事前分散比閾0.5に対して0.295でunderdispersedだった。
Gemma負対照は0/6（過去合算0/12、Wilson上限24.25%）で、成果物6/6が相異なっても
受理境界へ届かない固執型を観測した。local Breakoutは事前Beta-binomial 95%帯0..3に
対し1/6、tree 6/6 unique、合計2,719秒だった。

品質監査はBoN採用full 2対単発full 2の全4個体が同じ100点vector、C3 9/9一致、
tree 4/4 unique。n=4なので偏り不存在や同等性の統計主張は置かない。Luna 5窓の
選別勘定はOracle@6 2/5、Selector@6 2/5、条件付き回収2/2、Selection gap 0が5/5、
tree 30/30 unique（non-empty 29/30）。分散は資源だがfullの十分条件ではない。

収穫1（系列計器）: timestamp-bearing buildとCargo target別Mach-O UUID/signature差を
系列変数として露出した。決定的build、同一commit再build version一致、事前binary SHA、
支出前fail-closedを制度化し、`bon0-002/003`を理由付き除外、`bon0-001r`をtrial 0で
遮断した。収穫2（基準率）: 小分母の点pを既知母率として扱う設計を廃し、基準分母、
CI、Beta-binomial帯をschema必須にした。裁定者誤り系譜1a/1b/2は原宣言を改竄せず、
別証跡で保持する。

local full個体は一次`run_stop`、summary、campaign metadataで一致した一方、補助
acceptance sheetが`status=completed`をfullへ写像せず「未完了」と表示した。この差は
full本数を遡及変更せず、同sheetを独立selector入力にする前の修正要件として残す。

BoN-1裁定はsplit GO/NO-GO。earned-only run末選別、fixed N、全候補保存、
Oracle@N/Selector@N/Selection gap/Diversity公開はGO。17%固定値、普遍適用、途中刈り込み、
repair接続、予測ranking、品質母集団主張はNO-GO。有効Luna窓費用`$1.0267417`と、
計器事件で除外した実支出`$0.5281295`を別勘定にし、Gemma料金とlocal電力量は証跡が
ないため推定しない。詳細は`workspace/management/runs/f-bon-v-001/settlement-report.md`と
`evidence/settlement.json`を正本とする。

F-MAP-0（2026-08-04）: `band_aggregate.py --profile score-time-map`を稼働し、モデル×ユースケース族×構成を構成総所要・正式run全数平均到達スコアへ投影する正準Markdown/SVG（n<3非描画、final-only/checkpoint分離、成功1件あたり期待時間・費用併記）を新規生成物として確立した。

GUI 言語・情報設計（Issue 76、2026-08-16）: 運用 GUI の文言は日本語固定とし、
i18n 基盤や言語切替は導入しない。ページ冒頭はページ名と説明1行に圧縮し、モバイル
ではページ名1行だけを表示する。ページごとに固有のタブタイトルを持たせ、Assets は
主ナビから外して概要から参照する。装飾的な常時緑表示は廃止し、Trial 利用可否と
workspace lease の `idle` / `running` / `recovery_required` を読み取り専用 API から
投影する。内部イベント名、既存 API 識別子、profile/provider/status の保存値、`.anvil/`
状態は変更しない。

## P2F-0 — 第4因子の第2通貨の初計測（2026-08-05）

failed在庫44本のcensusから失敗クラス×開始スコア帯を固定seedで層別抽出した
10本へ、既存の保存済みrecovery UltraPlanをcopy上で各1周だけ適用した。P2F@1は
1/10=10.0%（Wilson 95% CI 1.79–40.42%）で、事前Beta-binomial full本数95%帯
0..9の内側。fix単独は8,971.866秒・$0.4039273、原failed runと合算したfull 1件の
観測調達単価は21,153.866秒・$0.7894095だった。現有比較ではBoN new Nが
19,967.500秒・$0.6508036/full、単発参考が18,391.500秒・$0.6000657/full。
無作為同時比較ではないため優劣の統計主張は置かない。

唯一のfullは開始スコア未到達帯から生じ、数値比較可能6本は改善0・横ばい1・悪化5。
開始スコアの単調な成功予測は観測せず、BoN-3 score gateは解除しない。原workspace
bytesは10/10不変、copy productは10/10変化、directive・追い周回・新規配線は0。
F-BoN-Vの自動BoN修復接続NO-GOを維持し、P2F-1人間指示版とは分離した。正本は
`workspace/management/runs/p2f-0/report.md`と`settlement.json`。
G-0 red-on-red違反（2026-08-15事後記帳）: `49feed64`のCI run `31175327521`がfailureのまま、親CI conclusionを確認せずG-0 `0b4dfad7`以降をpushした。後続の両greenは当時の違反を遡及的に正当化しない。
裁量コミット原則（再確定）: 開発主体の裁量で作るコミット列は、push前preflightでremote親SHAの`CI` conclusion=successを記録する。failure/pending/cancelled/unavailableなら停止・原文報告し、明示的な所有者指示なしにredの上へ積んでpushしない。

GUI-304（2026-08-16）: Trial status GETはconfirmation hashとevents metadataから
weak ETagを導出し、`If-None-Match`一致時はevents全読込・JSONL parse・terminal
acceptance sheet再生成より前に304を返す。clientは変化時1秒へ戻り、連続304時だけ
最大10秒まで指数backoffする。`PolledSession` schemaは不変。Next exportの
`_next/static/**`だけを1年immutableとし、`index.html`を含む他pathは`no-store`を
維持する。10分virtual-clock smokeは旧750 ms固定の801 callに対し65 call以下を
受理境界とする。
CM-0——Builder Plane成立性のGo判定材料。新プロダクト検証Phase 0対応・headless機械可読出力の装備。
CM-1b——Community Mini App profile契約をfixed化し、S/Z/B検証器と既知敵対的スイートを封緘して10/10検知を実測する。CM-1 Phase 1対応。

CM-4清算（2026-08-18）: Phase 0 headless、Phase 1既知敵対10/10、Phase 2
golden 34/36 full（Wilson 81.9–98.5%、p50 174.5秒、max $0.00252714）、
Phase 3モデル階級分離、Phase 4明示think・4並行隔離・R2納品再検証を一続きの
Builder Plane実証として清算した。qwen3.8 E mediumは8/12 full・p50 59秒、
F highも8/12・p50 148.5秒で、採用はowner裁定待ちのためqwen3.6/think未指定の
封緘運用既定を変えない。golden/schema/adversarial外側SHAは
`4ea74f2f…dad86` / `6242f354…72c1` / `792c9696…2b0b`で不変。意思決定窓の
provider費用は$0.49151232、数値正本がある除外窓込み既知下限は$0.54277734。
null窓とlocal電力は推計しない。QUEUEDはL2 verify起動形の一意化とglobal集約
（schema v0.2候補）の2件。

CM-4x清算v2（2026-08-18）: 初回4並行のschema版違反3件は供給schemaではなく、
campaign外側と計画内nested verifyが異なるbinary世代を使ったmachine/stale帰属と
三点対照で確定した。子process PATHとbinary SHAを一致させ、同一封緘suiteの再実行で
final acceptance 4/4、cross-contamination 0、effective speedup 3.297倍を実証。
core manifest path 1件は正しい供給に対するmodel署名として保持する。qwen3.8 medium
Eは同一計器24本を追加して24/36 full（Wilson 50.33–79.79%）、p50 61.5秒、
A差−16.67pp（Newcombe −36.92〜+14.38pp）。採用はowner裁定待ちで既定を変えない。
封緘3層は不変。CM意思決定窓は$0.54765442、pricing解決可能な既知下限は
$0.59891944。未返済QUEUEDはL2 verify起動形とglobal集約/schema v0.2の2件。

## E-01 — 未署名ローカル pack 供給契約 v0.1（2026-08-19）

P-0bのinventory→draft→review→seal手順を踏襲し、pack制度をrepository
`packs/`だけから、明示`--extension-root`配下の未署名operator-local供給まで
加法改訂した。Rust/APIの供給型は`PackSource`、保存値は
`admitted | repository | local`に固定した。localはTrial token＋Origin境界から
stage/verify/pin/retireできるが、未承認・帯域未計測・pack固有保証なしであり、
extension-root→repositoryの解決順とshadow警告を必須表示とした。retireは削除で
なく`RETIRED` markerによる新規選択不可で、bytes・pin・履歴を残す。

契約v0.1は`materials/*.md`を正式hash memberへ追加した。直下UTF-8 Markdown
だけ、1件65,536 bytes、合計262,144 bytes、symlink不可とし、relative path bytes
昇順で既存のpath/content長接頭辞列へ連結する。domain separatorは
`commandagent-pack-v0\0`を維持するため、materials無しの既存pack hashは不変。
E-17が実装する予約source名は`pack_material_document`、paramsは`file`と
`max_bytes`（既定16,384、上限65,536）に固定し、規約Markdownを命令でなく
credential-scrub済みuntrusted observationとして区切って描画する。

extension journalは`JournalEntry`と
`planner::pack::supply::journal::append`をAPI名とし、RFC3339 `ts`、
`gui|cli` actor、`stage|verify|pin|retire` action、exact pack identity、
`ok|error` result、scrub済み`detail`のappend-only JSONLへ固定した。local pinの
conformanceはstrict/identity/closed vocabulary/compatibility/floor/path・bound・
scrubを必須とし、実測fixtureとgoldenはadmitted昇格条件のまま維持する。

D-3c §4はpinned local/repositoryの明示選択と日本語の未承認表示を取り込み、
単に存在するYAMLを列挙しない原則を維持した。Phase Gへ残すのは署名、publisher
identity、trust root、revocation、remote transportであり、今回のoperator-local
供給を署名代替またはadmissionと扱わない。

## E-02 — 承認 profile への追加専用 overlay（Issue #105、2026-08-19）

E-17（Issue #116、`ef0703f6`）では、packだけでNext.jsの社内規約材料と登録済み
追加check 3種を、既存floorを弱めずに結合できることを確認した。通常の規約上乗せは
引き続きpackを使う。その範囲を越えてartifact cardinality、guidance、profile-bound
check、evidence targetをprofile契約として加える場合に限り、承認済みembedded profile
へ別`manifest.toml`を1個だけ上乗せするスロットをGOとする。

overlayは`metadata.status = "draft"`、`overlay.mode = "additive"`を必須とし、実効
profileはbaseの承認を継承しない。識別は
`(metadata.id, base_profile, ManifestSource, exact_byte_hash)`、sourceは
`repository | local`、merge順は`base -> overlay -> pack`と固定する。plan、
step_templates、vocabulary、base既存名との衝突、置換・除去・移設・弱化、alias base、
非承認base、overlay連鎖、複数overlayを拒否する。全check成功時も
`profile_not_admitted`によりassurance上限は`static`。GUI/CLIは
`<display_name>（下書き上乗せ）`とbase、source、hash、draft/上限を表示し、packは別表示
する。完全なTOML断片と拒否条件は`docs/dev/profile-manifest.md`を正本とし、実装は
E-18（Issue #117）へ委譲する。

## E-23 — 読者別ドキュメントとアプリ内ヘルプ対応（Issue #122、2026-08-20）

M2のCLI pack、GUI Trial／拡張／セットアップ実装を取り込んだ後、初見者、利用者、
拡張者、運用者、開発者の所有ページへ文書を分割した。旧`docs/user/gui.md`は既存
H2/H3 anchorを保持する互換索引とし、移設先を一意に指す。READMEのEN/JA
QuickstartはいずれもCLI、GUI、拡張の3レイヤへ直接到達する。

GUIの説明文、用語ヘルプ、空状態、actionは`docs/user/gui-help-map.md`で所有節と
1対1対応させ、rootとproxy base pathのbrowser smokeで実在文言を確認する。公開CLI
flag、設定key、slash command、`docs/guide/{en,ja}`のfile/H2/H3 parityは既存
`tests/doc_drift.rs`を正本として維持し、verification／acceptance／evidence境界と
`.anvil/` runtime schemaは変更しない。

## E-24 — G/BP1第三者1セル実測（Issue #123、2026-08-20）

E-3/E-4およびE-17/E-18の実装に参加していないIssue workerが、E-18の外部draft
`landing-page`を1セル追加した。セルはv1 `manifest.toml` 1件、既存の
`scaffold_files_present` check、`index.html` 1件で成立し、strict loadとfinal
verificationがgreen、manifest hashは
`sha256:ebe5c468d9ed2c030d53109a8891dd3351680cb6519758e7a7dff35c80c2ccb7`、
assurance上限は既定どおり`static`だった。catalog追加、overlay、production code変更、
provider呼び出しは0、外部課金はUSD 0.00である。

campaign全体は13ファイルで、内訳はセル1、実測fixture/test 2、指定scaffold出力4、
台帳・Issue報告6。`scaffold.py profile landing-page`自体は0.19秒だったが、生成物は
旧`[manifest]`形式かつ`scaffolds/profile/`配置で、E-18の
`profiles/<id>/manifest.toml` v1へ直接投入できなかった。したがってcatalogは既存の
再利用可能なtyped capabilityで表現不能な意味だけに増やし、overlayはartifact
cardinality、guidance、profile-bound check、evidence targetの追加専用範囲を広げない。
先にscaffoldを外部draft v1へ追従させる。工数・読んだ資料・全コマンドと初回doctorの
環境失敗は
[`20260820-bp1-one-cell/report.md`](../../workspace/management/runs/20260820-bp1-one-cell/report.md)
を正本とする。
