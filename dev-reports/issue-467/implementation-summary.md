# Issue #467 implementation

T3's unchanged writer now registers exactly `data/inquiries.json`. Before the
fix, the production `Source::parse` / `writer_paths` entry returned no paths and
marked `process` unsafe. Replacing only `${process.pid}` with `123` returned the
exact destination and removed that taint. The baseline test passed before the
production edit. All nine recognizer files at predecessor `612ec357` also match
the historical `9549f746` repository byte-for-byte. The causal claim is limited
to this reproduced input; it does not establish historical first-write timing.

The new lexer leaf recognizes only the exact three-token `process.pid` read in
a template interpolation. The template itself stays opaque. Assignments,
computed properties, calls, nested mutation, process/import/parameter shadowing,
chdir, unknown destinations and dynamic rename destinations still receive no
authority. Existing route binding, cwd, source/config/private/protected-path,
symlink and exact-output filters remain intact. No directory wildcard was added.

Preflight now wraps every return after checkpoint capture in a control audit,
including preparation failures, isolated source rejection and observer errors.
`recovery_preflight_control_audit` records the isolated result separately from
control state, original and observed hashes, actual restoration invocation,
success/failure, and retention. Both new event types include `observation_id`,
the checkpoint workspace's relative path. Equal control hashes at checkpoint
attempts 0, 64 and 128 remain distinct and each audit joins exactly to its
effects. This identifies observations independently of Recovery usage counts.
Existing event names and reason strings remain
compatible. In particular, the historical reason containing `restored` is still
emitted for isolated rejection, but its accompanying audit explicitly says
`restore_invoked=false` when control did not change. No restored counts or
post-restoration hash are emitted on that path.

Before restoration, a small leaf validates the snapshot's canonical namespace
and digest against the original host checkpoint. Missing, altered and escaped
snapshots are refused without calling the restore API or overwriting control.
After an actual restore, its result must still match that original digest.
The fixture matrix distinguishes source-validation refusal, actual copying
failure, successful restoration, and inability to observe control.

`recovery_preflight_effect_observation` adds source path/hash deltas at the
before-observation, Next.js capability, registered verification and completion
acceptance boundaries. Inventory uses the snapshot's existing confinement and
private-file exclusions. Exact generated-output membership accompanies each
delta; unregistered additions still fail the unchanged source hash gate.
Inventory failures stop preflight and still reach the control audit. These are
coarse host phases: the first write within a phase remains explicitly unknown.

The corpus includes the byte-identical T3 store, an executable structural
minimum with cwd/join/mutex/temp/rename and GET initialization, an audit matrix,
and separately attributed historical/new-replay evidence. Tests execute real
preflight verification in temporary workspaces, including actual filesystem
restoration and failures. Positive output recognition is tested for existing
and missing JSON, with source/config/private/protected and binding negatives.
The business-failure and required-persistence cases cannot become current success.

The parent's already completed, fresh isolated Next.js replay supplies the
build/start/GET/POST stage measurement as requested. Build, startup without HTTP,
and GET `/` without JavaScript left JSON absent; GET `/api/inquiries` created
2,380 bytes with SHA-256
`1185dd6d881d4d4842c6b31bb791a00d8b49bd8ba3f787dd89d2a00f0b640cc8`, matching the
historical isolated JSON. The diagnostic POST succeeded in that new replay.
GUI business operation and restart persistence were not evaluated. Historical
first write stays unknown, interaction_success stays false, and
`persistence_not_evaluated:no_mutation_observed` remains unchanged. Neither JSON
permission nor these deterministic tests promise Recovery initiation, adoption,
or final T3 application success.

No runner/loop growth baseline, retry budget, final acceptance, protection gate,
or historical evidence was relaxed or rewritten. The original develop tree,
source/control snapshots, live .anvil, caches and shared services were preserved.
No live model campaign, push, PR, external merge or Issue lifecycle action ran.
