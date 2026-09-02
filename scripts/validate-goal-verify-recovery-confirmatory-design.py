from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from eval_lib.goal_verify_recovery_confirmatory_design import validate_design


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("design", type=Path)
    args = parser.parse_args()
    result = validate_design(json.loads(args.design.read_text(encoding="utf-8")))
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0 if result["valid"] else 1


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    raise SystemExit(main())
