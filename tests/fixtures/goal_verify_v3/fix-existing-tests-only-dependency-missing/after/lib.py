"""Row parsing helpers."""


def parse_rows(text: str) -> list[list[str]]:
    return [line.split(",") for line in text.splitlines() if line.strip()]
