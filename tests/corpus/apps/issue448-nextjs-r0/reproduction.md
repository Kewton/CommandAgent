# R0 export / Promise / Recovery regression

This corpus freezes the retained original from campaign
`20260908-2310-standard10`, R0 session
`01a08187-2bf0-7853-b0c9-814043569f26` at product HEAD
`5572467ee5028ae43f8497f875e1ef16030cf470`. The original 14 source/config/lock
files were byte-for-byte checked against the campaign's
`runs/R0/original-sha256-after.json`. `provenance.json` records those hashes and
the source evidence hashes. `fixture-sha256.json` covers the extracted and
derived corpus. No dependency tree, business data or live runtime is copied.

The source evidence is read-only in the integration worktree
`../CommandAgent-develop/workspace/tmp/0908/`. See its
`result/commandagent-nextjs-capability-evaluation-r0-stop-20260909.md`,
`result/reviews/r0-stop-codex2-20260909/resolution.md`, and the above campaign.
`evidence/original-diagnostics.txt` is a compiler-only excerpt with ANSI escapes
removed. `evidence/recovery-events.json` retains selected event objects and
original line numbers, including the UI-only delta, rejected promotion and
unchanged control hash. The original completion contract is preserved verbatim.

## Independent compile matrix

Run from the repository root with Python 3.12+, Node and npm (recorded run:
Node 24.1.0, npm 11.3.0). Network access to the lockfile's npm artifacts is
needed for installation; no OpenAI key, model, browser or running app is used.

```sh
python3 scripts/issue448_nextjs_r0.py --output /tmp/issue448-build-results-new.json
python3 -m pytest -q tests/test_issue448_nextjs_r0.py
python3 -m ruff check scripts/issue448_nextjs_r0.py tests/test_issue448_nextjs_r0.py
cargo test --lib issue448 -- --nocapture
cargo test --test corpus_regression
```

Use a fresh output filename. The harness refuses existing output files, creates
disposable workspaces and runs `npm ci --include=dev --no-audit --no-fund`
against the unmodified R0 package lock (Next.js 14.2.35). `--include=dev` makes
TypeScript installation independent of the caller's production npm setting.
Each variant then runs the identical `npm run build` with a fresh `.next`
directory. The harness asserts strict type checking, original input hashes,
diagnostic presence/absence, exit status and all three retained API routes in
the successful build. It executes 13 values against the repaired status helper.
`evidence/build-results.json` contains the measured results and exact input
hashes. Full raw process logs are not committed.

| Variant | Changes to original | Required build outcome |
| --- | --- | --- |
| original | None | Missing export warning; projects Promise error; exit 1 |
| ui-only | Exact unpromoted R0 page.tsx | Same two defects; exit 1 |
| export-only | Add typed status predicate | Projects Promise error remains; exit 1 |
| promise-only | Repair release return typing in projects and tasks | Missing export becomes fatal TypeScript error; exit 1 |
| projects-and-export | Repair the two initially observed locations | Tasks release call has type void; exit 1 |
| repaired | Export + both Promise repairs | Full strict Next build; exit 0 |

The additional `tasks/route.ts` callability diagnostic was masked by the first
errors. Fixing only the projects signature would not yield a compiling fixture.
The Promise overlay gives both acquisition functions `Promise<() => void>`;
the tasks function returns its existing release callback instead of another
unresolved Promise. The original tasks route's three pre-existing `any`
annotations are retained verbatim; no repair introduces `any`, removes APIs,
changes build/config/lock files or disables checking. The export repair appends
an `unknown`-to-`TaskStatus` predicate without altering existing definitions.

## Recovery boundary tests and limits

The Rust tests use the actual import scanner, bounded build verifier and its compile diagnostic parser,
compile-repair scope filter, contract binding, isolated snapshot and
`RunnerRecoveryDriver::finish`. Fast offline tests replay the *measured*
compiler exits/diagnostics through shell scripts; these scripts are explicitly
scripted observations, not a second compiler or real business oracle.
The verifier retains the complete scripted command output before parsing it;
tests do not introduce a direct parser call or bypass the source-of-truth audit.

They cover failed UI-only execution, claimed-success execution followed by
build rejection for every defective variant, repaired build with a failing
additional check, missing acceptance evidence, deleted API, changed verifier
authority and a successful positive boundary control. Rejected controls must
retain the complete source SHA-256; successful promotion must exactly match the
treatment. UI-only product delta must contain just page.tsx and no additions or
deletions. `ISSUE448_BOUNDARY` lines expose compact measured gate decisions.

The positive boundary control uses a generic contract with all original required
paths plus the API paths, the original goal, replayed build and a scripted
additional verifier. It tests the promotion gate's conjunction, **not full R0
business/browser acceptance**. A separate test binds the original Next.js
contract byte-for-byte, retains its SHA-256, registered build command and browser
observer, and confirms that fixture type repair cannot change that authority.
The generic corpus runner's `compile.expect = "not_checked"` is literal: only
the explicit independent matrix command establishes the compile results.

Runtime lock deadlock and data loss were not observed in R0 or measured by this
compile/gate regression. In particular, the task-ID lock and concurrent writers
remain outside the experiment. Type repair proves neither mutual exclusion nor
persistence correctness. This fixture does not establish model repair efficacy,
full business requirements, or a successful live R0 campaign.
