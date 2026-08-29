# Issue 399 A14-A2 implementation and verification

- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Product execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Amendment: `v4-A14-A2`
- Status: implementation and exact-SHA CI verified; A14-A2 smoke completed NO-GO
- Inference boundary: the smoke is instrument diagnostics only. It must not be
  reported as evidence that Recovery improves population success.

## Implemented corrections

1. Next.js entrypoint knowledge is owned once in
   `src/planner/profiles/nextjs/knowledge.toml`. Lint, product verification, and
   client-component inspection now use the same 16 App Router / Pages Router
   TypeScript and JavaScript entrypoints. This removes the A13 regression where
   `src/app/page.js` was accepted by the profile but rejected by plan lint.
2. CLI completion contracts can register typed direct `argv`, expected exit
   code, and exact stdout observations. The registered observations replace
   implicit README, `--help`, unknown-option, and synthetic-test obligations
   for this explicit CLI contract; they do not weaken the observed behavior.
3. `python -m py_compile` and `compileall` remain syntax evidence but no longer
   satisfy `bound_verify_command` by themselves.
4. Completion-contract/profile mismatches are emitted as structured
   `profile_contract:*` failure kinds and remain outside automatic Recovery.
5. The opt-in A14 harness captures the actual workspace immediately before the
   first automatic Recovery Plan, excluding runtime caches, dependencies,
   `.anvil`, `.commandagent`, credentials, and environment files. The event
   records a cross-language content SHA-256 that the harness recomputes before
   attaching host-owned oracle capabilities.
6. A14-A2 uses one product invocation: the captured pre-Recovery snapshot is
   the control and the same run's final workspace is the treatment. No second
   stochastic initial attempt is called a paired control.
7. Fix tasks separate the frozen failing precondition from final success.
   Every cell-07 variant requires the same fixture and argv to fail before and
   exit zero after, plus a typed absence assertion preventing substitution of
   `fixture/control.json`.
8. Dependency-unavailable output is a third oracle state (`blocked`), not a
   product failure or pass. All cell-06 variants register the same bounded
   blocker vocabulary and are pre-registered outside Recovery when their
   dependency is unavailable.
9. The report requires shared history, boundary hash equality, a recorded
   handoff, valid oracle semantics, fix polarity, changed-path accounting,
   internal/external outcome matrices, and zero attribution from initial
   success. Incremental Recovery provider wall time and tokens are split at the
   recorded boundary.
10. Corpus, task contracts, external adapters, workspace registries, and the
    resource budget are SHA-256-bound in the A14-A2 contract and rechecked
    before execution. Campaign manifests also retain the binary, runner-source,
    exact-SHA CI, and frozen-input hashes.

## Local verification

- `cargo fmt --all -- --check`: passed
- `cargo clippy --all-targets -- -D warnings`: passed
- `cargo test -- --test-threads=1`: passed (full suite)
- `cargo test --test generality_guardrails`: passed without changing a baseline
- `cargo test --test corpus_regression`: passed (2/2)
- `PYTHONPATH=scripts python3 tests/eval/test_goal_verify_v3.py`: passed (31/31)
- `PYTHONPATH=scripts python3 tests/eval/test_goal_verify_main_v4.py`: passed (36/36)
- Ruff over all changed Python harness and test files: passed
- A14-A2 generated-file hash audit: 6 frozen inputs, 0 mismatches

### Next.js declarative-knowledge scenario matrix

Required by `docs/dev/dev-guardrails.md` for a change to
`src/planner/profiles/nextjs/knowledge.toml`:

- Space + Breakout:
  `cargo test space_and_breakout_contracts_keep_canvas_game_guidance --lib` — passed
- Quiz:
  `cargo test quiz_contract_uses_generic_interaction_guidance_only --lib` — passed
- Embedded knowledge parse/token guard:
  `cargo test embedded_matcher_knowledge_keeps_required_tokens --lib` — passed

No scenario vocabulary, capability contract, or expected guidance changed;
the declarative change only centralizes already-supported entrypoint variants.

## Freeze and smoke acceptance boundary

Before the four-case smoke, all of the following are required:

1. Commit the implementation without unrelated workflow or historical run
   files.
2. Require successful GitHub Actions `CI` for that exact implementation SHA and
   record `acceptance` when present.
3. Build a clean release binary whose `--version` embeds that exact SHA.
4. Regenerate the A14-A2 inputs as `status: frozen`, bind the exact-SHA CI
   evidence, and set only `smoke_collection_authorized: true`.
5. Run exactly the four pre-registered cases on
   `/Volumes/SSD_NX/tmp/commandagent_trial`: c01 initial-success sentinel, c04
   Next.js manifest regression, c06 dependency exclusion sentinel, and c07 fix
   polarity/harm sentinel.
6. Run the unchanged report script. A smoke GO means only that effect
   attribution is safe enough to design the later experiment; it does not
   authorize or replace the full paired collection.

Historical A14 and A14-A1 records remain immutable and are not rescored.

## Exact-SHA and smoke outcome

- Implementation SHA: `97fa75fa33167f0f5dcc9b7f85efa4de96e789a5`
- GitHub Actions `CI`: completed / success, run `33257977336`
- GitHub Actions `acceptance`: completed / success, run `33257977367`
- Clean release version: `commandagent 0.1.0 97fa75fa 2026-08-29T23:34:34+09:00`
- Four-case collection: completed 4/4 on the SSD execution root
- A14-A2 verdict: NO-GO because the report applied Recovery fix-polarity
  semantics to the preregistered c06 no-treatment exclusion

The completed A14-A2 run is immutable. The scope correction is preregistered
as A14-A3 with a new run ID; see `a14-a2-smoke-01-analysis.md`.
