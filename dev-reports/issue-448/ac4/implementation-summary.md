# AC4 follow-up implementation

Closed the post-CI coverage gap with an additional bounded finish/promotion
matrix using the byte-identical original Next.js completion contract:
`eb2b04647a28ab10508c21e9c1cd026bea293a1c7f95dda261f4bd5529d8c9ab`.

The positive treatment combines the existing R0 UI candidate with the existing
export/Promise repairs, exclusively in disposable workspaces. All 14 inputs
are hash-checked; the only four source differences are the existing page,
status-helper and projects/tasks collection-route overlays. The original
Next.js profile, Japanese business goal, port 60302, `npm run build`, eight
required paths, three capabilities, eight evidence requirements and
implementation obligation remain unchanged through control, treatment,
observation and final decision. No API or requirement is removed and no `any`
or compiler suppression is added.

The actual `RunnerRecoveryDriver::finish` runs isolated preflight, the Next.js
browser observer, registered build verification, source acceptance and normal
promotion. In the positive case both real builds pass, all eight static evidence
tiers are strong and all missing-requirement lists are empty. Control becomes
exactly the treatment. The five negatives preserve the entire control hash:
UI-only compile failure, failed interaction, missing interaction, HTTP 500 and
API-only repaired scaffold. The last four compile successfully; the scaffold
also passes its scripted route/interaction observations but still fails the
original implementation obligation.

`issue448_nextjs_contract.py` reruns the same matrix with real builds against
the original lock. The default Rust test uses source-bound measured build
replays. Both modes deliberately script only the HTTP/interaction responses
through existing cfg(test) input seams and the actual Next.js observation path.
Final interaction evidence is produced by the normal parser/serializer. These
are conditional promotion tests, not real browser/model/business acceptance.
The persisted record retains `probe.child_spawned: false` for interaction and
explicit fixture provenance; the native HTTP child is actually spawned/reaped.

The new `issue448-nextjs-promotion` corpus holds provenance, reproduction steps,
input fixtures, real observations and its own hash manifest. Additional records
in this directory show final real rerun and fast-replay agreement. Nine new
Python tests bind the records to exact source/contract bytes and reject
incomplete or false-pass records. The only shared Rust wiring is five cfg(test)
registration lines in `src/planner/auto_recovery.rs`. This overlap was reported
to the parent. #449's entire existing R0 corpus remains byte-identical to
`3952f99ff3b2afde40b8d4e88380141044815836`.

Production behavior, audit baselines, original evidence, live runtime state,
PR453 and remote branches remain unchanged. Runtime lock deadlock and data
loss were not observed; type repair does not prove mutual exclusion or
persistence correctness.
