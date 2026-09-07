# Issue #442 implementation

Next.js create guidance now specifies shared TypeScript types/exports, matching
UI/API JSON bodies and displayed errors, missing-versus-corrupt file behavior,
transaction-wide serialization plus temporary-file/rename writes, and whole-state
validation. The text lives in `knowledge.toml`. Generic generation, guidance,
runtime-contract and preset prompts use goal-neutral UI vocabulary while keeping
route-bound implementation, build, observability and restart obligations. The
core prompt includes the supplied goal's entity vocabulary. The duplicate
declarations in the embedded Next.js manifest are synchronized with knowledge;
the existing manifest/knowledge equality check is retained.

`response_shape.rs` adds one diagnostic check to `api_contract`, keeping the
existing route/export/Response.ok checks. It compares literal local fetch routes
and methods, immutable response/body bindings, known 2xx object keys and arrays.
The prefix remains `api_contract_failure:`; no event names or schema change.
S3's frozen candidate and the run-01/E3 pattern reductions detect mismatches;
matching bodies do not. Candidate-review controls cover non-2xx message/code
reads under `!res.ok`, optional feature/property guards, status conditionals,
comments/strings/regex, and unrelated nested return scopes.

Unknown shapes, indirect helpers, mutable/ambiguous bindings, dynamic paths,
typed return syntax and unsupported control flow stay runtime obligations. This
is a conservative static diagnostic, not a complete TypeScript/dataflow checker
or evidence of working end-to-end behavior. Required generation rules do not
turn unknown static analysis into successful runtime verification.

Frozen source fixtures contain 43 files from S1/E1/I2/S3 final retained originals
and the rejected S3 candidate, with per-file SHA-256 provenance. Source files
were copied before product edits. The separate response-shape corpus labels
run-01/E3 examples as pattern reductions, not original byte snapshots.

The standalone Python HTTP/disk oracle and offline TypeScript handler replay
exercise 51 cases for the four original adapters and the same 51 cases against
a correct control. They create non-sample records, inspect real responses and
files, and restore all managed related JSON bytes/modes. The race replay uses
a bounded read-completion barrier to freeze the unsafe async interleaving.
The external CLI can reuse the same cases against real Next.js servers on
marked disposable copies without that barrier.

The issue-owned `nextjs-domain.yml` workflow runs Python/Ruff on every PR/push
to the repository's main/develop/release branches, with Node 24.1.0, Python
3.12.3, locked TypeScript 5.9.3 and the existing pinned Python requirements.
There are no optional steps or missing-dependency skips. The replay resolves
its local locked TypeScript library rather than a global `tsc`. Shared
`scripts/ci.sh` and existing workflow files are unchanged; remote execution
and branch-protection configuration remain external work.

Full verification exposed an existing scheduler unit test whose supposedly
independent inputs were enriched by repository-wide matches for "Independent"
(including an already-tracked evaluation script). Its search enrichment is now
isolated in that one unit test; batch-size and ordering assertions are unchanged.
Production scheduler/search behavior is unchanged.

| Acceptance criterion | Implementation / evidence |
| --- | --- |
| 1: S3 mismatch, matching controls | `nextjs-domain-s3-candidate`, `nextjs-domain-response-shapes`, response-shape tests |
| 2: business prompts and game regression | `nextjs_domain_knowledge` business and Space/Breakout/Quiz matrix; existing corpus |
| 3: T2/T3 original failures and correct control | `nextjs-domain-{s1,e1,i2,s3}`, `cases.json`, `test_nextjs_domain_oracle.py` |
| 4: subsequent campaign metrics | Frozen `cases.json.future_campaign` and oracle README; measurement deferred to orchestrator |
| 5: profile/event constraints | No shell capability, executable profile template, event schema or guardrail-baseline changes |

Only #442's changelog entry was inserted after the MIT/contribution-guide item
within Added. No runner/loop chokepoint, `.anvil` state, historical evidence,
other lane's source, or external GitHub state was changed.

The local HTTP replay is not a Next.js build/start/UI run and does not establish
multi-process safety or crash durability. The normal-control fixture is a
test control, not a product template. The #439/#440 merged-binary campaign and
one-factor B-1 experiments remain the orchestrator's work; this implementation
claims no unperformed reduction in build failures, data loss or completion rate.
