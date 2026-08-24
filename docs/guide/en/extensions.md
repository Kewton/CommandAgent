# GUI extension layers

[GUI getting started](../../user/getting-started-gui.md) |
[Detailed supply lifecycle](../../user/gui-extensions.md)

The **Extensions** screen answers three questions consistently: what can be
extended, how far that extension can affect assurance, and which reviewed path
registers it. Every catalog item shows `layer`, `source`, `status`, exact
`hash`, `assurance`, and registration/promotion guidance.

## Four-layer definition

```text
Layer 1 capability vocabulary
  -> Layer 2 draft profile
    -> Layer 3 pack supply
      -> Layer 4 admission
```

| Layer | You can | You cannot | Registration path |
| --- | --- | --- | --- |
| 1. compiled capability vocabulary | implement a typed source/check with schema and tests | add arbitrary executable logic from the GUI, YAML, or Markdown | reviewed implementation Issue/PR with golden and corpus coverage |
| 2. extension-root draft profile | compose the closed vocabulary into a task family and contract | claim `admitted` or promote above `static / profile_not_admitted` | valid private manifest, Trial evidence, then a registration Issue/PR |
| 3. pack supply | edit bounded assist/eval/material members before pin, verify, pin exact bytes, and select in Trial | mutate after pin, overwrite a pin, delete, unretire, or gain admission by conformance | GUI wizard -> verify -> exact-hash pin -> Trial -> review Issue/PR |
| 4. admission | inspect reviewed identities and the assurance ceiling | add admission, a measured-band claim, or self-promotion from the GUI | measured repository evidence and maintainer review |

An upper layer depends on the lower layers. Passing a lower-layer check never
self-promotes an item.

## Root and unavailable states

The screen reports the extension root as configured, unconfigured, or action
required without exposing its private absolute path. It keeps concrete
unavailable reasons visible, including invalid content, incompatible
profile/intent, missing pin, exact-hash mismatch, and retirement.

## Layer 2 registration

Each valid registered draft profile shows its exact manifest hash and `static`
assurance ceiling. **Create a safe registration Issue** opens a prefilled
repository Issue using only the public profile ID and hash. Do not attach
secrets, private paths, or private manifest contents; place reproducible tests
and measured evidence in a repository PR.

Gate 1 and acceptance continue to project the effective profile/pack identity
and exact hash. The GUI has no control that changes Layer 1 vocabulary or
Layer 4 admission.

## Layer 3 workflow

The existing pack catalog, creation wizard, and **Use in Trial** handoff remain
the Layer 3 route. See the [detailed lifecycle](../../user/gui-extensions.md#pack-creation-wizard)
for stage, verification, pin, new-version, and retirement behavior.

## Contract and Suite references

Contract and Suite documents are grouped under **References**. They inform
review and measurement; they are not extension kinds and cannot register or
promote anything.
