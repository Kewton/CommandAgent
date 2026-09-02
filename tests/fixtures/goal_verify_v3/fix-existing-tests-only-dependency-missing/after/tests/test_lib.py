import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from lib import parse_rows  # noqa: E402


def test_parses_two_rows():
    assert parse_rows("a,b\n1,2\n") == [["a", "b"], ["1", "2"]]


def test_skips_blank_lines():
    assert parse_rows("\n\na\n") == [["a"]]
