# Fetch probe design

Status: **draft for review (2026-08-03)**

This document is the E-2a design-first draft for E-4 stage 2. It specifies a
bounded fetch-and-evidence boundary, but it does not change the current ingest
contract, runtime, event schema, or acceptance meaning. The sequence is:

1. review and adjudicate this draft;
2. fix the contract and fixture vocabulary;
3. implement the adjudicated boundary;
4. calibrate it with new campaigns.

Implementation must not begin by treating the recommendations below as already
fixed. Section 11 lists the decisions that must be adjudicated first.

Related fixed records are the [stage-1 ingest contract](ingest-profile-contract.md),
the [D-3c boundary design](d3c-shell-design.md), and the
[mechanism ledger](dev/mechanism-ledger.md). The existing process primitive is
[`bounded_process`](../src/bounded_process.rs).

## 1. Purpose and invariant

Stage 1 deliberately evaluates a local, pinned HTML/text snapshot. Stage 2 adds
one capability before that boundary: obtain a declared URL, preserve the exact
response body as a workspace snapshot, and record enough evidence to reproduce
and audit that acquisition.

The central invariant is:

> Network access ends at the fetch boundary. Ingest begins from a stored,
> content-addressed workspace snapshot.

After admission of the snapshot, the existing N1--N5 path runs unchanged and
with network access disabled:

- N1: bounded pipeline;
- N2: source binding;
- N3: candidate accounting;
- N4: output-format schema;
- N5: rerun stability.

The fetch probe therefore does not reinterpret N1--N5 and does not create a
second parser, comparator, or acceptance path. A fetch failure cannot be hidden
by a later ingest result, and an ingest success cannot repair missing fetch
evidence.

## 2. Proposed closed contract

The fetch declaration belongs to a suite/ingest contract, not to an assist or
eval pack. Packs remain unable to grant network authority. The following TOML
is illustrative schema text for adjudication; it is not yet a supported runtime
format.

```toml
[fetch]
schema_version = "commandagent.fetch/v0"
allowed_domains = ["data.example-city.jp"]
max_fetches = 2
max_http_requests = 3
timeout_seconds = 15
max_response_bytes = 8388608
freshness_max_age_seconds = 86400
cache_policy = "canonical-url-utc-day"
robots_policy = "respect"
user_agent = "CommandAgentFetch/0.1"
min_origin_interval_ms = 1000
redirect_policy = "reject"

[[fetch.sources]]
source_id = "announcements"
url = "https://data.example-city.jp/announcements.html"
snapshot_path = "data/snapshots/announcements.html"
authorization = "contract"
```

The proposed schema is closed: unknown fields, duplicate `source_id` values,
invalid enumerations, and undeclared source URLs are schema errors. In v0:

- `allowed_domains` contains exact DNS host names. Wildcards and implicit
  subdomain inheritance are forbidden.
- `sources[].url` is an exact URL, including its query string. Fragments and
  user-info are forbidden.
- only HTTPS GET is allowed;
- request bodies, cookies, credentials, arbitrary headers, and proxy settings
  are not expressible;
- every `snapshot_path` is a relative path below the run workspace's declared
  snapshot directory. Absolute paths, `..`, and symlink escape are rejected;
- `max_fetches` bounds requested source acquisitions and the loader requires
  `len(fetch.sources) <= max_fetches`; `max_http_requests` independently bounds
  all network requests, including robots requests and any future retry;
- the timeout applies independently to each bounded request. A campaign-level
  deadline remains an outer bound;
- `max_response_bytes` is enforced while streaming, before the full body is
  retained;
- `freshness_max_age_seconds` must be finite and non-negative.

The recommended v0 has no automatic retry. If review later admits retry, each
attempt must consume the request cap and must appear in evidence; retry cannot
silently weaken the timeout or idempotence rules.

### 2.1 Contract authority and Gate 1

A URL may be fetched only when one of these authorities is present:

1. `contract`: the exact URL is pinned in `fetch.sources`;
2. `gate1`: Gate 1 has displayed and persisted the exact URL, source ID,
   destination path, and contract hash, and the user has confirmed that card.

Gate 1 may select or confirm a URL only within a predeclared exact
`allowed_domains` entry. It cannot enlarge the domain allowlist. A domain
expansion requires a reviewed suite/contract change. The persisted confirmation
hash is referenced by fetch evidence so that a later string substitution cannot
inherit the earlier approval.

The LLM may propose a URL for Gate 1, but it receives neither a raw socket nor a
general `curl`, `wget`, browser, or shell-network tool. It sees the saved
snapshot only after the boundary has completed and validated the acquisition.

## 3. Fetch evidence

Every attempted source produces a typed entry, including failures. A successful
entry must contain at least the five required acquisition facts: URL, time,
HTTP status, content SHA-256, and saved path.

