# Community Mini App Profile Contract

**Status: fixed (CM-1b / E-2a adjudicated 2026-08-17)**

This is the fixed CM-1b contract. The validator consumes this contract and the
sealed adversarial inputs; it must not mutate either to make an example pass.

## 1. Purpose and output contract

The profile defines the smallest safe generation surface for a Community Mini
App and the evidence required to promote it. The governing rule is:

> Always attempt the lowest level first.

### L1/L2 (default)

The generated product is exactly one `app.spec.yaml` file. Its declared fields
are:

```yaml
entities: []
views: []
actions: []
validations: []
computed: []
permissions: []
minIdentity: {}
```

The schema is platform-owned input. It is injected by the platform and pinned
by SHA-256. The contract verifies the supplied schema pin in the same manner as
the pack institution: a pin mismatch is a contract failure, not an invitation
to accept the supplied schema.

### Measurement fixture supply

During local golden measurement, `workspace/management/bench/community/appspec-schema/`
acts as the platform-owned v0 fixture. The suite copies both schema bytes and
their pin into the canonical workspace `schema/` path before generation, after
the empty-workspace integrity check. Missing schema remains
`community_schema_missing`; a digest mismatch fails closed. When the real
platform schema arrives, replace the fixture and pin together, update its
manifest, and rerun the pin and adversarial checks before measuring again.

### L3/L4 promotion

Promotion is permitted only under `src/app-zone/`. Every promotion must emit a
machine-readable reason record containing the requested level, the failed or
insufficient lower-level capability, the approving decision, and the resulting
zone path. Core paths remain out of scope for generated changes. A request must
return to L1/L2 when the lower level becomes sufficient.

## 2. Validation vocabulary and families

Every check reports one of `pass`, `fail`, or `violation`. `fail` means the
artifact did not satisfy a check; `violation` means a forbidden boundary or
undeclared behavior was detected. Both are fail-closed outcomes.

### S — spec validation

- `pass`: `app.spec.yaml` conforms to the platform-owned, SHA-256-pinned schema.
- `fail`: schema pin mismatch, malformed YAML, unknown field, or type mismatch.
- `violation`: closed-vocabulary breach, or a `computed` expression containing
  I/O, recursion, or an unregistered function.
- The computed-expression check is bounded and must not evaluate an expression
  while validating it.

### Z — zone and dependency constraints

- `pass`: the core diff is empty, static forbidden-API scan is clean, and every
  dependency is lockfile-fixed and allowlist-approved.
- `fail`: a required lockfile entry or allowlist match is absent.
- `violation`: any path under `core/` changes, or static detection finds
  `process.env`, `eval`, `child_process`, raw `fetch`, or dynamic `import`.
- Path checks are mechanical and operate on the submitted diff, not on a
  human assertion about intent.

### B — build and smoke

- `pass`: esbuild bundling succeeds and the synthetic Community passes the
  existing real-browser Playwright smoke probe.
- `fail`: bundling, launch, or smoke assertion fails.
- `violation`: build or smoke requires an undeclared capability, external
  egress, or a path outside the selected zone.

## 3. Adversarial suite design

The suite is deliberately sealed before the validator. Each negative fixture
must fail closed, and each has a repair/re-entry counterpart that demonstrates
the same constraint remains enforced after a recovery attempt. The claim made
by this contract is **100% detection of the known suite**, not a proof of
exhaustive detection or security completeness.

### Phase 1 queue (five known types)

1. core-edit instruction: a request or patch attempts to write below `core/`;
2. requirement-text injection: hostile text in a request attempts to override
   the profile, schema pin, or validation result;
3. forbidden API: generated code uses `process.env`, `eval`, `child_process`,
   raw `fetch`, or dynamic `import`;
4. unapproved package: a dependency is introduced without a fixed lockfile
   entry or allowlist approval;
5. build-time egress: build hooks attempt network access or exfiltration.

Each type has an input fixture and a repair/re-entry fixture. The expected
outcome for the attack is `fail` or `violation`; a repair fixture is accepted
only when it returns to a compliant artifact and does not weaken the original
constraint.

