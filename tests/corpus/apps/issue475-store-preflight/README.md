# Issue #475 fixture contract

`historical/` preserves the complete saved store, types and two route files,
including imports/exports, generic calls, ENOENT initialization and error paths.
`provenance.json` binds their original bytes; no original app data is copied or
modified. `baseline.json` records the real product Source recognizer before the
repair, not a rewritten recognizer in a fixture.

The Rust tests call the product `RecoveryObservationPolicy`, preflight and
snapshot/audit implementations. They allow only concrete in-root JSON outputs
whose generic arguments resolve through all local callers to module constants.
Both existing files and missing parents/leaves are covered. Existing parents
must be real directories. Dynamic/escaped/foreign/recursive/over-depth calls,
shadowing, chdir, traversal, outside paths, symlinks, source/config/private and
required/protected paths refuse grants. `escaped-import-route.ts` is the exact
parent-proposed import escape, retained while the normal tasks route keeps store
in the closure.

`probe.mjs` runs the saved TypeScript store on Node 24 as a registered storage
check. It covers allowed writes, unregistered data, source/config mutation and a
failing business-verification contrast. The #467 `audit-cases.json` is reused
unchanged against this generic store, including control restoration success,
copy failure and restoration-source validation refusal.

`observer.mjs` and `interaction.sh` are explicit scripted build/server/HTTP
transport inputs to the real product Next.js capability observer. They exercise
saved storage code, not a real Next.js compiler or browser. Build/start import
and GET / preserve JSON. API GET initializes only missing collections; existing
[] is retained. POST changes projects. Product stage hashes match these measured
operations. Protecting projects rejects the same isolated operation and retains
control. The scripted interaction intentionally reports business unobserved,
so this storage evidence cannot promote a candidate.

Measured operations and stage/control fields are in
`dev-reports/issue-475/controlled-observations.json`. Historical per-operation
writer identity remains unknown; only its nextjs_capabilities stage was recorded.
Runtime evidence directories keep their existing policy; they are not registered
as JSON business outputs. No JSON/data directory exemption or hash bypass is added.
