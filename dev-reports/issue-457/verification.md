# Issue #457 verification

- Status: `passed`

Verified on 2026-09-09 in the dedicated #457 branch after fast-forwarding verified
predecessor `32f997f4e98523df3659fadb408994afbe14de53`. This status covers the
deterministic implementation and fixture checks below. Expected failures in negative
fixtures are required regression outcomes, not successful application acceptance.

## Checks

- `cargo test --lib issue457`: `passed`
- `cargo test --lib response_shape`: `passed`
- `cargo test --lib issue456_original_nextjs_contract_reaches_repair_without_inspection_build_or_write -- --nocapture`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test protection_coverage_audit`: `passed`
- `ISSUE457_TOOLCHAIN_ROOT=/private/tmp/issue457-matrix-06/original cargo test --lib issue457_installed_toolchain -- --ignored --nocapture`: `passed`
- `node --check scripts/issue457_nextjs_contracts.mjs`: `passed`
- `node --check scripts/issue457_nextjs_interaction.mjs`: `passed`
- `node scripts/issue457_nextjs_contracts.mjs --work-root /private/tmp/issue457-matrix-06 --output /private/tmp/issue457-matrix-06.json --playwright-module /private/tmp/claude-501/-Users-maenokota-share-work-github-kewton-MyCodeBranchDesk/05c21a81-8f2e-467b-ab0c-5497fc1e71e8/scratchpad/npmprobe/node_modules/playwright`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `git diff --exit-code 32f997f4 -- src/planner/auto_recovery/issue456_nextjs_tests.rs tests/protection_coverage_audit.rs docs/dev/dev-guardrails.md tests/generality_guardrails.rs tests/corpus/apps/issue456-create-recovery`: `passed`
- `git diff --check`: `passed`

The final full Rust run exited 0 through integration tests and doctests; the library
reported 2,480 passed and 17 ignored. The new installed-toolchain regression is one
of the explicitly ignored tests and was separately invoked above (1 passed).
It observed the actual Next failure: old extraction retained one frame, supplementary
checking retained 33 diagnostics across four files, and normal/compact repair prompts
included shared types/store imports and the registered functional check.

Development verification exposed an unintended intermediate invariant repair that
changed #456's inspected source, and the protection audit required an explicit
`NormalizedVerifyCommand` binding. Production wiring/type annotation were corrected;
the unchanged #456 hash assertion, protection audit and full suite then passed.
No audit exception or guardrail baseline was added. Release version output was
`commandagent 0.1.0 32f997f4+dirty 2026-09-09T20:59:37+09:00` (pre-commit worktree build).
Raw command logs remain in disposable `/private/tmp/issue457-*` locations, unstaged.

## Archived-source matrix

Environment: Node v24.1.0, TypeScript 5.9.3, Next.js 14.2.35, frozen archived
package lock. Every variant uses the same strict noEmit TypeScript program and
`npm run build`; all builds visibly reached Next's type-check phase. The shared
`experimental.cpus: 2` setting only bounds build workers. No type/lint gate was
disabled. Source and overlay hashes were checked against the retained
[structured results](fixture-results.json); originals remained unchanged.

| Fixture | Type diagnostics | Build exit | Actual feature result |
| --- | ---: | ---: | --- |
| original | 33 | 1 | Build failure; no GUI run |
| role-only | 32 | 1 | Build failure; no GUI run |
| missing-list | 0 | 0 | Fails: directory absent, no member selectable |
| wrong-field | 0 | 0 | Fails: UI assignment PATCH gets 400; old assignment survives reload |
| aligned | 0 | 0 | Passes list, selection, assignment, reload, reassignment and unassignment |

The aligned case also preserves status filtering and deletion. All three built
variants check six real error responses: project GET/POST, project-task GET/POST,
task PATCH and task DELETE against damaged fixture storage. They return HTTP 500
with a string `error`; project errors contain `Failed to read projects.json`, and
the page displays the message without `[object Object]`. Damaged and related data
files are not overwritten by requests. Internal `StoreError` remains structured;
routes map its message/details into the documented HTTP envelope.

Ten separate controls pass strict TypeScript and the Rust no-failure checks:
optional local directory defaults; ordinary/destructured/arrow helper parameters;
mixed direct, negated, typeof and computed body property reads; optional request
defaults. Optional guarded collection reads remain guidance only. Hard mismatch
inference requires proven exhaustive consumption and guarded successful returns.

## Browser reliability and evidence

Read the current develop-root `AGENTS.md` Browser reliability section at the
authorized read-only source root and the current-session skill:
`/Users/maenokota/.codex/plugins/cache/openai-bundled/browser/26.825.51511/skills/control-in-app-browser/SKILL.md`.
Verified that its `scripts/browser-client.mjs` and `scripts/browser-service.mjs`
entries exist. The already authorized standalone Playwright fallback was used;
this makes no claim that the Browser plugin was broken or repaired.

Verified the installed Playwright 1.58.2 module named in the matrix command and
its browser executable before reuse:
`/Users/maenokota/Library/Caches/ms-playwright/chromium-1208/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`.
Actual control used Chromium **145.0.7632.6**, headless, an isolated temporary profile,
viewport **1280 × 900**, locale **ja-JP**, timezone **Asia/Tokyo**. These conditions
were identical across the three GUI cases.

Each case launched its own Next process on an OS-assigned loopback port and ended
that process afterward. Targets were `http://127.0.0.1:50139/` (missing-list),
`http://127.0.0.1:50155/` (wrong-field), and `http://127.0.0.1:50167/` (aligned).
Before the feature sequence, each page showed readable `プロジェクト作成`, accepted
the harmless draft text `Preflight` without submission, and saved a screenshot.
This established actual navigation/read/input/capture separately from installation
and HTTP reachability. Existing browser profiles, shared services and historical
evidence were not modified.

Retained screenshots were visually inspected:

- [Preflight read and draft input](browser/aligned-preflight.png)
- [Missing directory control](browser/missing-list-result.png)
- [Rejected assignment with previous member after reload](browser/wrong-field-result.png)
- [Aligned reassignment to Ren after reload](browser/aligned-assigned-reloaded.png)
- [Readable actual store error](browser/aligned-store-error.png)

Full per-case settings/stages and source hashes are in `fixture-results.json`.
The fixture only retains the archive's existing JSON storage; it adds no mandatory
storage requirement to R0. Build success alone is neither final acceptance nor
candidate promotion. Live-model improvement remains an unmeasured #452 task.
