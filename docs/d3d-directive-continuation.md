# D-3d Boundary Directive Continuation

Status: **fixed (2026-07-31)**

This document fixes the v0 contract for continuing a failed boundary-shell run
with one or more explicit human directives. It extends the D-3c four-gate
lifecycle without creating an in-run chat channel, changing a profile contract,
or treating human prose as acceptance evidence.

## 1. Scope and invariant

The v0 operation is:

```text
failed run at Gate 4
  -> human directive
  -> scrub and immutable persistence
  -> human confirmation of the persisted directive
  -> same-workspace, same-lineage recovery/fix continuation
  -> normal profile acceptance
  -> Gate 3 or Gate 4
```

Only a failed run may enter this path. A request to modify a full result is not
a failure continuation and is permanently outside v0; a post-full revision
flow is queued as D-3d v0.1. Execution-time intervention, interactive steering
inside a phase, and mutation of an already running provider call are also out
of scope.

The directive is guidance material at a human boundary. It cannot remove,
relocate, downgrade, reinterpret, or satisfy a contract check. The selected
profile's contract floor, manifest, runtime, evidence rules, and assurance
projection remain exactly those of an ordinary run.

## 2. Persisted directive artifact

Before confirmation, the shell scrubs the proposed text for credentials and
writes an immutable artifact below the shell state root:

```text
boundary-directives/<sha256>.json
```

The JSON object is strict and contains exactly these semantic fields:

| Field | Meaning |
|---|---|
| `raw` | The scrubbed directive text, preserved verbatim |
| `epoch` | Unix epoch seconds when the artifact was issued |
| `target_run_id` | The failed run whose lineage is continued |
| `round` | Positive, monotonically increasing directive round |
| `issued_gate` | Fixed value `gate_4` in v0 |

The filename hash is computed from the exact persisted bytes. Existing content
at that path must match byte-for-byte; a mismatch is a hard failure. Empty,
oversized, or credential-bearing input is rejected before persistence.

Confirmation is a second immutable record, analogous to D-3c Gate 1
confirmation. It binds the artifact hash and confirmation epoch. A directive
artifact alone never authorizes execution.

## 3. Typed injection source

`human_directive` is added to the closed assist-source vocabulary. Its only v0
owners are the existing fix/recovery implementation and repair guidance points.
It is rendered as a bounded, verbatim block with:

- source ID `human_directive`;
- the persisted artifact hash and directive round;
- the target run ID;
- an explicit contract-floor invariant; and
- the scrubbed text without paraphrase.

The source is subject to the same pack-institution vocabulary and compatibility
checks as every other injection source. It is not a new free-form bypass around
the pack registry. The hash, bound, and verbatim rendering make the material
auditable; they do not make the instruction true or earned.

## 4. Continuation plan and lineage

The shell uses the failed run's existing recovery/fix synthesis. It loads the
recorded recovery plan, injects the directive only into its implement/repair
guidance, and persists a derived continuation plan. Inspection, verification,
acceptance, and every profile-specific check are otherwise unchanged.

The continuation runs in the same workspace and keeps explicit lineage to the
failed run. Its meta and boundary events add:

```text
directive_round = 1, 2, ...
directive_hash = sha256:...
directive_target_run_id = ...
```

The directive-free path does not serialize empty or zero-valued directive
fields. Its prompts, events, and evidence therefore remain byte-for-byte
identical. A fixture over the ordinary recovery path is the compatibility
guard.

## 5. Gate 4 lifecycle

Gate 4 adds one next action, `human_directive` (displayed as “add a directive
and rerun”). Its state sequence is fixed:

```text
failure_ready
  -> directive_proposed
  -> awaiting_directive_confirmation
  -> directive_confirmed
  -> directive_running
  -> acceptance_ready | failure_ready
```

The REPL accepts directive text only in `failure_ready`. It displays the exact
persisted text, artifact hash, target run, and round, then requires explicit
confirmation of that hash. Dispatch checks both the immutable artifact and its
confirmation record. Tests must prove that a missing, stale, or mismatched
confirmation results in zero runner calls.

The transcript records the verbatim directive, confirmation hash, continuation
command identity, and terminal sheet. Conversation may navigate this sequence,
but only persisted records authorize dispatch.

## 6. Configuration accounting

`directive_round` is part of the measured configuration. Round 0, round 1, and
round 2 are separate band columns even when model, goal, workspace family, and
pack pin are otherwise identical. The associated directive hash remains in
run metadata and evidence references; a band row may not collapse distinct
rounds into an unlabelled aggregate.

Scripted directives and suite-authored T2F benchmarks are queued for the F
phase. v0 accepts directives from a human at Gate 4 only.

## 7. First live acceptance

`d3c-shakedown-002` is the required first live artifact. It starts a
`python-cli × create` request, records the failed pre-directive sheet, persists
and confirms this exact directive:

> README.mdの使用例の出力を、実際の実行結果に合わせて修正してください

It then records the continuation sheet and the before/after C3 claim-binding
comparison. A round-1 row is added to the CLI band whether the model transcribes
the observed output or repeats the testimony violation. The measurement asks
whether an explicit human directive crosses the CLI testimony wall; it does
not change what C3 accepts.

## 8. Required guards

Implementation is accepted only when all of the following are green:

1. directive text is scrubbed before any artifact or event is written;
2. `human_directive` is rejected at incompatible injection points;
3. a directive cannot dispatch without its exact persisted confirmation;
4. the ordinary directive-free prompt and event fixtures are byte-identical;
5. recovery still reaches the normal profile acceptance runtime and cannot
   weaken the contract floor; and
6. band aggregation distinguishes directive rounds.
