# 結果サマリ

`cm2f-resume-001`の`community_spec_closed_vocabulary` 3件を生成物原文とpinned schema v0で解剖した。3件は同一bytesで、schema専用metadataの`schema_version`と`fields`を`app.spec.yaml` rootへ書き、許可された7 root fieldsを`fields`配下へ入れていた。CM-2eの最小字義例自体がこの混在形を配布していたため、3件とも分類(a) guidance gap (DATA-1)である。schema v0の自然要求表現不足(b)と、許可語彙配布後のモデル発明(c)を示す証拠はない。

## 原文対照

正本schemaは`workspace/management/bench/community/appspec-schema/app-spec.schema.yaml`、SHA-256は`73a0ceba54802185f5210ed2bffce207c765fe02771cb4f216fe4f6f7d695527`である。該当節は次のとおり。

```yaml
schema_version: community.app-spec/v1
fields:
  entities: list
  views: list
  actions: list
  validations: list
  computed: list
  permissions: list
  minIdentity: mapping
```

ここで`schema_version`と`fields`はschema文書のmetadataであり、`fields`のkey集合が`app.spec.yaml`の許可rootである。全runの生成物root原文は次の形だった。

```yaml
schema_version: community.app-spec/v1
fields:
  entities:
    - name: counter
      fields:
        count: number
  views:
    - name: count
      entity: counter
  actions:
    - name: increment
      entity: counter
  validations: []
  computed: []
  permissions:
    - name: read
      subject: minIdentity
  minIdentity:
    mode: anonymous
```

| run | 生成root語 | 許可root集合 | 欠落扱いとなった語 | 分類 |
|---|---|---|---|---|
| warikan_001 | `schema_version`, `fields` | `entities`, `views`, `actions`, `validations`, `computed`, `permissions`, `minIdentity` | 許可7語すべて（`fields`配下へ誤配置） | a |
| warikan_002 | `schema_version`, `fields` | 同左 | 同左 | a |
| warikan_003 | `schema_version`, `fields` | 同左 | 同左 | a |

保存した各生成物のSHA-256はいずれも`3a1b98db8cd7c203d34b7d2ad6d22038f1f02c289a0477c37f543fd08176b197`で、`observed/`に原文を固定した。

## 帰属

- (a) guidance gap: 該当。CM-2eの字義例はschema metadataとapp specを同一rootに置き、モデルのplanにも`schema_version`と`fields mapping`がそのまま現れた。
- (b) schema design gap: 非該当。3生成物はcounter例の複写であり、金額・参加者参照・割勘計算を表そうとして拒否されたものではない。schema pin改訂の根拠はない。
- (c) model invention: 非該当。正しいroot境界を示す一貫した字義例が配布されていなかったため、「配布済みかつ表現可能なのに逸脱」の条件を満たさない。

## 返済

`guidance()`はroot vocabularyと各top-level kindをpinned schema fixtureの`fields`から機械生成する。schema専用の`schema_version`/`fields`をapp specへ書かない規則を明記した。最小字義例から両metadataを除去し、そのexact bytesを製品`verify_spec`へ通すtestを追加した。entity field typeとcomputed functionは各validator定数を単一正本としてguidanceと検証に共用する。v0ではview/actionの名前はgoal-definedであり、別のkind enumは宣言されていないことも明記した。

schema fixture、pin、sealed suite、sealed adversarial fixturesは変更していない。
