# 結果サマリ

- 対象は`cm3-matrix-001`のD失敗5件とB失敗3件。推測補修前に、保存済み
  plan/spec/eventsと最終artifactを照合した。
- Dの`community_package_missing` 3件は**伝達**。plannerへ配布されたL3
  guidanceも生成planも`src/app-zone/index.html`と`app.ts`だけを要求し、B系が
  必須とする`package.json`/`package-lock.json`を計画成果物に含めていない。
- Dの`community_spec_artifact_missing` 2件は**モデル**。overall/step planには
  `app.spec.yaml`作成stepが存在するが、executorがinspectで存在しないspecを
  `Read`して停止し、implement stepへ到達していない。
- Bの`community_spec_closed_vocabulary` 3件は**モデル**。全件で配布済みの
  完全ルート語彙7語から`validations`だけを欠落させた。注入形の欠落ではない。
- 較正対象はDのpackage成果物伝達だけ。Dのspec 2件とB 3件はclassesを保持し、
  本弧では較正しない。

# CM-3b matrix-001 failure dissection

## 1. 正本と判定規則

- 計器revision: `b913268f8f045bb77dc07a320b597740f0542877`
- matrix summary SHA-256:
  `a2f6216c4140392f5a9961bc8e858a305e91b8b58aa25abc41831a37ece0ff81`
- D plan正本: `cm3-matrix-001` arm D artifact内の
  `.anvil/plans/ultra-plan-*.yaml`と`plan-*.yaml`
- B spec正本: arm B artifact内の`app.spec.yaml`
- 分類は次の二値に固定した。
  - **伝達**: 必須物がguidance/計画成果物へ表現されず、実行は計画どおり。
  - **モデル**: 計画に必須物が明記されたのに、モデル実行が作成stepを履行しない。

## 2. D失敗5件

| run | stop class | plan原文とguidance要求 | 実行原文・最終物 | 分類 |
|---|---|---|---|---|
| `d_warikan_002` | `community_package_missing` | overallは`src/app-zone/index.html と src/app-zone/app.ts`、step expected_pathsも同2件。guidanceは「index.html and app.ts and run B verify」。`package.json`/lockの語はない | spec、promotion、index.html、app.tsを生成。package/lockなし | 伝達 |
| `d_warikan_003` | `community_package_missing` | UI phase/stepはindex.htmlとapp.tsおよびB verifyを要求。package/lockを成果物として宣言しない | spec、promotion、index.html、app.tsを生成。package/lockなし | 伝達 |
| `d_mochimono_003` | `community_spec_artifact_missing` | overall:「app.spec.yaml を作成」。step `create-app-spec` expected_paths: `app.spec.yaml` | inspectが`Read app.spec.yaml`を呼び、`tool_execution_error`。stdout: `path does not exist: app.spec.yaml`。implementへ未到達 | モデル |
| `d_vote_002` | `community_package_missing` | `implement-voting-page-and-b-verify` expected_pathsはindex.html/app.tsだけ。guidanceにもpackage/lockなし | spec、promotion、index.html、app.tsを生成。package/lockなし | 伝達 |
| `d_vote_004` | `community_spec_artifact_missing` | overall:「app.spec.yaml に定義」。step `write-app-spec` expected_paths: `app.spec.yaml` | inspectが`Read app.spec.yaml previously nonexistent?`を呼び、`tool_execution_error`。implementへ未到達 | モデル |

B系実装は`verify_build_and_smoke`でworkspace rootの`package.json`を必須読込し、
`scripts.build`に`esbuild`を要求する。またZ系はpackageが存在すればlockfileを
必須とする。したがって3件のpackage停止は検証器の偶発ではなく、guidanceと
計画lintがB系の必要材料を伝えていなかった差である。

## 3. B失敗3件

配布済み原文は全runで同じである。

> Roots: entities:list, views:list, actions:list, validations:list,
> computed:list, permissions:list, minIdentity:mapping.

最小例にも`validations: []`が含まれ、step planのProfile generation rulesへ
同じ文字列が注入されている。

| run | 生成specのルート原文 | 許可集合との差 | 分類 |
|---|---|---|---|
| `b_warikan_003` | `entities, views, actions, computed, permissions, minIdentity` | `validations`欠落。加えて後続type gateなら`minIdentity`がlistだが、今回の停止は先行closed-vocabulary gate | モデル |
| `b_mochimono_002` | `entities, views, actions, computed, permissions, minIdentity` | `validations`欠落 | モデル |
| `b_mochimono_003` | `entities, views, actions, computed, permissions, minIdentity` | `validations`欠落 | モデル |

3件とも完全一覧と最小例を受信済みで、同じ1語を省略した。したがって
「ローカル方言向けの語彙注入形不足」を示す証拠はなく、DATA-1文言の追加は
行わない。検証とbounded repairはfail closedのまま保持する。

## 4. 原文証跡SHA-256

| evidence | SHA-256 |
|---|---|
| D `d_warikan_002` overall / L3 step | `473ca57c1ceaa297fc62be3db8f59474b7f63db3ce31fd0d45328b61a0fa50d2` / `01054c7a1b884db210c12ab03e2d0a33b84636362c9090613284a97d8b8d1082` |
| D `d_warikan_003` overall / L3 step | `b3e01147e4934fae2416e782e6290bcf4af9f8bcdcaa8c1e9742b01f0f06246b` / `5308534f954135ea54effa587eac5f35a91667b43dd22fd497e15cc019fe86ec` |
| D `d_mochimono_003` overall / spec step | `1f6162137f280a8b7788d1890c1ffa06c2e7b71bede06af7cf4af7cf7b9b481a` / `62e680e5da8eb8d3d20626a80b8a711c0b0086668123d3e84bfeb9a543aedd7e` |
| D `d_vote_002` overall / L3 step | `d2a88f5fc4a57b83d5f18bdaa7f01d6abc8743093049d9b2bec70d771e26da31` / `2687d6712e1fa80d501a6544346c2fdd5cd7b3335f5cbff60d21d866afb89eef` |
| D `d_vote_004` overall / spec step | `4680b12b7d79e0672841827e1b82ef630ce9bf52dc7016094c8d73d2c65c5129` / `d78a7f2493549ca9f4d0775f8894575d230cb62e1fff3e5da337c76fefcfdbfb` |
| B `b_warikan_003` spec | `976f89aec96564032e0301fd6b51696bcd009af5b2a06a3c4f0bbc2c225f1310` |
| B `b_mochimono_002` spec | `2913b4bbbb7f48f4d019f0a93c4f5a56d25bc90510adf3a43e3683bd18fd78d4` |
| B `b_mochimono_003` spec | `d9e099a504aec080acabd2e6553df0b71675e8d6ed7fe48dea13b8ac3447fead` |

生campaignは資格情報とraw logをrepositoryへ取り込まず、この原文引用とhashを
診断証跡として封緘する。
