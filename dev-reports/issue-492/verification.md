# Issue #492 verification

- Status: `passed`

All required controls passed, including the fresh real-07 Next.js controls on
clean committed source `577c412d9ba0372e78915cbcf194fac350874382`.
The following commit changes only this verification report. Production source,
tests and fixtures therefore match that verified implementation commit.

Baseline: `d51cc6ed9e04f8d9747d015ff788845ea1b1e59b`.
All runtime output is below
`/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/runtime/`.

Every Cargo/test process uses:

```text
CARGO_TARGET_DIR=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/target
CARGO_HOME=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/cache/cargo
TMPDIR=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/tmp
npm_config_cache=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/cache/npm
```

The real controls also use separate apps and runtime directories, unset NODE_ENV
and NODE_OPTIONS, retain devDependencies, and restore the selected SSD TMPDIR in
the observation wrapper's real npm child. No HOME or production allowlist change
is needed.

## Checks

Commands ran in the dedicated issue worktree with the exports above. Offline
Cargo flags use the specified cache; npm installation is an explicit preparation
step outside Verify. No command changes production policy.

- `cargo test --offline --lib issue492 -- --nocapture`: `passed`
- `cargo test --offline --lib recovery_step_plan_binding -- --nocapture`: `passed`
- `cargo test --offline --lib issue490 -- --nocapture`: `passed`
- `cargo test --offline --lib issue488 -- --nocapture`: `passed`
- `cargo test --offline --lib setup_step_policy -- --nocapture`: `passed`
- `cargo test --offline --test corpus_regression --test generality_guardrails --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --offline --all-targets -- -D warnings`: `passed`
- `cargo test --offline --lib child_with_no_response_times_out_and_is_reaped -- --nocapture`: `passed`
- `RUST_TEST_THREADS=4 cargo test`: `passed`
- `node --check tests/corpus/apps/issue492-build-duty/prepare-controls.cjs`: `passed`
- `node --check tests/corpus/apps/issue492-build-duty/npm-observer.cjs`: `passed`
- `node tests/corpus/apps/issue492-build-duty/prepare-controls.cjs /Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/apps/real-07 /Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/runtime/real-07`: `passed`
- `cargo test --offline --lib issue492 --no-run`: `passed`
- `cargo build --offline --bin commandagent`: `passed`
- `/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/target/debug/commandagent --version`: `passed`
- `cargo test --offline --lib issue492_real_nextjs_good_broken_missing_and_profile_controls -- --ignored --nocapture --test-threads=1`: `passed`
- `git diff --check`: `passed`

Focused results: 7 passed, 1 opt-in ignored (explicitly executed below). Related
binding: 57 passed; #490: 10 passed / 1 opt-in ignored; #488: 12 passed; setup:
22 passed. Corpus/guardrails/protection audit: 7/10/2 passed. Full suite:
**3,000 passed, 0 failed, 43 ignored** across 81 test result groups, including
doc-tests. The Issue #492 opt-in test separately passed all five cases in 33.31s.
Logs: `focused.log`, `related-binding.log`, `related-490.log`, `related-488.log`,
`related-setup.log`, `corpus-guardrails.log`, `fmt-check.log`, `clippy.log`,
`port-control.log`, `full-test.log`, `prepare-real-07.log`,
`committed-test-build.log`, `committed-cli-build.log`, and `real-07/test.log`.

For portable planner evidence, `ISSUE492_EVIDENCE_DIR` was
`/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/runtime/planner`.
For the opt-in run, `ISSUE492_APPS_DIR` was
`/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/apps/real-07`,
`ISSUE492_LOG_ROOT` was the corresponding `runtime/real-07`, and its `bin`
directory was prepended to PATH. NODE_ENV, NODE_OPTIONS and ISSUE492_CASE were
unset. Preparation resolved real npm to
`/opt/homebrew/lib/node_modules/npm/bin/npm-cli.js` and ran
`npm ci --include=dev --no-audit --no-fund`; its output is `real-07/npm-ci.log`.

## AC1–AC2: original duty and ordinary registration

The original build instruction, kind, pass expectation, empty expected_paths and
ordered mixed verify array survive verbatim. The array is explicitly copied
from the historical sanitized snapshot:

```text
npm run build
node -p "String(require('./package.json').scripts.build)=='next build' ? true : process.exit(1)"
```

P1 uses the entire ordinary parse → host augmentation → sanitize → preset
conversion → Admission → save/reload → before_phase → registration path. It
coexists with acquired package readers, the application owner and the trailing
build. `planner/P1.json` records all fields and order at each stage. The normal
sanitizer does **not** add this guard to a bare build in this plan: it is present
in both raw proposals and unchanged at every observed boundary. Host
augmentation updates the application instruction and normalizes duplicate
package-reader paths to empty; sanitation leaves those values unchanged; pure
package conversion restores reader input paths. The trailing mixed build stays
unchanged throughout. Returned, saved/reloaded and registered plans match.

