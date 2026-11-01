# `artifact_contract_check` — operator-registered adapters bound to final acceptance

Issue #933. This capability runs one external adapter that the **operator**
fixed in advance and binds its result to final acceptance through a generic
envelope. The product only understands protocol names, request/check/target
IDs, input digests, statuses, counts, and bounded diagnostics. It does not
know any adapter's field names, ID rules, plans, tables, suites, DSL, or
domain verdict. Domain logic stays in the adapter.

Code: `src/planner/artifact_contract_checks/` (`mod.rs` registry, receipt, and
host checks; `process.rs` the child process; `protocol.rs` the envelope) and
`src/planner/capability_catalog/artifact.rs` (catalog entry).

## Supply

| Who | Owns | Where |
| --- | --- | --- |
| Operator | Registry root, descriptors, executable, tooling root, members, links, required rows, initial input pins, limits | `COMMANDAGENT_ADAPTER_CONFIG_ROOT`, set before the run, outside the workspace |
| Profile or pack author | That a final check uses `adapter_id` with a `request` path | manifest `[[checks.*]]` or local draft pack `eval.yaml` |
| Model | The request file and the inputs it names | the workspace |

The catalog params are closed: `adapter_id` (a lowercase slug) and `request`
(a workspace-relative path). An executable, argv, module URL, or model cannot
be written in a manifest, pack, or request. The capability is final
acceptance only. A pack may add it only as an opt-in additive check of a local
draft profile with intent `create`; compiled profiles do not admit it.

Descriptors live at `<root>/adapters/<adapter_id>.json` with schema
`artifact-contract.adapter/v1`:

```json
{
  "protocol": "artifact-contract.adapter/v1",
  "adapter_id": "example-lines",
  "executable": {"path": "/abs/realpath/to/runtime", "sha256": "<64 hex>"},
  "tooling_root": "/abs/realpath/to/tooling",
  "entrypoint": "adapter.mjs",
  "args": ["--fixed"],
  "members": {"adapter.mjs": "<64 hex>", "package.json": "<64 hex>", "pnpm-lock.yaml": "<64 hex>"},
  "links": {"node_modules/@scope/lib": "packages/lib"},
  "required": [{"check_id": "lines/total", "targets": ["doc#1"]}],
  "optional": [{"check_id": "review"}],
  "inputs": [{"input_id": "plan", "sha256": "<64 hex>"}, {"input_id": "declaration"}],
  "limits": {"timeout_seconds": 30, "max_stream_bytes": 24000}
}
```

The operator records the fingerprints of source, dist, lockfile, and contract
documents as `members`. Nothing installs, builds, or updates tooling
automatically; an unknown or mismatching toolchain is unavailable.

## Trust boundary

1. **Registry.** The root is resolved with `realpath`. While the manifest or
   pack loads, before any provider is built, the host refuses an unset or
   relative root, a missing or symlinked `adapters/` directory or descriptor,
   an unregistered ID, an unknown descriptor field, a repeated `members` or
   `links` key, an invalid value, and operator tooling that is missing or does
   not match its executable pin, member pins, or links (including any
   unregistered symlink in the tooling tree). The load path does not know the
   workspace, so the separation from the workspace is checked by
   `artifact_contract_checks::preflight(workspace, profile)` for the adapters
   that the run's own profile and effective pack declare, taken from the same
   declarations final acceptance runs: the effective profile's registered
   manifest, re-read only as the exact bytes that were loaded (no symlink, no
   FIFO wait, the loader's 256 KiB bound, and the registration hash; a
   manifest changed since loading is refused), and the effective pack
   selection (the CLI's `--pack` or one inherited through
   `COMMANDAGENT_PACK_*`), verified for identity, exact-byte hash, and pin as
   final acceptance verifies it. Only `artifact_contract_check` entries under
   the top-level `checks` bindings count, and a malformed declaration is an
   error. Other loaded or previously registered profiles are not consulted.
   The CLI calls it once the workspace, profile, and pack selection are known
   and before any provider is built (`src/lib.rs`), and a workflow child node
   calls it again for its own origin workspace and profile before its
   provider is built; a refusal exits with code 2 (a workflow child node
   fails). The pack side of the preflight does not check the pack's
   profile/intent fit or run conformance; those are checked when final
   acceptance loads the pack, which refuses a mismatched pack, so the
   preflight may verify a superset of the adapters final acceptance runs.
   What the preflight reads, separately:
   - The operator registry (and adapter descriptors and tooling) only for the
     adapters that the profile or the pack selection declares. With no such
     declaration it never touches the registry.
   - The pack selection variables, always; and, when a pack is selected, that
     pack's members (`assist.yaml`, `eval.yaml`, `materials/*.md`, and
     `pack.sha256`) whether or not they declare the check, because the
     declaration can only be known by loading the verified pack. Every member
     is read through the shared safe read: opened with
     `O_NOFOLLOW | O_NONBLOCK`, required to be a regular file by the opened
     descriptor's metadata, and read with the loader's existing byte limits:
     `MAX_FILE_BYTES` for `assist.yaml` and `eval.yaml`, 4 KiB for the pin,
     and for materials both the 65,536-byte per-file limit and the
     262,144-byte aggregate limit. Every one of these limits is enforced on
     the bytes actually read, not only on the earlier `lstat` size: each
     material is read with at most the smaller of its own limit and the
     aggregate budget left by the materials read before it. A member swapped
     for a FIFO or a symlink after the loader's `lstat` check fails at once
     (`NotRegularFile`); a member that grew past its own limit after the size
     check fails as `TooLarge` or `MaterialTooLarge`, and materials that grew
     within their own limits but together past the aggregate fail as
     `MaterialsTooLarge`, before any of those bytes are kept.
   - The effective profile's registered manifest, only when that profile
     declares the check, through the same safe read and hash check.
   At run time the descriptor is read again, and a
   registry,
   descriptor, tooling root, or executable that resolves inside the workspace
   is refused as self-approval.
   There is no command, tool, or file in the workspace through which the model
   can add or update a descriptor, pin, or expected input hash.
