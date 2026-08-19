# Issue 116 Design: Next.js convention-pack source and checks

## Scope and predecessor state

Integrate the verified Issue 104 supply contract and Issue 107/108 catalog
commits, then implement only the runtime vocabulary needed by the repository
`nextjs-acme@1.0.0` fixture:

- one bounded `pack_material_document` assist source;
- three shell-free, additive final-acceptance checks;
- one unadmitted repository fixture pack and its documentation/tests.

The existing Next.js manifest remains the contract floor. Its build command,
browser probes, hook checks, testimony binding, assurance, and release gates
are not removed, weakened, relocated, or represented by the pack.

## Material membership and rendering

`planner::pack` will load direct regular UTF-8 `materials/*.md` members under
the Issue 104 limits (65,536 bytes each, 262,144 bytes total), reject symlinks,
nested/unknown members, and append normalized material path/content entries to
the existing exact-byte hash in bytewise path order. `LoadedPack` retains the
validated bytes so rendering does not open an arbitrary workspace path.

`pack_material_document` accepts a required safe basename `file` and optional
positive `max_bytes` (default 16,384, maximum 65,536). It is compatible only
with the four Next.js create injection points fixed by Issue 104. A leaf
renderer emits a fixed non-instruction preamble, pack/source/path labels,
delimiters, UTF-8-safe truncation status, and only credential-scrubbed text.
Conformance fails closed when a material contains a credential-shaped value;
the exact source text is never copied to events or errors.

## Generic check registration and execution

Add a `PackInternalCheck` capability family with these closed parameter sets:

- `path_layout_conforms`: required glob list (1..64) and optional forbidden
  glob list (0..64);
- `design_tokens_only`: CSS glob list (1..64), confined token file, and an
  optional literal allowlist;
- `lint_config_present`: confined file path and an optional literal list.

Glob strings are workspace-relative, reject absolute paths, backslashes, NUL,
and parent components, and must compile with `globset`. File walking is
deterministic, ignores engine/private and common generated directories, does
not follow symlinks, and never launches a child process. Check results use a
closed summary (`id`, pass/fail, bounded reasons). The pack runtime executes
only selected `{kind: final_acceptance}` bindings, emits one backward-compatible
additive `pack_check_result` event per check, and returns an aggregate result to
the plan final-contract boundary. A failed or unavailable pack check makes
acceptance fail; it is never downgraded to a warning.

The floor merger continues to require every existing profile floor check and
permits only registered additive checks at their registered final-acceptance
boundary. No new check is inserted in the Next.js manifest, so the baseline
build/browser/hook obligations are byte-for-byte unchanged.

## Fixture and contract updates

Add `packs/nextjs-acme/1.0.0` with two material injections, all three checks,
one additive JSON artifact schema, two Markdown materials, and a matching
`pack.sha256`. Do not add it to `ADMITTED_PACKS`; it remains an unapproved
repository fixture. Update the capability golden, institution vocabulary and
compatibility tables, Next.js profile contract, and pack README. Focused tests
cover strict decoding and limits, material membership/hash/rendering, every
check's positive/negative/path boundary, additive floor merge, fixture
conformance, event emission, and preservation of the existing Next.js floor.

## Verification

Run focused pack/catalog/check tests first, then the conformance, protection,
doc-drift, and generality guards. Because shared pack loading, capability
registration, and final acceptance are touched, finish with formatting,
Clippy with warnings denied, and the complete Rust test suite.
