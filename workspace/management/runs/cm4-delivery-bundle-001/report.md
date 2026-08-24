# CM-4 delivery bundle and deterministic reverification

## Result

The successful L2 result `e_warikan_001` was packaged as one portable R2
delivery unit. The manifest covers 11 files, including the AppSpec, headless
summary, acceptance evidence, explicit L2 promotion non-applicability record,
source-generation metadata, and deterministic reverification record. Two
successive reverifications produced the same `reverification.json` bytes and
the same `full` verdict.

## Source selection

| Field | Value |
|---|---|
| campaign | `cm4-planner-cand-001` |
| source run | `e_warikan_001` |
| source result | full, repair cycles 0 |
| artifact level | L2 |
| generation duration | 65 s |
| generation provider cost | $0.00160414 |
| executor | `gpt-5.6-luna` |
| planner | `qwen3.8:27b-mlx`, think=`medium` |
| generation instrument | `b9f9818602d34c1b383a1910bcaf0c8737d596bcf0d792f5b3e0399d330c13fa` |

The selected artifact has no `app-zone`, so promotion is not applicable. The
bundle therefore records `status=not_applicable_l2` and
`evidence_claim=false`; it does not invent promotion evidence.

## Creation and reverification

The machine entry point is
`workspace/management/scripts/community_bundle.py`. Creation copied only the
contract inputs and evidence, projected the existing verification event into
the existing `commandagent.headless-summary/v1` schema, made all delivery paths
bundle-relative, and generated the SHA-256 inventory mechanically.

Reverification executes these gates in order:

1. every declared bundle file size and SHA-256, plus absence of unlisted files;
2. the executing binary SHA-256 against the pinned instrument;
3. the product's offline `community-mini-app` verifier, deriving `full` again;
4. the independent Python reference S/Z implementation on the same files;
5. equality with the manifest's original `full` verdict.

The exact product validation form was:

```text
commandagent --offline --profile community-mini-app --prompt "Validate app.spec.yaml against the pinned schema; fail on violation." --cwd <bundle>/artifacts --state-dir <temporary-state> --no-footer --summary-json
```

Because this is L2, applicability is `S+Z`; B is explicitly
`not_applicable_l2`, as required by the level matrix. The two consecutive
reverification outputs were both:

```json
{"applicability":"S+Z","artifact_level":"L2","expected_verdict":"full","families":{"B":"not_applicable_l2","S":"pass","Z":"pass"},"instrument_sha256_verified":true,"manifest_verified":true,"product_exit_code":0,"product_verdict":"full","reference_exit_code":0,"reference_verdict":"full","schema_version":"commandagent.community-reverification/v1","verdict_equal":true}
```

## Hash anchors

| Object | SHA-256 |
|---|---|
| `bundle-manifest.json` | `9e0865ea86a35070d7f3e6b87d615aaf2ac6c9f80aec7cb5958b2c6ed8eefdb8` |
| `reverification.json` | `febecece2ab74bc06ac70df5cc4f079b485ffa15c880a4e116ffe27168c37af2` |
| `app.spec.yaml` | `9aa14d18077c291afdd0d718eeabbef424a28e0060e54eccbfe37557b57e9937` |
| source verification events | `6e7215c3cbff6a47e3cdaafd49d1918e76af4e10de263a291cad741e37fc87a1` |

The bundle scrub check returned `ok=true`, with zero findings.

## Reference-implementation parity found during the proof

The first replay exposed a pre-existing reference-only mismatch: the Rust
verifier admits an L2 artifact with no declared package material, while the
Python reference required `package.json` and `package-lock.json`
unconditionally. The contract says dependency inspection applies to declared
material. The Python implementation was aligned to the existing Rust behavior:
no package declaration is pass, while a declared package still requires a
lockfile. Focused positive and negative tests cover both branches. No product
verdict, product code, or sealed fixture was changed.

## Negative fixtures

`test_community_bundle.py` proves that content tampering, an unlisted file, and
missing L3 promotion evidence fail closed. Corpus fixtures pin the L2 promotion
and reverification vocabulary. The existing L3 verifier path remains required
to run S/Z/B and is not weakened by this L2 delivery proof.
