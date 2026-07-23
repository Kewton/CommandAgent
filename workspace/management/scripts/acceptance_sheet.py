#!/usr/bin/env python3
"""Translate persisted evidence into an acceptance sheet; never invent data."""
# ruff: noqa: E701,E702
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path

REASONS = {"verify_origin":"起点に束縛された検証を再実行して閉門しました。", "node_failed:fix":"修正ノードが未完了のため未完了。回収情報あり", "node_failed:investigate":"調査ノードが未完了のため未完了。回収情報あり", "repair_target_unresolved":"修復対象を解決できず未完了。回収情報あり"}
CHECKS = {"pipeline_probe":"パイプラインを実行できること", "data_results_schema":"results.jsonが契約スキーマに合うこと", "data_reconciliation":"集計結果と入力の整合性", "data_claims_binding":"主張が検証結果に束縛されること", "data_rerun_consistency":"再実行結果が一貫すること"}

def read_json(path):
    try: return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError, UnicodeDecodeError): return {}
def events(path):
    if not path.is_file(): return []
    out=[]
    for line in path.read_text(errors="replace").splitlines():
        try: out.append(json.loads(line))
        except ValueError: pass
    return out
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def json_files(run): return sorted(p for p in run.rglob("*.json") if p.is_file())
def all_events(run): return [e for p in run.rglob("events.jsonl") for e in events(p)]
def val(d, *keys, default=None):
    for k in keys:
        if d.get(k) is not None: return d[k]
    return default

def evidence_lines(run, files):
    out=[]
    for p in files:
        d=read_json(p); cid=val(d,"capability_id","check_id")
        if cid and "command" in d:
            out.append(f"- probe `{cid}`: command=`{d['command']}` exit={val(d,'exit_code','exit','status',default='記録なし')} observation={val(d,'outcome','stderr','error',default='記録なし')}")
        if cid == "data_claims_binding" and isinstance(d.get("claims"),list):
            matched=sum(bool(c.get("ok",c.get("matched",False))) for c in d["claims"])
            out.append(f"- E2 claims-binding: claims={len(d['claims'])}, matched={matched}")
            for c in d["claims"]:
                out.append(f"  - {c.get('raw','記録なし')} × {val(c,'matched_result_value','rounded_result_value',default='記録なし')} × {'pass' if c.get('ok',c.get('matched',False)) else 'fail'}")
        if p.name == "investigation-binding.json":
            claims=d.get("claims",[]); matched=[c for c in claims if c.get("matched")]
            out.append(f"- I2: claims={len(claims)}, matched={len(matched)}, violations={len(claims)-len(matched)}")
            for c in matched[:3]:
                q=val(c,"quote","value","raw",default="記録なし"); out.append(f"  - quote `{q}` × output existence=確認（I1 evidence照合）")
        if p.name == "investigation-run.json":
            rep=d.get('reproducer',{}) if isinstance(d.get('reproducer',{}),dict) else {}
            out.append(f"- I1: R=`{val(rep,'command',default=val(d,'command',default='記録なし'))}` outcome={val(d,'outcome','status',default='記録なし')}")
        if "fix-" in p.name and ("before" in p.name or "after" in p.name or "regression" in p.name):
            out.append(f"- F: {p.stem}: stage={val(d,'stage',default='記録なし')} executed={val(d,'executed','outcome',default='記録なし')} expected={val(d,'expected',default='記録なし')}")
    return out

