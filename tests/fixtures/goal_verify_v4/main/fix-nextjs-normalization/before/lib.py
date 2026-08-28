"""Normalization intentionally omits Unicode digit conversion."""


def normalize(value: str) -> str:
    return value.strip()