The build retains empty expected_paths, so this change adds no output owner,
required input path or package-only precheck eligibility to it. Its profile
short-circuit predicate is false. Actual runtime controls below confirm build
execution rather than assuming plan preservation proves execution.

P1's live pre/post contracts are compared independently from StepPlan equality.
Normal registration adds exactly `verify-ui.cjs`, `README.md` and
`node verify-ui.cjs`, with the `verifier-owner` Implement/pass producer. Existing
artifact-only checks and exact profile checks are filtered under unchanged
rules. Preexisting paths and checks remain. The live and saved/reloaded contract
match. Build was already in the run contract and is **not** counted as proof of
the core duty. A second registration records the distinct no-write branch:
unchanged live/disk contracts, producers and persistence-event count.

## AC3–AC4: real execution and independent failure controls

Each case enters the actual profile preset's core phase via ordinary prepare
and phase execution on an explicitly provisioned workspace. No prior setup
success or final-application acceptance is fabricated. All five plans carry the
same original mixed build array, with captured parsed/augmented/sanitized/
converted stages, saved plan and registered plan. Fixed operations issue the
real Write tool for implementation and prevent subsequent source repair.

| Case | Profile checks | Target build observation | Outcome |
| --- | --- | --- | --- |
| R1 | pass | once, after implement-page, exit 0, 7159ms | core build passed |
| R2 | pass | once, after implement-page, exit 1, 4840ms | implementation_compile_error, source unchanged |
| R3 | pass | zero; before_attempted=false | dependency_missing / dependency_setup_blocked_offline |
| R4-port | dev-port predicate exit 1 | zero | profile failure; no configuration repair |
| R4-build | exact build-script predicate exit 1 | zero | profile failure; no configuration repair |

R1/R2 record `phase_id=core-implementation`, `step_id=verify-build`, step index 3,
completed steps `[package-read-original, implement-page]`, and real
`npm run build`. Neither emits a build `step_short_circuited` event. The observer
delegates unchanged arguments to real npm; package predicates alone cannot
supply these compiler results. R2 reports `src/app/page.tsx:2:9`,
`Type 'string' is not assignable to type 'number'.` The normal repair path is
entered after this initial failure, but the fixed client refuses further
operations: no repair edits or second build occur. This is a compile verdict,
not a dependency/environment failure. R1 means passing the core build only.

R3 has no node_modules, remains without it, invokes no npm command, and does
not use a foreign Next installation that exists on inherited PATH. Its existing
dependency diagnostic includes that rejected foreign toolchain; no external
workspace is executed or modified. Setup is disabled by the standard offline
configuration. R4 uses the standard `verify_step` report to record the exact
failed predicate, then the ordinary executor's unsuccessful package precheck
reaches the fixed client's stop boundary. No profile check is short-circuited
as success and no automatic configuration repair masks the defect.

Each case uses fresh state and .next output under `apps/real-07/<case>`; the
seed was never built. Node **v24.1.0**, npm **11.3.0**, Next **14.2.35**, React
**18.3.1**, TypeScript **5.9.3**; direct types are node **20.19.43**, react
**18.3.31**, react-dom **18.3.7**. Dev dependencies are present except in R3.
NODE_ENV and NODE_OPTIONS are unset, type checking remains enabled, and
next.config uses one CPU with no error suppression or external assets.

Frozen identities (SHA-256):

| Input | Hash |
| --- | --- |
| R1 page, before build and after run | `84fefa504f4f6ea295f378bfa6b459e4ade1483743c049e3e9b1ca92f3a887fc` |
| R2 page, before build and after run | `bf179cf817da84ee1884cb7b8cd83d3dc80ffe96b4c68d6218e0503235f7b655` |
| package-lock.json | `19183c1586a7809405afc6f4bab879c2df33a91ef1d99555bbcd13d9b8f10d78` |
| package.json | `5e3fc95d30c260395cf5319ccd71bdb5e566562b738c03aef39c026efab6e438` |
| next.config.mjs | `0bdce753c4ddbe10b6dcdf4712ee8ed40a499f88a7416ae1e40811ac0d7c79b9` |
| tsconfig.json | `4e8e00ef567e6ad982116dfe42b2d131ecd640cea6eccdf7f0de225e9f71c2f9` |
| layout.tsx | `4fcfceb76bbc48663db9e88c8ad3b0fc0fc2be376316958c42531f7f5b4342db` |
| dependency file/symlink inventory | `d85113010006100aec4a60a2b2b64b2f9373289d68d16cf43b608dfc9ec61c1e` |

