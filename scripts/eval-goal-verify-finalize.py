#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_finalize import finalize


def main() -> int:
    parser = argparse.ArgumentParser(description="Finalize Phase 6 live A/B evidence")
    parser.add_argument("--campaign-dir", type=Path, required=True)
    parser.add_argument("--blind-dir", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument(
        "--template",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-matrix.json"),
    )
    parser.add_argument(
        "--config", type=Path, default=Path("eval/goal_verify/v0/baseline-config.json")
    )
    args = parser.parse_args()

    def absolute(path: Path) -> Path:
        return path if path.is_absolute() else ROOT / path

    result = finalize(
        root=ROOT,
        campaign_dir=absolute(args.campaign_dir),
        blind_dir=absolute(args.blind_dir),
        template_path=absolute(args.template),
        config_path=absolute(args.config),
        attempt_id=args.attempt_id,
    )
    print(
        f"[done] final_decision={result['final_decision']} "
        f"same_script_replay_byte_identical={str(result['same_script_replay_byte_identical']).lower()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
