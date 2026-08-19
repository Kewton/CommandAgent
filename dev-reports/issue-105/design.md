# Issue #105 Design: additive profile overlay slot

## Decision and scope

Permit one optional, additive-only overlay manifest on an admitted embedded
profile. The overlay creates a distinct draft effective profile; it does not
modify, rename, or inherit the admission of its base. Ordinary organizational
prompt material and registered final checks should continue to use a pack.
The overlay slot is for requirements that need profile-manifest semantics,
namely additional artifact cardinality, guidance variants, profile-bound
checks, and their evidence-target mappings.

This issue fixes the contract only. Issue #117 (E-18) owns parsing, resolution,
GUI/CLI selection, merge implementation, and runtime tests. No production
manifest or runtime state changes in Issue #105.

## Overlay document shape

An overlay is a separate `manifest.toml`. Its closed v1 root shape is:

```toml
[metadata]
id = "acme-nextjs"
display_name = "ACME Next.js"
schema_version = "v1"
status = "draft"

[overlay]
base_profile = "nextjs"
mode = "additive"

[artifacts]
required = ["docs/architecture-decision.md"]

[[artifacts.groups]]
id = "security-report"
cardinality = "exactly_one_of"
paths = ["reports/security.md", "reports/security.json"]
preferred = "reports/security.md"

[guidance.variants.security-review]
triggers = [{ condition = "always" }]

[guidance.variants.security-review.messages]
policy = "Record the security review in the required report."

[[checks.security-review]]
id = "registered_security_review"
params = { report = "reports/security.md" }

[evidence_targets.mappings]
registered_security_review = ["reports/security.md"]
```

`metadata` and `overlay` are required. `artifacts`, `guidance`, `checks`, and
`evidence_targets` are optional, but at least one must contain an addition.
Their nested v1 types and validation rules are the same as the corresponding
base-manifest sections. `plan`, `step_templates`, and `vocabulary` are not
overlay fields. Unknown root and nested fields are rejected.

## Identity, source, and selection

- `metadata.id` is the effective profile id and uses the existing profile-id
  lexical rules. It must differ from every registered canonical id and alias.
- `overlay.base_profile` is a canonical embedded profile id. It must resolve to
  an admitted manifest-backed profile; aliases, generic fallback, and another
  overlay are rejected.
- `metadata.status` is exactly `draft`, and `overlay.mode` is exactly
  `additive`. Neither field has a permissive default.
- Runtime origin is not self-declared. E-18 introduces the closed
  `ManifestSource` values `repository` and `local`; embedded manifests remain
  base profiles and cannot be selected as overlays.
- The selected identity is `(metadata.id, base_profile, source,
  exact_byte_hash)`. The hash is lowercase `sha256:` over the exact manifest
  bytes. Path or display name is not identity.
- One run selects zero or one overlay. Overlay chaining and multiple-overlay
  precedence are intentionally unsupported in v1.

## Additive merge and rejection

The effective contract is merged in the fixed order `base -> overlay -> pack`.
Every stage is monotonic: all base obligations remain, overlay obligations are
added, and a selected pack is then checked against that effective floor.

Loading or selection fails before provider construction when any of these is
true:

1. TOML decoding, required fields, enum values, lexical constraints, exact-byte
   hash, or path confinement fails.
2. The base is missing, aliased, unregistered, not manifest-backed, not
   embedded, or not admitted; the overlay id collides with a registered id or
   alias; the source is not `repository` or `local`; or another overlay is the
   base.
3. `metadata.status != "draft"`, `overlay.mode != "additive"`, a forbidden
   root section is present, no additive entry is present, or a second overlay
   is requested.
4. An artifact path/group id, guidance variant name, check binding name, or
   check capability id collides with the base or another selected extension.
   V1 rejects collision rather than attempting a "stricter" comparison.
5. Existing manifest safety/cardinality/guidance validation fails, a check is
   unknown or unresolved, or a check names a phase not declared by the base.
   Omitted check phases retain the existing final-acceptance default.
6. An evidence-target mapping does not belong to a check added by this overlay,
   omits a required overlay check mapping, or attempts to replace a base
   mapping.
7. Pack merge would remove, relocate, replace, or weaken either base or overlay
   obligations. Namespace collisions between overlay and pack entries fail
   closed.

No overlay may change base metadata, plan order/prompts/placeholders,
step-template behavior, vocabulary references, admission rules, assurance
thresholds, capability implementations, or event schemas.

## Judgment and presentation

Base checks run unchanged and overlay checks are additional necessary
conditions. Failure or unavailability of either set is an honest acceptance
failure. Even when all checks pass, the effective profile remains draft and
the existing draft admission cap limits terminal assurance to `static` with
reason `profile_not_admitted`. The base manifest and admitted badge are not
mutated.

GUI, CLI summaries, doctor output, and confirmation surfaces must distinguish:

- effective display: `<display_name>（下書き上乗せ）`;
- base: canonical id plus its admitted state;
- overlay: id, `repository`/`local` source label, and exact-byte hash; and
- judgment: `draft` and the `static` assurance ceiling.

If a pack is also selected, it is shown separately with its own source and
hash. A plain base-profile run remains byte-compatible with current display
and saved records. New overlay fields in persisted/output schemas must be
optional so old records remain readable; existing event names and fields are
unchanged.

## E-17 evidence and E-18 boundary

Issue #116 commit `ef0703f6` proved that a pack can add bounded convention
material and three registered checks while preserving the Next.js floor. That
is sufficient for most organizational convention use, so the overlay is not a
second arbitrary pack language. E-18 should implement the separate manifest
decoder and leaf merger, keep runner wiring minimal, add positive and every
rejection-path test above, and prove that a base-only run is unchanged.