The proposed envelope is:

```json
{
  "schema_version": "commandagent.fetch-evidence/v0",
  "run_id": "uat-example-001",
  "contract_ref": "bench/suites/example.toml",
  "contract_sha256": "<64 lowercase hexadecimal characters>",
  "entries": [
    {
      "source_id": "announcements",
      "requested_url": "https://data.example-city.jp/announcements.html",
      "canonical_url": "https://data.example-city.jp/announcements.html",
      "authorization": "contract",
      "authorization_ref": "fetch.sources[announcements]",
      "authorization_sha256": "<64 lowercase hexadecimal characters>",
      "fetched_at_utc": "2026-08-03T01:02:03Z",
      "fetched_at_epoch_ms": 1785728523000,
      "http_status": 200,
      "content_sha256": "<64 lowercase hexadecimal characters>",
      "content_bytes": 12482,
      "snapshot_path": "data/snapshots/announcements.html",
      "outcome": "fetched",
      "elapsed_ms": 438,
      "cache": {
        "policy": "canonical-url-utc-day",
        "utc_date": "2026-08-03",
        "cache_key_sha256": "<64 lowercase hexadecimal characters>",
        "source_fetched_at_epoch_ms": 1785728523000
      },
      "robots": {
        "robots_url": "https://data.example-city.jp/robots.txt",
        "checked_at_utc": "2026-08-03T01:02:02Z",
        "http_status": 200,
        "decision": "allow",
        "rule_group": "CommandAgentFetch/0.1",
        "evidence_sha256": "<64 lowercase hexadecimal characters>"
      }
    }
  ]
}
```

Angle-bracket strings above are schema examples, not values that an execution
may accept. Runtime evidence must contain concrete hashes and identifiers.

### 3.1 Byte identity and storage

For a successful acquisition:

- request `Accept-Encoding: identity` so the stored object and the hashed
  object have one unambiguous byte representation;
- hash the response-body bytes after HTTP framing and before parsing,
  normalization, or character decoding;
- stream into a workspace-local temporary file while enforcing the size and
  timeout bounds;
- verify the byte count and SHA-256, durably flush the staged file, then
  atomically publish to the declared snapshot path;
- reject a destination that resolves outside the snapshot root;
- do not include the raw body in an event, console log, or error message.

Only HTTP 200 admits a new snapshot in the recommended v0. Redirects and other
statuses are still recorded, but their content is not handed to N1--N5.
Partially written and unverified files are never published.

Evidence itself is written under the run's evidence directory, for example
`evidence/fetch-evidence.json`. Its snapshot path is relative to the run
workspace so scrubbed evidence neither leaks a host path nor becomes ambiguous
when copied.

## 4. Network and process chokepoint

All network acquisition must pass through one dedicated fetch boundary. The
boundary owns:

- contract and Gate 1 authorization checks;
- URL canonicalization and domain/IP safety checks;
- robots and courtesy decisions;
- request count, byte, time, and campaign cancellation bounds;
- caching, content hashing, atomic snapshot publication, and evidence writing;
- typed failure conversion.

If the transport uses a child process, spawning, cancellation, timeout, and
output collection must use the existing `bounded_process` facility. A raw
`Command`, direct shell escape, or independently implemented timeout is not an
alternate fetch path. If the transport is in-process, it still lives behind the
same dedicated fetch interface and must expose equivalent bounds and typed
results.

The implementation should be a leaf module with minimal wiring at planner and
minimal-loop chokepoints. The first implementation commit must also register an
audit guard that detects direct network transports or unregistered fetch
dispatch sites outside the boundary. This is the D-3c lesson applied on day
one: merely having a correct boundary is insufficient if a direct provider or
transport call can bypass it.

No URL string produced by a model is interpolated into a shell command. No
provider credential, generic environment credential, cookie store, or proxy
environment is forwarded to a transport child. Cancellation and timeout return
typed failure; they do not panic.

### 4.1 URL and endpoint safety

Before a request, the proposed v0 boundary must:

1. parse and canonicalize the URL without fetching it;
2. require HTTPS, an exact allowed host, an allowed port, and an exact declared
   or Gate 1-confirmed URL;
3. reject embedded credentials, fragments, malformed percent encoding, and
   ambiguous host spellings;
4. resolve DNS and reject loopback, private, link-local, multicast, unspecified,
   and otherwise reserved destinations;
5. bind the request to a validated resolution and verify the connected peer so
   DNS rebinding cannot cross the allowlist;
6. reject every redirect in v0, including a redirect to another allowed host.

These rules protect the local runtime as well as the remote site. Domain
permission alone is not an SSRF defense.

## 5. Robots and courtesy policy