2. **Executable.** The descriptor names an absolute realpath and its SHA-256.
   There is no PATH lookup; a workspace `node`, `node_modules/.bin`, or other
   file is never a candidate. The working directory is the tooling root.
3. **Tooling.** Every pinned member is hashed. Every allowed link must be a
   symlink whose realpath equals its expected package root inside the tooling
   root, and the tooling tree may hold no other symlink. The executable,
   members, and links are verified right before the spawn and again after the
   adapter exits, so replacement is detected. This is not a complete operating
   system TOCTOU seal: a change that happens and is reverted inside the run is
   not observable.
4. **Environment.** The child starts from an empty environment holding only
   `LANG=C`, `TZ=UTC`, and `COMMANDAGENT_ADAPTER_TOOL_ROOT` (the tooling root).
   `HOME`, `PATH`, `NODE_PATH`, `NODE_OPTIONS`, and every API credential are not
   inherited. The launch does not use `verifier_env` or
   `bounded_process::spawn_child`, which restore parts of the parent
   environment. Keys the operating system or its runtime add on their own
   (macOS `__CF_USER_TEXT_ENCODING`) are recorded separately as
   `platform_env_keys`.
5. **Inputs.** The request names inputs by opaque ID and safe workspace-relative
   path. Parent or absolute components, symlinks, and credential paths are
   refused, and the request must name exactly the registered input IDs. The
   request cannot carry an expected hash: initial-input pins come only from the
   descriptor. The host hashes every input before the spawn and again after it,
   and a change fails the run. Each file the host hashes or reads (inputs, the
   request, the descriptor, the executable, and tooling members) is opened with
   `O_NOFOLLOW | O_NONBLOCK` and must be a regular file by the opened
   descriptor's own metadata, and inputs are re-resolved component by component
   after the run. An input swapped for a FIFO, device, or symlink therefore
   fails promptly instead of blocking the host. A directory component above
   the file can still be swapped in the time between the component checks and
   the open: the checks shorten that interval but do not close it, and this
   is a statement about which bytes are read, not a wait bound (no read here
   waits on a special file either way). A hash first taken after generation proves which
   bytes were checked, not that they were sealed before generation.
6. **Limits.** Request and stdin envelope 16 KiB; each stream 24,000 bytes,
   enforced while reading; 30 seconds; at most 512 rows, 64 diagnostics (16 per
   row, code 64 bytes, message 512 bytes), and JSON depth 16. Hashed inputs
   are at most 64 MiB each, hashed tooling files at most 1 GiB each, and the
   tooling tree at most 200,000 entries and 64 levels. On overflow,
   timeout, or cancel the whole process group is stopped and the leader is
   reaped. The group is stopped after a normal exit as well.
7. **Network and models.** Not calling a network, a model, Jev, or a factory is
   part of the trusted adapter's contract. A hash fixes code identity; it does
   not restrict what that code can do. This first version provides no operating
   system network isolation and records `network_isolation: not_provided`.
   Hiding model calls inside an adapter is not permitted.

The existing `command_check` allowlist, environment, and semantics are
unchanged.

## Envelope

