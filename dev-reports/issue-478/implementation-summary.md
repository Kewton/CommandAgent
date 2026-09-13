# Issue #478 implementation

The saved model proposals now form an executable split in two planner requests:
the original smoke producer retains its exact 587-character instruction and
`smoke-check.js`; `ensure-package-json` retains the complete host Profile duty
and `package.json` as Implement/pass. Neither owner contains the other owner's
full instruction. `node smoke-check.js` and `npm run build` remain separate
Verify/pass steps after the application, smoke and configuration owners.

## Reproduction and evidence

- [pre-fix-replay.json](pre-fix-replay.json) records the actual product Runner's
  expected failure on base `fdc98a9b`: first mixed smoke/package refusal, then
  the lost-original-requirements refusal through the existing three-attempt
  exhaustion. Saved attempts 2/3 are replayed as requests 1/2; request 3 repeats
  saved attempt 3. The source proposals are preserved as corpus fixtures.
- [formation-replay.json](formation-replay.json) records the actual fixed
  Runner's model input, host-augmented proposal, separately retained model/host
  scopes, split input and formed plan. Every step includes its instruction,
  kind, outputs, result and commands; array order records the execution boundary.
- The temporary replay workspace supplies a package manifest, App Router
  entrypoints and a node_modules directory as planner context. These match the
  already-scaffolded shape relevant to formation. No installed dependency,
  successful application build or live-model reproduction is claimed.

The actual current host guidance is 2,237 characters, including restart and
explicit-port duties beyond the saved split owner's 1,893-character prefix.
The original mixed step was bounded to 2,500 characters after augmentation.
The new authority retains the complete host guidance before that truncation and
rejects its loss. An exact guidance prefix can be extended without duplicating
it; the final configuration instruction contains all 2,237 characters.

## Code changes

- `recovery_step_plan_binding/profile_augmentation.rs` extracts the existing
  last-owner augmentation from the driver and returns a host-produced origin
  record. Exact text containment/prefix extension avoids duplicate guidance.
- `recovery_step_plan_binding/formation_scope.rs` retains the original model
  owner separately from host instructions/outputs/result. Model duties continue
  to use the existing strict owner, instruction, output, result and command
  checks. Host duties use the same checks on their own scope. Verify steps with
  no output declarations retain their original ordering after preceding output
  owners. There is no inference of obligation origin from a textual marker.
- `recovery_step_plan_binding/admission.rs` captures that origin on formation
  failure, validates current host duties before closure, and enforces both
  scopes on retries and fallback. Existing event names, `original_scope` shapes
  and schema version are retained. The optional additive
  `original_obligation_sources` field and retry prompt identify the two scopes.
- `setup_step_policy/implementation_duties.rs` and its minimal policy/conversion
  wiring keep broad Next.js Profile
  implementation owners out of package-only Verify conversion and setup
  short-circuit paths. The same three package checks are retained byte-for-byte
  as post-implementation requirements, alongside any original commands. The
  runtime test executes them against a passing package and a wrong-port refusal.
  Both prompt layouts retain the full instruction when
  evaluating the shortcut. Narrow setup conversion remains covered by its
  existing tests. This text classification only disables an optimization; it
  cannot grant preservation authority.
- The driver contains only the capture wiring; its production line count
  decreases. Existing augmentation tests call the extracted leaf directly. No
  guardrail baseline changes.

## Controls and scope limits

Six focused tests cover the saved positive, eight model refusal variants, five
host refusal variants (also through fallback), runtime shortcut policy, forged
model Profile markers, and host truncation on an initially separated proposal.
The source corpus drives the saved inputs and model refusal variants.

Existing #466 closed-contract/owner/tamper controls and #465 related-source
repair, pending-owner, confirmation, no-op, unrelated edit and processing-loss
controls remain in the suite. Recovery contract persistence, hashing, budgets,
classifiers and acceptance gates are unchanged. Historical evidence, live
`.anvil/`, campaign conditions and unrelated worktrees are unchanged.

The comparison proves planner formation and duty retention. Parent-owned
Codex2 review, exact-commit UAT/CI, integrated release version/hash and later
live evaluations remain separate gates. Issue #479's verification formation
work must preserve these model/host boundaries.
