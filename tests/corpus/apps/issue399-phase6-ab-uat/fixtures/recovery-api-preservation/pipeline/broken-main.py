from pathlib import Path


def summarize(source: Path) -> dict:
    return {"source": source.name, "used_rows": 2}


def inspect(source: Path) -> dict:
    return summarize(source)
