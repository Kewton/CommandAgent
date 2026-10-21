#!/usr/bin/env python3
"""Verify saved Issue 450 GUI smoke artifacts; does not repeat browser actions."""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from browser_preflight.evidence import CONTRACT, diagnose, file_hash, read_json
from browser_preflight.viewports import requirements, size


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--viewports", type=Path, required=True)
    args = parser.parse_args()
    root = args.root
    configured = requirements(read_json(args.viewports))
    results = {}
    for method, expected in (
        ("isolated-profile", []),
        ("plugin", ["conditions_unverified"]),
    ):
        reports = list((root / method / "ledger").glob("*/report.json"))
        assert reports, f"Missing {method} report"
        path = max(reports, key=lambda p: read_json(p)["finished_at"])
        report = read_json(path)
        assert report["contract"] == CONTRACT
        assert report["request"]["required_viewports"] == configured
        current = diagnose(report, report["observation"], path.parent)
        assert all(report[k] == v for k, v in current.items()), "Evidence changed"
        assert report["blockers"] == expected, report["blockers"]
        assert report["observation"]["stage"] == "complete"
        assert report["artifacts"]["screenshot"]["format"] in ("PNG", "JPEG")
        for required in configured:
            image = report["artifacts"]["views"][required["name"]]["screenshot"]
            assert size(image) == size(required)
        if method == "plugin":
            assert report["observation"]["owned_tab_closed"] is True
            assert report["observation"]["tabs_count"] == 0
            assert report["observation"]["viewport_override_reset"] is True
            assert not (root / method / "must-not-freeze").exists()
            assert read_json(root / "plugin-server/smoke-result.json")[
                "owned_server_closed"
            ]
        else:
            smoke = read_json(root / method / "smoke-result.json")
            assert smoke["status"] == "passed"
            assert all(
                smoke[k] is True
                for k in (
                    "owned_browser_closed",
                    "owned_server_closed",
                    "healthy_reused",
                    "changed_target_launch_refused",
                )
            )
            assert smoke["model_generation_started"] is False
            manifest = read_json(root / method / "campaign/browser-manifest.json")
            assert manifest["report_sha256"] == file_hash(path)
            assert manifest["conditions"] == report["observation"]["conditions"]
            assert manifest["required_viewports"] == configured
        results[method] = {
            "capabilities": "passed",
            "gate_blockers": expected,
            "image_sha256": report["artifacts"]["screenshot"]["sha256"],
            "viewport_images": report["artifacts"]["views"],
        }
    print(json.dumps({"status": "passed", "results": results}, indent=2))


if __name__ == "__main__":
    main()
