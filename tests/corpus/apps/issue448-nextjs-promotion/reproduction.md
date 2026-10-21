# Original Next.js contract promotion regression (#448 AC4)

This is a separate observation corpus. `../issue448-nextjs-r0/` remains the
immutable R0 source, original completion contract, lock and six-case compiler
corpus used by #449. The follow-up adds no application source.

The positive treatment is a disposable copy of `original/`, overlaid with the
existing `ui-only/`, `export/` and `promise/` files. Only page.tsx, types.ts and
the projects/tasks collection routes differ from control. The tasks/[id] API,
all other inputs and strict compiler configuration remain unchanged. All 14
input hashes are checked before either kind of build and again after finish.

## Two explicit verification modes

From the repository root on Unix, with Rust, Python, Node and npm installed:

```sh
cargo test --lib issue448_original_contract_finish_matrix -- --nocapture
python3 -m pytest -q tests/test_issue448_nextjs_r0.py tests/test_issue448_nextjs_contract.py
```

The default Rust test is a fast measured build replay, using frozen exits and
diagnostics only after input-byte/hash checks. The combined positive source is
also bound to the real build record in `measured-contract-results.json`.

To rerun actual locked Next.js builds through the same finish test:

```sh
python3 scripts/issue448_nextjs_contract.py --output /tmp/issue448-contract-new-result.json
```

Choose a new output path: the harness refuses to overwrite evidence. It runs
`npm ci --include=dev --no-audit --no-fund` in disposable storage, with the exact
original package and lock. It records versions and asserts Next 14.2.35 and
TypeScript 5.9.3. It then configures the Rust test with
`ISSUE448_REAL_NODE_MODULES` and `ISSUE448_REAL_NPM`. Each candidate's local npm
transport checks the source inputs and delegates the unchanged `npm run build`
to the real npm binary. Successful builds must type-check, generate all six
pages and retain all three APIs. No dependency downloads occur in the default
Rust test. Full mode needs registry access and loopback port 60302 must be free.
The harness owns all temporary dependency/cache/source writes and cleans them
up; it never starts or changes the R0 app or campaign.

## What is exercised and what is simulated

Every case invokes the actual `RunnerRecoveryDriver::finish`, including
transaction snapshot, contract/observer identity binding, isolated preflight,
`observe_nextjs_recovery_capabilities`, original registered verification,
static runtime acceptance and the ordinary promotion/control-retention path.
The original contract is checked byte-for-byte in control, bound treatment and
observation, before and after finish. Its Next.js profile, goal/port, command,
three capabilities, eight evidence requirements and implementation obligation
are never rewritten. `UnusedClient` makes a model call fail the test.

Only HTTP and browser interaction responses are scripted in both modes. The
original `npm run start` is handled by a test transport launching the existing
one-response native HTTP fixture, on the original port 60302. Readiness performs
its real HTTP request and owns/reaps that child. `interaction.json` is an
explicitly scripted heuristic probe response (no claim of application contract
hooks). The existing cfg(test) availability/result seams pass it through the
real interaction response parser, evidence serializer and Next.js observer.
Final `browser-interaction.json` is produced by that path, never prefilled by
the test. This is conditional promotion regression evidence, **not real browser,
model, end-to-end business acceptance, persistence or concurrency evidence**.
No original business requirement or verification command is removed.

## Six decisions

| Case | Build | Original observation / requirement | Decision |
| --- | --- | --- | --- |
| all_original_observations_pass | passes | scripted route/interaction pass; real static original contract passes | promote |
| build_failure | fails | UI-only R0 candidate retains API defects | retain control |
| failed_interaction | passes | scripted input has no state change | retain control |
| missing_interaction | passes | interaction unavailable; no final interaction evidence | retain control |
| http_failure | passes | HTTP 500 through readiness; no interaction | retain control |
| missing_implementation | passes | API-only repaired source retains original scaffold | retain control |

The last case is particularly useful: both builds and the scripted interaction
pass, but the original `implementation` obligation rejects the scaffold. Build
success alone never establishes promotion. All five negatives assert unchanged
whole-control source hashes. The positive asserts control equals the repaired
whole-treatment hash after promotion. Records preserve all input hashes,
contract hash, original requirements, observed readiness/interaction, static
acceptance and final event decision. Scripted observations are labeled as such.

Runtime lock deadlock and data loss were not observed. Type repair does not
prove mutual exclusion, durability or browser behavior.