The recommended policy is explicit, conservative, and measurable:

- obtain and cache `robots.txt` per origin before content acquisition;
- evaluate the product user-agent group, falling back to `*` when applicable;
- obey `Disallow` and a parseable `Crawl-delay`;
- serialize requests per origin and wait the larger of the declared minimum
  interval and the applicable crawl delay;
- treat robots 404 or 410 as “no published rules” and allow the declared URL;
- treat robots 401 or 403 as an explicit denial;
- fail closed on robots timeout, transport failure, oversize response, or parse
  ambiguity;
- never evade a denial, impersonate a browser, solve a challenge, or rotate
  identities;
- count the robots request in `max_http_requests`.

The product user agent should identify CommandAgent and provide a stable project
information URL once that contact value is adjudicated. A suite cannot weaken
robots behavior. It may make courtesy stricter by lowering caps or increasing
the minimum interval.

Robots permission is not authorization to fetch an undeclared URL, and contract
authorization is not permission to ignore robots. Both must pass.

## 6. Same-URL, same-day idempotence

The cache key is derived from the canonical URL and its UTC calendar date. For
the same URL on the same UTC day, a valid workspace cache entry is used without
a network request.

A cache hit must:

- revalidate the current contract/Gate 1 authority and destination path;
- verify the cached byte count and content SHA-256 before use;
- materialize the exact cached bytes at the declared snapshot path;
- report `outcome = "cache_hit"` and the original acquisition timestamp;
- use the original acquisition timestamp, not the cache-hit time, for freshness;
- consume one declared source-acquisition slot but no HTTP-request allowance.

Cache corruption, missing metadata, or a hash mismatch fails closed. It does not
silently refetch, because that would make a nominally idempotent rerun perform
new network work. A later explicit policy may authorize repair, but such repair
must be visible as a new acquisition.

The cache is workspace-local. A global cache, cross-contract trust, conditional
GET, and server validator semantics are outside v0.

## 7. Freshness verification: proposed N6

The smallest addition to the N family is a proposed
`N6 ingest_fetch_freshness` check. The name and number are provisional pending
Section 11 adjudication; N1--N5 retain their existing meanings either way.

N6 consumes fetch evidence rather than page prose. For each source:

```text
age_ms = evaluation_started_at_epoch_ms - source_fetched_at_epoch_ms
pass iff 0 <= age_ms <= freshness_max_age_seconds * 1000
```

On a cache hit, `source_fetched_at_epoch_ms` is the original fetch time. HTTP
`Date`, `Age`, `ETag`, and `Last-Modified` may be preserved as diagnostics, but
they do not replace the observed acquisition time in v0. A negative age is a
clock anomaly and cannot pass.

The check emits or stores a small additive result containing source ID, contract
hash, fetch-evidence hash, acquisition time, evaluation time, age, configured
maximum age, and `pass`/`violation`. It performs no network request and invokes
no new judge.

The recommended stage-2 acceptance relation is:

```text
valid fetch evidence
AND N6 pass for every required source
AND existing N1, N2, N3, N4, and N5 pass
```

A fetch failure or stale source does not enter N1--N5 as if it were an empty
page. Its typed acquisition/freshness outcome remains visible. Exact mapping to
the existing final-verdict vocabulary is an adjudication item; this draft does
not change current `full` semantics.

## 8. Typed outcomes and honest failure

The boundary must distinguish at least:

- undeclared domain or URL;
- missing or mismatched Gate 1 confirmation;
- unsafe scheme, endpoint, DNS result, or peer address;
- robots denial or robots-unavailable failure;
- request-cap, fetch-cap, timeout, cancellation, or response-size exhaustion;
- redirect or non-200 status;
- snapshot path escape, I/O failure, or atomic-publication failure;
- cache metadata or content corruption;
- content hash mismatch;
- stale evidence or clock anomaly.

These are policy, environment, or machine observations, not model failures. The
report may separately state that no ingest attempt occurred. Verification and
acceptance must not rewrite one of these outcomes to success merely to complete
a campaign.

Proposed additive lifecycle events are `fetch_authorized`, `fetch_completed`,
`fetch_cache_hit`, `fetch_failed`, and `ingest_fetch_freshness`. Their final
names and envelopes require schema review; none is introduced by this draft.
Existing event names and schemas remain byte-for-byte unchanged.

## 9. Explicit v0 exclusions

Stage 2 v0 is only “fetch declared URLs, prove what was fetched, save it, and
connect the snapshot to the existing ingest path.” It excludes:

- autonomous choice of links, page transitions, crawling, and arc-2 navigation;
- discovery of additional URLs from page content;
- authentication, session cookies, paywalls, forms, POST, and uploads;
- JavaScript execution, browser rendering, and client-side application state;
- CAPTCHA, anti-bot bypass, or robots override;
- arbitrary redirects and cross-origin delegation;
- global or shared caches and background refresh;
- claims that one fetched page is current, complete, or representative of an
  entire site;
