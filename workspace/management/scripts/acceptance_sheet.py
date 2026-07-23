#!/usr/bin/env python3
# ruff: noqa: E701,E702
"""Generate a human-readable acceptance sheet from persisted evidence only."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path

REASONS = {"verify_origin": "起点に束縛された検証を再実行して閉門しました。", "node_failed:fix": "修正ノードが未完了のため未完了。回収情報あり", "node_failed:investigate": "調査ノードが未完了のため未完了。回収情報あり"}
CHECKS = {"pipeline_probe": "パイプラインを実行できること", "data_results_schema": "results.jsonが契約スキーマに合うこと", "data_reconciliation": "集計結果と入力の整合性", "data_claims_binding": "主張が検証結果に束縛されること", "data_rerun_consistency": "再実行結果が一貫すること"}

def events(path):
    if not path.is_file(): return []
    out=[]
    for line in path.read_text(errors="replace").splitlines():
        try: out.append(json.loads(line))
        except json.JSONDecodeError: pass
    return out

def sha(path):
    h=hashlib.sha256(); h.update(path.read_bytes()); return h.hexdigest()

def generate(run: Path) -> str:
    run=run.resolve(); ev=events(run/"workflow-events.jsonl")
    circle=run/"workflow-circle.json"; circle_data=json.loads(circle.read_text()) if circle.is_file() else None
    all_events=ev + events(run/"investigate-events.jsonl") + events(run/"fix-events.jsonl")
    starts=[e for e in all_events if e.get("event")=="run_start"]
    start=starts[0] if starts else {}
    adjud=[e for e in ev if e.get("event")=="workflow_adjudicated"]
    stop=adjud[-1] if adjud else next((e for e in all_events if e.get("event")=="run_stop"), {})
    verdict=(circle_data or {}).get("verdict") or stop.get("verdict") or stop.get("status") or "記録なし"
    reason=(circle_data or {}).get("reason") or stop.get("reason") or "記録なし"
    lines=["# Acceptance Sheet", "", "## 1. 依頼", "", f"- goal: {start.get('goal') or (circle_data or {}).get('origin',{}).get('goal') or '記録なし'}", f"- profile: {start.get('profile') or '記録なし'}", f"- intent: {start.get('intent') or '記録なし'}", f"- model/provider: {start.get('model','記録なし')} / {start.get('provider','記録なし')}", "", "## 2. 判定", "", f"- verdict: **{verdict}**", f"- assurance: {('定義された検証を全て実行し成立' if verdict in ('full','full_success','circle_full') else REASONS.get(reason, '未完了。回収情報あり' if verdict != '記録なし' else '記録なし'))}", "", "## 3. 完成の定義", ""]
    evidence=sorted(p for p in run.rglob('*') if p.is_file() and ('evidence' in p.parts or p.name.endswith('.json')))
    ids=[]
    for p in evidence:
        try:
            d=json.loads(p.read_text()); ids.extend([d.get('check_id'),d.get('capability_id')])
        except (json.JSONDecodeError,UnicodeDecodeError): pass
    for cid in sorted({i for i in ids if i}): lines.append(f"- `{cid}`: {CHECKS.get(cid,cid)}")
    if not ids: lines.append("- 記録なし")
    lines += ["", "## 4. 検証の実録", ""]
    for p in evidence:
        lines.append(f"- {p.relative_to(run)}: 実在ファイルを参照（観測値は原文のまま）")
    if not evidence: lines.append("- 記録なし")
    if verdict not in ('full','full_success','circle_full'):
        lines += ["", "## 5. 失敗・次の一手", "", f"- {REASONS.get(reason, reason)}", "- recovery/repair prompt: 記録なし（実在パスのみ転記）"]
    lines += ["", "## 6. 証拠台帳", "", "この紙の主張は全てここから機械生成された。"]
    for p in evidence: lines.append(f"- {p.relative_to(run)} sha256={sha(p)}")
    return "\n".join(lines)+"\n"

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--run',required=True); ap.add_argument('--out'); a=ap.parse_args()
    out=Path(a.out) if a.out else Path(a.run)/'acceptance-sheet.md'; out.write_text(generate(Path(a.run)), encoding='utf-8'); print(out)
if __name__ == '__main__': main()
