# Issue #441 verification

- Status: `passed`

## Checks

- `cargo test --lib issue441`: `passed`
- `cargo test --test issue441_placeholder --test issue428_bash_path_tokens --test bash_workspace_confinement --test corpus_regression --test generality_guardrails --test conformance`: `passed`
- `cargo test --lib independent_sessions_measure_recovery_build_and_business_acceptance_separately -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --all-targets`: `passed`
- `cargo test --doc`: `passed`
- `git diff --check`: `passed`

## Results

- Issue-specific coverage: 5 leaf-module tests and 7 integration tests. Both a
  single placeholder call and the three original I2 runtime-policy commands in
  one batch reach a corrective model turn, then complete a real implementation
  step with a relative Bash read, successful Write, and declared verification.
  No repeated-error stop or failed step is emitted in these corrected runs.
- The unchanged #428 tests (5) and workspace-confinement tests (2) pass.
  Real workspace `cd` still strips; outside paths, redirects, traversal,
  symlinks, and unapproved writes retain their rejection behavior.
- Corpus regression: 7 passed. Generality guardrails: 10 passed. Conformance:
  18 passed, 1 existing ignored test. No baseline, threshold, or test exclusion
  was changed. `loop_run.rs` and `runner.rs` remain unchanged.
- The full all-target run completed outside the sandbox with exit 0, including
  all integration suites: 2,728 passed and 38 existing ignores across 64 suites.
  Its library suite passed 2,400 tests with 16 existing ignores. Both compile-fail
  doctests passed separately.
- Private evidence tests compare independently calculated SHA-256 values with
  event and evidence fields, assert exact first-64-byte preservation including
  split UTF-8, enforce 0700/0600 permissions, and reject symlink ancestors or
  public evidence directories without executing the command. Evidence-write
  failures remain explicit rejections.
- Recovery file tests cover real and anonymized workspace paths, paths with
  spaces, external home paths, and the saved prompt/report/YAML routes. Unit
  coverage includes sibling-prefix rejection, traversal, adjacent shell
  redirection, whitespace/UTF-8, and preserved secret redaction.

## Execution context and corrected intermediate failures

Verification ran on 2026-09-08 in the assigned feature worktree, based on
`36f73eeb312c105a62ed488ce54e480b9e3c8855`. No live provider, browser UAT,
campaign, or external lifecycle action was invoked. The original campaign
and referenced develop files were read-only; the committed event fixture is a
documented projection, not rewritten historical evidence.

The initial sandbox full-suite attempt encountered existing HTTP-fixture socket
failures (`Operation not permitted`) and was interrupted. Complete outside-
sandbox runs supersede it. An intermediate outside-sandbox run caught a real
Recovery regression: saving normalized YAML while retaining a differently
rendered in-memory plan failed the existing strict equality gate. Both paths
now use the same root-aware builder; the failing existing test and full suite
pass without weakening that gate.

Intermediate focused checks also caught the corpus parser's repository-specific
unquoted fixture-key convention and growth budgets in repair/telemetry modules.
The fixture follows the existing convention; display helpers were extracted
to the new leaf module and telemetry reduced to one wiring call. The final
guardrail check passes with the original baselines. Early new-test corrections
aligned secret-redaction expectations with the existing scrubber and shared
the scripted client's turn counter across provider clones.

The one-error and batch-correction cases use a scripted model; actual model
compliance and campaign outcome remain for orchestrator-owned CI/UAT. Exact
placeholder rejections do not consume the generic repeated-error counter;
ordinary confinement errors preserve their counts and global step iteration /
wall-clock limits remain unchanged.

No Python harness was changed. The orchestrator's advisory about the unrelated
non-hermetic Python dependency-batching test did not arise in these required
Rust checks, so its fixture patch was not imported.
