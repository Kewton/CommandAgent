# Issue #442 domain oracles

`cases.json` freezes the T2/T3 inputs independently of product knowledge. The
adjacent `nextjs-domain-{s1,e1,i2,s3}` directories contain unedited source files
from the final retained originals of campaign `20260907-1317-standard10`.
`nextjs-domain-s3-candidate` is the rejected recovery treatment, used only for
static response-shape regression. Each `provenance.json` records the session
and SHA-256 of every copied source; tests verify all hashes before and after.
Source provenance is rooted at the original campaign's
`commandagent_trial/sessions/<source_session>` directory. No historical runtime
state, credentials, logs or evaluation outputs are copied or modified.

Each `adapter.json` fixes the real route/body/response mapping and the expected
failing cases. S1's staff response is bare but shifts are wrapped. S3 uses
wrapped responses and `/staff/:id` deletion. E1/I2 responses are bare. Adapter
expectations describe per-case results, not a blanket failure for all operations:
the original 0444 failures are correct; S3 synchronous creates also retain both
records in this single-process replay.

Run the offline replay regression with Node 24.1.0 and the locked TypeScript 5.9.3 dependency:

```sh
npm ci --ignore-scripts --include=dev --prefix tests/nextjs_domain
python3 -m pytest tests/test_nextjs_domain_oracle.py -q
ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/nextjs_domain_oracle.py tests/test_nextjs_domain_oracle.py
```

The replay uses the TypeScript compiler's import/type erasure without changing
the frozen source, a WHATWG Request/Response adapter for `next/server`, and a
private loopback HTTP server. This tests the original handler logic and disk
effects. It does not build or start Next.js, emulate its cache, prove UI behavior,
or measure generation quality. Import/start/setup failures fail the test.

The same HTTP/disk oracle can run against a real generated app's server on a
disposable source copy. Create `.issue442-oracle-scratch` in that copy, supply
an adapter reviewed against its routes, then run:

```sh
python3 scripts/nextjs_domain_oracle.py --base-url http://127.0.0.1:PORT --workspace /path/to/disposable-copy --adapter tests/corpus/apps/nextjs-domain-s1/adapter.json
```

It creates non-sample parent and child records through HTTP, confirms GET/disk
agreement, then tests both related files against invalid JSON, replacement by a
directory, and 0444 writes. It also tests empty arrays and two concurrent POSTs,
SKU aggregation/exact stock/one short, shift 30-minute alignment/overlap/adjacency/
parent deletion/strict break boundary, and expense zero/negative/exact budget/
one over/month scope. Rejected operations must preserve all managed bytes and
modes. Each case and the whole run restore those files, including absent files.
Use an isolated process/data directory; external concurrent writers would make
restoration inappropriate.

The offline replay arms a bounded read-completion barrier for the two concurrent
creates, exposing the unsafe async read-modify-write interleaving deterministically.
It performs real reads and writes. A serialized correct control releases the
first read on timeout, then reads the committed version. The external CLI uses
ordinary concurrent HTTP without this scheduler seam; a pass there covers the
observed interleaving only. Multi-process safety and crash recovery require the
later campaign/environment checks.

Future effect measurement belongs to the orchestrator after #439/#440 merge and
the one-factor B-1 experiments. Reuse the same inventory/shift/expense goals × 3,
fix binary SHA, model digest, order and oracle hash, and record build/start
type/export failures, domain UI completion and data loss. Among started runs,
corrupted JSON overwrite and lost concurrent writes target zero. Scores and
scaffold-block counts are not substitutes. No future effect is claimed here.

The dedicated `.github/workflows/nextjs-domain.yml` job runs on every PR to main/develop/release branches and pushes to those branches, with no path filter or optional/skip condition. Node 24.1.0, Python 3.12.3, the TypeScript lockfile and pinned `requirements/ci.txt` make the dependency path reproducible. Missing dependencies fail the job. Repository branch-protection settings are external to this implementation.
