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

## BoN series predeclaration v2

A suite with `suite.bon_series` must receive `--bon-predeclaration` before the
harness allocates a workspace or starts a paid run. Schema
`commandagent.bon-validation-predeclaration/v2` requires all instrument pins
from v1 plus an uncertainty-aware baseline and prediction:

- `baseline_rate` records `full_count`, `trial_count`, their exact `estimate`,
  non-empty evidence `sources`, and the recomputed Wilson 95% interval.
- `predictive_distribution` records a Jeffreys-prior Beta-binomial model, its
  posterior parameters, the suite-sized probability of at least one full run,
  expected full count, and the shortest contiguous 95% predictive band.

The harness recomputes every derived value. A declaration using the former
point-probability binomial fields, a point-binomial model, an incorrect
denominator/interval, or an incorrect predictive band fails closed before
spend. The validated objects are copied into `uat-meta.json` and must also be
present when `bon_select.py` validates a campaign pin.

## Pack A/B measurement template

Pack効果は「同じ仕事へ何を注入したか」だけを独立変数として測る。
`pack-001`〜`pack-003`で確立した正準手順は次のとおり。

1. **Aを凍結する。** profile、intent、family、goal、入力hash、planner、
   executor/provider、preset、権限、suite run行列を固定し、pack未指定の
   対照窓を選ぶ。Aは同じ契約・admission・runtime世代でなければならない。
2. **Bはpinだけを変える。** 同じsuiteを使い、差分を
   `pack.id@version × exact-byte hash`だけにする。pack IDとhashを
   `uat-meta.json`、レポート、band行へ転記し、未指定経路の既存bytesが
   fixture/snapshotで不変であることを確認する。
3. **窓と延長規則を先に宣言する。** 初期run数、同一pinで合算できる
   追加窓、renderer露出がない場合の延長上限、到達0時に効果実験を閉じて
   到達率差を別課題へ昇格する規則を、実行前にレポートへ書く。
4. **分母を段階表示する。** 全run、対象check到達run、renderer露出run、
   下流判定runを混ぜない。合算は同じexact-byte hash、suite、モデル構成、
   契約世代の窓だけで行い、日付とendpointも残す。
5. **live runを三点監査する。** 注入**材料**が必要情報を覆ったか、
   修復**照準**が正しい成果物を指したか、write**圧力**が同じanchorへ
   向いたかを原文で確認する。三点の一つでも欠ければ効果仮説は未検証で
   あり、モデル効果ゼロとは裁定しない。
6. **下流差を測る。** violation署名、check別pass/fail、assurance、
   verdict、full率をA/Bで比較する。三点完備かつrenderer live後にも
   同じ違反署名が残った時だけ、残余をモデル行動へ帰属できる。
7. **交絡を独立記録する。** 到達率変化、モデルendpoint drift、
   harness/machine欠陥、契約・試験変更はpack効果へ畳み込まない。
   scrub、正直終端、偽成功ゼロは両arm共通の受理床である。

最低限のA/B主表は
`arm / pack pin / n / check reached / renderer live / 三点完備 /
下流pass-fail / full / stop class`を持つ。結論は「改善」「非改善」に
二値化せず、`未露出`、`交絡あり`、`三点完備後も同一`まで区別する。

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
