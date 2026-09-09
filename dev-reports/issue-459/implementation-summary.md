# Issue #459 implementation

The archived new-R0 create failure now has an integrated recovery corpus that
executes real reads/edits, registered verification, Browser acceptance and
production candidate decisions. The exact #457 atomic helper remains an honest
rejection control; a semantically equivalent repair with explicit stable rename
destinations exercises successful production adoption.

## Changes

- Reuse the immutable #456 source archive and #457 dependency lock, diagnostic
  controls and repair overlays through a SHA-256 manifest. Add seven source
  variants and nine executable Recovery scenarios. Compiler counts 33/32 are
  diagnostic counts, not independent defects.
- Add an external Rust integration test against the normally compiled library.
  Only provider replies are deterministic. The test enters actual create
  verification failure, follows the generated child, and uses real Read/Edit/Bash
  tools and the production binding/finish/adoption path. It does not inject
  execution outcomes, successful events, browser results or acceptance states.
- Supply real ranged Reads per bound inspection step, including split diagnostic
  steps. Assert retained goal, diagnosis and API/store/types definitions in the
  actual execution-provider messages of each inspection attempt. Require no
  inspection mutations/builds, real repair edits, and matching candidate hashes.
- Keep the original contract unchanged in original-contract scenarios. Five
  explicitly stronger fixture contracts also register the identical installed
  strict tsc and a protected Node test using the real assignment/reload oracle.
  Missing lists, wrong update fields and an unexecuted oracle fail those same
  registered commands. No requirement or acceptance condition is removed.
- Assert exact attempt counts, stop and finish reasons, actual promotion events,
  control retention before continuation, fresh treatment bytes, all source/data
  hashes after rejection, and promoted control identity with the checked candidate.
  The replay summary retains `recovery_plan_auto_run_complete` for real successes.
- Add a small product leaf recognizer for a real Node fs rename's provable complete
  destination argument. Existing lexical/path/protected/source/symlink restrictions
  still apply. Opaque parameters, dynamic destinations and temporary UUID paths
  receive no authority. Source deletion and unrelated writes still fail the
  existing snapshot gate. No runner chokepoint, event schema or baseline changes.
- Preserve atomic temporary-file replacement and generation checks in the positive
  fixture. Both stable destinations are spelled out in actual rename branches;
  the unchanged fallback handles other paths. The existing #457 source is not
  modified. Both variants undergo the same type/build and functional matrix.

## Scope and evidence

The corpus README contains commands, expected outcomes and dependency versions.
`fixture-results.json` records the independent type/build/business matrix;
`replay-results.json` and `inspection-evidence.json` record actual Recovery
candidate decisions and compact provider-message evidence. Selected browser
screenshots supplement the machine assertions. Temporary runs retain full
requests/events/logs; raw logs and runtime state are not committed.

The generated create artifact is pre-materialized from the original archive;
its earlier 487-event conversation is not re-executed. Create failure and all
subsequent Recovery tools/gates are newly executed. Replies are authored for this
regression, not a previously successful live-model conversation. Fixed Next.js
CPU limits and dependency links are documented fixture infrastructure.

The sample member directory is #457's explicit fixture specification. A new
live R0 must decide its own member source; unexecuted or unspecified functionality
is not passed by these results. Replay demonstrates integration, not improved
model repair ability. #452 retains the authorized new R0, nine comparisons and
B-1 sequence. R0's actual original contract, installed toolchain, member oracle,
Browser availability and final adoption prerequisites must pass before those
comparisons begin. An unchanged opaque atomic helper can still be rejected.

Verified #456/#457/#458 commits were inspected and imported by fast-forward only.
The parent is `008f4f5d4c1eab6697f015c6d304401e055beb3e`. The designated develop
root and historical sources/evidence were read only. No live model campaign,
push, PR, external merge, Issue state change or live `.anvil/` change was made.
