# Issue #456 implementation

Create-origin automatic Recovery now binds its initial investigation to read-only steps and retains the original goal, concrete diagnosis, API target, and related definition locations after generated steps are replaced.

## Changes

- Capture the actual executing UltraPlan intent in the existing run-authority leaf. Capture typed handoff data separately from model-generated StepPlans.
- At treatment startup, persist a new read-only inspection context with original intent, goal, diagnostic text, allowed source paths/ranges, and the exact completion-contract hash. The existing fix-origin evidence is neither fabricated nor replaced.
- Discover related JS/TS files from validated handoff targets and local imports, including `@/` imports. Canonical confinement, normal task exclusions, credential names, protected contract paths, bounded file sizes and a bounded relation set apply before suggested source reads.
- Replace premature implementation/build steps with host-owned inspection. For JS/TS targets, inspection tool dispatch permits scoped, ranged Read calls and rejects Write/Edit, Bash, glob/grep and unrelated/protected/external paths. Other languages retain existing workspace read permissions and inspect mutation guards. Read completion needs no mutation. Profile post-step repair is suppressed for these inspections.
- Generate contiguous short ranges that fit compact tool results. Export boundaries retain small interfaces together; large definitions use multiple windows. The fixture's Member range includes `role: string`, and store API signatures and response contracts survive the actual conversation result path.
- Keep the registered final-success command set and its host execution. Repair uses existing editing and protection policies. Existing verification and promotion gates remain unchanged.
- Split large diagnostic instructions into inspect steps within the existing 2,500-character and 12-step limits. No diagnostic truncation or guardrail increase is used to admit them.

## Continuations and compatibility

Each `recover` execution validates its saved context before initializing the new capture. Continuation candidates carry the validated context as well as the original intent. This preserves the original goal/diagnosis even when a later transaction uses the original workspace config, as anticipated by #458. Source ranges are recomputed against the later treatment; changed contracts or conflicting context intent are rejected.

No provenance means no new authority. Manual Recovery YAML, unsupported intents and old fix-origin-only executions retain their previous production-start behavior. Old fix-origin-only executions still receive the old fix binding. Missing contracts/registered commands receive no new inspection context. Invalid new context remains an error. This change does not enable a second automatic attempt or alter retry classification.

The strict read set is limited to validated existing targets with `js`/`jsx`/`ts`/`tsx` extensions because the reused import resolver only understands JS/TS. Other languages (or missing targets) receive non-exhaustive suggestions and retain normal workspace reads, including related files absent from `required_paths`. This policy flag is persisted and inherited without upgrading unscoped continuations. A Python/Rust corpus regression exercises create and fix through real driver startup and inspection dispatch, reads helper definitions, rejects inspect Write, and reaches repair. Outside/private Read and repair-phase protected Edit rejection are verified separately through the actual tool registry.

Non-text assets listed in `required_paths` do not produce textual ranges or fail context binding. Their completion-contract requirements are preserved. The same language regression includes a required binary asset.

## Evidence and limits

The new corpus preserves 14 archived source/config files and the original Next.js completion contract byte-for-byte; `provenance.json` records their hashes and the archived central event hash. Package-lock/dependencies and historical runtime data are not copied into the fixture. The three required reports and the new fixture are the only new evidence; the referenced develop worktree and archives remain unchanged.

The original Next.js contract replay reaches `repair-unknown` after read-only inspection with identical treatment/control product hashes, no successful mutation/Bash calls, no dependency/build lifecycle, and no write-required escalation. It intentionally ends when planner responses for repair are absent, before a real Next.js build or model repair evaluation.

A separate deterministic replay uses the same API/store/types source with an explicit generic profile and a declared annotation check. It traverses the real transaction start, generated-plan binding/lint, final execution prompt, actual Read/Edit tools, registered host verification and transaction finish/promotion. Adding a failing registered command rejects the edited candidate and preserves control. This proves phase/edit/gate reachability, not full repair of the archived application's 33 TypeScript diagnostics or its UI/API business contracts.

No live model evaluation, Browser operation, push, PR, merge, release publication, or Issue state change was performed. Live model repair efficacy remains unmeasured and belongs to #452.
