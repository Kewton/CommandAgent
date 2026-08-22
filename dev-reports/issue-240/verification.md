# Issue #240 Verification

- Status: `passed`

## Checks

- `cargo test model_probe::tests --lib`: `passed`
- `cargo test --test doc_drift english_and_japanese_guides_have_matching_files_headings_and_tables -- --exact`: `passed`
- `cargo test --test doc_drift maintained_markdown_links_and_github_anchors_are_valid -- --exact`: `passed`
- `cargo test --test doc_drift configuration_keys_match_english_reference -- --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release`: `passed`
- `target/release/commandagent --version`: `passed`
- `git diff --check`: `passed`

The final link check was repeated after the measurement record gained its exact
cleanup identifiers. The full suite completed with no test failure; ignored
tests retained their existing ignored status.

## Live role-pair probe

The release candidate was run twice for each of these exact command shapes:

- `target/release/commandagent --cwd /private/tmp/commandagent-issue240-probe/baseline --preset issue240 --model-probe`: `passed`
- `target/release/commandagent --cwd /private/tmp/commandagent-issue240-probe/split-9b --preset issue240 --model-probe`: `passed`
- `target/release/commandagent --cwd /private/tmp/commandagent-issue240-probe/split-4b --preset issue240 --model-probe`: `passed`
- `target/release/commandagent --cwd /private/tmp/commandagent-issue240-probe/classifier-4b --preset issue240 --model-probe`: `passed`

Here `passed` means the probe command completed and wrote valid v3 evidence. It
does not relabel a task-level `partial` or `failed` completion band as success.
Those bands, timings, model digests, binary identity, and artifact checksums are
preserved in
`docs/guide/model-probe-results/2026-08-22-local-role-pairs.md`. The temporary
probe workspaces were removed after evidence capture.

## Live runtime cleanup

The eight exact v3 role-qualified basenames recorded in the measurement record
identified the only Issue #240 files under `~/.anvil/model-profiles`. SHA-256
checksums for all eight JSON profiles and all eight Markdown cards were recorded
before deletion. Cleanup used explicit paths only, without a glob or recursive
directory operation.

The final audit loaded those eight recorded basenames, required exactly eight,
checked that both extensions were absent for every basename, and checked that
the pre-run `gemma4-31b-cloud-20260708-085309.{json,md}` and
`m-20260822-023929.{json,md}` sentinels remained present:

- `issue240_profiles_dir=/Users/maenokota/.anvil/model-profiles; issue240_created_basenames=("${(@f)$(rg '^executor-' docs/guide/model-probe-results/2026-08-22-local-role-pairs.md)}"); (( ${#issue240_created_basenames} == 8 )); for issue240_basename in $issue240_created_basenames; do test ! -e "$issue240_profiles_dir/$issue240_basename.json" && test ! -e "$issue240_profiles_dir/$issue240_basename.md" || exit 1; done; test -e "$issue240_profiles_dir/gemma4-31b-cloud-20260708-085309.json" && test -e "$issue240_profiles_dir/gemma4-31b-cloud-20260708-085309.md" && test -e "$issue240_profiles_dir/m-20260822-023929.json" && test -e "$issue240_profiles_dir/m-20260822-023929.md"`: `passed`

No pre-existing live runtime file was targeted or altered.
