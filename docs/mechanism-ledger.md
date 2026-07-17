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
| Corpus closure | complete | 2026-07-07 | Golden local single-model GAME full-pass fixture in [local-single-qwen36-game-full-pass](../tests/corpus/apps/local-single-qwen36-game-full-pass/expectations.toml). |
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
`git show --stat` under `tests/corpus/apps/`. UAT report paths
are included only where the report exists under `workspace/management/runs/`.

| ID | mechanism | commit | motivation (discovery UAT) | corpus fixture | verification (result UAT) | state |
|---|---|---|---|---|---|---|
| T1 | Port-grep semantic rewrite | `923b623` | `test0710_bs_002` combo2-002 ([report](../workspace/management/runs/uat-test0710-bs-002/uat-report.md)) | `test0710_stage0_pivot` / `package-port-only-grep-normalization.jsonl` | Non-recurrence in `test0710_bs_003` ([report](../workspace/management/runs/uat-test0710-bs-003/uat-report.md)) | admitted |
| T2 | Import/JSX boundary static guard | `5991fba` | `test0710_bs_002` combo1-001/003 ([report](../workspace/management/runs/uat-test0710-bs-002/uat-report.md)) | `test0710_stage0_pivot` / `static-import-guards.jsonl` | Non-recurrence from `test0710_bs_003` onward ([report](../workspace/management/runs/uat-test0710-bs-003/uat-report.md)) | admitted |
| T3 | `write_required` rung | `8949be9` | `test0710_bs_002` read-only exhaustion group ([report](../workspace/management/runs/uat-test0710-bs-002/uat-report.md)) | `test0710_stage0_pivot` / `read-only-write-required.jsonl` | Fired in `test0710_bs_003`; wrong target exposed and handled by T4 ([report](../workspace/management/runs/uat-test0710-bs-003/uat-report.md)) | admitted |
| T4 | Evidence-to-target mapping | `19c1d10` | `test0710_bs_003` combo1 wrong `package.json` target ([report](../workspace/management/runs/uat-test0710-bs-003/uat-report.md)) | `test0710_stage0_pivot` / `read-only-write-required-evidence-targets.jsonl` | Removed in `test0710_bs_004`; related route later recurred and was unified by T20 ([report](../workspace/management/runs/uat-test0710-bs-004/uat-report.md)) | superseded_by_T20 |
| T5 | Hook snapshot and restoration rung | `f1af34e` | `test0710_bs_003` combo3 primary hook loss ([report](../workspace/management/runs/uat-test0710-bs-003/uat-report.md)) | `test0710_stage0_pivot` / `hook-snapshot-primary-restore.jsonl` | Class stopped dominating later runs | admitted |
| T6 | Deterministic step template | `3ae842b` | `test0710_bs_003` planner-time dominance ([report](../workspace/management/runs/uat-test0710-bs-003/uat-report.md)) | `test0710_stage0_pivot` / `deterministic-step-plan-used.jsonl` | `test0710_bs_004` observed event counts 2/2/1 ([report](../workspace/management/runs/uat-test0710-bs-004/uat-report.md)) | admitted |
| T7 | Deterministic restart repair (rung C) | `-` | `test0710_bs_004` ([report](../workspace/management/runs/uat-test0710-bs-004/uat-report.md)) | `-` | Not built by design; decomposed into T9/T16/T19 | not_built |
| T8 | Hook grep quote-independent normalization | `fb1689d` | `test0710_bs_004` combo3 quote false negative ([report](../workspace/management/runs/uat-test0710-bs-004/uat-report.md)) | `test0710_bs_004_command_normalization` / `combo3-hook-grep-normalized.jsonl` | Same pair full in `test0710_bs_005` ([report](../workspace/management/runs/uat-test0710-bs-005/uat-report.md)) | admitted |
| T9 | State-binding diagnosis | `4e6c4c8` | `test0710_bs_004` combo1-family interaction failures ([report](../workspace/management/runs/uat-test0710-bs-004/uat-report.md)) | `test0710_stage0_pivot` / `state-binding-diagnosis.jsonl` | `bs_005`/`bs_006` returned undeterminable, then T16 calibrated/prevented the class ([test0710_bs_005](../workspace/management/runs/uat-test0710-bs-005/uat-report.md), [test0710_bs_006](../workspace/management/runs/uat-test0710-bs-006/uat-report.md)) | superseded_by_T16 |
| T10 | Inspect-command normalization | `fb1689d` | `test0710_bs_004` combo2 timeout ([report](../workspace/management/runs/uat-test0710-bs-004/uat-report.md)) | `test0710_bs_004_command_normalization` / `combo2-inspect-command-normalized.jsonl` | Fired in later runs | admitted |
| T11 | Preset UltraPlan | `c268c53` | `test0710_bs_004` plan-isomorphism evidence ([report](../workspace/management/runs/uat-test0710-bs-004/uat-report.md)) | `test0710_stage0_pivot` / `preset-ultra-plan-used.jsonl` | `test0710_bs_006` fired 6/6; default later rolled back by T24 while opt-in remained ([report](../workspace/management/runs/uat-test0710-bs-006/uat-report.md)) | admitted_optin |
| T12 | Adversary vocabulary, translation, and probe-dimension scan | `0be0f74` | `test0710_bs_006` breakout combo1 arbitration false negative ([report](../workspace/management/runs/uat-test0710-bs-006/uat-report.md)) | `test0710_bs_006_breakout_combo1` | `test0711_bs_001` #7 full ([report](../workspace/management/runs/uat-test0711-bs-001/uat-report.md)) | admitted |
| T13 | Edit-anchor recovery ladder | `8aa3c2c` | `test0710_bs_005` combo2 anchor x8 ([report](../workspace/management/runs/uat-test0710-bs-005/uat-report.md)) | `test0710_bs_005_anchor_recovery_combo2` | Old terminal state disappeared in `test0711_bs_001`; T18 completed the interlock ([report](../workspace/management/runs/uat-test0711-bs-001/uat-report.md)) | admitted |
| T14 | Route-unbound wiring guidance | `f1bfb82` | `test0710_bs_005` combo1 ([report](../workspace/management/runs/uat-test0710-bs-005/uat-report.md)) | `test0710_bs_005_route_unbound_combo1` | `test0711_bs_002` #7 recovered in live run and reached full ([report](../workspace/management/runs/uat-test0711-bs-002/uat-report.md)) | admitted |
| T15 | Probe preflight | `49325f6` | `test0710_bs_006` space combo3 infrastructure failure ([report](../workspace/management/runs/uat-test0710-bs-006/uat-report.md)) | `test0710_bs_006_probe_preflight_space_combo3` | Recorded in every later run | admitted |
| T16 | Input-coupled diagnosis and contract wording | `764094b`, `926d3cd` | Calibration from `bs_005`/`bs_006` real artifacts ([test0710_bs_005](../workspace/management/runs/uat-test0710-bs-005/uat-report.md), [test0710_bs_006](../workspace/management/runs/uat-test0710-bs-006/uat-report.md)) | `test0710_bs_005_006_state_binding_input_coupled` | Contract wording acted preventively; class disappeared, while the diagnosis path remained unexercised | admitted_prevention |
| T17 | Tier-coupled preset default | `b08e92f` | `test0711_bs_001`/`test0711_bs_002` A/B ([test0711_bs_001](../workspace/management/runs/uat-test0711-bs-001/uat-report.md), [test0711_bs_002](../workspace/management/runs/uat-test0711-bs-002/uat-report.md)) | `test0711_bs_001_plan_preset_tier` | Did not fire in `test0711_bs_003`, fixed by T23, then `test0711_bs_004` measured distribution degradation and T24 rolled it back ([test0711_bs_003](../workspace/management/runs/uat-test0711-bs-003/uat-report.md), [test0711_bs_004](../workspace/management/runs/uat-test0711-bs-004/uat-report.md)) | rolled_back |
| T18 | Anchor x stagnation interlock | `8cba4ef` | `test0711_bs_002` #2 ([report](../workspace/management/runs/uat-test0711-bs-002/uat-report.md)) | `test0711_bs_007_anchor_stagnation_interlock` | `test0711_bs_003` showed carryover firing ([report](../workspace/management/runs/uat-test0711-bs-003/uat-report.md)) | admitted |
| T19 | Contract-attribute-missing guidance | `53a967d` | `test0711_bs_001` #6 ([report](../workspace/management/runs/uat-test0711-bs-001/uat-report.md)) | `test0711_bs_001_6_contract_attribute_missing` | Fired in `test0711_bs_003` #1 ([report](../workspace/management/runs/uat-test0711-bs-003/uat-report.md)) | admitted |
| T20/T20b | Target-resolution unification and escalation carryover | `f8388cf6`, `7e920fb0` | `test0711_bs_002` #1 `package.json` recurrence ([report](../workspace/management/runs/uat-test0711-bs-002/uat-report.md)) | `test0710_stage0_pivot` / `final-acceptance-target-resolution.jsonl`; `test0711_bs_008_escalation_carryover` | `test0711_bs_003` showed `page.tsx` target resolution and carryover telemetry ([report](../workspace/management/runs/uat-test0711-bs-003/uat-report.md)) | admitted |
| T21 | Compile diagnostic extraction | `99807bc` | `test0711_bs_002` #8 webpack-internal location ([report](../workspace/management/runs/uat-test0711-bs-002/uat-report.md)) | `test0711_bs_008_compile_diagnostic_extraction` | `test0711_bs_003`/`test0711_bs_004` recorded real source locations ([test0711_bs_003](../workspace/management/runs/uat-test0711-bs-003/uat-report.md), [test0711_bs_004](../workspace/management/runs/uat-test0711-bs-004/uat-report.md)) | admitted |
| T22 | Implementation-detail grep demotion | `1188842` | `test0711_bs_002` #3 `addEventListener` grep ([report](../workspace/management/runs/uat-test0711-bs-002/uat-report.md)) | `test0711_bs_002_source_detail_grep_advisory` | Not exercised later; zero demotions also confirmed the gate was not broadly relaxed | admitted_unexercised |
| T23 | Tier decision from resolved model | `fe690b1` | `test0711_bs_003` default non-firing ([report](../workspace/management/runs/uat-test0711-bs-003/uat-report.md)) | `test0711_bs_003_resolved_planner_tier` | `test0711_bs_004` fired 6/6 ([report](../workspace/management/runs/uat-test0711-bs-004/uat-report.md)) | admitted(対象機構はrolled_back) |
| T24 | Satisfied-setup short-circuit, preset-step conversion, and default-none rollback | `042880f` | `test0711_bs_004` #1/#7 ([report](../workspace/management/runs/uat-test0711-bs-004/uat-report.md)) | `test0711_bs_004_preset_setup_no_progress` | Verification UAT passed P0/P1 ([report](../workspace/management/runs/uat-test0711-bs-005/uat-report.md)) | admitted |

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
| T30 | assurance投影のprofile dispatch | 本コミット | data早期失敗が汎用full seedによりpartialへインフレした契約違反を、[investigation-01.md](../workspace/management/runs/uat-test0713-data-001/investigation-01.md) に基づき厳格化方向へ修正 | data 4-run判定表、E1/E3未達negative conformance、Next.js早期失敗互換 | **fixed** |

