"""Text normalization helpers."""

_FULLWIDTH_DIGITS = str.maketrans("０１２３４５６７８９", "0123456789")


def normalize(text: str) -> str:
    return text.strip().translate(_FULLWIDTH_DIGITS)
