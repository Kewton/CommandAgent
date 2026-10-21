# Issue #484 implementation

The saved `node -p "require('./package.json').scripts.dev"` now has an admitted
preclosure refinement based on the original owner's literal `next dev -p 60302`.
The real runner rejects the weak original, then accepts the saved complete
proposal on attempt two. It uses no live model or current artifact to acquire
the expectation.

## Change

- `package_script_check.rs` recognizes one root-package/static-script grammar.
  The refinement reads the same value, uses strict string equality, then prints
  the observed value. Its bounded ASCII literal grammar excludes shell/JS
  escapes, expansions, operators and whitespace that normalization would alter.
- `package_script_formation.rs` captures model and host source plans before
  sanitization, with source file, JSON pointers, owner, acquisition stage and
  SHA-256 hashes. It retains the original commands and registered contract
  separately. Fixed declarations are parsed as a complete finite grammar;
  unknown trailing language is refused, including overrides that omit the key.
  The known saved configuration suffix is supported explicitly. Same-key equal
  literals agree; conflicting literals or host/registered constraints refuse.
  Host port-only guidance restricts an existing literal and cannot supply one.
- The existing equivalence projection admits only the exact output-preserving
  replacement. Additive event fields record the original/replacement commands,
  hashes, sources, expectation and formed plan. Raw wrapper proposals remain
  refused even if the general shell normalizer could strip their fallback.
- The shared command policy's existing setup/server predicate was extracted to
  `verify/setup_command_policy.rs`, reducing `verify.rs` within its existing
  growth budget. It recognizes this complete comparison so the string
  `next dev` is not mistaken for launching a server. Actual server commands and
  unsupported Node code retain the existing policy checks.

## Complete owner preservation

The saved model instruction is 547 characters and the host instruction is
2,237 characters. Combining them exceeds the unchanged 2,500-character limit.
The saved valid proposal therefore retains both verbatim in two ordered
Implement/pass package writers and executes the complete original check group
after both. `package_owner_scope.rs` proves this bounded split before the
existing model/host preservation checks: exact source text and model identity,
new unique host ID, exact normalized package scope, no third writer, original
host verify empty, and the complete check group after both writers.

Lint recognizes only this concrete ordered package split, with the full known
host guidance, exact literal comparison and all writer checks after both.
Other duplicate ownership remains rejected. Admission independently matches
the immutable source duties. Preset/runtime setup optimizations preserve these
owners and observations; passing package prechecks cannot short-circuit either
owner's work. The instruction limit, planner/Recovery budgets, existing growth
baselines and acceptance/evidence requirements are unchanged.

## Tests and evidence

`tests/corpus/apps/issue484-package-script-formation/` contains the exact saved
proposal/scope/source snapshots, their historical source manifest, the complete
formed proposal, and refusal controls. `formation-evidence.json` records selected
actual admission events and source mappings from the offline runner replay.
Hashes over source plans/steps are compact serde-serialized object hashes;
the source manifest separately records historical file and fixture byte hashes.

Focused tests cover real Node stdout/exit behavior; absent script/object/file,
invalid JSON and value mismatch; unsafe literals; unknown/conflicting sources;
changed target, result or expected literal; removed owners/duties/checks;
wrappers, catch and output suppression; registered contract conflicts and
unchanged frozen contract bytes/hash. The comparison is `StaticSyntax` and
cannot supply business Test or implementation-artifact credit.

The parent design and owner-split feedback is incorporated. `owner-refusals.json` and the
owner tests cover text/result/scope changes, writer deletion, third/aliased or
non-normalized writers, ID reuse, checks before/between writers, partial checks,
and nonempty original host verify. #478 duty preservation, #479 pure-import and
fixed diagnostics, #480 execution predicates, and genuine Test classification
remain regression checks.

The final code-review follow-up is covered by `issue484_capture_tests.rs`.
Pending sources follow the current proposal's augmentation stage until its first
retention, including Ready-then-lint-retry paths; retention freezes that matching
source. Double strengthening without a check is refused explicitly. Source
pointers continue to resolve into the emitted immutable snapshots, and the
positive runner test reaches saved-source Admission and lint, rather than treating
the standalone lint predicate as final acceptance.

PR, exact-commit CI/UAT, integration and release version/hash are parent work.
No push, PR, Issue mutation, service operation, live model probe, historical
evidence rewrite, runtime namespace change or other worktree edit was performed.
