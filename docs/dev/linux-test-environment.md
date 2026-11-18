# Linux test environment: Node and locale (Issue #968)

Two test groups depend on the environment they run in, not only on the code:

- **Recovery obligation (#465)**: the fixture `tests/corpus/apps/issue465-repair-obligation`
  registers `node checks/target.js` as its checker. The obligation is discharged only when
  that checker really runs and passes, so these tests need a runnable `node`.
- **TUI integration** (`tests/tui_integration.rs`): the presentation glyphs follow the locale.
  `tui::terminal::utf8_locale` reads `LC_ALL`, then `LANG`, and chooses ASCII (`--`, `->`,
  `ok`) unless the value names UTF-8. This fallback is the product's behaviour (#50) and
  is not changed here.

## What the tests now state about it

- Every #465 case starts with `require_node_checker`
  (`src/planner/recovery_repair_obligation/tests.rs`). When `node` cannot run, the case
  fails with `PRECONDITION MISSING: the Issue #465 registered checker ... cannot run`, and
  the message tells the two causes apart: `node` not on the verifier PATH, or `node` on that
  PATH but refused through the verifier's shell and OS sandbox. A case never skips, and
  nothing stands in for the checker. `issue465_positive_cases_fail_as_a_missing_precondition_without_node`
  runs the positive cases in a child with no `node` on PATH and requires this failure.
- Each TUI test body runs alone in a child process of the test binary with `LC_ALL` set
  explicitly (`own_locale_process` in `tests/tui_integration.rs`). The UTF-8 group runs
  under `LC_ALL=C.UTF-8` with its expectations as written. The ASCII control
  (`primary_ultra_plan_run_renders_plan_then_activity_then_summary_ascii`) runs the same body
  under `LC_ALL=C` with the same expectations turned through `tui::glyphs::for_locale(.., false)`,
  in the same order. A panic in one body ends only its own child, so it no longer poisons a
  shared lock and fails the tests after it. A child that ran no test fails its parent.
- `a_c_locale_child_fails_the_utf8_glyph_expectations` is the control for the Linux failure:
  the UTF-8 expectations in a `LC_ALL=C` child fail on the ASCII fallback.

The ambient locale of the runner therefore no longer decides the TUI result, and a missing
`node` is reported as such instead of as a Recovery decision.

## Running the groups in a local Linux container

`scripts/ci-linux/run-tests.sh <out-dir>` builds the lib and `tui_integration` test binaries
with `cargo-zigbuild` for a Linux target on the host and runs them in a local image:

```bash
scripts/ci-linux/run-tests.sh /path/to/out
CI_LINUX_REPEAT=50 scripts/ci-linux/run-tests.sh /path/to/out     # repeat each group
CI_LINUX_HIDE_NODE=1 scripts/ci-linux/run-tests.sh /path/to/out   # observe the missing-node failure
```

On the shared development Mac, wrap the command in the heavy-lock `with-cargo-slot` runner.

- Requirements: `docker`, `cargo-zigbuild`, `python3`, the Rust target
  (`aarch64-unknown-linux-gnu` by default; set `CI_LINUX_TARGET` for another architecture),
  and the image already present locally (`node:22` by default; `CI_LINUX_IMAGE`).
- The container is started with `--rm --pull=never --network none --user 1000:1000`. The
  script never pulls or builds an image, and no Dockerfile is involved. The source tree is
  mounted read-only; the cargo target directory is mounted so the binaries and
  `CARGO_TARGET_TMPDIR` resolve at their host paths.
- Groups, each run once per locale in `CI_LINUX_LOCALES` (default `C.UTF-8 C`, set as the
  ambient `LC_ALL`): `tui_integration`, and the lib filters `issue465`,
  `recovery_repair_obligation`, `tui::terminal`, `tui::glyphs`.
- A run passes only when it exits 0 with at least one passed test. Ignored tests are
  reported in the counts and never counted as passes. The script exits 0 only when every
  run passed.
- The record (`<out-dir>/summary.txt`, logs under `<out-dir>/logs/`) holds only: source
  HEAD and its changed-file count, the build's `rustc -V`, node presence and `node -v`,
  `uid`, the ambient `LC_ALL`/`LANG`, and each run's exit code and `test result` line. It
  does not dump the environment, read keys, or load `.env`.
- At the end the script compares the container count before and after and fails if a
  container of this run is left behind. Check by hand with `docker ps -a`.

## Differences from GitHub CI

| | GitHub CI (`.github/workflows/ci.yml`) | Local container |
|---|---|---|
| OS | `ubuntu-latest` (x86_64) | Debian bookworm (`node:22`), aarch64 on Apple Silicon |
| Node | 24.1.0 (`actions/setup-node`) | v22 (the image's; v22.23.2 when measured) |
| Rust | `dtolnay/rust-toolchain@stable`, built in place | host toolchain, cross-built with `cargo-zigbuild` |
| Locale | runner default (not recorded in CI logs) | set per run; the tests set `LC_ALL` for their own children |
| Scope | `scripts/ci.sh`, every target | the two groups above |

The Node major version differs (24 in CI, 22 here). The #465 checker uses only
`node:assert/strict` and ES modules, which both versions support, but a result from the
container is not a result from CI's Node. Report the two separately. A Linux failure
outside these groups has a cause of its own and is not part of their result.
