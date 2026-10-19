# CI ignored-test inventory

The shared CI suite runs every non-ignored Rust target. The ignored tests remain
opt-in for explicit environmental reasons; none is silently filtered by the
workflow. The ten pseudo-terminal cases run nightly (see below).

## How the count was taken

The count comes from the enumeration of the built targets, not from a grep of
`#[ignore]` in the source:

```bash
cargo test --all-targets -- --list --ignored
```

Measured on macOS aarch64 with default features: **66** ignored tests. The
`#[ignore]` attributes in `src/` and `tests/` number 70, and four of them are
compiled out on macOS because they sit under `#[cfg(target_os = "linux")]`:

- `tests/issue502_os_sandbox_read.rs`: two Landlock "applied result" cases
- `tests/issue502_os_sandbox_write.rs`: `linux_applied_cargo_build_succeeds`
- `tests/issue502_os_sandbox_network.rs`: `linux_dependency_setup_child_stdin_is_null_child`

On Linux the two `#[cfg(target_os = "macos")]` cases
(`applied_cargo_build_succeeds`, `macos_dependency_setup_child_stdin_is_null_child`)
drop out and the four above are added, so Linux has **68**. The Linux figure is
derived from the `cfg` attributes (the Linux listing was not run here); re-run the
command above on a Linux host to confirm it.

Other factors, checked:

- Cargo features: the only feature (`gui`) gates `tests/gui_server.rs` and
  `src/bin/gui_server.rs`, none of which contains an ignored test, so enabling it
  does not change the count.
- Helpers and doc comments: lines that merely mention `#[ignore]` in a comment
  are not attributes and are not counted.
- `tests/tui_pty.rs` returns early (a pass) unless `COMMANDAGENT_PTY_TESTS=1`; its
  `cfg!(windows)` guard is a run-time check, so all 10 are listed on every target.
- `--all-targets` does not include doctests; no ignored doctest was counted.

## Inventory (macOS aarch64: 66)

| Category | Count | Reason / opt-in path |
| --- | ---: | --- |
| pseudo-terminal tests | 10 | Require a real PTY (`script(1)`); run nightly by `.github/workflows/pty-nightly.yml` through `scripts/ci-pty.sh`, or locally with `COMMANDAGENT_PTY_TESTS=1 bash scripts/ci-pty.sh`. |
| subprocess child entry points | 31 | Names ending in `_child`; invoked by their parent harnesses with controlled environment variables. Running them directly would bypass the parent assertions. |
| provider HTTP smoke tests | 10 | `tests/live_provider.rs`; require live provider endpoints and credentials. |
| parent harnesses and moved cases | 6 | Browser-probe harnesses (2), behavioral reprobe cases (2), probe-unavailable partial case (1), promoted profile run (1); each is covered by another test or run by its owning harness. |
| installed-toolchain checks | 5 | Need installed Node/TypeScript/Next.js or Chromium (issue457, issue465 x2, issue492, issue459). |
| benchmarks | 2 | `sensitive_data` `bench_*`; run with `--release --ignored --nocapture`. |
| heavy sandbox result | 1 | `applied_cargo_build_succeeds` (macOS Seatbelt); run explicitly to record the applied result. |
| container-only | 1 | `dangerous_sentinel_same_uid_other_group_survives`; runs only inside the isolated container (`COMMANDAGENT_ISSUE673_CONTAINER=1`). |

## Nightly PTY run

`scripts/ci-pty.sh` runs only the ten `tui_pty_*` cases, selected by name with
`--ignored --exact`; it never uses a blanket `--include-ignored`, so the live
provider smoke tests and container-only helpers are not started. It logs the
platform, whether `script(1)` gave the child a terminal, the expected and listed
case counts, each case name, and the final `10 of 10` line. It exits non-zero, and
so never counts as a pass, when:

- no pseudo-terminal is available (exit 3, `UNVERIFIED`);
- the ignored cases listed by `tests/tui_pty.rs` differ from the ten expected
  names, or a case did not print `ok`, or libtest did not report 10 passed, 0
  failed, 0 ignored (exit 4);
- the suite exceeds `PTY_SUITE_TIMEOUT_SECS` (default 1500, exit 5; the workflow
  also has a 45-minute job limit);
- a case fails (exit 1).

If a case is added to or removed from `tests/tui_pty.rs`, update the name list and
count in the script, otherwise the run fails by design.
