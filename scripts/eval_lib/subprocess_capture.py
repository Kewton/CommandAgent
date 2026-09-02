from __future__ import annotations

from typing import Final

MAX_CAPTURE_BYTES: Final = 1024 * 1024


def normalize_subprocess_capture(
    value: str | bytes | None, *, max_bytes: int = MAX_CAPTURE_BYTES
) -> tuple[str, bool]:
    """Return a bounded UTF-8 string for subprocess output, including timeouts."""
    if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or max_bytes < 0:
        raise ValueError("max_bytes must be a non-negative integer")
    if value is None:
        return "", False
    if isinstance(value, str):
        raw = value.encode("utf-8")
    elif isinstance(value, bytes):
        raw = value
    else:
        raise TypeError(f"unsupported subprocess capture type: {type(value).__name__}")
    clipped = raw[:max_bytes]
    return clipped.decode("utf-8", errors="replace"), len(raw) > max_bytes
