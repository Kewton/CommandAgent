#!/usr/bin/env python3
from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import cm4_parallel_probe


class Cm4ParallelProbeTests(unittest.TestCase):
    def test_final_json_line_requires_headless_schema(self) -> None:
        parsed = cm4_parallel_probe.final_json_line(
            'progress\n{"schema_version":"commandagent.headless-summary/v1","run_id":"r"}\n'
        )
        self.assertEqual(parsed["run_id"], "r")

    def test_isolation_detects_foreign_workspace_reference(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            records = []
            for name in ("one", "two"):
                workspace = root / "workspaces" / name
                state = root / "states" / name
                workspace.mkdir(parents=True)
                state.mkdir(parents=True)
                events = state / "events.jsonl"
                events.write_text(
                    json.dumps({"workspace_root": str(workspace)}) + "\n",
                    encoding="utf-8",
                )
                records.append(
                    {
                        "name": name,
                        "workspace": str(workspace),
                        "state_dir": str(state),
                        "summary": {
                            "run_id": name,
                            "artifacts_dir": str(workspace),
                            "events_path": str(events),
                        },
                    }
                )
            self.assertTrue(
                cm4_parallel_probe.verify_isolation(records)[
                    "cross_contamination_zero"
                ]
            )
            foreign = records[1]["workspace"]
            Path(records[0]["summary"]["events_path"]).write_text(
                foreign, encoding="utf-8"
            )
            self.assertFalse(
                cm4_parallel_probe.verify_isolation(records)[
                    "cross_contamination_zero"
                ]
            )


if __name__ == "__main__":
    unittest.main()
