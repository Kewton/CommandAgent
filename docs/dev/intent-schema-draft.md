# IntentSchema draft（E-2a、設計先行）

本稿はrule of twoを満たす既存create／fix／investigateの構造比較からの抽出であり、実装を変更しない。

## 構造比較（出典）

| 軸 | create | fix | investigate |
|---|---|---|---|
| フェーズ | manifest presetの5段（`src/planner/manifest.rs`のpreset定義） | reproduce-before／isolate-cause／repair／verify-regressions（`src/planner/fix_plan_synthesis.rs:17-60`） | reproduce-candidate／diagnose／bind-verify（`src/planner/investigation_plan_synthesis.rs`） |
| 所有権 | manifest段が成果物と契約を所有 | repair段が変更を所有、verify段は検証のみ（同:141-227） | diagnose段が`output/diagnosis.md`を所有、bind段が束縛を所有 |
| verify束縛 | createの契約check集合 | F1同一R、F2、回帰5件（同:286-331） | I1 reproducer、I2 claims、lineage |
| evidence | E系（probe／schema／claims） | F1-F3と回帰 | I1/I2とbinding |
| assurance | 契約集合全件成立 | before/after/回帰全件成立 | I1/I2全件成立 |
| 生成側ガイダンス | goalと契約材料 | F1失敗・診断・target材料を注入（同:195-224） | R出力と診断要件を注入 |

## IntentSchema草案（固定語彙のみ）

```yaml
intent: create
phases:
  - {id: manifest, role: implement, owns_outputs: true, verify_binding: contract_checks}
  - {id: verify, role: verify, owns_outputs: false, verify_binding: contract_checks}
evidence: [probe, schema, claims_binding]
assurance: {full: all_required_evidence_pass, failed: any_required_evidence_failed}
```

```yaml
intent: fix
phases:
  - {id: reproduce-before, role: reproduce, owns_outputs: false, verify_binding: F1_lineage}
  - {id: isolate-cause, role: isolate, owns_outputs: false, verify_binding: F1_evidence, material_injection: diagnosis}
  - {id: repair, role: implement, owns_outputs: true, verify_binding: target_binding, material_injection: diagnosis}
  - {id: verify-regressions, role: verify, owns_outputs: false, verify_binding: F2_and_regressions}
evidence: [F1, F2, F3, regression]
assurance: {full: all_required_evidence_pass, failed: any_required_evidence_failed}
```

```yaml
intent: investigate
phases:
  - {id: reproduce-candidate, role: reproduce, owns_outputs: false, verify_binding: I1_lineage}
  - {id: diagnose, role: implement, owns_outputs: true, verify_binding: I2_claims, material_injection: R_output}
  - {id: bind-verify, role: bind, owns_outputs: false, verify_binding: I1_and_I2}
evidence: [I1, I2, investigation_binding]
assurance: {full: all_required_evidence_pass, failed: any_required_evidence_failed}
```

対応表: `id`は各合成関数のphase id、`role`はStepPlan kind、`owns_outputs`はevidence生成箇所、`verify_binding`は既存lineage／contract参照、`material_injection`は既存プロンプト材料、`evidence`は保存JSONの識別子に対応する。対応しないフィールドは記録なしとして追加しない。

## 宣言できないもの

照合器の実体・正準化、チョークポイント、裁定演算、reproducer事前検証、材料注入の具体的本文は宣言しない。YAMLは構成、検証と裁定の実体はRustに残す。`assurance.full`も条件式を任意式にせず、固定語彙`all_required_evidence_pass`の参照に限定する。

## 移行設計

推奨は、合成計画スナップショットを互換検証器として固定し、既存3 intentの出力・イベント・evidenceをbyte互換で比較してからschema駆動へ段階移行する方法である（D-0骨格抽出と同じ流儀）。別案として、既存3 intentをRust参照実装として凍結し、IntentSchemaは第4 intent以降だけに適用する方法もある。前者は共通化の検証可能性、後者は既存経路のリスク最小化が利点である。採用はレビュー側が決定する。
