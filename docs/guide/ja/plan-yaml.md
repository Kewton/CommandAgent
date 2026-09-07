# Plan YAML の編集

[English](../en/plan-yaml.md) | [ガイド目次](../README.md) | [CLI リファレンス](cli-reference.md)

`--plan-steps` と `--ultra-plan` は、実行前のレビューと編集を目的としたコメント付き
YAML を保存します。コメントは各フィールドを説明し、YAML parser には無視されます。
保存パスは引き続き出力の 1 行目で、その後に対象ファイル用の正確な検証コマンドと
実行コマンドを表示します。

## 編集、検証、実行

plan を生成し、表示されたパスをテキストエディタで編集し、何も実行せずに検証してから
実行します。

```bash
commandagent --plan-steps ドキュメントを更新する
commandagent --validate-plan .anvil/plans/plan-<id>.yaml
commandagent --run-plan .anvil/plans/plan-<id>.yaml
```

UltraPlan では `--ultra-plan` で生成し、検証後に
`--run-ultra-plan <PATH>` を使います。`--validate-plan` は offline かつ read-only であり、
provider client を初期化せず、plan を実行しません。

## 編集可能なフィールド

document はトップレベル mapping 1 個のままにし、`steps` または `phases` のどちらか一方を
含めます。`#` で始まる YAML コメントは残す、変更する、削除する、のいずれも可能です。

| フィールド | plan | 編集時の契約 |
| --- | --- | --- |
| `goal` | 両方 | 空でない全体の成果。 |
| `steps` | step | 順序付きで空でない step list。 |
| `id` | 両方 | 一意の識別子。step ID は小文字の kebab-case。 |
| `kind` | step | `inspect`、`setup`、`implement`、`verify`、`report` のいずれか。 |
| `expected_result` | step | `pass` または `fail`。 |
| `instruction` | step | shell command ではない、焦点を絞った自然言語の指示。 |
| `expected_paths` / `verify` | step | YAML string の list。空 list も可。検証コマンドは実行時の安全 policy を満たす必要があります。 |
| `profile` / `style` / `intent` | ultra | 意図的に変更する場合を除き、生成された実行 context を維持します。 |
| `phases` / `prompt` | ultra | 焦点を絞った phase task の順序付き list。各 phase には一意の ID と空でない prompt が必要です。 |

`:`, `#`, bracket、先頭の記号を含む文字列には quote を推奨します。生成される template は
すでに安全な quote を使用します。

`implement` step の `expected_paths` に含まれるページがエンジン内蔵の雛形と一致する間は、
その step は完了しません。別の API route が run 全体の契約を満たしていても、Read のみや
無関係なファイルへの Write では省略できません。対象ページを実装したうえで通常の契約・
step 検証を通す必要があります。Bash による対象ファイルの内容変更も変更として記録します。
雛形ではない既存成果物が契約を満たす場合は、不要な書込を要求せず省略できます。
iteration の `step_short_circuited` は step ID、Write/Edit の観測、雛形パス、開始・終了時の
内容 hash、充足した契約と `satisfaction_basis` を記録し、検証済みの無変更成功を識別できます。

## 検証診断

成功時は plan type と次の実行コマンドを表示します。

```text
Valid step plan: /workspace/.anvil/plans/plan-<id>.yaml
Next: commandagent --run-plan /workspace/.anvil/plans/plan-<id>.yaml
```

失敗時は非 0 で終了し、利用できる各 source location を
`path:line:column: reason` 形式で表示します。構文エラーと field type エラーは YAML parser、
意味上のエラーは実行時と同じ plan lint rule に基づきます。すべて修正して
`--validate-plan` を再実行してください。検証は意図的に実行時の読み込みと同等以上に厳格です。

## Recovery plan

Recovery UltraPlan は引き続き `--run-ultra-plan` と互換です。先頭コメントには次の試行に
必要な限定的な差分が要約されます。

- 維持する変更済み path、
- 不足している path と capability、
- repair target、
- 再実行する deterministic check。

差分は情報提供用のコメントだけであり、実行可能 field の追加や recovery metadata の変更は
行いません。`--validate-plan` の成功時にも、記録された失敗 scope、failure kind、維持する
artifact、および正確な `--run-ultra-plan` コマンドを表示します。

## トラブルシューティング

- document に `steps` と `phases` の両方があるという診断では、実行する shape だけを残します。
- `verify` entry を指すエラーでは、shell pipeline や chain を plan policy が許可する個別の
  deterministic check に分割します。
- 旧形式の手書き plan が実行できても検証に失敗する場合は、現在のコメント付き template に
  示された明示 field を追加してから編集を続けます。
- 別の action flag と `--validate-plan` が競合する場合は、
  [CLI の排他関係](cli-reference.md#排他関係と組み合わせ)を確認します。
