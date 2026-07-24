# bench v0 UAT harness

`bench.py` turns the repeatable parts of a UAT measurement protocol into a
reproducible run, while leaving adjudication to reviewers.

## Usage

From the repository root, choose a workspace outside the checkout:

```bash
python3 workspace/management/scripts/bench.py run \
  --suite dfix-synthesis --workspace-root /tmp/commandagent-bench --dry-run
```

Omit `--dry-run` to execute each suite run once. `--resume` finds the newest
campaign for the suite, skips terminal runs, and records any run left in
`starting`/`running` as `interrupted(environment)` without retrying it.
Use `--skip-suite-tests` only when explicitly accepting the recorded preflight
deviation. `--min-head` overrides the suite's ancestor requirement.

Each campaign contains `uat-meta.json`, `report-skeleton.md`, per-run copied
inputs, precheck logs, the unwrapped product command, console tails, and
archived `.anvil/` evidence.

## Scrub check

`bench.py scrub --path <directory>` scans evidence before it is considered for
publication. The same scan runs after every run is archived; a failure is
recorded as `scrub_failed` in `uat-meta.json` and warned about without stopping
evidence retention. Findings are reported masked as the first two characters
and total length. Name mentions without a value are allowed: the target is
secret existence, not vocabulary. This is an E2-style precision refinement,
not a relaxation; unconditional provider-key, JWT, and private-key patterns
remain failures. Dangerous `.env`/key files, derived directories, oversized
files, and long environment dumps also fail. Optional suite `scrub_allow`
entries require both a regular expression and a reason and are transferred to
metadata and the skeleton for review.

## Retention policy

Reports, metadata, evidence, source-checks, and `events.jsonl` remain tracked
audit assets. Large derived trees (`node_modules/`, `.next/`, and `target/`)
are ignored under run artifacts and must be regenerated from recorded inputs.
A future two-tier external archive is acceptable only when its verification
and reproducibility are demonstrably equivalent; it must not silently weaken
the in-repository audit trail.

## Rules made mechanical

| Protocol lesson | Harness guarantee |
|---|---|
| Instructions are self-contained | Suite TOML carries the complete goal, model, provider, profile, and run matrix. |
| Procurement must be real | Only declared `copy` paths are copied; symlinks and `.git` are rejected and every declared input hash is checked. |
| Execute the exact command | argv is built from the suite, lexically checked for wrappers/control tokens, and launched directly. |
| Execute in segments | Runs are sequential and each run is started at most once. |
| Show a search method for absence claims | The report records the recursive `events.jsonl` glob, `event` field, and regex patterns used. |
| Do not skip environment gates | Clean git, HEAD/ancestor, cargo tests, release install/version, and `NODE_ENV` are recorded before procurement. |

## Deliberately not automated

The harness does not decide pass/fail, classify an incident, consume retries,
or write settlement comments. It transfers verdict, assurance, and terminal
reason text when present and leaves the corresponding fields blank in the
report skeleton for human review.
# Acceptance sheets

退避フェーズ後、各`artifacts/<run>/`へ`acceptance-sheet.md`を自動生成する。生成失敗は`sheet_generation_failed`として`uat-meta.json`へ記録し、計測は継続する。円環workspaceは、次のように単独実行した人手計測のworkspaceを`--run`へ渡して生成する。

```bash
python3 workspace/management/scripts/acceptance_sheet.py --run <workflow-circle workspace> --out <report>/acceptance-sheet.md
```
# ローカル受理前の必須検査

CIのPython checksと同一内容を、コミット前にリポジトリルートで実行する。

```bash
python -m pip install --disable-pip-version-check ruff==0.16.0
ruff check workspace/management/scripts
python -m unittest discover -s workspace/management/scripts -p 'test_*.py'
```

CIとの差異を防ぐため、対象ディレクトリを省略したRuff実行や別のunittest探索範囲を受理判定に使わない。Ruffのimport順・format違反は機械的整形で解消し、ロジック変更を伴う警告は別途レビューへ回す。

検収シートはgit追跡された監査本体（events/evidence/meta）のみを導出元とする。raw log・未追跡ファイルを参照してはならず、第三者がリポジトリだけから同一シートを再生成できることを不変条件とする。人手計測は`manual-timing.md`が存在する場合のみ、機械evidenceと分離した参考行として表示する。
