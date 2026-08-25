import json
from pathlib import Path

import jsonschema
import pytest

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "eval/goal_verify/v0/verification-spec.schema.json").read_text()
)
FIXTURES = ROOT / "tests/fixtures/verification_spec_v0"


@pytest.mark.parametrize("intent", ["create", "fix", "investigate"])
def test_golden_provider_proposals_conform(intent: str) -> None:
    proposal = json.loads((FIXTURES / f"{intent}.json").read_text())
    jsonschema.Draft202012Validator(SCHEMA).validate(proposal)


def test_unknown_golden_is_outside_v0_schema() -> None:
    proposal = json.loads((FIXTURES / "unknown.json").read_text())
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(proposal)
