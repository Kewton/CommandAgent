import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import cli  # noqa: E402


def test_first_five_values():
    assert [cli.fib(n) for n in range(1, 6)] == [1, 1, 2, 3, 5]


def test_invalid_input_exits_2():
    assert cli.main(["cli.py", "x"]) == 2
    assert cli.main(["cli.py"]) == 2
    assert cli.main(["cli.py", "0"]) == 2
