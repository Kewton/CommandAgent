# Issue #475 implementation

The saved route/store shape now registers exactly `data/projects.json` and
`data/tasks.json`, whether those files already exist or need lazy creation.
The original failure had two independent recognition barriers: an opaque
basename diagnostic poisoned the imported path binding, and the concrete output
was behind generic collection/atomic-writer parameter boundaries.

`generic_renames.rs` resolves the complete rename destination backwards through
at most eight local parameter boundaries. Every reference must be a supported
call and every path argument must resolve to an immutable module constant.
Unknown/escaped callers, alias escapes, shadowing/reassignment, recursion,
excessive depth and unsupported expressions refuse the new grants. Module
names and filenames are not hardcoded. Other closure modules are scanned
conservatively; namespace/dynamic imports, Unicode escape syntax, opaque
evaluation and foreign references to the generic helper disable this inference.
This intentionally retains false negatives for unsupported legitimate programs.

The lexer defers only the exact `receiver.basename(identifier)` diagnostic read
and Source proves that receiver is an unshadowed, unmodified Node path import.
The template's value stays opaque; it never supplies an output path. Existing
literal normalization, config/required/protected/source/private filters and
symlink checks still apply. An existing non-directory parent is now explicitly
rejected. Missing real directory components and concrete leaves remain allowed.

The existing snapshot, effect-stage and control/restore implementation is reused
without production changes. Only test-module wiring changes in auto_recovery.rs;
runner, loop, final verification/adoption gates and event schemas are unchanged.
No JSON/data directory exclusion or protected-hash bypass was added.

## Evidence and focused controls

- Corpus preserves saved store/types/routes byte-for-byte with provenance and
  the real pre-repair recognizer output, rather than a fixture imitation.
- Positive and refusal tests call the real RecoveryObservationPolicy entry.
  The parent's escaped `atomic\u0057rite` importer reproduced an unsafe new
  grant before the escape guard and is rejected after it. The normal tasks
  route keeps store in the closure in both runs.
- Real preflight executes the saved TypeScript store on Node 24. Existing and
  missing JSON are allowed only in isolation; an intentional business check
  failure remains failed. Unregistered data, source/config mutation and
  observation errors still reject.
- All 12 existing #467 control/restore matrix cases run again with the generic
  saved store, plus source/config mutation controls. Actual restoration success,
  actual copy failure, invalid-source refusal without restore, and unchanged
  control are distinguished by actual audit fields and hashes.
- The product Next.js observer runs an explicit scripted build/server/HTTP
  transport: import during build/start and GET / preserve bytes; API GET seeds
  missing collections only; POST changes projects. Exact operation hashes match
  the nextjs_capabilities stage. A protected-projects variant rejects the same
  write and audits unchanged control. The interaction intentionally remains
  business-unobserved. This is not actual Next.js compilation/browser UAT.

Historical first operation/writer remains unknown, and the legacy reason
containing `restored` is not used as restoration evidence. The saved R0 goal,
gates, original records and live .anvil were not changed. JSON recognition is
neither original business success nor Recovery candidate adoption. Parent owns
PR/CI, targeted UAT orchestration, merge and integrated release identity; this
worker supplies its commit and local verification only.

Codex2 completed two read-only discussions, both exit 0 with replies from
history and DONE markers. See discussion.md, escaped-import-reproduction.md,
controlled-observations.json and verification-evidence.json.
