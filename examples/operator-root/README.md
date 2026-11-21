# Two-stage operator-root example

A minimal extension built on the operator root: a profile with two structured
stages, each stage one operator-pinned `/bin/bash` process, and a v1 run budget.
Stage `parse` turns `input/items.txt` into `work/items.json`; stage `check`
checks that file and proposes `out/report.json`.

The walkthrough is the user guide page
[Build a two-stage extension](../../docs/guide/en/operator-root-tutorial.md)
([日本語](../../docs/guide/ja/operator-root-tutorial.md)). Start with
`bash setup.sh <new directory>`. The pinned processes run on macOS only, and
`rehearse/fake_model.py` (Python 3, standard library) stands in for a model.
