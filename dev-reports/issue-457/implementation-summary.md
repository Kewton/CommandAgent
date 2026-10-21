# Issue #457 implementation

Based on verified predecessor `32f997f4` imported by fast-forward; implementation is
on `feature/issue-457-nextjs-recovery-ui-api`. Original archives and runtime state
were read-only references.

After a Next.js type failure, the build verifier runs the already installed
TypeScript checker with `--noEmit --incremental false --pretty false`. It uses the
existing bounded, normalized verifier environment, never downloads a toolchain, and
keeps the original build failed even when supplementary checking succeeds or cannot
run. Its command binding explicitly uses `NormalizedVerifyCommand`, preserving the
normalizer/executor boundary and passing the existing protection audit without an
allowlist or baseline change. The original frame remains; additional plain TypeScript diagnostics are
retained beyond the legacy five-frame extraction. Executor summaries are excluded
from path parsing and colocated distinct diagnostics remain distinct.

Repair context now reports diagnostic counts by file and groups local named imports
by shared declaration file. Source reads are bounded and workspace/path-policy
checked; the map does not copy imported file contents or grant extra editing
authority. It identifies candidates for inspection rather than asserting independent
root causes. Normal and compact prompts retain this context, existing source-frame
guidance and registered verification commands.

The final response contract check now follows the exact local checked JSON wrapper
used by the archive. A disjoint literal update payload fails only when body reads are
exhaustive and every observed successful return is guarded by a supplied-key check.
Aliased destructuring is handled. Shadowed helper names (including parameters),
unknown transformations/returns, mixed direct/computed body access and default-only
handlers stop inference. Guarded optional directory reads only add repair guidance;
they do not introduce an acceptance rejection. Existing generation rules already
cover shared types, HTTP shapes and reload, so knowledge.toml was unchanged.
The new request check runs at final verification and also supplies repair guidance.
It does not become a new intermediate invariant: that would trigger the existing
configuration-repair path immediately after read-only Recovery inspection, before
the authorized repair phase. The unchanged #456 regression verifies source identity
and reachability; existing intermediate checks and all final gates remain intact.

The new corpus reuses #456's immutable archived sources and adds five variants.
Role alone leaves 32 of 33 diagnostics. Shared store result/error types, awaited
mutex results, assignment objects and dynamic project listing yield type/build
success. Separate type-valid variants still fail actual directory selection or
assignment PATCH. The aligned variant provides a documented sample team through
the existing GET and uses Member objects/null across UI, API and store. HTTP errors
are strings, distinct from structured StoreError. All six HTTP operations are
checked against damaged storage for 5xx string errors and no overwrite; the UI shows
readable errors. No API/required feature was removed and no any/ignore/type bypass
was added to make the archive compile.

Ten strict-TypeScript ambiguity controls protect valid defaults, parameter shadowing,
mixed body access and optional request fields. Rust regressions exercise the real
profile verifier. An explicitly invoked toolchain regression observes the actual
Next build and demonstrates one original frame versus 33 diagnostics across four
files, with API/store/types imports and verification commands in both repair prompts.

The independent browser matrix executes the compiled page/API/store, not a mock of
the business flow. It checks list, selection, task creation with assignment,
reassignment, reload, unassignment, status filtering, deletion and storage errors.
Browser settings, preflight read/input/screenshot, results and selected screenshots
are retained in this report directory.

Runner chokepoints, guardrail baselines, event schemas, final acceptance and candidate
promotion conditions are unchanged. Existing R0 gains no new required JSON storage
or oracle. These results establish deterministic repair support and fixture behavior;
live-model improvement remains unmeasured for #452. No push, PR, merge, Issue mutation,
shared-service operation, or historical evidence rewrite was performed.