Stdin (`artifact-contract.envelope/v1`) carries the request ID, a fresh
per-run nonce, the workspace path, and each input's ID, path, and host digest.

Stdout must be exactly one `artifact-contract.result/v1` document:

```json
{
  "protocol": "artifact-contract.result/v1",
  "request_id": "<from the request>",
  "nonce": "<from the envelope>",
  "input_digests": [{"input_id": "plan", "sha256": "<64 hex>"}],
  "checks": [
    {"check_id": "lines/total", "target_id": "doc#1", "status": "passed",
     "counts": {"expected": 1, "executed": 1, "passed": 1, "failed": 0, "unresolved": 0},
     "diagnostics": [{"code": "line-total", "message": "3 lines"}]}
  ]
}
```

Unknown or duplicate result fields are refused, so an adapter cannot report
its own domain verdict. The host requires:

- the same protocol, request ID, and run nonce (a replayed or stale report
  fails), and input digests equal to the host-observed digests;
- every registered required `(check_id, target_id)` row exactly once, no
  unregistered row, and no required row with `not_provided`;
- `executed = passed + failed + unresolved`, `executed <= expected`, and a status
  consistent with its counts. `passed` needs `executed > 0` and, when
  `expected` is given, `executed = expected`; a check that did not run is
  `not_run` and is never a zero-test success;
- exit code 0 exactly when every required row passed and no row failed, and 1
  otherwise. Any other exit, a signal, a timeout, an overflow, or a cancel is an
  execution failure.

Exit 0, a `PASS` line, or rereading a report the adapter wrote earlier never
passes on its own.

## Verdict

Rows give generic facts: M (a row `failed`), a required row `unresolved`, a
required row `not_run`, and an optional row unverified (`unresolved`,
`not_run`, or `not_provided`, for example an intentionally unprovided review).
A registered optional row the adapter leaves out entirely counts as unverified
too, and the receipt records how many were left out (`optional_missing`).

| Condition (first match wins) | Domain | Native final acceptance |
| --- | --- | --- |
| Pin mismatch, spawn/timeout/overflow/cancel, invalid envelope, changed input or tooling | none | failure |
| Any row `failed` (M), with or without unresolved rows | none | failure |
| A required row `not_run` | none | failure |
| A required row `unresolved` | partial | failure |
| Optional row unverified, every required row passed | partial | pass |
| Every row passed | partial (first-version cap; uncapped full) | pass |

`full` is never emitted: every verdict passes through the `partial` cap, and
the receipt records `domain_cap_applied`. A known mismatch is `none` even when
unresolved rows coexist; this is stricter than an adapter that would report
`partial` there, and is not claimed to be the same rule.

The domain verdict is a receipt only (`native_effect: failure_only`). A native
failure is pushed into the existing `VerificationReport`; nothing is removed
from it, so an earlier native failure keeps its reason. External profiles stay
`draft`: the existing admission cap still turns earned `full`/`partial` into
`static` with `profile_not_admitted`, and keeps an original `static`,
`failed`, or `reduced` reason unchanged.

## Receipt

Each run emits `artifact_contract_check_result` with `source`
(`draft_profile` or `local_pack`), `owner_id`, `adapter_id`, `request`,
`request_id`, `status`, `stopped_at` (`registry`, `trust`, `pin`, `request`,
`input`, `spawn`, `binding`, `process`, `envelope`, or `verified`),
`descriptor_sha256`, `executable_sha256`, `input_digests`, the process outcome
(`ended`, `exit_code`, `elapsed_ms`, `reaped`, `group_stopped`, stream sizes),
`child_env_keys`, `platform_env_keys`, `network_isolation`, row facts,
`domain_verdict`, `domain_cap`, `domain_cap_applied`, `native_effect`, and
bounded reasons, plus one summary line. No absolute host path is recorded. The
receipt is never written into the workspace, so a profile's required
artifacts cannot depend on host output that is produced after them. Each run
uses a new nonce; an earlier receipt is never reused for a newer candidate. An
operator who deliberately selects an older version issues a new request, which
runs afresh and is bound to the selected bytes.

## Accepted diagnostics in the repair context and the receipt (Issue #955)

An accepted result's diagnostics are carried as untrusted data, never as a
verdict.

- **Origin.** Diagnostics are taken only after the existing validation
  accepted the result (registry, tooling, process, envelope, input digests,
  nonce, row coverage, counts, exit code). A refused, timed-out, cancelled,
  overflowing, or misbound run, and raw stdout/stderr, carry none. The wire
  bounds are unchanged: at most 16 diagnostics per row and 64 per result, a
  non-empty code of at most 64 bytes, a message of at most 512 bytes; a
  result over any of them is still refused, never cut to fit.
