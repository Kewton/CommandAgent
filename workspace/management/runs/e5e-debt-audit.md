# E-5e residual-debt audit

Date: 2026-07-30  
Baseline: `c3abfade17c23830fe9aceabc3650eda954990c1`

## Dev-server port determinism

### Mechanism

The two measurement contaminants were not caused by an assertion accepting an
occupied port. The runner test helper allocated from a process-global
`AtomicUsize`, beginning at port 34011, checked the chosen port, and released
the check socket before the child server bound it. The E-2b A-B-A record
observed `EADDRINUSE` on that predictable port. Three runner tests that start
the same fake dev-server family also omitted the existing lifecycle mutex.
This combined a reusable global sequence, a bind/check/drop TOCTOU window, and
parallel process lifecycle.

E-5e removes the global sequence. Every runner test now asks the OS for an
ephemeral port through `bind(("127.0.0.1", 0))`; the reservation helper owns
the listener while a test prepares its inputs, and ordinary callers release it
only when returning the selected port. The three previously unguarded
dev-server lifecycle tests now use the same lifecycle mutex as the other
runner dev-server tests. Production port-in-use detection remains unchanged.

### Allocation inventory

`rg -n "TcpListener::bind" src tests -g '*.rs'` found 24 call sites before
this change.

| allocation shape | sites | disposition |
|---|---:|---|
| Listener retained by an in-process or ignored-child fake server | 15 | Already deterministic: the OS reservation remains owned for the server lifetime. Includes conformance, TUI, provider, browser/interaction, and runner child fixtures. |
| OS-ephemeral lookup followed by release for a subprocess | 4 | Three existing helpers already used port 0. The runner helper is changed from shared Atomic allocation to this shape. |
| Deliberately released endpoint used to test “unreachable” behavior | 3 | Intentional negative fixtures; an unexpected new owner yields the same honest connection outcome and is not a dev-server success fixture. |
| Availability probe against a caller-specified port | 2 | Production preflight and the former runner double-check. The runner double-check is removed with the Atomic allocator; production preflight remains an intentional observation boundary. |

After the change there are 22 bind sites: the deleted Atomic helper’s second
availability bind and fallback allocation bind disappear. There are no
fixed-range or Atomic dev-server test allocators. The conformance fixture’s
port 3011 is retained deliberately because it verifies the public default-port
contract inside one matrix scenario; changing it would weaken that contract.

### Decision

Disposition **(a), small deterministic change**. OS-assigned ephemeral
allocation plus runner lifecycle serialization removes the known repeatable
collision source without changing dev-server verification or production
failure semantics. No EADDRINUSE debt remains queued for the runner test
allocator. The public default-port conformance remains an honest external-port
conflict detector, not a free-port allocator.

## Production panic boundary

### Scope and method

The inventory covers production code in:

- adjudication: `src/planner/adjudication/`;
- projection: `src/completion_metadata.rs` and
  `src/completion_metadata/`;
- comparator/runtime leaves: the data, CLI, and ingest profile modules;
- registry: `src/planner/profile.rs` and `profile_behavior.rs`.

The search is `rg -n '\.(unwrap|expect)\('` with `#[cfg(test)] mod tests`
sections and dedicated test modules excluded. Non-panicking APIs such as
`unwrap_or`, `unwrap_or_else`, and `unwrap_or_default` are not counted.

### Inventory and adjudication

| layer | initial sites | allowed static/infallible invariant | locally guarded but queued for typed conversion | run-killing conversion now |
|---|---:|---:|---:|---:|
| Adjudication | 1 | 0 | 0 | 1 |
| Completion/assurance projection | 0 | 0 | 0 | 0 |
| Data/CLI/ingest comparators and their leaf parsers | 26 | 18 | 8 | 0 |
| Profile registry and embedded manifests | 6 | 6 | 0 | 0 |
| **Total** | **33** | **24** | **8** | **1** |

The converted site was
`src/planner/adjudication/create.rs:evidence_failure_reason`. A preferred
release-evidence failure reason was first proven present and then extracted
with `unwrap()` at the adjudication boundary. It now uses one exhaustive
`Option` match: the same reason is returned byte-for-byte, while absence stays
on the existing HTTP/fallback path. This removes the only panic-capable site
from the run-level adjudication and projection/registry boundaries.

The 24 permitted sites are shipped constant definitions: static regular
expressions, JSON-string serialization, and embedded fixed manifests validated
by conformance/manifest tests. Their failure means a programmer changed a
repository constant and is appropriately caught by review and CI rather than
classified as a model run failure.

The eight locally guarded sites are:

- regex capture zero after a successful capture (3);
- the sole year after an exact `len() == 1` check (1);
- a CLI placeholder after the same predicate returned `Some` (1);
- CSV iterator/field access after `peek().is_some()` or a non-empty seed (3).

None is reachable merely through malformed model/workspace input without its
local proof first holding. They are nevertheless implementation assertions,
not permitted run-boundary panic policy. Converting all eight to typed
violations is **QUEUED** as the bounded remainder of the wider unwrap
eradication; it is not required to remove the only upper-layer run-killing
site.

### Policy

The durable boundary is documented in
`docs/dev/panic-boundaries.md`. In short: repository-owned static definitions
and tests may panic; adjudication, projection, registries resolving runtime
input, and comparator paths handling model/workspace/evidence data must return
typed failure. A local proof can explain residual risk but does not expand the
permitted panic layer.