- 据え置きの根拠: 実害ゼロ（非ゲームは候補要素クリック経路が補完、バンド窓78runで本件起因の偽陰性/偽陽性ゼロ）に対し、修正には裁定層のタイミング再設計を要するため、コストが見合わない。
- 再訪条件: (a) 第4のシナリオ族でdispatch不足起因の偽陰性が実測されたとき、(b) 近縁profile（Vue等）がプローブ拡張を要求したとき。再訪時はタイミングモデル再設計（listener登録待ち・rAF同期・リトライ付きdispatch）を含む適正スコープで行う。

## Repository migration (2026-07-14)

- 移送元: `Kewton/Anvil@anvilminimal-migration-base`。filter前の
  `develop` HEADは`ec1519958c2210e3bcadcd19d7c23e51146a82ce`。
- 方式: `git filter-repo`で旧クレートsubtreeと`workspace/management`の
  2系統を`--path`選択し、旧クレートsubtreeをリポジトリrootへrenameした。
  抽出履歴はsquashせずCommandAgentの履歴へmergeした。
- 旧新SHA対応表: [`docs/migration/anvil-commit-map.txt`](migration/anvil-commit-map.txt)。

本文書内の移行以前のコミットハッシュは旧Anvil上のSHAであり、
`docs/migration/anvil-commit-map.txt`およびAnvilリポジトリの凍結タグ
`anvilminimal-migration-base`で解決する。