- any LLM access to general network or shell-network tools.

PDFs, attachments, and new content types require their own bounded parser and
contract decision. A URL being declared does not implicitly enable them.

## 10. Verification plan and predeclared estimate

Implementation must begin with contract and boundary fixtures, then add the
smallest wiring needed to connect a verified snapshot to ingest. Minimum
negative coverage is:

| Area | Required fixture |
|---|---|
| closed schema | unknown key, invalid enum, duplicate source, missing cap |
| authority | undeclared domain/URL, wildcard, Gate 1 hash mismatch |
| URL safety | HTTP, user-info, path ambiguity, private/loopback DNS, peer mismatch |
| redirect | 3xx is recorded and rejected even when target is allowed |
| robots | allow, disallow, 404/410, 401/403, timeout, malformed response |
| bounds | request/fetch cap, timeout, cancellation, oversize body |
| status | 200 admission and non-200 honest failure |
| storage | exact SHA/bytes, partial-write cleanup, symlink/path escape |
| cache | same URL/day zero-network hit, original time, corruption failure |
| freshness | boundary pass, stale violation, negative-age violation |
| chokepoint | no direct network dispatch and no raw LLM network tool |
| ingest connection | admitted snapshot follows unchanged N1--N5 path offline |

Golden fixtures must assert both semantic results and evidence envelopes.
Historical run evidence remains read-only.

### 10.1 Line forecast

Before implementation, the production-Rust forecast is frozen as follows. Test
and fixture lines are reported separately at settlement.

| Work | Production Rust forecast | Contents |
|---|---:|---|
| comparator/checkers | 650--1,050 | closed contract, URL/domain/IP validation, robots decision, cache validation, N6 freshness |
| plumbing | 700--1,200 | transport adapter, bounded execution, request caps, atomic snapshot, evidence, minimal catalog/assurance wiring |
| total | **1,350--2,250** | comparator plus plumbing; no autonomous navigation |
| tests/fixtures (non-production) | 1,100--1,900 | contract negatives, local HTTP harness, storage/cache/freshness and chokepoint guards |

Choosing a child-process transport rather than a bounded in-process client may
move work within the plumbing band, but does not remove any boundary obligation.
Settlement must report actual added production lines by comparator and plumbing
using the same counting rule as E-4; the band must not be raised after the fact
to admit the implementation.

### 10.2 Three-category preflight and residual floors

The implementation checklist is applied before the first campaign:

| Category | Preflight items | Predicted residual machine floors |
|---|---|---:|
| transmission | closed loader; exact authorization handoff; caps/timeouts/cancellation; atomic evidence; snapshot-to-N1 wiring; direct-dispatch audit | 1--2 |
| semantics | canonical URL/byte identity; robots status matrix; cache timestamp rule; failure attribution; N6 clock boundary | 2--4 |
| stage design | Gate 1 card persistence; no LLM network surface; offline N1--N5 phase; activation/acceptance mapping | 1--2 |
| total | checklist applied before measurement | **4--8** |

Contract revision is tracked separately from machine floors; the predeclared
forecast is 0--1 contract revisions. Initial calibration should reserve 5--10
campaigns across allow/deny, live/cache, fresh/stale, timeout/oversize, and Gate
1 authorization cells. These are forecasts, not permission to weaken honest
failure or redefine a fixture after observation.

## 11. Adjudication required

Review must explicitly decide the following before implementation:

1. whether the freshness sibling is fixed as `N6 ingest_fetch_freshness`, and
   how fetch failure/N6 violation maps to the final-verdict vocabulary;
2. whether the closed TOML shape and exact-domain/exact-URL authority model are
   accepted, including Gate 1 being unable to expand domains;
3. whether HTTPS-only, HTTP-200-only, no retry, and reject-all-redirects are the
   correct v0 defaults;
4. whether robots 404/410 may allow while 401/403 denies and transport/parse
   uncertainty fails closed, plus the product user-agent/contact value;
5. whether same canonical URL plus UTC date is the cache key and corruption
   must fail without implicit refetch;
6. whether acquisition time alone is authoritative for the v0 freshness check
   and where `freshness_max_age_seconds` is owned;
7. whether to use a bounded child transport or an in-process client, while
   preserving the single-boundary and audit-guard invariants;
8. whether the line bands, 4--8 residual-floor forecast, and 5--10 campaign
   calibration envelope are accepted.

Until those decisions are recorded, this document remains a design draft. No
production source, live suite schema, runtime event, assurance rule, band, or
historical run record is changed by F-C-1a.