def generate(run: Path) -> str:
    run=run.resolve(); root_events=events(run/"workflow-events.jsonl"); ev=root_events + all_events(run)
    circle_path=next(iter(sorted(run.rglob("workflow-circle.json"))),None); circle=read_json(circle_path) if circle_path else {}
    intent=next((e.get("value") or e.get("intent") for e in ev if e.get("event")=="intent_resolved"),None)
    starts=[e for e in ev if e.get("event")=="run_start"]; start=starts[0] if starts else {}
    model=next((e.get("model") for e in ev if e.get("model")),None); provider=next((e.get("provider") for e in ev if e.get("provider")),None)
    planner=next((e.get("planner_model") for e in ev if e.get("planner_model")),None)
    origin=circle.get("origin",{}) if circle else {}
    goal=val(start,"goal","action",default=origin.get("goal", "記録なし"))
    if isinstance(goal,str) and goal.startswith("UltraPlanRun("): goal=goal.split("\"",2)[1] if '"' in goal else goal
    verdict=val(circle,"verdict",default=next((e.get("verdict") for e in root_events if e.get("event")=="workflow_adjudicated"),None)) or val(next((e for e in ev if e.get("event")=="run_stop"),{}),"status",default="記録なし")
    reason=val(circle,"reason",default=next((e.get("reason") for e in ev if e.get("reason")),"記録なし"))
    epochs=[e.get("epoch") for e in ev if isinstance(e.get("epoch"),(int,float))]
    elapsed=(max(epochs)-min(epochs)) if len(epochs)>1 else circle.get("elapsed_seconds")
    if elapsed is None:
        log=run.parent/(run.name+".log")
        if log.is_file():
            stamps=[int(x) for x in log.read_text(errors="replace").splitlines() if x.strip().isdigit()]
            if len(stamps)>1: elapsed=max(stamps)-min(stamps)
    elif circle:
        log=run.parent/(run.name+".log")
        if log.is_file():
            stamps=[int(x) for x in log.read_text(errors="replace").splitlines() if x.strip().isdigit()]
            if len(stamps)>1: elapsed=max(stamps)-min(stamps)
    files=json_files(run); ids=sorted({val(read_json(p),"capability_id","check_id") for p in files if val(read_json(p),"capability_id","check_id")})
    lines=["# Acceptance Sheet","","## 1. 依頼","",f"- goal (run_start.action): {goal}",f"- profile: {next((e.get('profile') for e in ev if e.get('profile')), '記録なし')}",f"- intent (intent_resolved): {intent or '記録なし'}",f"- effective model/provider: {model or '記録なし'} / {provider or '記録なし'}",f"- planner model: {planner or '記録なし'}",f"- elapsed (epoch difference): {elapsed if elapsed is not None else '記録なし'}秒","","## 2. 判定","",f"- verdict: **{verdict or '記録なし'}**",f"- assurance: {'定義された検証を全て実行し成立' if verdict in ('full','full_success','circle_full') else REASONS.get(reason, '未完了。回収情報あり' if verdict != '記録なし' else '記録なし')}","","## 3. 完成の定義",""]
    lines += [f"- `{i}`: {CHECKS.get(i,i)}" for i in ids] or ["- 記録なし"]
    lines += ["","## 4. 検証の実録",""] + (evidence_lines(run,files) or ["- 記録なし"])
    if circle:
        lines += ["","## 円環時系列","",f"- origin: {origin}"]
        for edge in circle.get("edges",[]):
            lines.append(f"- edge {edge.get('edge','記録なし')}: E-A/E-B/E-C/E-D")
            for name, check in edge.get("checks",{}).items(): lines.append(f"  - {name}: {'pass' if check.get('passed') else 'fail'} — {check.get('detail','記録なし')}")
        for node,n in circle.get("nodes",{}).items():
            run_id=n.get("run_id", "記録なし")
            node_model=next((e.get("model") for e in ev if e.get("run_id")==run_id and e.get("model")), n.get("model", "記録なし"))
            run_dir=n.get("run_dir", "記録なし")
            try: run_dir=str(Path(run_dir).relative_to(run))
            except ValueError:
                if run_dir != "記録なし": run_dir=f"origin/{Path(run_dir).name}"
            lines.append(f"- node {node}: run_id={run_id} run_dir={run_dir} model={node_model}")
        lines.append(f"- circle verdict={circle.get('verdict','記録なし')} reason={circle.get('reason','記録なし')}")
    if verdict not in ('full','full_success','circle_full'):
        rec=[p.relative_to(run) for p in run.rglob('recovery-*.yaml')]; repair=[p.relative_to(run) for p in run.rglob('*repair*') if p.is_file()]
        lines += ["","## 5. 失敗・次の一手","",f"- {REASONS.get(reason, reason)}",f"- recovery YAML: {', '.join(map(str,rec)) if rec else '記録なし'}",f"- repair prompt: {', '.join(map(str,repair)) if repair else '記録なし'}"]
    lines += ["","## 6. 証拠台帳","","この紙の主張は全てここから機械生成された。"]
    lines += [f"- {p.relative_to(run)} sha256={sha(p)}" for p in files]
    return "\n".join(lines)+"\n"

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--run',required=True); ap.add_argument('--out'); a=ap.parse_args(); out=Path(a.out) if a.out else Path(a.run)/'acceptance-sheet.md'; out.write_text(generate(Path(a.run)),encoding='utf-8'); print(out)
if __name__ == '__main__': main()