- Rename: anvilminimal → commandagent（crate/binary: `d05a410`、生きた参照: `835c04f`、本コミット）。以後のUATレポートのversion表記は commandagent。旧名は歴史的記録内で有効。機械出力はversion/CLI、banner、remediation/再現コマンド、truncation marker、probe User-Agent、およびevalの`engine_label`/`binary_kind`/`subject`/レポート見出しのみ新名へ更新し、イベント名・JSONキー・スキーマは不変。
- M-4移行ゲート（`test0714_m4_001`、2026-07-14）: G1/G2 **PASS**（新buildから6/6を各1回実行・正直終端・収集完了、data失敗5/5がassurance契約準拠、partialインフレ0）につきRepository migration完了を正式宣言し、Phase Bを再開。Evidence: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_001/uat-report.md` / `aggregate.json`。
- B-2d（DATA-7/8/9、2026-07-14）: verify lint拒否原文テレメトリ（`82fff89`）、`.anvil`一貫私有化＋有界フィードバック（`bdca35c`）、Python traceback抽出・決定的修復注入・`traceback_mapped`ターゲット解決（本コミット）を、固定data契約を変更せず導入。一次資料: [`investigation-b2d.md`](../workspace/management/runs/uat-test0714-m4-001/investigation-b2d.md)。
- DATA-10 / DATA-7段2 / FF-1b（2026-07-15）: dataチェックと正準成果物のフェーズスコープ化（`10d0143`）、`data_inspection_schema`（`4d9a4da`）、verify書き換え拡張＋runtimeテレメトリ（`0ba612f`）、contract instrumentation欠落ガイダンス配線（本コミット）を導入。一次資料: [`investigation-data10.md`](../workspace/management/runs/uat-test0714-m4-004/investigation-data10.md)。
- B-2f（2026-07-15）: inspectionの5キー字義例と実測値拘束をmanifest・修復ガイダンスへ追補（`bc9ec91`）し、data契約をassertする実測 `python/python3 -c` verifyを対応カタログチェックへ正準化（本コミット）。既存13件の正準化とNext.jsバイト列は維持。一次資料: [`uat-report.md`](../workspace/management/runs/uat-test0715-ff1-002/uat-report.md)。
- B-2g E2較正（2026-07-15）: 偽陽性49件（日付分割36＋照合域13）を除去。モデル起因の違反は0件だった（[`investigation-e2.md`](../workspace/management/runs/uat-test0715-ff1-002/investigation-e2.md)）。契約§6ネガティブ維持＋照合域の新設ネガティブで非緩和を担保。
- B-2h DATA-11／inspection行数照合／nearest_miss修復注入（2026-07-15）: 動的・正準最終フェーズから他フェーズ明示束縛チェックを除外してE1〜E4のみをfullゲート化（`2d42ae4`）、inspection報告行数を実CSV/TSV論理行数と照合（`4ddffcf`）、claims-bindingの違反claim・最近傍キー／値・差分をstep／最終受け入れ修復へ注入（本コミット）。根拠: [`uat-report.md`](../workspace/management/runs/uat-test0715-data-005/uat-report.md)。
- B-2i DATA-12（2026-07-15）: data stepを全expected_paths実在＋実verify全pass時だけモデルターン前に短絡し、動的phaseで正準化後に空となったverify stepへphase別の既定checkを束縛する。根拠: [`uat-test0715-data-006`](../workspace/management/runs/uat-test0715-data-006/uat-report.md)。
- B-2k DATA-7b（2026-07-16）: verify lintのシェル制御構文判定をshクォート対応とし、引用payload内の`; | & || &&`の偽陽性を除去。E2較正と同属の検証器精度修正であり、Next.js方向の影響は偽陽性減少のみ。既存のクォート外制御構文・ファイル書き込みredirectネガティブ維持で非緩和を担保。根拠: [`uat-test0716-data-008`](../workspace/management/runs/uat-test0716-data-008/uat-report.md) Run 6。

## Incident: semantic false-full (2026-07-14)

| ID | 機構 | コミット | 動機 | 検証 | 状態 |
|---|---|---|---|---|---|
| FF-1 | heuristic合格のfull資格剥奪 | `7dd98ad` | uat-test0714-m4-003 Run 6: 初の意味論的false-full。クイズのゴール（3問・スコア・リトライ）に対しシューティングゲームが生成され、契約フックゼロ（contract_hook_status=primary_missing、action_hooks=[]）のままheuristicプローブのmarker差分合格（element_count変化）によりfull_successを獲得した。根因: browser-interaction.json のprobe_mode / contract_hook_status がレポート整形専用で、release gateの判定に接続されていなかった（潜在欠陥、回帰ではない）。機械的偽装（evidence捏造・検証迂回・プローブ不実行のfull投影）はゼロ継続 | 回帰fixture tests/corpus/apps/test0714_m4_003_run6_false_full（full_eligible=false）、interaction_qualification 単体テスト、パリティ6本PASS（uat-test0715-ff1-001、full 3本すべて probe_mode=contract、heuristic-full 0） | fixed |

- 発見経緯: 機械はfullと判定したが、人間の監査（成果物の目視）がgoal種別との不一致を検出した。UAT報告者がG1 FAILと判定し台帳更新を保留した対応は、本プロジェクトの検収規律の実演である。
- 残存する境界（本修正の対象外）: 契約フックを正しく備えた「goal種別と異なる成果物」は依然fullを獲得しうる。goal種別と要求surfaceの契約束縛は未実装であり、検収可能性階層の実測された境界として記録する（対処は契約設計の将来課題。安易なgoal語彙マッチは偽陰性を量産するため急がない）。
- 過去バンドへの影響監査: 実施済み（[audit-report.md](../workspace/management/runs/ff1-band-audit/audit-report.md)参照）。ウィンドウ内full 31件はすべてcontract-mode（非空state dimensions）で、heuristic-only / unverifiableは0件。既存バンドは有効。

## Data profile first fulls (2026-07-15)

[`uat-test0715-data-007`](../workspace/management/runs/uat-test0715-data-007/uat-report.md) は、dataプロファイルで初めてのfullを同一固定コード上の2 runで記録した。

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

UAT #7ではqwen35 profileでinspection書き込み非追従が2件、gemma31 profileでは0件で完走となり、executor横断で発生率が異なるモデル分散クラスと確定した。機械側は字義JSON例、欠落キー列挙、`nearest_miss`まで導入済みであり、以後は[Spaceの4%分散](../workspace/management/runs/band_summary.md)と同じバンド特性として受容・文書化する。DATA-10の機械的なフェーズ束縛欠陥は根治済みであり、この残存クラスとは区別する。

### 集計注記

バンド集計は`final_acceptance_status`と`evidence/data-assurance.json`を正とする。B-2j以前の完走runでは、獲得済みfullがterminal projectionの`completion_contract_not_bound`によりpartialへデフレした値を含む。B-2j（`13b994f`）は`full_success`時にE1〜E4の実在evidenceからassuranceを再導出して投影し、evidence不在・不整合時の保守側投影と早期失敗のT30判定を維持する。歴史的イベントは改変しない。

- data × create バンド宣言（2026-07-15）: 機構安定後窓は2/6 full、全期間窓は2/38 full（観測48 run中、操作誤り・preflight未達の10 runを理由付きで分母外）。再計測は集計スクリプトのみとし、原表は[`band_summary_data.md`](../workspace/management/runs/band_summary_data.md)を参照する。
- B-3 admission gate（2026-07-16）: `draft` profileのassurance宣言を`static / profile_not_admitted`に上限制限し、dataを[バンド宣言](generality.md#measured-capability-bands-data--create)に基づく初の`admitted` profileへ昇格した。以後、新profileはdraft起点でfull宣言不可。

## Phase B settlement: data profile cost (2026-07-15)

data契約のfixedから初バンド宣言までを、次の再現可能な境界で集計した。
移行前SHA `2c982fc` は
[`docs/migration/anvil-commit-map.txt`](migration/anvil-commit-map.txt) 上の
現リポジトリSHA `4f57714`（2026-07-13 21:23:57 JST）に対応する。終点は
`68fdaf0`（2026-07-16 00:19:41 JST、7月15日キャンペーンのバンド宣言）
で、実時間は50時間55分44秒、キャンペーン日では3日である。移行mergeが
別履歴を含むため単純な `rev-list` は使わず、data契約・実装パス、上記の
B-2台帳行、調査/UAT記録、バンド生成物からfull SHAを集めて重複排除した。

### タスク、コミット、計測

| 集計項目 | 実測 | 集計規律 |
|---|---:|---|
| 台帳上のタスクID | 18 | B-0〜B-3の4 ID、B-2a〜B-2jの10 ID、下記4調査。B-2は配下a〜jのumbrellaなので、重複しない実行タスク数は17 |
| 一次資料調査 | 4 | [`investigation-01.md`](../workspace/management/runs/uat-test0713-data-001/investigation-01.md)、[`investigation-b2d.md`](../workspace/management/runs/uat-test0714-m4-001/investigation-b2d.md)、[`investigation-data10.md`](../workspace/management/runs/uat-test0714-m4-004/investigation-data10.md)、[`investigation-e2.md`](../workspace/management/runs/uat-test0715-ff1-002/investigation-e2.md) を `rg --files workspace/management/runs` で列挙 |
| fixed→初バンドのscoped commits | 38 | `4f57714`〜`68fdaf0`からdata/B系の契約・実装・調査・UAT・バンドcommitを対象パスと台帳で選び、full SHAで重複排除 |
| B-0〜B-3の全ライフサイクルcommit | 42 | 上記38に、fixed直前のB-1 schema/doc 2件（`bb510b7`、`62a3320`）と、バンド後のB-3 gate/admission 2件（`2c2d154`、`8930784`）を加算。B-4清算commitは含めない |
| 観測キャンペーン | 9 set | 正式data UAT #1〜#7の7 setに、無効計測 `uat-test0714-m4-002` / `m4-004` の2 setを加えた「7 set + α」 |
| 観測run | 48 | [`band_summary_data.md`](../workspace/management/runs/band_summary_data.md) の走査行数。正式分母38、操作上のmodel-ID誤り5とpreflight未達・未完了5の計10は理由付き分母外 |
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
| UAT | 2 set / 12 run | [`uat-test0716-data-008`](../workspace/management/runs/uat-test0716-data-008/uat-report.md) と [`uat-test0716-data-009`](../workspace/management/runs/uat-test0716-data-009/uat-report.md)、各6 run・再試行なし |
| 追加commit | 5 | `git rev-list --count fcb9ac8..207dd33`。UAT #8（`46d9e34`）、B-2k 2件、UAT #9（`c4d5727`）、族別band生成（`207dd33`）。本封緘docs commitは自己参照を避けて境界外 |
| 族別分布 | aggregation 2/38、timeseries 0/12 | Window Bはaggregation 2/6、timeseries 0/6。機械生成原表は[`band_summary_data.md`](../workspace/management/runs/band_summary_data.md) |

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
| intent軸集計・band宣言 | `e4aee4a` / `c9d5ae1` | intent列、fix 2族、F1〜F3 false-full abortを機械化し、[`band_summary_fix.md`](../workspace/management/runs/band_summary_fix.md)を生成。既存nextjs/data bandはbyte不変 |

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
