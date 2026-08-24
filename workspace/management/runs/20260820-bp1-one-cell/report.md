# G/BP1 third-party one-cell measurement

## Result

- Status: measured and verified
- Issue: #123 / E-24
- Cell: external draft profile `landing-page` (`create`, task family `Quiz`)
- Manifest identity:
  `sha256:ebe5c468d9ed2c030d53109a8891dd3351680cb6519758e7a7dff35c80c2ccb7`
- Assurance ceiling: `static` (`profile_not_admitted`)
- Catalog additions: 0
- Overlays: 0
- CommandAgent provider calls: 0
- External provider/API charge: USD 0.00

The Issue 123 worker did not implement E-3/E-4, E-17, or E-18. It inspected the
already committed E-17/E-18 contracts and the required predecessor chain
#122 `74794b25` -> #115 `efc2f2d2` -> #119 `60ee8588` before adding this cell.

## Effort and measurement cost

The measured window started at `2026-08-20T02:43:16+0900` and final required
verification completed at `2026-08-20T02:58:26+0900`: **15 minutes 10 seconds
agent wall-clock**. This is not a human person-hour estimate. Codex session
billing is not exposed to this worker and is therefore not invented.

Captured command `real` times sum to 633.32 seconds. The sum includes commands
run in parallel and the 481.77-second sandbox attempt, so it is a command-cost
inventory rather than elapsed wall-clock:

| command / action | real time | result |
|---|---:|---|
| `python3 workspace/management/scripts/scaffold.py profile landing-page` | 0.19 s | generated four admission-scaffold files |
| `cargo run --quiet --bin commandagent -- --doctor --json --extension-root workspace/management/runs/20260820-bp1-one-cell/extension-root` | 28.25 s | manifest loaded; overall doctor failed on sandboxed Ollama/state probes |
| initial `cargo test --test issue123_bp1_one_cell` | 1.95 s | passed before predecessor fast-forward |
| final `cargo test --test issue123_bp1_one_cell` | 6.19 s | passed |
| `cargo test --test doc_drift` | 6.54 s | passed |
| root-cwd `python3 -m unittest workspace/management/scripts/test_scaffold.py` | 0.20 s | invocation error: `scaffold` was not on `sys.path` |
| scripts-cwd `python3 -m unittest test_scaffold.py` | 0.10 s | passed |
| initial `cargo fmt --all -- --check` | 1.01 s | found one new-test formatting delta |
| final `cargo fmt --all -- --check` | 0.95 s | passed after `cargo fmt --all` |
| `cargo clippy --all-targets -- -D warnings` | 18.42 s | passed |
| sandboxed `cargo test` | 481.77 s | socket tests hit `Operation not permitted`; stopped after no progress |
| permission-enabled `cargo test` | 86.75 s | passed without exclusions |

The failed doctor did not make an inference call. Its `profile.extensions`
check independently reported one `landing-page` draft with the hash above and
no manifest warning. The provider-free focused test then separated that
successful load from unrelated environment checks.

## Touched files

The final Issue diff touches **13 files**:
`git diff --cached --stat` reports `13 files changed, 434 insertions(+), 1 deletion(-)`.

| category | count | paths |
|---|---:|---|
| Executable cell | 1 | `extension-root/profiles/landing-page/manifest.toml` |
| Measurement fixture and focused proof | 2 | `workspace/index.html`; `tests/issue123_bp1_one_cell.rs` |
| Required scaffold start | 4 | `workspace/management/scaffolds/profile/landing-page/{ADMISSION.md,conformance.md,contract.md,manifest.toml}` |
| Ledger and Issue reports | 6 | `docs/dev/{integration-notes.md,mechanism-ledger.md}`; this report; `dev-reports/issue-123/{design.md,implementation-summary.md,verification.md}` |

The scaffold also created an empty `corpus/` directory; Git does not track it,
so it is not counted as a touched file. Required predecessor commits are base
history and are likewise excluded from the Issue 123 diff.

## Contract floor and outcome

The cell uses one existing catalog check, `scaffold_files_present`, bound to
the required `index.html`. Strict extension loading accepted exactly one v1
manifest with status `draft` and no warnings. The manifest-driven runtime
registered `landing-page`, retained the `static` assurance cap, and passed
`verify_profile_final` against the measured workspace. No pack, capability
band, overlay, production branch, event/schema change, or admission claim was
needed.

## Knowledge used

- `AGENTS.md`, `docs/dev/dev-guardrails.md`, and the `$codex-issue-worker` flow
- Issues #103, #116, #117, and the supplied Issue #123 dispatch
- `docs/dev/profile-manifest.md`, `docs/dev/extension-catalog.md`, `PROFILES.md`
- E-18's `static-site` manifest and `tests/issue117_extension_profiles.rs`
- generated `workspace/management/scaffolds/profile/landing-page/ADMISSION.md`
- required predecessor implementation/verification reports for #122, #115,
  and #119

## Friction

`scaffold.py profile landing-page` completed in 0.19 seconds, but its output is
the older admission scaffold: singular `scaffolds/profile/`, a `[manifest]`
table, and `admission = "off"`. E-18 instead loads plural
`profiles/<id>/manifest.toml` with the eight-section v1 schema. Consequently,
the generated checklist could be reviewed, but its manifest could not be used
or incrementally completed into the requested external cell without consulting
E-18 documentation and copying its manifest shape. The repository-root
`ADMISSION.md` named in the Issue does not exist; the generated per-scaffold
checklist is the available owner.

## Scope recommendation

1. Keep E-17's catalog thin. Add a typed source/check only when existing
   capabilities cannot express reusable semantics across profiles. This cell
   required zero catalog changes.
2. Keep E-18 overlays additive and limited to artifact cardinality, guidance,
   profile-bound checks, and evidence targets. Do not admit plan,
   `step_templates`, vocabulary replacement, or weakening. This cell needed a
   standalone draft and no overlay; ordinary organizational conventions remain
   pack territory.
3. Before broadening either scope, update `scaffold.py profile` to emit the
   E-18 external-draft v1 layout and a checklist appropriate to static-capped
   external supply.

This proposal was posted to parent Issue #103:
<https://github.com/Kewton/CommandAgent/issues/103#issuecomment-5346033527>.
