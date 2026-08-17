# 結果サマリ

golden-005の支出前gate後、warikan_001でroot閉語彙返済は実効したが、生成computed entryが`function/source`形となり`community_computed_expression_missing`で停止した。warikan_002は直ちに環境中断しrun非消費、残り34runは未起動。これはCM-2h(a)返済で要求された完全語彙配布をcomputed entry shapeまで実装し切れていなかったDATA-1追補床であり、同じ計器系列の判定分母から除外する。

## 原文

```yaml
computed:
  - name: participantCount
    function: len
    source: participants
```

製品verifierの正形は`name`、`expression`、`type`の3 field exactlyであり、events正本は次を記録した。

```text
community_profile_violation:community_computed_expression_missing
```

## 返済

guidanceへcomputed entryのexact field set、`expression`の意味、type集合、registered functionsをvalidator定数から配布した。`function/source`への置換を明示禁止し、最小字義例を非空computedに変更した。字義例exact bytesは引き続き製品`verify_spec`へ通すため、例とverifierの乖離を構造的に拒否する。Rust側のentity/computed entry field checkもPython参照実装と同じ閉集合へ揃えた。

## instrument window

- warikan_001: completed failed, 248秒、primary verification class=`community_computed_expression_missing`。
- warikan_002: `interrupted(environment)`、50秒、run非消費。
- OpenAI usageはeventsに存在するがこの窓は判定分母・費用分布へ混ぜない。費用はpricing正本から別途機械算出し、最終reportの除外費用に明示する。
