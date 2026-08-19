# Issue 104 Design: 未署名ローカル pack 供給契約 v0.1

## 範囲と前提

本 Issue は pack 制度の設計改訂だけを行う。`--extension-root`、catalog、供給
API、GUI wizard、`pack_material_document` renderer、Next.js check は実装しない。
実装先は Issue 109、114、115、116 である。

親 Issue 103 の次の決定を前提として固定する。

- 未署名ローカル供給は未承認・実験用であり、署名付き供給ではない。
- pack 解決順は extension-root、repository `packs/` の順である。
- repository root、execution root、extension root は互いに素である。
- extension-root の操作履歴は `journal.jsonl` へ追記し、pack は削除せず退役
  させる。

既存の profile/intent floor、strict decoder、conformance、exact-byte pin、
human confirmation、honest-failure semantics は緩めない。

## 選択肢比較

### materials の identity

| 選択肢 | 長所 | 短所 | 裁定 |
| --- | --- | --- | --- |
| YAML から参照した材料だけを hash する | identity が小さい | 未参照ファイルの差替え、参照変更との競合、bundle と pin の不一致を許す | 不採用 |
| pack 全体を tar/canonical YAML 化して hash する | 単一 blob にできる | YAML 再直列化差、tar 実装差、既存 pin 全変更を招く | 不採用 |
| 正式メンバーを exact bytes で順序付き hash する | 既存方式と同じで再現可能。未使用材料も bundle identity に含む | ファイル名・上限・順序を厳密に固定する必要がある | 採用 |

`assist.yaml`、`eval.yaml` の既存エンコードと domain separator
`commandagent-pack-v0\0` は維持し、その後ろへ direct member の
`materials/*.md` を正規化 relative path の UTF-8 bytes 昇順で追加する。各
entry は既存と同じ
`u64be(path byte length) || path bytes || u64be(content byte length) || content`
である。materials が無い pack は従来 hash と完全に一致する。

### 供給元と信頼

| 選択肢 | 結果 | 裁定 |
| --- | --- | --- |
| repository 外は署名必須 | 強い provenance を得るが、今回の実験供給を開始できない | Phase G に維持 |
| local を admitted と同一表示する | 選択は簡単だが review、測定、署名を偽装する | 不採用 |
| `admitted | repository | local` を型と表示で保持する | 実行可能性と制度上の信頼を分離できる | 採用 |

Rust/API の型名は `PackSource`、serde/JSON 値は `admitted`、
`repository`、`local` に固定する。`admitted` は reviewed registry との exact
tuple 一致、`repository` は repository 内の pin 済みだが未承認の pack、
`local` は operator が extension-root に pin した未署名 pack である。
repository と local は strict decode、closed vocabulary、floor、hash、path、
profile/intent compatibility が green なら明示選択して Trial/run に使えるが、
それだけで admission や測定済み band を獲得しない。

### materials source の適用範囲

`pack_material_document` は Issue 116 が登録する予約 source とする。初期の
互換 point は Next.js create の `project-setup`、`core-implementation`、
`contract-wiring`、`build-verification` だけに閉じる。他 profile への一般化は
実測 fixture と別の契約改訂なしには行わない。

## 契約決定

### pack directory と materials 上限

v0.1 の正式 hash member は optional `assist.yaml`、optional `eval.yaml`、
zero or more `materials/<name>.md` である。assist/eval の少なくとも一方は
引き続き必須である。

- `<name>` は `^[A-Za-z0-9._-]+\.md$` に一致する basename とし、
  `materials/` 直下だけを許す。
- material は 1 UTF-8 text file とし、1 file は 65,536 bytes 以下、全 material
  content の合計は 262,144 bytes 以下とする。
- directory、nested path、absolute path、`..`、backslash、NUL、symbolic
  link は拒否する。pack directory 自体と正式 member の symlink も拒否する。
- hash は内容を truncate、改行変換、Unicode normalization、再直列化せず、
  disk 上の exact bytes に対して計算する。
- `pack.sha256`、`RETIRED`、その他の管理ファイルは hash member ではなく、
  実行入力にもならない。正式 member 以外のファイルは strict に拒否する。

### `pack_material_document`

`assist.yaml` の `inject[].source` に予約する名前は正確に
`pack_material_document` とする。params は閉じた mapping である。

- `file`: required basename。`^[A-Za-z0-9._-]+\.md$` に一致し、runtime は
  `materials/<file>` としてだけ解決する。
- `max_bytes`: optional positive integer。default 16,384、maximum 65,536。
  pack identity は material 全 bytes を hash し、`max_bytes` は renderer の
  bounded projection だけを狭める。

Issue 116 が Rust registry と renderer を実装するまでは、名前を YAML に書いても
strict vocabulary check は失敗する。renderer は material を instruction ではなく
untrusted observation として前置き、source/path、明確な delimiter、truncation 状態を
付け、資格情報 scrub 後だけ prompt へ渡す。

### conformance と admission

local/repository pin に必要な最小 gate は strict schema、identity agreement、
closed vocabulary、source-before-point compatibility、contract-floor comparison、
path/bound、credential scrub、exact-byte hash/pin 一致である。conformance の
production acceptance 接続確認と negative fixtures は実装/registry contract として
維持する。

