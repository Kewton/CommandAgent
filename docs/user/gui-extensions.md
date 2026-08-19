# GUI extensions

[GUI index](gui.md) | [Pack reference](../../packs/README.md) |
[Extension developer catalog](../dev/extension-catalog.md)

This page is for pack/profile extenders. The visible **拡張** screen is a
read-only catalog. Supply mutations are available only through the bounded,
authenticated lifecycle API; the application currently has no browser editing
wizard.

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

## Lifecycle workflow

Treat the API sequence as a review wizard with explicit state transitions:

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