- **Rows.** Only the rows the reasons already name carry diagnostics: a
  `failed` row, or a required row that did not pass (`unresolved`,
  `not_run`). Passed rows and optional unverified rows carry none, and no row
  the adapter did not report is invented. A message stays an opaque string;
  the host parses no field path from it and derives no repair template.
- **Order of processing.** Each display string (check, target, code, message,
  and the receipt's source, owner, adapter, request, and request ID) is
  handled as one complete decoded value: first the existing credential scrub
  (registered values, secret-like tokens, home paths), then control and bidi
  characters become visible `\u{..}` text, then `&`, `<`, `>`, and the
  backtick are written as `&amp;`, `&lt;`, `&gt;`, and `&#96;`. Nothing is cut
  or stripped before the scrub. A secret split across several fields is not
  reassembled; the guarantee is per field.
- **Receipt.** `artifact_contract_check_result` gains one additive field,
  `adapter_diagnostics`: `{source, owner, adapter, request, request_id,
  items: [{check, target?, row_status, code, message}], omitted}`. It is
  absent when there is nothing to carry, so such an event keeps its earlier
  shape byte for byte. Its `"adapter_diagnostics":{...}` member holds at most
  16 items and 8,192 serialized UTF-8 bytes (keys, ownership, `omitted`, and
  escapes included). The same projection is used for the local pack check.
  Existing fields, reasons, counts, and the summary line are unchanged.
- **Repair context.** The manifest-driven profile returns the receipt's
  projection in `VerificationReport::artifact_diagnostics` (one group per
  receipt), report merging appends groups without deduplication, and the
  existing bounded final repair adds one section after the profile failures
  in both the Stable and the Legacy prompt layout. The section is a fixed
  label, a line `<artifact-check-diagnostics>`, one line of JSON
  (`{"groups": [...], "omitted": n}`), and a line
  `</artifact-check-diagnostics>`. Because no value holds a raw `<`, newline,
  or control character, a value cannot close the block or start a role or
  fence line. The whole section, label and delimiters included, holds at most
  16 items and 8,192 UTF-8 bytes across every receipt of the report; items are
  dropped whole from the end and counted in `omitted`, and earlier omitted
  counts are kept. A report without diagnostics adds nothing, so the prompt is
  unchanged.
- **Ownership.** The section is built only from the acceptance report of the
  current cycle, in memory. No event file, earlier receipt, other run, global
  cache, or workspace report is read; the rerun's report replaces it for the
  next attempt.
- **No authority.** Diagnostics never change the facts, status, reasons,
  domain, binding, receipt, repair attempts, provider calls, tokens, cost, or
  deadline. Their presence, absence, scrubbing, or omission is neither a
  failure nor a pass. The repair target, expected paths, and verify commands
  are chosen by the host as before. The label states that the data grants no
  instruction or permission; this frames the data, it does not prove that a
  model ignores injected text. The step-plan runner, where local pack checks
  run, still has no bounded repair, and none is added.

## Where the check runs

With the current runners, a manifest `artifact_contract_check` runs in the
profile final acceptance of the UltraPlan runner, and a local pack
`artifact_contract_check` runs in the final acceptance of the step-plan
runner. These are the paths the tests exercise. Other entry points (for
example a plain `--prompt` run) are not verified to run either check.

## Unverified meaning and first-version limits

- `partial` means the adapter's required deterministic checks ran and passed
  under the generic envelope. It does not mean independent review, phase seals,
  or semantic correctness were established.
- No operating system network or file sandbox; the adapter runs with the
  operator's user rights.
- Pin checks run before and after the run; this is not a complete TOCTOU seal.
- A descendant that leaves the process group can escape the group stop; the
  run then fails if it still holds an output stream.
- `full` is not unlocked. Unlocking it, phase gates and seals, independent
  review, shared model budgets, and structured generation are follow-up work.
- The final-acceptance entry has no cancel source today; the 30 s bound applies.
- The request is written to stdin without blocking, in the same loop and
  under the same deadline as the run, and the host closes its stdin end before
  returning on every path. A child or a descendant that holds a full stdin
  pipe therefore cannot hold the host or keep a host thread or descriptor
  alive; a request that did not fully reach the child fails the run
  (`stdin_complete: false` in the receipt).
- GUI jobs start the CLI binary with a cleared environment whose allowlist
  does not include `COMMANDAGENT_ADAPTER_CONFIG_ROOT`. Under the GUI this
  capability therefore fails closed as "registry not set"; it is usable from
  the CLI only.