R1/R2 differ only by `const count: number = 1` versus `"broken"` in the page.
All other listed build inputs are byte-identical to the committed fixture at
the actual build boundary. Preparation validates identical dependency bytes
and symlink targets. `real-07/environment.json` records each initial identity;
each case's `observation.json`, `events.jsonl`, `commands.jsonl` (when npm ran)
and `npm-*.stdout/stderr` hold the actual stages, immediate hashes, command,
order, exit, compiler output and product lifecycle. The observer reads absolute
real npm/log/TMPDIR paths from its generated __dirname sidecar; production
environment filtering is unchanged.

## AC5–AC6: acquired duties and compatibility

Each of N1–N4 has separate state and the same normal acquisition prefix. The
first proposal acquires original model and host scopes at the existing mixed
producer formation retry; the second/third proposals split those owners
correctly and vary only the targeted original duty. All three Admission events
retain identical acquired scopes and obligation sources. The original extra
`test -f src/app/page.tsx` is acquired before N4 removes it. Events are in
`planner/N1.json` through `N4.json`.

| Case | Later change | Targeted Admission refusal |
| --- | --- | --- |
| N1 | remove npm run build | lost original check/expected result or complete scope and boundary: npm run build |
| N2 | move build before implementation | moved original check before its output owners: npm run build |
| N3 | change expected result to fail | dropped original instruction/expected result for verify-build |
| N4 | remove other original artifact check | lost original check/expected result or complete scope and boundary: test -f src/app/page.tsx |

Each exhausts exactly three proposals and registers nothing; no unrelated
failure is credited. Existing #478/#488 tests cover last-valid/fallback
revalidation and atomic registration. No retry expansion or command-equivalence
change was made.

The 22 setup controls cover pure package/port setup, non-template paths,
application/game implementations, preset=false, final-verification phase and
data-profile behavior. Added mixed-command controls preserve unknown commands
and all metadata across setup/implement/verify kinds while the normalizer still
rejects npm install. Exact passing profile predicates remain convertible;
different-port and non-pass duties remain explicit. Existing additive profile
implementation checks retain their prior behavior. Related #488 checks retain
strengthening/freezing/final judgment, #490 retains reader ownership and runtime
checking, and #478 retains host duties and fallback refusal.

## AC7: red/green provenance and binary identity

The new B0 preservation assertion failed on unchanged production source (exit
101, `runtime/pre-fix.log`), demonstrating the original instruction/check/path
replacement. Its current expectation preserves the same sanitized input. The
historical JSON SHA-256 remains
`e15f1024da83e208592cb9375fd76fad78a6b10041e4320b6f1d456960a6891e`.

The exact red command was
`cargo test --offline --lib issue492_original_build_duty_survives_preset_conversion -- --nocapture`.
The same input and retention expectation pass in the focused suite; the old
loss snapshot remains historical data at the baseline SHA. New corpus fixtures
are portable and never read the former worktree or workspace/tmp at runtime.

The real-07 run used the test executable below, built from clean implementation
commit `577c412d9ba0372e78915cbcf194fac350874382`. The CLI was independently built
and its --version verified; it is not substituted for the executable that ran
the core integration test. `committed-identity.json` records empty git status
and both identities. Every case observation also records the actual test binary.

| Binary | Version | SHA-256 |
| --- | --- | --- |
| target/debug/deps/commandagent-a0a62cbae623c387 | `0.1.0 577c412d 2026-09-18T01:39:40+09:00` | `e5634bc6adf42310237f10b783ab8580ee5fd5917186946a146dd8f26b23e3a0` |
| target/debug/commandagent | `commandagent 0.1.0 577c412d 2026-09-18T01:39:40+09:00` | `a132a5fcc7f756ce3f5d95644035a317b7188271e4efab74f2d42509e99c1f05` |

These paths are under the dispatched SSD base, not the repository. The final
report commit does not rebuild or alter either verified binary. No PR, push,
Issue mutation, release or whole-business-app completion is part of this result.

## Earlier attempts (not final evidence)

Development attempts remain separated: real-01 stopped at dependency-copy
identity validation; real-02 stopped at Write authorization; real-03 failed in
the observer because custom child variables were filtered. None is an R1/R2
compiler verdict. real-04 observed real builds but used a non-preset enclosing
plan and is not conversion-path proof. real-05 used the actual preset/core
prepare/executor and passed every case. real-06's stricter input-hash assertion
detected package.json key-order normalization; values were identical. The final
fixture uses the product's canonical JSON ordering so byte hashes stay fixed.
Initial full-suite test audit failures were test harness issues (profile literals
and direct command validation); they were corrected without changing guardrail
baselines or command policy. `full-test-initial.log` retains that failed audit.
The next whole-suite run encountered the existing browser-probe free-port race
(`port_in_use` instead of the expected child timeout); see
`full-test-port-race.log`. The unchanged test passed in isolation, and the full
suite then passed with RUST_TEST_THREADS=4. No timeout assertion or production
behavior was weakened. Only real-07 and the final successful required checks
support the passed status above.
