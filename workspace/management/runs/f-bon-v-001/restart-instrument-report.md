# F-BoN-V 再開計器

## 0. 事前宣言

記載時刻: 2026-08-03 17:51:29 JST (`+0900`)

コミットAの検算を、実行前に次のとおり固定する。

- 同じGit commitを、`SOURCE_DATE_EPOCH`なし・別target directoryで2回buildした
  `COMMANDAGENT_VERSION`文字列は完全一致する。
- build時刻は現在時刻ではなくGit commit時刻となる。
- `commandagent.bon-validation-predeclaration/v1`は`binary_sha256`欠落を拒否する。
- BoN preflightで宣言SHAとbuild SHAが異なる負例は、installおよびproduct起動より前に
  `BenchError`でfail closedする。
- selectorはpreflightが保存したrevision・suite・binaryの系列ピンを再検算し、
  不一致をvalid measurementとして扱わない。
- 既存bytesは変更しない。旧`bon0-002/003`は結果を失敗へ読み替えず、理由付きの
  除外窓として新規証跡へ記録する。

最初にfocused Rust/Python testとRuffを実行し、その後fmt、clippy、全cargo testへ
広げる。いずれかが不成立ならコミットせず、期待との差を記録する。

## 1. 実測

最初のRust focused testでは、2回のversion文字列自体は一致したが、test fixtureが
`Cargo.lock`をcommit前に持たず、初回buildで未追跡lockfileを生成したため
versionへ`+dirty`が付いた。cleanを期待した最後のassertが失敗した。このfixture
不備を隠さず、lockfileを事前commit対象へ固定して再実行した。

修正後の実測は次のとおり。

- `cargo test --test release_build`: 4 passed
- 同一commit・別target 2回buildのversion文字列: 完全一致
- versionの時刻部分: fixtureのGit commit時刻`2026-08-03T00:00:00Z`
- `workspace/management/scripts/test_bench.py`: 28 passed
- `scripts/test_bon_select.py`: 8 passed
- 関連Python 4ファイルのRuff: green
- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- `cargo test`: green（主crate 1843 passed / 15 ignoredを含む全target）

binary pin不一致負例ではrelease buildまで実行し、期待SHAと観測SHAの不一致で
`BenchError`となった。記録したcommand列に`install`と`commandagent`起動はなく、
product/API支出前に停止した。

## 2. 検算

事前宣言した6項目はすべて成立した。

- 現在時刻の埋込みをGit commit時刻へ置換し、同一commitのversion文字列を決定化した。
- v1 schemaの`binary_sha256`欠落負例を拒否した。
- suite SHAはpreflight開始前、revisionはcargo test前、binary SHAはinstall前に照合する。
- selectorも保存済み系列ピンの7条件を再検算し、不一致負例をinvalidにした。
- `bon0-002/003`は`evidence/excluded-windows.json`で理由付き除外窓とし、各窓の
  full実測値は改変していない。
- 既存run evidenceおよびcalibration bytesは変更していない。

したがってコミットAを作成できる。コミット後のclean revisionを2回release buildし、
version文字列とbinary SHA-256の一致をもう一度確認して、そのSHAだけを再開事前宣言へ
転記する。

## 3. コミット後の計器ピン確定

コミットA `49002050dc00ddab15e6709ebbba7f1beb5a3c7f`をclean detached worktreeへ
checkoutし、別々のtarget directoryで`cargo build --release --locked --bin
commandagent`を2回実行した。

| build | version | binary SHA-256 |
|---|---|---|
| A | `commandagent 0.1.0 49002050 2026-08-03T18:05:55+09:00` | `3fa2978aed3fc09aadc84ae873133bc477117bb03bf4116cc5092cee91c68988` |
| B | `commandagent 0.1.0 49002050 2026-08-03T18:05:55+09:00` | `3fa2978aed3fc09aadc84ae873133bc477117bb03bf4116cc5092cee91c68988` |

versionとbinary bytesはともに一致した。このSHAを再開系列の唯一の計器ピンとする。
旧`bon0-001`のSHA
`5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a`は
新ピンと一致しないため、指示された条件分岐どおり旧001も再開分母から除外し、
新規4窓を調達する。

## 4. 固定build pathでのraw SHA訂正

上記の別target directory 2本は互いには一致したが、実campaignが使う固定clean
worktreeの`target/release/commandagent`とは一致しなかった。最初の再開preflightは
宣言`3fa2978a...`に対して観測`1eb01906...`となり、設計どおりinstall・product/API
支出前にfail closedした。差分はMach-O `LC_UUID` 16 bytesと、それを覆うlinker生成
ad-hoc署名32 bytesで、version文字列は一致していた。

したがって「同一commitなら異なるtarget directoryでもraw binary SHAが一致する」との
上記推論は撤回する。build.rs決定的化の受理対象であるversion文字列一致は成立したまま、
raw SHA系列ピンは全campaignが実際に使う固定build pathで採取・再build一致を確認した
`1eb01906f23524da10463b35bf4ea5d58cf40078f4672431f2f6f25a69f18de1`へv2事前宣言で
訂正した。支出0のpreflight事件は`evidence/luna-restart-preflight-incident.json`へ保存した。
