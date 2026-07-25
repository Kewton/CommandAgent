# uat-test0724-cli-001 v2: bench empty workspace受理

実施日: 2026-07-25 (JST)

対象: bench v0.3 sources無しsuite(create型)サポート

テストrevision: `23ce00242c6e521554c43dc524d7dffeb7df6a25`

前回記録: `workspace/management/runs/uat-test0724-cli-001/uat-report.md`

## 結論

**PASS**。

前回のsuite-load blocker
`bench_suite_schema:create_empty_sources_unsupported`を、既定値を
`sourced`に保った`workspace_mode = "empty" | "sourced"`で閉鎖した。
`cli-create`のdry-runはfull preflight後に6/6 `dry-run-ready`となり、
全runで新規workspace作成とentry 0の無垢性検証がmetaへ記録された。

このv2は既存UAT記録を上書きせず、別run directoryに追加した。対象はbench
schemaとdry-run経路であり、LLMを起動する6本の製品UATやC1〜C4裁定の代替ではない。

## 1. Acceptance scenarios

| scenario | expected | result |
|---|---|---|
| empty正常 | sourcesとrun setなしでsuiteを受理 | PASS |
| empty矛盾拒否 | `[[sources]]`またはrun setを持つempty宣言を拒否 | PASS |
| sourced非退行 | 既定sourced、sources必須の従来文言・metadata形を維持 | PASS |
| 無垢性検証 | run directoryを新規作成し、空を確認してmeta記録 | PASS |
| 汚染検出 | 作成直後にentryがあればrunをblock | PASS |
| cli-create dry-run | full preflight後、6/6 ready | PASS |

## 2. Automated checks

```text
ruff 0.16.0
ruff check workspace/management/scripts
All checks passed.
```

```text
python -m unittest discover -s workspace/management/scripts -p 'test_*.py'
Ran 46 tests
OK
```

unittestはempty正常、sources矛盾、run set矛盾、sourced必須文言、
既定sourced metadata形、無垢性成功、汚染検出を含む。

## 3. Manual CLI scenario

### Preconditions

