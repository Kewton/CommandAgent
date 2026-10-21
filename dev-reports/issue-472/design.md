# Issue #472 design

Ordinary reads of files that have not yet been created must return an actionable
failure to the model instead of ending the implementation or inspection step.
The Read itself remains unsuccessful. Original implementation obligations,
registered verification, read-only inspection and protection rules remain in
force.

The parent developed this isolated candidate from fetched `origin/develop`
`43c6bdf2cdcd592ef994aa99de4bcc96a82d0f5a`, with five cmate-delegate discussions
and reviews by Codex2. The initial design and failing production-loop replay
preceded implementation; their local records are under
`workspace/tmp/0912/missing-read-investigation-01/`. This document consolidates
that design and its reviewed revisions, rather than claiming a separate worker
implemented the candidate.

## Read boundary

`tools/read_missing.rs` captures the original ordinary relative request before
lookup. A valid canonical root, allowed raw and normalized paths, existing real
directory ancestors and protected-input checks are required. The new typed
failure is created only for an actual IO NotFound followed by a successful
recheck of the original root and ancestors. The Read registry supplies the
actually selected path, including suffix fallback, so another target's failure
cannot be attributed to the raw request.

Workspace-contained symlinks, absolute/salvaged requests, protected missing
inputs, policy rejection, permission errors, non-directory ancestors and root
changes retain their prior error behavior. Existing successful reads, including
allowed internal symlinks and directory listings, retain their behavior.
Unix device/inode identity is used for directory rechecks; other platforms do
not receive this additional recoverability. This change does not make the
successful Read/open operation atomic or solve all TOCTOU races.

## Feedback and bounded correction

Typed Missing detection precedes string-based error classification. Existing
`tool_validation_error` events receive the additive `read_path_missing` kind,
and the failed tool result states that no content was read. It instructs an
inspection to report absence; creating a file is conditional on the original
task and the current step's writing permission.

A per-run_session map counts misses for each normalized target. Different
paths, line ranges, display spellings and unrelated successful tools do not
reset it. Only a successful Read of that target clears its count. The existing
two feedbacks/third failure stop remains unchanged. Distinct missing targets
can be inspected once each; existing iteration, artifact-recovery and outer
Recovery budgets remain in force. In particular, max_iterations alone is not
the existing hard provider-call cap when a completion contract grants a
bounded artifact-recovery allowance.

## Structure and verification

Error proof and counting live in new leaf modules. Shared path resolution and
protected matching only expose existing helpers. Path feedback is separated
from the small generic dispatcher without changing text. No growth baseline,
acceptance gate or retry admission list changes.

Focused production-loop and Runner tests exercise Read -> feedback -> Write ->
registered Node verification, Inspect -> Implement handoff, forbidden Inspect
Write, invalid/empty content, false completion, repeats and boundaries. A
thread-local one-shot hook exists only in cfg(test) to delete a selected suffix
fallback between selection and lookup in the actual registry. It is absent
from production builds. Corpus fixtures document these deterministic controls.

This product change does not demonstrate automatic-Recovery promotion or
live-model capability. A new R0 and subsequent comparisons belong to #452/#468
after integration/release validation. #451/#460 external Browser repair and
Issue lifecycle changes are outside this implementation.
