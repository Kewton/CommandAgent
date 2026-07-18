# v5 starting-point provenance

Procurement start: 1784351618 / Sat Jul 18 14:13:38 JST 2026
Procurement end: 1784351825 / Sat Jul 18 14:17:05 JST 2026
Elapsed: 207 seconds

Source chain: `uat-test0717-dfix-002/analysis/source-provenance.json` →
`uat-test0717-dfix-002/artifacts/source-checks/{pipe-a,pipe-b,schema-a,schema-b}`
→ six new independent directories under
`test0717_dfix_004_v5`. No source was synthesized.

Only `pipeline/`, `output/`, and `data/` were copied. A recursive check before
the R prechecks found no `.git`, `reproducer.*.log`, or
`catalog-helper-*.log` in any new run directory.

| Set | Runs | `pipeline/main.py` SHA-256 | Principal output SHA-256 | Other output SHA-256 |
|---|---|---|---|---|
| pipe-a | `dfix4_pipe_qwen35_001` | `4944322105b422f71338513405e3de57d2c698aab565f313fac794e4163ad1d3` | `output/results.json` absent | `output/inspection.json` `0e76d134335ffd290fb457321c2b0cd94412bd844f1f4732b9a089116aa5a6a7` |
| pipe-b | `dfix4_pipe_gemma31_001`, `dfix4_pipe_qwen35_002` | `b27e8aaffef74dac171ff19dffb89eb891c97ee2038045887dae8d43719a511b` | `output/results.json` `af452a62d26b5e377453bbd1daddc27282bd61c806aeff30d6616dd02afbf6f2` | `output/inspection.json` `6bd2ec134cbbc6fda43c8f256e941779849e63152cee8f36d4c4df2f6d1b4558` |
| schema-a | `dfix4_schema_qwen35_001`, `dfix4_schema_qwen35_002` | `5e2d7efe794c78a20dded6ca1b0b4e449293fe2609c8274c302b102ddb0c7c96` | `output/results.json` `a0e3a1dfd4a2378598efead1da29673d2d91e09c6fdebecb1f9b1db5e7dd07ca` | `output/report.md` `17af1dd02de623845341e4a427c0c7ec5b26254de6524030bc5842d295165f97` |
| schema-b | `dfix4_schema_gemma31_001` | `5e2d7efe794c78a20dded6ca1b0b4e449293fe2609c8274c302b102ddb0c7c96` | `output/results.json` `a0e3a1dfd4a2378598efead1da29673d2d91e09c6fdebecb1f9b1db5e7dd07ca` | `output/report.md` `17af1dd02de623845341e4a427c0c7ec5b26254de6524030bc5842d295165f97` |

`data/sales.csv` was present in all six copies and, both before and after the
R prechecks, had SHA-256
`2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`.

R prechecks, each executed once in its independent run directory:

- pipe-a: `python3 -B pipeline/main.py` → exit 1,
  `ValueError: invalid literal for int() with base 10: ''` at line 53; same
  class and location as the dfix-002 source stderr.
- pipe-b (both copies): `python3 -B pipeline/main.py` → exit 1,
  `TypeError: list.append() takes exactly one argument (2 given)` at line 164;
  same class and location as the dfix-002 source stderr.
- schema-a/schema-b (three copies):
  `python3 -c "import json; d=json.load(open('output/results.json')); assert 'reconciliation' in d and 'values' in d"`
  → exit 1, `AssertionError`.

Qualification: four historical source sets, six fresh run copies, zero
synthesized broken artifacts, and one unique historical schema source copied
independently as A/B exactly as recorded by dfix-002.