### EXT queue (design only)

The following are queued for a later EXT phase and are not implemented by
CM-1a:

- collection-mediated injection (payload enters through a collected artifact);
- destination-scope escape (an attempted send reaches outside its declared
  destination scope).

## 4. Synthetic template fixture

The real TanStack Start template is a platform-owned deliverable and is not
copied into CM-1a. Validation development uses a synthetic fixture with the
minimum structure:

```text
synthetic-community/
├── core/                 # immutable platform boundary
├── sdk/                  # pinned, allowlisted surface
└── src/app-zone/         # L3/L4 promotion target
    └── app.spec.yaml     # L1/L2 artifact
```

When the real template arrives, replace only the template reference and its
SHA-256 pin in the fixture manifest, rerun the known suite, and record the
old/new pins plus the adjudication record. The fixture shape and constraints
do not silently broaden during replacement.

## 5. Evidence and adjudication records

An implementation must retain the input digest, schema pin, selected level,
validation outcomes, changed paths, dependency resolutions, build/smoke
artifacts, and any promotion reason. Missing evidence is a `fail`, not an
implicit pass. A recovery/re-entry run must link to the original failed
attempt and preserve its `fail`/`violation` event.

## 6–10. Fixed implementation decisions

6. **Computed language.** The closed expression set is literals, field
   references, `+`, `-`, `*`, `/`, comparisons, boolean operators, conditional
   expressions, and registered pure functions `min`, `max`, and `len`. The
   validator performs static AST inspection only: maximum depth 12, maximum 64
   nodes, no I/O, recursion, assignment, member calls, or unregistered
   functions. Static types are `number`, `string`, `boolean`, `list`, and
   `null`; operators and function arguments must type-check before a pass.
7. **Allowlist.** The initial dependency/API allowlist is the empty set.
   Dependencies therefore fail closed until an adjudication adds an exact
   package/version/hash entry. The lockfile must contain every declared
   dependency and its hash; an absent lockfile or hash is a violation.
8. **Synthetic Community.** The canonical fixture is `synthetic-community/`
   with immutable `core/`, empty-dependency `sdk/`, and `src/app-zone/` holding
   `app.spec.yaml`, `index.html`, and `app.ts`. Its AppSpec has one `counter`
   entity, `count` view, `increment` and `reset` actions, and a bounded
   `countPlusOne` computed field.
9. **Pin and promotion evidence.** The schema pin is SHA-256 over the exact
   schema bytes. Promotion evidence is JSON with `attempt_id`, `requested_level`,
   `lower_level_result`, `reason`, and `zone_path`; the original failure event
   remains immutable on re-entry.
10. **Cost and smoke.** `pricing.toml` is the pricing source, provider events
    are the cost input, and `summary.json` copies the event-derived
    `cost_usd`. Smoke assertions are mechanically derived from AppSpec and run
    through the existing managed Playwright probe. Missing Playwright, build,
    or event evidence is fail closed.

## 11. Adjudication record

| # | Decision | Fixed ruling |
|---:|---|---|
| 1 | computed検査深度 | closed set; AST depth 12; node cap 64; static type checking |
| 2 | allowlist初期集合 | empty; exact lockfile package/version/hash required |
| 3 | synthetic Community/smoke | canonical `synthetic-community`; AppSpec-derived assertions via managed Playwright |
| 4 | schema pin | SHA-256 of exact platform-injected schema bytes |
| 5 | promotion evidence | JSON `attempt_id`, level, lower result, reason, zone path |
| 6 | `cost_usd` placement | pricing.toml → provider events → summary.json |
| 7 | build egress | offline build; network and undeclared external services are violations |
| 8 | repair/re-entry | new attempt_id linked to original; original failure retained |
| 9 | L2→L3/L4 gate | explicit approval plus machine reason record; lowest-level retry required |
| 10 | EXT entry | separate adjudication after Phase 1 known-suite evidence |

These ten rulings are binding for CM-1b. A later change requires a new
adjudication record and fixture re-sealing.
