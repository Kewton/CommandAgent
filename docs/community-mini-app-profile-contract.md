# Community Mini App Profile Contract

**Status: draft for review (CM-1a / E-2a: draft → adjudication → implementation)**

This document is a design contract only. CM-1a adds no product code and no
validator. The adversarial inputs are sealed before any validator is written.

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

## 6–10. Implementation boundary (reserved)

These sections are reserved for the post-adjudication implementation plan:
validator interfaces, event names, fixture manifest format, runner wiring, and
CI/acceptance integration. CM-1a intentionally defines none of them.

## 11. Items awaiting adjudication

The following decisions must be resolved before implementation:

1. the exact inspection depth and resource bounds for the `computed` expression
   language;
2. the initial dependency and API allowlist, including version and transitive
   dependency policy;
3. the canonical synthetic Community shape and Playwright smoke assertions;
4. the schema-pin transport and rotation procedure for platform injection;
5. the machine-readable promotion-reason record schema and event ownership;
6. whether `cost_usd` belongs in the run summary, event ledger, or an
   acceptance-side artifact, and which source is authoritative;
7. the precise build-time egress boundary and the allowed local build services;
8. the repair/re-entry protocol, including attempt identity and evidence
   retention;
9. the review owner and acceptance threshold for promoting L2 to L3/L4;
10. the EXT queue entry criteria for collection-mediated injection and
    destination-scope escape.

No item above is silently decided by this draft. Implementation starts only
after adjudication records each decision and updates this contract from
`draft for review` to an approved status.
