# GUI 拡張レイヤー

[GUI 入門](../../user/getting-started-gui.md) |
[供給ライフサイクル詳細](../../user/gui-extensions.md)

**拡張**画面は「何を拡張できるか」「保証へどこまで影響できるか」「どのレビュー済み
導線で登録するか」を一貫して示します。各カタログ項目には `layer`、`source`、
`status`、exact `hash`、`assurance`、登録／昇格方法が表示されます。

## 4 レイヤー定義

```text
Layer 1 能力語彙
  -> Layer 2 下書きプロファイル
    -> Layer 3 パック供給
      -> Layer 4 admission
```

| レイヤー | できること | できないこと | 登録導線 |
| --- | --- | --- | --- |
| 1. compiled 能力語彙 | 型付き source/check を schema・test と共に実装 | GUI、YAML、Markdown から任意の実行ロジックを追加 | golden・corpus を含む実装 Issue/PR review |
| 2. extension-root 下書きプロファイル | 閉じた語彙から task family と contract を構成 | `admitted` を申告、`static / profile_not_admitted` より上へ自己昇格 | 有効な private manifest、Trial evidence、登録 Issue/PR |
| 3. パック供給 | pin 前の bounded assist/eval/material を編集、検証、exact-byte pin、Trial 選択 | pin 後の変更、pin 上書き、削除、unretire、conformance による自己 admission | GUI wizard -> verify -> exact-hash pin -> Trial -> review Issue/PR |
| 4. admission | review 済み identity と assurance ceiling を確認 | GUI から admission、計測帯 claim、自己昇格を追加 | repository の計測 evidence と maintainer review |

上位レイヤーは下位レイヤーへ依存します。下位レイヤーの検証に合格しても自己昇格は
起きません。

## Root と利用不可状態

画面は private な絶対 path を公開せず、extension root を「設定済み」「未設定」
「要対応」で表示します。不正内容、profile/intent 非互換、未 pin、exact hash 不一致、
retired などの具体的な利用不可理由も隠しません。

## Layer 2 の登録

有効な登録済み draft profile ごとに manifest の exact hash と `static` assurance ceiling
を表示します。**安全な登録 Issue を作る**は public な profile ID と hash だけを含む
repository Issue を準備します。秘密、private path、private manifest 内容は添付せず、
再現可能な test と計測 evidence を repository PR に置いてください。

Gate 1 と acceptance は有効な profile/pack identity と exact hash を引き続き投影します。
Layer 1 の語彙や Layer 4 admission を変更する GUI control はありません。

## Layer 3 の導線

既存の pack catalog、作成 wizard、**トライアルで使う**導線は Layer 3 として維持されます。
stage、検証、pin、新 version、retire の詳細は
[供給ライフサイクル](../../user/gui-extensions.md#pack-creation-wizard)を参照してください。

## Contract / Suite 参照資料

Contract と Suite は**参照資料**にまとめられます。review と計測の入力であり、拡張種別
ではありません。ここから何かを登録・昇格することもできません。
