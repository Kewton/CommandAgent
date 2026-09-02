import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from lib import normalize  # noqa: E402


def test_strips_surrounding_whitespace():
    assert normalize("  abc  ") == "abc"


def test_keeps_ascii_digits():
    assert normalize("123") == "123"
