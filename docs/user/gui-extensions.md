# GUI extensions

[GUI index](gui.md) | [Pack reference](../../packs/README.md) |
[Extension developer catalog](../dev/extension-catalog.md)

This page is for pack/profile extenders. The visible **拡張** screen combines
the read-only catalog with an authenticated local-pack creation wizard. Every
mutation still goes through the bounded lifecycle API and its `SupplyRoot`
write boundary; the browser never writes an extension directory directly.

## Four extension layers

The extension boundary is one dependency chain, not a collection of equally
powerful plug-in types:

```text
Layer 1 compiled capability vocabulary
    -> Layer 2 extension-root draft profile
        -> Layer 3 verified and exact-hash-pinned pack supply
            -> Layer 4 measured, maintainer-reviewed admission
```

| Layer | Source | Status | Hash | Assurance | Registration / promotion |
| --- | --- | --- | --- | --- | --- |
| 1. capability vocabulary | compiled Rust catalog | reviewed closed vocabulary | build and repository commit | a capability alone earns no assurance | implementation, schema, golden, corpus, Issue/PR review |
| 2. draft profile | private `extension-root/profiles/<id>` | always `draft` when externally loaded | manifest exact-byte hash | ceiling `static / profile_not_admitted` | place a valid manifest, then open a registration Issue with reproducible measurement evidence |
| 3. pack supply | repository or private `extension-root/packs/<id>/<version>` | `staged -> verified -> pinned -> retired` | exact pack bytes and `pack.sha256` | a pack cannot grant admission or exceed the profile ceiling | use the wizard to verify/pin/Trial, then request repository review |
| 4. admission | compiled catalog plus measured repository evidence | maintainer reviewed | reviewed profile, pack, and evidence identity | earned only by the executed gates and capped by admission | evidence-backed Issue/PR and maintainer admission review |

Layer 1 may add a typed source or check only through reviewed implementation;
the GUI, YAML, and Markdown cannot add arbitrary logic or vocabulary. Layer 4
is also display-only in the GUI: a local profile or pack cannot add admission,
claim a measured band, or promote itself.

The screen shows `--extension-root` as **configured**, **unconfigured**, or
**action required** without exposing the private absolute path. An unconfigured
or unreadable root is unavailable. Individual pack rows separately preserve
parse, compatibility, retirement, missing-pin, and hash-mismatch reasons.

## Layer 2 draft profiles

The Layer 2 tab lists each registered draft profile with the common `layer`,
`source`, `status`, `hash`, `assurance`, and registration/promotion fields.
Only a valid registered manifest appears; startup remains fail-closed for an
invalid profile catalog. The registration-Issue link includes the public
profile ID and exact manifest hash, but never the private root path or file
contents. Add reproducible tests and measured evidence in the corresponding
repository PR.

Layer 2 is a composition boundary over the Layer 1 closed vocabulary. It is not
an admission bypass. Gate 1 pins the draft manifest identity, and terminal
acceptance remains capped at `static / profile_not_admitted`.

## Contract and Suite references

**Contract** and **Suite** appear under **参照資料**. They are read-only inputs
to review and measurement, not extension kinds and not registration controls.
Changing a displayed document cannot register a capability, profile, pack, or
admission decision.

## Pack creation wizard

Choose **pack 作成ウィザードを開く** on the **パック** tab. The five visible
steps preserve the lifecycle boundary:

1. **対象セル** fixes the profile and intent whose contract floor will be
   used during verification.
2. **出発点** starts from a minimal assist scaffold or the complete
   `nextjs-acme` example for Next.js create.
3. **編集** owns the ID, semantic version, optional assist/eval YAML, and
   direct `materials/*.md` text members. With token authentication enabled,
   use the same tab-scoped Trial access token as the Trial screen.
4. **検証** stages the complete member map and displays the server's strict
   conformance, scrub, and exact-byte hash result. A failed item has an
   **該当項目へ移動** action that returns focus to the responsible identity,
   YAML, material, or token control; the failure itself is never suppressed.
   **保存済み bytes を再検証** reloads those persisted exact bytes into the
   editor as well, so unsaved edits cannot diverge from the following pin.
5. **pin** sends only the hash returned by the successful verification. The
   resulting local pack remains **ローカル（未承認・帯域未計測）** and can be
   handed to Trial with **Trial で使う**.

The `nextjs-acme` starting point lets an operator complete the example entirely
in the GUI: keep `nextjs-acme@1.0.0` or assign a new identity, inspect/edit its
two YAML documents and two Markdown materials, verify, pin, then follow the
Trial handoff. If the local identity matches the repository example, the
existing local-precedence warning applies.

A pinned editor is read-only. Retirement requires a separate irreversible
acknowledgement, removes the Trial handoff, and enters a terminal read-only
state. There is no edit-after-pin, pin overwrite, delete, or unretire control;
create a new version for any byte change. After pin or retirement, choose
**新しい version を作る** to copy the displayed members into an editable next
patch version without reloading. The copy is only a local draft until
**保存して検証** stages it through the same lifecycle API.

