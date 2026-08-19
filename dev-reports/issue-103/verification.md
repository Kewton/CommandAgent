# Issue 103 verification

- Status: `passed`

## Checks

The product checks below were run from the Issue #123 aggregate worktree at
`5a149765`, which contains all four required predecessor commits. The final two
diff checks cover the aggregate predecessor range and this Issue #103 branch,
respectively.

- `git merge-base --is-ancestor 74794b25 5a149765 && git merge-base --is-ancestor efc2f2d2 5a149765 && git merge-base --is-ancestor 60ee8588 5a149765`: `passed`
- `npm ci --include=dev --offline`: `passed`
- `uv run --offline --with PyYAML cargo test --test cli_pack --test pack_actions --test issue117_extension_profiles --test issue123_bp1_one_cell --test conformance --test setup_script --test gui_read_only_guard --test doc_drift --test generality_guardrails --test protection_coverage_audit`: `passed`
- `uv run --offline --with PyYAML python3 workspace/management/scripts/pack_conformance.py --pack packs/nextjs-acme/1.0.0`: `passed`
- `node --check scripts/smoke.mjs && node --check scripts/session-index-smoke.mjs && bash -n ../scripts/setup.sh`: `passed`
- `npm run typecheck`: `passed`
- `npm run lint`: `passed`
- `npm run build`: `passed`
- `uv run --offline --with PyYAML cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `npm run smoke -- --wizard-only --output /private/tmp/commandagent-issue103-wizard-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `npm run smoke -- --read-only --output /private/tmp/commandagent-issue103-read-only-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `npm run smoke:session-index -- --output /private/tmp/commandagent-issue103-session-index-smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `uv run --offline --with PyYAML cargo test`: `passed`
- `uv run --offline --with PyYAML cargo test --features gui`: `passed`
- `git diff --check 81c22d18..5a149765`: `passed`
- `git diff --cached --check`: `passed`

## Evidence notes

- Next.js pack conformance reported `status: conformant`, exact-byte hash
  `sha256:6dab3671f1750a85830185486cf94f199b227cd4f3d4eccfe03a30742cee7ac0`,
  `effective_check_count: 3`, and `schema_count: 1`. The pack uses the single
  registered `pack_material_document` source kind.
- Wizard and read-only smoke reports both recorded `ok: true` for `/` and
  `/proxy/commandagent/`, with no unexpected console errors. The wizard used
  the expected 422 failure before repair and verified pin/handoff/retirement.
- Session-index smoke recorded `ok: true` for both base paths, maximum runtime
  request concurrency one, hidden-tab pause, visible-tab resume, stale-data
  retention, and base-path-safe navigation.
- The host Python did not include PyYAML, so the repository's cached PyYAML
  6.0.3 environment was used without network access. The first sandboxed
  typecheck could not write `tsconfig.tsbuildinfo`; the permission-enabled
  rerun passed. The first focused GUI-server attempt was made before `gui/out`
  existed and returned one homepage 404; after the required `npm run build`,
  the same complete 26-test command passed, and the later GUI full suite passed
  it again.
