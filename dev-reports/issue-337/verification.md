# Issue #337 verification

- Date: 2026-08-23 (JST)
- Issue: https://github.com/Kewton/CommandAgent/issues/337
- Base: `develop` at `ff35f8c08f524c55a30b9ba200aa0d8cdeb3c73b`
- Verification worktree: `/private/tmp/commandagent-issue337.qYP0UZ` (detached; removed after verification)

## Change

- Added `gui/scripts/run-evidence.mjs` with canonical-first `.commandagent/runs` lookup.
- Falls back to legacy `.anvil/runs` only when the candidate is absent (`ENOENT`). Other read errors remain failures.
- Wired the full GUI smoke event copy through the helper.
- Added four Node regression tests: canonical-only, legacy-only, canonical precedence, and both paths missing.
- Added the `smoke:run-evidence` package script.

No verification, acceptance, Gate, or honest-failure condition was weakened.

## Verification

| Command | Result |
| --- | --- |
| `npm run smoke:run-evidence` | PASS — 4 passed |
| `npm run lint` | PASS |
| `npm run typecheck` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --release` | PASS |
| `uv run --offline --with PyYAML==6.0.3 cargo test` | PASS |
| `npm run smoke -- --output <evidence> --commandagent-bin ../target/release/commandagent` | PASS — exit 0 / overall `ok: true` |

The first plain `cargo test` run reached 2123 passing tests and failed one existing Python reference test because Apple developer-tools Python did not provide the repository-pinned `yaml` module. Re-running the complete suite in the established offline PyYAML 6.0.3 environment passed. No dependency was installed into the repository.

## Full GUI smoke

Evidence: `workspace/management/runs/20260823-092547-issue337-verification/gui-smoke/`

| Case | Result | Elapsed | Primary session | Honest terminal result | Events |
| --- | --- | ---: | --- | --- | ---: |
| `/` | PASS | 182.009 s | `01a02c03-46c5-7233-8549-c1c1236b858e` | Gate 4 / failed | 111 |
| `/proxy/commandagent/` | PASS | 121.291 s | `01a02c05-9c3e-72e0-965f-f1565051e41a` | Gate 4 / failed | 106 |

Both cases copied canonical event evidence, completed reconnect/close/new-run lifecycle checks, and reached a second terminal session. Product Trials retained their existing honest Gate 4 results; the smoke harness correctly treated the observable lifecycle contract as passing. The scratch runtime was removed after success.

## Checksums

```text
6e50c000dd61ee3dc62cb81ceab6d7c9098121debae741049142483a35da6410  gui-smoke/browser-smoke.json
9c4ef4e121c1834e05dea80072a25ea77e37874cf562c42a32f8c8d7f6751474  gui/scripts/run-evidence.mjs
04dda54a1529c64060491cde4686b3fd5d2ca6a40cbf602c3bdd0dae9949c4d8  gui/scripts/run-evidence.test.mjs
```

## Delivery state

The fix was implemented and verified in the local `develop` worktree before delivery. The direct `develop` commit and push are recorded in repository history; Issue #337 remains OPEN until its post-push checks and lifecycle prerequisites are confirmed.
