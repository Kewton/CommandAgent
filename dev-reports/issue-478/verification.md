# Issue #478 worker verification

- Status: `passed`

## Checks

- `cargo test issue478 --lib`: `passed`
- `cargo test issue466 --lib`: `passed`
- `cargo test issue465 --lib`: `passed`
- `cargo test setup_step_policy --lib`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `shasum -a 256 target/release/commandagent`: `passed`
- `git diff --check`: `passed`

## Results and evidence

The final full suite ran outside the filesystem/network sandbox and exited 0:
2,567 library tests passed, 19 were ignored; integration tests and doc-tests also
passed. Across the 71 reported suites, 2,919 passed and 42 were ignored. These
are the repository's default ignored tests, not new skips. In particular, the
#465 focused filter passed 23 and ignored its two installed-compiler/framework
tests; no success is claimed for ignored tests.

The #478 filter passed all six tests, including the eight corpus-driven model
refusal variants, five host refusal/fallback variants, Profile-marker provenance,
first-proposal truncation, and runtime shortcut/real package-check controls.
The #466 filter passed 18 (including #478); setup policy passed 18, guardrails
passed 10, and corpus regression passed 7. Guardrail baselines were not changed.

The saved smoke instruction remains exactly 587 characters and SHA256
`75c69f8375ad4c8492158c3f65745a6532a82f26850cab51901015e8faa492e0`.
The split passes in two requests within the existing three-attempt planner
budget. All three pre-fix package checks remain byte-identical on the executable
Profile owner. Its complete instruction contains 2,237 characters, including
the host's restart and port duties. The package checks actually pass for the
valid fixture manifest and fail after changing the dev port to 3000.

- [pre-fix-replay.json](pre-fix-replay.json): expected red-phase failure, actual
  Runner on `fdc98a9b`, exit 101; original mixed ownership and subsequent
  combined-instruction refusal are preserved as extracted scope evidence.
- [formation-replay.json](formation-replay.json): actual fixed formation scopes
  and boundary order. Regenerate with
  `ISSUE478_FORMATION_EVIDENCE=dev-reports/issue-478/formation-replay.json cargo test issue478_saved_split --lib`.
- [corpus](../../tests/corpus/apps/issue478-model-host-obligations/README.md):
  exact saved model proposals, host check commands and driven refusal cases.

The first sandbox full-suite attempt encountered local HTTP-dependent test
failures and was interrupted after starting the outside-sandbox rerun. An
intermediate outside-sandbox run caught guardrail placement/growth violations;
the code was moved into leaf modules without raising baselines, then focused
and full checks were rerun successfully. The final results above supersede
those development failures; raw execution logs are not committed.

## Worker release build

This pre-commit build verifies the final production source tree:

```text
commandagent 0.1.0 fdc98a9b+dirty 2026-09-13T03:44:56+09:00
SHA256 33e8a697aecd205c6094c49b71373faab5d9f5efd51d9bb811cf9952021c3d83
```

The `+dirty` marker is expected before this task's commit and is not presented
as a clean-commit or integrated release. The worker will rebuild the committed
HEAD and return that version/hash in its final handoff.

## Parent-owned pending gates

Codex2 review, exact-commit UAT and CI, publication/integration, integrated
release version/hash, and live R0 evaluation remain pending with the parent as
explicitly assigned. The worker-local `passed` status does not assert those
external gates passed. No live model probe, push, PR, Issue lifecycle mutation,
CommandMate operation or historical-evidence rewrite was performed.
