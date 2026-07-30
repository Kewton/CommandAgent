# CommandAgent packs

This directory contains reviewed, in-repository assist/eval packs governed by
`docs/pack-institution-contract.md`.

Each pack uses this layout:

```text
packs/<pack-id>/<version>/
├── assist.yaml
└── eval.yaml
```

At least one YAML file is required. Pack identity is the exact-byte SHA-256
composition defined by the fixed contract; formatting changes therefore
change the hash. Unknown keys and unregistered vocabulary are rejected.

Runtime builtins preserve the no-pack behavior and are represented by the
same typed route registry. They are not unsigned external supply. Signed
external supply remains queued for Phase G.

`packs/builtin/` contains reviewed reference packs. Run their schema,
vocabulary, contract-floor, and exact-byte checks with:

```bash
python3 workspace/management/scripts/pack_conformance.py \
  --pack packs/builtin/ingest-create/1.0.0
```
