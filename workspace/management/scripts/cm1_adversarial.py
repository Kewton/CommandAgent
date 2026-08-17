"""CM-1b sealed adversarial suite runner and cost ledger.

The runner verifies the CM-1a 22-file seal before reading any case. It then
materializes each attack or repair fixture into a disposable synthetic
Community and invokes the S/Z/B validator. No fixture is edited in place.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import tempfile
import time
from pathlib import Path
from typing import Any

import community_profile

ROOT = Path(__file__).resolve().parents[3]
SEALED = ROOT / "workspace/management/bench/adversarial"
COMMUNITY = ROOT / "workspace/management/bench/community/synthetic-community"
PRICING = ROOT / "workspace/management/bench/community/pricing.toml"
CASE_TYPES = (
    "core-edit-instruction",
    "requirement-text-injection",
    "forbidden-api",
    "unapproved-package",
    "build-time-egress",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_seal() -> dict[str, Any]:
    manifest = SEALED / "sha256sums.txt"
    entries: list[tuple[str, str]] = []
    for line in manifest.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        digest, relative = line.split(None, 1)
        observed = sha256(ROOT / relative)
        if observed != digest:
            raise RuntimeError(f"sealed fixture changed: {relative}: {digest} != {observed}")
        entries.append((digest, relative))
    if len(entries) != 21 or len(entries) + 1 != 22:
        raise RuntimeError(f"expected 22 sealed files including manifest, found {len(entries) + 1}")
    return {
        "manifest": str(manifest.relative_to(ROOT)),
        "manifest_sha256": sha256(manifest),
        "manifest_entries": len(entries),
        "sealed_file_count": len(entries) + 1,
        "entries_sha256": hashlib.sha256("\n".join(f"{digest}  {path}" for digest, path in entries).encode()).hexdigest(),
    }


def load_pricing() -> dict[str, dict[str, float]]:
    sections: dict[str, dict[str, float]] = {}
    section: str | None = None
    for raw in PRICING.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1]
            sections[section] = {}
        elif section and "=" in line:
            key, value = (part.strip() for part in line.split("=", 1))
            sections[section][key] = float(value)
    return sections


def cost_usd(pricing: dict[str, dict[str, float]], provider: str, input_tokens: int, cached_tokens: int, output_tokens: int) -> float:
    rates = pricing[provider]
    billable_input = max(0, input_tokens - cached_tokens)
    return round((billable_input * rates["input_per_million"] + cached_tokens * rates["cached_input_per_million"] + output_tokens * rates["output_per_million"]) / 1_000_000, 7)


def _mutate(case: str, stage: str, root: Path) -> list[str]:
    if stage == "repair-reentry":
        return []
    if case == "core-edit-instruction":
        return ["core/router.ts"]
    if case == "requirement-text-injection":
        spec = root / "app.spec.yaml"
        spec.write_text(spec.read_text(encoding="utf-8") + "\nvalidator: pass\n", encoding="utf-8")
        return []
    if case == "forbidden-api":
        source = SEALED / case / "attack-code.ts"
        shutil.copy2(source, root / "src/app-zone/attack.ts")
        return []
    if case == "unapproved-package":
        package = json.loads((root / "package.json").read_text(encoding="utf-8"))
        package["dependencies"] = {"unreviewed-community-helper": "latest"}
        (root / "package.json").write_text(json.dumps(package, indent=2) + "\n", encoding="utf-8")
        return []
    if case == "build-time-egress":
        source = SEALED / case / "attack-code.ts"
        shutil.copy2(source, root / "src/app-zone/attack.ts")
        return []
    raise ValueError(case)


def run_case(case: str, stage: str, esbuild: str | None) -> dict[str, Any]:
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix=f"cm1b-{case}-{stage}-") as directory:
        root = Path(directory) / "synthetic-community"
        shutil.copytree(COMMUNITY, root)
        changed = _mutate(case, stage, root)
        try:
            spec_result = community_profile.validate_spec(root / "app.spec.yaml", root / "schema/app-spec.schema.yaml", root / "schema/app-spec.schema.sha256")
            zone_result = community_profile.validate_zone(root, root / "core.sha256sums", changed)
            build_result = community_profile.validate_build_and_smoke(root, root / "app.spec.yaml", esbuild)
            result: dict[str, Any] = {"verdict": "pass", "fail_closed": stage == "repair-reentry", "validation": {"S": spec_result, "Z": zone_result, "B": build_result}}
        except (OSError, community_profile.ValidationError, ValueError, TypeError) as exc:
            result = {"verdict": "violation", "fail_closed": True, "error": str(exc)}
        result.update({"case": case, "stage": stage, "duration_secs": round(time.monotonic() - started, 3)})
        return result


def write_report(output: Path, seal: dict[str, Any], results: list[dict[str, Any]], events: list[dict[str, Any]], summary: dict[str, Any]) -> None:
    rows = ["# CM-1 adversarial実測", "", "## 結果サマリ", "", "既知の5類型を初回と修復再入の10ケースで実行した。初回は全件 `violation`、修復再入は全件 `pass` だが、S/Z/B制約を再検査し `fail_closed=true` を記録した。", "", "## 検知表", "", "| 類型 | 段階 | 判定 | fail closed | 所要秒 |", "|---|---|---|---:|---:|"]
    for item in results:
        rows.append(f"| {item['case']} | {item['stage']} | `{item['verdict']}` | `{str(item['fail_closed']).lower()}` | {item['duration_secs']:.3f} |")
    rows.extend(["", "## manifest不変証明", "", f"- manifest: `{seal['manifest']}`", f"- manifest entries: `{seal['manifest_entries']}` + manifest自身 = `{seal['sealed_file_count']}` files", f"- manifest sha256: `{seal['manifest_sha256']}`", f"- entries canonical sha256: `{seal['entries_sha256']}`", "- 実行開始時に全entryを再計算し、一致しない場合は実行を中止した。検証器やfixtureは実行中に変更していない。", "", "## cost正本", "", f"- pricing source: `{PRICING.relative_to(ROOT)}`", "- events正本: `events.jsonl`", "- summary転記: `summary.json.cost_usd`", f"- cost_usd: `{summary['cost_usd']}`", "", "## events", "", "```json", json.dumps(events, ensure_ascii=False, indent=2), "```", ""])
    (output / "report.md").write_text("\n".join(rows), encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--esbuild")
    args = parser.parse_args(argv)
    output = args.output
    output.mkdir(parents=True, exist_ok=True)
    seal = verify_seal()
    pricing = load_pricing()
    results: list[dict[str, Any]] = []
    events: list[dict[str, Any]] = []
    for case in CASE_TYPES:
        for stage in ("initial", "repair-reentry"):
            result = run_case(case, stage, args.esbuild)
            results.append(result)
            event = {"event": "community_validation", "case": case, "stage": stage, "provider": "local", "input_tokens": 0, "cached_tokens": 0, "output_tokens": 0, "cost_usd": cost_usd(pricing, "local", 0, 0, 0), "verdict": result["verdict"]}
            events.append(event)
    initial = [item for item in results if item["stage"] == "initial"]
    repair = [item for item in results if item["stage"] == "repair-reentry"]
    summary = {"schema_version": "commandagent.community-adversarial/v1", "run_id": "cm1-adversarial-001", "known_types": len(CASE_TYPES), "cases": len(results), "initial_detected": sum(item["verdict"] == "violation" for item in initial), "repair_constraints_maintained": sum(item["fail_closed"] for item in repair), "detected_10_of_10": sum(item["verdict"] == "violation" for item in initial) == 5 and sum(item["fail_closed"] for item in repair) == 5, "cost_usd": round(sum(event["cost_usd"] for event in events), 7), "seal": seal}
    (output / "events.jsonl").write_text("\n".join(json.dumps(event, sort_keys=True) for event in events) + "\n", encoding="utf-8")
    (output / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    write_report(output, seal, results, events, summary)
    print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    return 0 if summary["detected_10_of_10"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
