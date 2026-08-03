from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT = ROOT / "data/snapshots/events.html"
OUTPUT = ROOT / "output"


def main() -> None:
    text = SNAPSHOT.read_text(encoding="utf-8")
    match = re.search(r"<article>(.*?)</article>", text, flags=re.DOTALL)
    if match is None:
        raise SystemExit("recorded article missing")
    records = [{"name": match.group(1).strip()}]
    OUTPUT.mkdir(parents=True, exist_ok=True)
    (OUTPUT / "records.json").write_text(
        json.dumps(records, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    (OUTPUT / "report.md").write_text(
        "# Ingest report\n\nrecords: 1\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
