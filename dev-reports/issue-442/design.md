# Issue #442 design

Implement approved C-1–C-3 against parent `36f73eeb312c105a62ed488ce54e480b9e3c8855`.
No required predecessors are listed; #439/#440 are independent implementation
lanes and prerequisites for later effect measurement, not assumed merged here.
Read the complete frozen issue JSON (no comments), action-items document and
parallel-development plan in the develop worktree, plus dev guardrails.

1. Freeze issue-specific source fixtures and provenance hashes from campaign
   `20260907-1317-standard10`: S1/E1/I2/S3 final artifacts and S3 candidate.
   Keep historical sources/evidence read-only. Add externally runnable HTTP and
   disk oracles for corruption, directory replacement, read-only writes, empty
   data, concurrent creates, inventory aggregation, budget scope, references
   and shift boundaries. Replay copied original handlers and a correct control;
   clearly distinguish handler replay from a Next.js build or live campaign.
2. Put declarative integration/persistence rules in nextjs `knowledge.toml`.
   Make generic create/preset wording goal-neutral while preserving required
   observability hooks and restart semantics. Include the supplied goal's
   entities in the implementation prompt. Keep domain-specific oracle numbers
   outside product knowledge.
3. Add a bounded response-shape analyzer in a leaf module, wired as one extra
   `api_contract` check. Match method and route, local response/body bindings,
   top-level object keys and arrays. Diagnose only statically established
   mismatches; unknown/dynamic shapes do not constitute a successful runtime
   verification. Preserve route/export/Response.ok checks and diagnostic prefix.
4. Focused regression tests cover historical mismatches, matching/error/unknown
   shapes and binding isolation. Run the Space/Breakout/Quiz knowledge matrix,
   corpus, guardrails and conformance, then scripts/ci.sh-equivalent checks and
   Python oracle tests/Ruff. Do not change gate thresholds or event schemas.

Commit only implementation, fixtures and these reports. The orchestrator owns
push/PR/CI/UAT/lifecycle and the merged-binary campaign plus one-factor B-1
experiments. Future primary metrics are type/export build/start failures and,
among started runs, corrupted-data overwrite and lost concurrent writes (target
zero); no unperformed generation effect or score improvement will be claimed.
