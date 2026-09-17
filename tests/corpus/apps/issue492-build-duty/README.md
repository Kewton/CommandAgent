# Issue #492: retain the core build duty

The immutable historical input is `../issue490-verify-readers/known-build-loss.json`
(SHA-256 `e15f1024da83e208592cb9375fd76fad78a6b10041e4320b6f1d456960a6891e`).
Its `preset_converted` value is loss evidence at
`d51cc6ed9e04f8d9747d015ff788845ea1b1e59b`, not the current expectation.
The B0 regression requires exact equality with `sanitized` after conversion.

P1 extends the portable #490 P02-D proposals with that snapshot's complete build
step: original instruction/result/empty paths and both `npm run build` and the
exact `String(require('./package.json').scripts.build)=='next build'` guard.
The guard is explicit input here: the new full-plan sanitizer does not add it to
a bare build. Test-only observations record the actual parsed, host augmented,
sanitized and converted proposals from the ordinary driver. The existing phase
checkpoint replay then checks Admission, returned/saved/reloaded plan,
before_phase and normal registration. No old worktree or run directory is read.

The first proposal acquires package-reader, mixed verifier/documentation,
application and trailing build duties at the normal formation retry boundary.
The valid second proposal separates the verifier and documentation producers
while retaining every original requirement and host duty. N1–N4 each start a
fresh run with that same acquisition prefix plus `test -f src/app/page.tsx` on
the build step. Only the later proposal changes: remove build, move its complete
step before implementation, change expected result, or delete that other check.
The original acquired scopes are checked on all three admission events. Failure
must name the target duty/boundary and must not register anything. This is not an
initial build-free proposal or an unrelated syntax failure.

Contract assertions retain preexisting commands/paths, add exactly README.md,
verify-ui.cjs and `node verify-ui.cjs`, and assign the latter to its Implement/pass
producer. Artifact-only and exact profile commands are filtered by existing rules.
The trailing build remains in the StepPlan even though build was already in the
run contract. Repeat registration observes no contract write or producer change.

Unknown nonempty checks preserve metadata and order, subject to existing policy.
Only existing exact package predicates for the current goal and a passing result
remain eligible for setup conversion. Build is never classified as a reader.
No path is added to or deleted from the original build duty.

## Real controls

`nextjs/` pins Next 14.2.35, React 18.3.1, TypeScript 5.9.3 and all direct type
dependencies; the committed lockfile pins transitive dependencies. Node 24.1.0
and npm 11.3.0 are the measured runtime. Type checking is enabled, with no build
error suppression, and Next uses one CPU. NODE_ENV/NODE_OPTIONS are unset;
devDependencies are installed. No external assets or servers are needed.

`prepare-controls.cjs` requires fresh output directories, runs an explicit
`npm ci --include=dev`, and copies identical dependency bytes and symlink targets
into independent R1/R2/profile controls. R3 has no node_modules. R1/R2 differ only
in the page's `const count: number` initializer (`1` versus `"broken"`). R4-port
changes only scripts.dev; R4-build changes only scripts.build. Source, lock and
dependency hashes are recorded before execution. Each control has separate state
and .next output. The seed is never built.

Example (choose external apps/log/cache/tmp/target paths as appropriate):

```sh
export ISSUE492_REAL_NPM=/absolute/path/to/real/npm
node tests/corpus/apps/issue492-build-duty/prepare-controls.cjs "$ISSUE492_APPS_DIR" "$ISSUE492_LOG_ROOT"
export PATH="$ISSUE492_LOG_ROOT/bin:$PATH"
cargo test --offline --lib issue492_real_nextjs_good_broken_missing_and_profile_controls -- --ignored --nocapture --test-threads=1
```

The opt-in Rust test selects the actual profile preset and enters its core phase
on the explicitly provisioned workspace through ordinary phase prepare, save,
before_phase, registration and the ordinary phase executor. It does not assert a
prior setup/build success or run final application acceptance. It uses a fixed
Write operation; no
repair may precede or replace the initial build observation. The npm observer
records the exact command, active phase/step, already completed steps, immediate
source/config hashes, environment, versions, exit and full compiler output, then
delegates the same arguments to real npm. A generated sidecar beside the wrapper
provides absolute npm/log paths and the selected SSD TMPDIR, because the product
filters custom child environment variables. Production environment policy stays
unchanged; npm_config_cache uses its existing allowed environment prefix. The
wrapper is instrumentation, not a fake compiler.
The test also checks the product's dependency/build lifecycle and source identity.
The required observation is the core build, not overall application completion.
Normal CI runs the portable planner controls; the opt-in test must additionally
be run for this Issue's verification contract.
