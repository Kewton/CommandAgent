import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from env_compat import _resolve  # noqa: E402


def resolve_case(values: dict[str, str]) -> tuple[str | None, list[str]]:
    warnings: list[str] = []
    value = _resolve(
        "COMMANDAGENT_TEST_VALUE",
        values,
        set(),
        lambda legacy, current: warnings.append(f"{legacy}->{current}"),
    )
    return value, warnings


def test_environment_precedence_matrix() -> None:
    assert resolve_case({"COMMANDAGENT_TEST_VALUE": "current"}) == ("current", [])
    assert resolve_case({"ANVIL_TEST_VALUE": "legacy"}) == (
        "legacy",
        ["ANVIL_TEST_VALUE->COMMANDAGENT_TEST_VALUE"],
    )
    assert resolve_case(
        {"COMMANDAGENT_TEST_VALUE": "current", "ANVIL_TEST_VALUE": "legacy"}
    ) == ("current", [])
    assert resolve_case({}) == (None, [])


def test_legacy_only_environment_warns_once() -> None:
    warnings: list[str] = []
    warned_names: set[str] = set()
    for _ in range(2):
        assert (
            _resolve(
                "COMMANDAGENT_TEST_VALUE",
                {"ANVIL_TEST_VALUE": "legacy"},
                warned_names,
                lambda legacy, current: warnings.append(f"{legacy}->{current}"),
            )
            == "legacy"
        )
    assert warnings == ["ANVIL_TEST_VALUE->COMMANDAGENT_TEST_VALUE"]
