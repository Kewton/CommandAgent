#!/usr/bin/env python3
"""Run the sealed CM-1b attacks through the Rust product path."""
from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "workspace/management/bench/adversarial"
BASE = ROOT / "workspace/management/bench/community/synthetic-community"
BINARY = ROOT / "target/debug/commandagent"
OUT = ROOT / "workspace/management/runs/cm2a-product-adversarial"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest_ok() -> bool:
    rows = [line.split() for line in (FIXTURES / "sha256sums.txt").read_text().splitlines() if line.strip()]
    return len(rows) == 21 and all(digest(ROOT / row[1]) == row[0] for row in rows)


def mutate(root: Path, kind: str) -> None:
    attack = FIXTURES / kind
    if kind == "core-edit-instruction":
        (root / "core/README.md").write_text((root / "core/README.md").read_text() + "\nATTACKER_EDIT\n")
    elif kind == "requirement-text-injection":
        spec = root / "app.spec.yaml"
        spec.write_text(spec.read_text() + "\nunknown_injected_key: true\n")
    elif kind == "forbidden-api":
        shutil.copy2(attack / "attack-code.ts", root / "src/app-zone/attack.ts")
    elif kind == "unapproved-package":
        package = json.loads((root / "package.json").read_text())
        package["dependencies"] = {"evil-package": "1.0.0"}
        (root / "package.json").write_text(json.dumps(package, indent=2) + "\n")
    elif kind == "build-time-egress":
        shutil.copy2(attack / "attack-code.ts", root / "src/app-zone/attack.ts")


def run_case(kind: str, stage: str) -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix=f"cm2a-{kind}-{stage}-") as temp:
        workspace = Path(temp) / "community"
        shutil.copytree(BASE, workspace)
        if stage == "initial":
            mutate(workspace, kind)
        command = [
            str(BINARY), "--cwd", str(workspace), "--profile", "community-mini-app",
            "--offline", "--summary-json", "--no-footer", "--prompt", "CM-2a verify community artifact",
        ]
        completed = subprocess.run(command, text=True, capture_output=True, check=False)
        lines = [line for line in completed.stdout.splitlines() if line.strip()]
        summary = json.loads(lines[-1])
        expected_failed = stage == "initial"
        passed = (completed.returncode != 0) == expected_failed and (
            summary.get("stop_class") == "community_profile_violation" if expected_failed else summary.get("verdict") == "full"
        )
        return {"kind": kind, "stage": stage, "exit": completed.returncode, "passed": passed, "summary": summary}


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    if not manifest_ok():
        raise SystemExit("sealed adversarial manifest mismatch")
    rows = [run_case(kind, stage) for kind in sorted(p.name for p in FIXTURES.iterdir() if p.is_dir()) for stage in ("initial", "repair")]
    (OUT / "summary.json").write_text(json.dumps({"manifest_entries": 21, "rows": rows}, indent=2) + "\n")
    lines = ["# CM-2a product adversarial probe", "", "Manifest: 21 sealed fixture entries verified before execution.", "", "| Type | Stage | Exit | Verdict | Stop class | Result |", "|---|---|---:|---|---|---|"]
    for row in rows:
        summary = row["summary"]
        lines.append(f"| {row['kind']} | {row['stage']} | {row['exit']} | {summary.get('verdict')} | {summary.get('stop_class') or ''} | {'PASS' if row['passed'] else 'FAIL'} |")
    (OUT / "report.md").write_text("\n".join(lines) + "\n")
    return 0 if all(row["passed"] for row in rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
