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
