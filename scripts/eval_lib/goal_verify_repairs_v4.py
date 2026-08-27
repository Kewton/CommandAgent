from __future__ import annotations

import copy
import hashlib
import json
from typing import Any


def apply_meaning_preserving_repairs(
    proposal: dict[str, Any],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    repaired = copy.deepcopy(proposal)
    repairs: list[dict[str, Any]] = []
    for oracle in repaired.get("oracles", []):
        if not isinstance(oracle, dict):
            continue
        oracle_id = str(oracle.get("id", "unknown"))
        before_sha256 = _binding_hash(oracle)
        kinds = []
        setup = oracle.get("setup")
        input_value = oracle.get("input")
        observation = oracle.get("observation")

        if (
            isinstance(setup, dict)
            and "fixture_paths" not in setup
            and isinstance(input_value, dict)
            and input_value.get("kind") != "fixture"
        ):
            setup["fixture_paths"] = []
            kinds.append("default_empty_fixture_paths")

        if (
            isinstance(input_value, dict)
            and input_value.get("kind") == "http"
            and "route" in input_value
            and "path" not in input_value
        ):
            input_value["path"] = input_value.pop("route")
            kinds.append("http_route_to_path_alias")

        if (
            isinstance(input_value, dict)
            and input_value.get("kind") == "dom"
            and "computed_style_property" in input_value
            and "property" not in input_value
        ):
            input_value["property"] = input_value.pop("computed_style_property")
            kinds.append("dom_computed_style_property_alias")

        if _can_move_actions(oracle, input_value, observation):
            input_value["actions"] = observation.pop("actions")
            kinds.append("interaction_actions_to_input")

        if not kinds:
            continue
        after_sha256 = _binding_hash(oracle)
        lineage = oracle.get("lineage")
        if isinstance(lineage, dict):
            lineage["concretized_binding_sha256"] = after_sha256
            lineage["semantic_equivalence"] = True
            lineage["repair_kind"] = "+".join(kinds)
        repairs.append(
            {
                "oracle_id": oracle_id,
                "repair_kinds": kinds,
                "semantic_equivalence": True,
                "before_binding_sha256": before_sha256,
                "after_binding_sha256": after_sha256,
            }
        )
    return repaired, repairs


def _can_move_actions(oracle, input_value, observation) -> bool:
    return (
        oracle.get("strategy") == "interaction"
        and isinstance(input_value, dict)
        and input_value.get("kind") == "dom"
        and "actions" not in input_value
        and isinstance(observation, dict)
        and isinstance(observation.get("actions"), list)
        and set(observation) == {"kind", "expected", "actions"}
        and observation.get("kind") == "interaction"
    )


def _binding_hash(oracle: dict[str, Any]) -> str:
    semantic = {
        key: value
        for key, value in oracle.items()
        if key not in {"id", "lineage", "lifecycle", "result", "observed_strength"}
    }
    payload = json.dumps(
        semantic, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()
