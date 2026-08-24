# CommandAgent packs

This directory contains reviewed, in-repository assist/eval packs governed by
`docs/pack-institution-contract.md`.

Each pack uses this layout:

```text
packs/<pack-id>/<version>/
├── assist.yaml
├── eval.yaml
├── materials/
│   └── <name>.md
└── pack.sha256
```

At least one YAML file is required. Pack identity is the exact-byte SHA-256
composition defined by the fixed contract; formatting changes therefore
change the hash. Unknown keys and unregistered vocabulary are rejected.
Direct UTF-8 Markdown material members are also exact-byte hashed, bounded,
and available only through a registered renderer.

Runtime builtins preserve the no-pack behavior and are represented by the
same typed route registry. They are not unsigned external supply. Signed
external supply remains queued for Phase G.

`packs/builtin/` contains reviewed reference packs. Run their schema,
vocabulary, contract-floor, and exact-byte checks with:

```bash
commandagent --pack-verify packs/builtin/ingest-create/1.0.0
```

The management wrapper exposes the same conformance report for repository
automation:

```bash
python3 workspace/management/scripts/pack_conformance.py \
  --pack packs/builtin/ingest-create/1.0.0
```

`packs/nextjs-acme/1.0.0` is an unadmitted repository conformance fixture. It
demonstrates two bounded convention-material injections plus the three
additive, shell-free checks `path_layout_conforms`, `design_tokens_only`, and
`lint_config_present`. Its presence and pin do not add it to the admitted
catalog or grant a measured band.

Use `commandagent --pack-pin <DIR>` for an unpinned conformant scaffold. It
creates `pack.sha256`, treats a matching existing pin as a no-op, and refuses
to replace a stale pin. `commandagent --profile <PROFILE> --intent <INTENT> --packs`
lists compatible admitted packs; add `--extension-root <DIR>` to
include conformant local packs with their source labels.

For the private extension lifecycle, naming, retirement, materials, and review
path, see [`docs/user/gui-extensions.md`](../docs/user/gui-extensions.md).
Maintainers adding a typed source or check must follow
[`docs/dev/extension-catalog.md`](../docs/dev/extension-catalog.md); pack YAML
cannot introduce executable logic.
