# 10分デモ実演手順書

## 準備

```bash
git clone https://github.com/Kewton/CommandAgent.git
cd CommandAgent
python3 workspace/management/scripts/acceptance_sheet.py --run workspace/management/runs/uat-test0715-ff1-002/artifacts/data4_gemma31_none_002 --out workspace/management/runs/c1-acceptance-sheets/full/acceptance-sheet.md
python3 workspace/management/scripts/acceptance_sheet.py --run workspace/management/runs/uat-test0722-circle-elev-007/run1 --out workspace/management/runs/c1-acceptance-sheets/failed/acceptance-sheet.md
python3 workspace/management/scripts/acceptance_sheet.py --run workspace/management/runs/uat-test0722-circle-elev-008/run1 --out workspace/management/runs/c1-acceptance-sheets/circle/acceptance-sheet.md
```

## 開く順序

1. `workspace/management/runs/c1-acceptance-sheets/full/acceptance-sheet.md`（goal、契約、E2）
2. `workspace/management/runs/c1-acceptance-sheets/failed/acceptance-sheet.md`（停止理由、recovery）
3. `workspace/management/runs/uat-test0722-circle-elev-003/`（捏造診断の拒否記録）
4. `workspace/management/runs/c1-acceptance-sheets/circle/acceptance-sheet.md`（円環全体）
5. `workspace/management/runs/band_summary_circle.md` と `workspace/management/runs/band_summary_fix.md`（値札）

## 任意ライブ幕

円環をその場で回す場合は、人手方式で次の1行だけを実行する。並列実行・監視・中断はしない。

```bash
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin <新規に用意した実失敗runのorigin> ; date +%s
```

所要はローカル実測で数十秒〜数分を見込む。失敗した場合は失敗を隠さず、生成済みのfailed形シートを開き「それも本物の証拠」と説明する。ライブ実行は本編の再生成条件ではなくオプションである。

## 所要実測

- リハーサル日時: ____________________
- 実測合計: __________ 分 __________ 秒
- 備考（停止・再試行を含む）: ____________________________________
