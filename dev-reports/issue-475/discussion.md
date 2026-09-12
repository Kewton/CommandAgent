# Codex2 investigation discussion

CLI exit: 0. Source: `history`. Target: `commandagent-develop / codex-2`.
Self: `commandagent-issue-475-cli-recovery-store-json-preflight / codex`.
No files changed by the reviewer; DONE received.

- **認識関数の見立て**：保存shapeを実際の `nextjs_outputs::registered_paths` に通す方針は妥当です。`Source::writer_paths` は閉じたNode組み込み・literal path・限定forward/renameだけを扱い、`normalized_literal` は絶対、`..`、backslash、NUL等を拒否します（`nextjs_outputs.rs:102-131,180-192`）。`src/app` 等の保護領域、設定JSON、登録済みrequired/protected pathも後段で除外されています（`recovery_observation_policy.rs:55-71`）。

- **採用してよい最小修正**：まず実認識関数へ `store.ts` の実shapeを入力し、`allowed_generated_paths` が空になる地点を固定fixtureで確認することです。許可する場合も、具体的な相対JSON pathと既知writer形状だけに限定してください。全JSON許可、dynamic引数、任意 `chdir`、root外、path traversal、symlink、source/config JSON、保護pathのhash除外は不採用です。

- **重要な負例**：writer引数が変数・環境値・property計算・テンプレート、未認識import/alias、同じwriterを別pathへ渡すhelper、rename元だけ一致して宛先が動的、親がfileまたはsymlink、`package.json`/`tsconfig.json`、required/protected pathとのprefix重複を拒否する必要があります。`confined_path` は欠損componentを許容するため、欠損親を「遅延生成として許可する」のか「既存directory必須」とするのかを受入条件で明記してください。

- **observer書込みは別問題**：`auto_recovery.rs:898-911` で許可集合を結び、`preflight_effects.rs:35-73` はstageごとのpath/hash差分を記録しますが、`first_write_within_stage` は明示的に `unknown` です。したがって過去R0の内部write主体をこの実装から特定できません。操作→stage→path→hashを制御observerで結ぶ試験を先に行い、fixtureの模倣だけで主体や因果を確定しない判断に賛成です。

- **安全境界**：許可JSONは `current_preflight_source_sha256` から除外されますが、effectsには `allowed_generated` として記録されます（`recovery_snapshot.rs:104-135`、`preflight_effects.rs:42-66`）。未許可source/config差分は除外されず、`auto_recovery.rs:981-1001` でUnavailableとなります。control側は `preflight_audit.rs:29-74` が別途監査するため、これをR0 gate緩和の根拠にしてはいけません。

- **追加の懸念**：`registered_paths` はroute-bound closure内の静的writerを広く拾うため、「store由来であること」自体は証明しません。store呼出元とwriterを結ぶ必要が契約なら、import closureだけでなくcall-site ownershipを負例付きで確認すべきです。不要なら、具体的literal出力の登録契約として範囲を限定して記録してください。

- **未確定点**：保存R0の過去writeがどのobserver・どのstageで発生したか、`projects.json` が製品契約上の許可出力か、欠損親directoryを許可する仕様かは、現ソースと保存差分だけでは確定しません。認識試験とobserver因果試験を別々に保存し、unknownを維持してください。

DONE: 認識とobserverを分離した局所調査方針は妥当 — 具体的literal出力だけを許可候補とし、write主体・store契約・欠損親の扱いは制御試験前に確定しない。

Worker disposition: retain all listed refusal controls, allow missing directories only
when every existing component is a real directory and the concrete JSON leaf is
confined, and reject generic helpers referenced outside their local proven chain.
Historical operation remains unknown; the recorded historical stage is known.

## Implementation review

CLI exit: 0. Source: `history`. Same target; no files changed.

- **阻害バグ：現時点では確認できません。** `registered_paths` はliteral JSON path、既知Node writer、route-bound closureに限定し、`normalized_literal` が絶対path・`..`・NUL・backslashを拒否します。今回追加された `confined_path` の中間component directory検査も、通常fileを親にしたpathを許可しないため、過剰grantを抑えています。

- **安全側の制限**：`generic_renames.rs` は最大8段、全参照call、module-scope定数、単純path値に限定しています。foreign helper、namespace/dynamic import、引数計算、rename先の変更は拒否するため、主な影響はfalse negativeです。正当なstore形状の許可漏れは残り得ますが、任意path grantには直結しません。

- **必要な負例**：
  - 中間directoryが通常file、symlink、dangling symlink。
  - `path.basename` のreceiver再代入、別import、namespace import、dynamic import、re-export。
  - generic helperの外部参照、同名helperが別routeに存在する場合。
  - 8段を超える相互helper、再帰、複数call-siteの一部だけdynamic。
  - rename元・rename先の取り違え、write対象とrename対象の不一致。
  - `data/unregistered.json`、source/config変更、protected/required path変更、`.commandagent`/`.anvil`出力。
  - observerが成功後に失敗し、control hash不変・変更あり・restore失敗をそれぞれ返すケース。

- **残る懸念**：`foreign_references` はroute closure全体の語を保守的に収集するため、合法なexport helper名が別routeで使われると許可漏れになる可能性があります。これは安全側ですが、保存storeと実routeの組み合わせで許可集合が空になる場合は、原因を「不正拒否」とせず、どのhelper境界で証明が切れたかを記録してください。

- **残る観測限界**：`preflight_effects` のstage差分は操作主体を確定せず、`first_write_within_stage` はunknownのままです。Node24 scripted build/server/HTTP試験は認識・制御境界の確認には使えますが、実Next build、Browser UAT、元R0の業務成功の証拠にはしないでください。

- **追加確認**：許可JSONを `current_preflight_source_sha256` から除外するため、許可path以外のsource/config差分が必ず失敗し、`evidence` 配下への書込みが製品出力許可と混同されないことを、observer制御試験で明示してください。

DONE: 現時点でmajorな不正grantは見当たらず、候補は安全側のfalse negative中心。残る確認はhelper境界の許可漏れとobserverのstage/path/hash因果です。

Adopted: add explicit escaped-import and depth fixtures; retain unknown historical
operation and scripted-observer limits. Reuse existing #467 restoration matrix
with the generic saved store. No reviewer instruction expands authorization.