- branch / revision: `develop` / `23ce002`
- git status: clean
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0724_cli_001`
- suite: `cli-create`, `workspace_mode=empty`, sources 0

### Steps

1. 指定workspace rootが存在しないことを確認する。
2. 次をwrapperなしで実行する。

```text
python3 workspace/management/scripts/bench.py run --suite cli-create --workspace-root /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0724_cli_001 --dry-run
```

3. `uat-meta.json`からpreflight、6 run status、`workspace_integrity`を再読する。
4. campaign全体をbench scrubする。

### Expected result

- full `cargo test`、release build、install/version確認がgreen
- 6本の製品argvが計画表示される
- 各runはfresh directoryで開始し、空であることがmetaへ記録される
- 製品を起動せず6/6 `dry-run-ready`

### Actual result

初回campaign `cli-create-dry-run-20260725-050708`はpreflight
`cargo test`でexit 2。benchの有界stderr抜粋には失敗test名が残らなかった。
コード変更なしの`cargo test --lib`は
`1626 passed / 15 ignored / 0 failed`だったため、新規campaignへ1回再実行した。

受理campaign `cli-create-dry-run-20260725-051004`はexit 0。
開始epoch `1784956195`、終了epoch `1784956354`、159秒。

| preflight | result |
|---|---|
| HEAD / ancestor | `23ce002` / `27d787b` verified |
| cargo test | 33 suites、1784 passed / 30 ignored / 0 failed |
| release build | exit 0 |
| installed version | `commandagent 0.1.0 23ce002 2026-07-25T05:12:11Z` |
| build/install SHA-256 | `01a375a79d320162e21b3fdbb33200918ab5debdc442db54dca71e2c8af9d5ca` 一致 |
| `NODE_ENV` | `production` |

## 4. Six planned commands (verbatim)

### stats_qwen35_001

```text
commandagent --yes --intent create --context-budget 65536 --model qwen3.6:35b-a3b-coding-nvfp4 --provider ollama --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama --ultra-plan-run --profile cli '数値CSVを集計するCLIツール cli/main.py を作成してください。--column で対象列名を指定し、件数・合計・平均を表示します。--help で使い方を表示します。サンプル入力 data/sample.csv を同梱し、実行例と出力例を README.md に記載してください。'
```

### stats_gemma31_001

```text
commandagent --yes --intent create --context-budget 65536 --model gemma4:31b --provider ollama --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama --ultra-plan-run --profile cli '数値CSVを集計するCLIツール cli/main.py を作成してください。--column で対象列名を指定し、件数・合計・平均を表示します。--help で使い方を表示します。サンプル入力 data/sample.csv を同梱し、実行例と出力例を README.md に記載してください。'
```

### stats_qwen35_002

```text
commandagent --yes --intent create --context-budget 65536 --model qwen3.6:35b-a3b-coding-nvfp4 --provider ollama --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama --ultra-plan-run --profile cli '数値CSVを集計するCLIツール cli/main.py を作成してください。--column で対象列名を指定し、件数・合計・平均を表示します。--help で使い方を表示します。サンプル入力 data/sample.csv を同梱し、実行例と出力例を README.md に記載してください。'
```

### filter_qwen35_001

```text
commandagent --yes --intent create --context-budget 65536 --model qwen3.6:35b-a3b-coding-nvfp4 --provider ollama --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama --ultra-plan-run --profile cli 'テキストファイルから指定パターンを含む行を抽出するCLIツール cli/main.py を作成してください。--pattern で検索文字列、--count で件数のみ表示を指定できます。--help で使い方を表示します。サンプル入力 data/sample.txt を同梱し、実行例と出力例を README.md に記載してください。'
```

### filter_gemma31_001

```text
commandagent --yes --intent create --context-budget 65536 --model gemma4:31b --provider ollama --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama --ultra-plan-run --profile cli 'テキストファイルから指定パターンを含む行を抽出するCLIツール cli/main.py を作成してください。--pattern で検索文字列、--count で件数のみ表示を指定できます。--help で使い方を表示します。サンプル入力 data/sample.txt を同梱し、実行例と出力例を README.md に記載してください。'
```

### filter_qwen35_002

```text
commandagent --yes --intent create --context-budget 65536 --model qwen3.6:35b-a3b-coding-nvfp4 --provider ollama --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama --ultra-plan-run --profile cli 'テキストファイルから指定パターンを含む行を抽出するCLIツール cli/main.py を作成してください。--pattern で検索文字列、--count で件数のみ表示を指定できます。--help で使い方を表示します。サンプル入力 data/sample.txt を同梱し、実行例と出力例を README.md に記載してください。'
```

## 5. Empty workspace evidence

6本すべてのmeta原文は同形である。

```json
{
  "workspace_mode": "empty",
  "created": true,
  "checked": true,
  "empty": true,
  "entry_count": 0,
  "entries": []
}
```

集計値:

- `dry-run-ready`: 6/6
- `workspace_integrity` pass: 6/6
- `input_sha256_expected/observed`: 全run `{}`
- precheck: 全run `null`
- acceptance sheet生成: 6/6
- run artifact scrub: 6/6
- campaign scrub: `ok=true`, findings 0

## 6. Regression and safety

- 実装はPython harness層のみ。Rust production/test変更0。
- sourced suiteは既定値のまま。既存suite TOMLの追記不要。
- sourced metadataへ新keyを追加せず、従来source/hash/precheck/archive経路を維持。
- emptyとsources/run setの矛盾宣言を拒否し、検証を緩和していない。
- dry-runなのでLLM・executorは未起動。
- 初回失敗campaignは削除せず、再実行は別campaignに保存した。

## 7. Evidence on failure

将来の再発時は、`uat-meta.json`の`preflight`、
run別`workspace_integrity`、`protocol_reason`、campaign scrub結果、
および失敗時のbench stderrを保存する。entryが1件でもあれば
`empty workspace integrity check failed`でblockedとなることをunit testで固定した。
