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

Pending commit 3.