実測 fixture と golden rendering は `admitted` 昇格の必要条件である。未承認 pack
がそれらを任意に添付しても admission にはならない。admission は reviewed registry
の exact `id@version + hash + point` tuple だけが与える。

### extension-root lifecycle と journal

extension-root は operator 所有で、owner 以外が書込み可能な root は拒否する。
layout は次に固定する。

```text
<extension-root>/
  packs/<id>/<version>/assist.yaml
  packs/<id>/<version>/eval.yaml
  packs/<id>/<version>/materials/*.md
  packs/<id>/<version>/pack.sha256
  packs/<id>/<version>/RETIRED
  journal.jsonl
```

`stage` は未 pin の新しい `id@version` へ原子的に置く。`verify` は書込み前後に
同じ conformance を行う。`pin` は green hash を `pack.sha256` に新規作成し、
既存 pin を上書きしない。`retire` は `RETIRED` marker を新規作成し、pack bytes、
pin、journal を削除・改変しない。retired pack は一覧/監査/bundle には残るが、
新規選択と `locate_pinned` から除外する。

`journal.jsonl` は UTF-8 JSON Lines、append-only で、1 operation が 1 closed JSON
object である。schema の field 名と enum は次に固定する。

```json
{"ts":"<RFC3339>","actor":"gui|cli","action":"stage|verify|pin|retire","pack":{"id":"<pack-id>","version":"<semver-core>","hash":"sha256:<64-lowercase-hex>"},"result":"ok|error","detail":"<bounded scrubbed text>"}
```

全 field は required、unknown field は書かない。`ts` は timezone を持つ RFC 3339、
`detail` は資格情報を含めず 4,096 UTF-8 bytes 以下とする。journal 自身は pack hash
対象外であり、既存行の書換え、truncate、操作失敗の成功化を禁止する。API 名は
`planner::pack::supply::journal::append(root, &JournalEntry)`、entry 型名は
`JournalEntry` に固定する。

## 表示と許可操作

| `PackSource` | 日本語表示 | 選択/run | verify | stage/pin/retire | 制度上の意味 |
| --- | --- | --- | --- | --- | --- |
| `Admitted` / `admitted` | `承認済み` | exact admitted tuple のみ可 | 可 | extension API では不可 | code-equivalent review と admission 済み。band は別 pin がある場合だけ測定済み |
| `Repository` / `repository` | `リポジトリ（未承認）` | pin/conformance green の明示選択だけ可 | 可 | source control/review 経由のみ | repository にあるが admitted registry 外 |
| `Local` / `local` | `ローカル（未承認・帯域未計測）` | operator 認証、pin/conformance green、明示選択時だけ可 | 可 | extension API/CLI で可 | 未署名の実験供給。pack 固有保証なし |

Gate 1 card、受入 sheet、GUI 一覧は上の日本語をそのまま表示する。
`--summary-json` は locale 非依存の `source` enum を投影する。`local` 値そのものが
未承認・帯域未計測を意味し、別の boolean で矛盾する状態を作らない。local が同一
`id@version` の repository pack を shadow した場合は
`ローカル優先: 同名のリポジトリ pack より拡張ルートを優先` も Gate 1 と GUI に
表示する。

未承認/未計測は run の earned check を自動的に `failed` に書き換える意味ではない。
profile/intent floor と pack の追加 check は通常どおり正直に評価するが、結果から
pack 固有の band、admission、署名 provenance を推定してはならない。表示上の保証は
`pack 固有保証なし（既存 profile/intent の earned assurance のみ）` とする。

## 脅威と対策

### 規約文書による prompt injection

repository や operator が置いた Markdown も命令として信用しない。source は観測
delimiter 内に置き、固定の非命令ラベル、出典、切詰め状態を付ける。YAML/Markdown
で floor、tool policy、system instruction、acceptance verdict を変更できない。攻撃的
文面であっても exact hash には含め、後から差し替えられないようにする。

### materials の資格情報混入

pin 前に全 material exact bytes を credential scrub する。検出時は conformance/pin
を fail closed とし、journal `detail`、API response、rendered prompt に secret 原文を
複製しない。hash や pin は scrub 済み別内容を暗黙生成せず、operator が source file
を直して新 hash を得る。

### hash 偽装、path substitution、TOCTOU

`pack.sha256` を信用せず、正式 member の exact bytes から毎回再計算して constant-time
比較する。symlink、nested material、unknown member、非正規 path を拒否する。
stage は一時 directory から atomic rename し、verify と pin の間に再読込・再 hash する。
pin 後の変更は mismatch として run 前に拒否し、同名競合は extension-root precedence と
shadow warning を表示する。

## Phase G の位置づけ

Phase G に残るのは署名、配布者 identity、trust root、revocation、remote catalog/
transport である。v0.1 の local supply は operator 認証境界内の未署名ファイル供給で
あり、Phase G の前倒し実装、署名代替、admission ではない。今回の改訂で従来の
「外部供給すべてを Phase G へ延期」は「署名付き/remote supply を Phase G へ延期」へ
狭める。

## 検証方針

文書 drift test で v0.1 の hash ordering/bounds、`PackSource` 三値、日本語表示、
`pack_material_document`、journal fields/enums、D-3c local selection、Phase G の限定を
固定する。production behavior と event schema は変更しないため corpus fixture は不要。
repository の共通完了条件に従い fmt、clippy、full test も実行する。