## Extensions catalog

Open **拡張** to inspect pack supply. Compiled catalog entries are labeled
**承認済み**. Other pinned repository packs are
**リポジトリ（未承認）**, and packs below
`<extension-root>/packs/<id>/<version>` are
**ローカル（未承認・帯域未計測）**. A local pack with the same `id@version`
as a repository pack wins display resolution and carries a local-precedence
warning.

Each row shows `pack.sha256` and the hash recomputed from exact pack bytes. A
missing pin, parse failure, retirement marker, or hash mismatch stays visible
as a warning and is never presented as approved. **Trial で使う** appears only
for a non-retired admitted row or a conformant, exact-byte pinned local row.

A local pack may target an external draft profile when its `pack.profile`
value exactly matches that profile's registered ID in the same configured
extension root. Gate 1 shows and pins both identities. The pack remains local
and unadmitted, and successful execution cannot raise the draft profile above
`static / profile_not_admitted`. A repository pack cannot gain draft-profile
compatibility from the same string match.

## Lifecycle workflow

The browser wizard delegates to this API sequence with explicit state
transitions:

```text
new version -> staged -> verified -> pinned -> selectable
                       \-> verification failed (repair staged bytes)
pinned -> retired (terminal; create a new version to continue)
```

1. Stage `assist.yaml`, `eval.yaml`, and optional bounded `materials/*.md` as a
   new semantic version.
2. Read the returned verification report. Fix strict-schema, vocabulary,
   compatibility, floor, path/size, or credential-scrub failures in `staged`.
3. Pin only the returned exact-byte `sha256:` value. A stale existing pin is
   never overwritten.
4. Confirm the catalog label and Trial eligibility.
5. Retire a version when it must no longer be selected. Retirement writes a
   marker and preserves bytes, pin, and journal history.

There is no unretire. Correcting pinned or retired content requires a new
version.

## What can and cannot change

| Allowed before pin | Never allowed by this lifecycle |
| --- | --- |
| bounded UTF-8 `assist.yaml`, `eval.yaml`, direct `materials/*.md` members | arbitrary paths, symlinks, unknown schema keys, shell-bearing free-form capabilities |
| a new pack ID/version with registered sources/checks | overwrite of a stale pin, mutation after pin, deletion, unretire |
| draft profile manifests below the private extension root | local self-admission, measured-band claims, weakening a compiled profile floor |

Materials are exact-byte hashed, size bounded, credential scrubbed, and
rendered as untrusted observations only through a registered source. Passing
conformance does not make a local pack admitted.

## Extension supply API

Configure a private `--extension-root`. GET routes require Trial
authentication. POST routes also require same-host or allowlisted Origin, JSON,
and a complete body no larger than 1 MiB.

| Route | Operation |
| --- | --- |
| `GET api/extensions/packs` | List local packs as `staged`, `pinned`, or `retired` with hash and conformance state. |
| `GET api/extensions/packs/{id}/{version}` | Return editable UTF-8 members and latest verification. |
| `POST api/extensions/packs` | Atomically stage members, then verify them. |
| `POST api/extensions/packs/{id}/{version}/verify` | Re-run strict verification and scrubbing. |
| `POST api/extensions/packs/{id}/{version}/pin` | Create the pin only for the matching submitted hash. |
| `POST api/extensions/packs/{id}/{version}/retire` | Create `RETIRED` without deleting evidence. |

There are no PUT, PATCH, DELETE, unretire, or pin-overwrite routes. Stable
errors distinguish invalid input, failed verification, conflict, and disabled
extensions. All filesystem changes are delegated to
`planner::pack::supply::SupplyRoot` and append scrubbed records to
`journal.jsonl`.

## Naming conventions

- Pack IDs use stable lowercase ASCII kebab-case and never embed the version.
- Versions use exact `MAJOR.MINOR.PATCH`; every byte-changing experiment gets a
  new version.
- Material names are direct, descriptive `.md` members with no subdirectories.
- Preset names are local operator conveniences such as `nextjs_acme`; a preset
  points to an exact `pack = "id@version"` and does not redefine pack identity.
- Draft profile IDs also use stable lowercase kebab-case. A manifest claiming
  admission remains draft when loaded from an extension root.

## Preparing a repository pull request

1. Reproduce the proposed version under `packs/<id>/<version>/` without copying
   secrets or the private `journal.jsonl`.
2. Run `commandagent --pack-verify <directory>` and preserve the exact pin.
3. If the pack needs a new source or check, implement the registered capability
   and follow the [developer catalog](../dev/extension-catalog.md); do not encode
   logic in YAML or Markdown.
4. Add the focused schema/golden/corpus tests and update `packs/README.md`.
5. Submit the repository change for review. Repository presence and a valid pin
   are still not admission; admission requires an explicit compiled catalog
   entry and measurement evidence.
