from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

_REGISTERED_NEXT_POLICIES = {
    "14.2.35": {
        "safe_build_argv": ["npx", "next", "build"],
        "safe_start_argv_template": ["npx", "next", "start", "-p", "<port>"],
        "server_policy": "next_14_production_build_start_v1",
    },
    "16.3.1": {
        "safe_build_argv": ["npx", "next", "build", "--webpack"],
        "safe_start_argv_template": ["npx", "next", "start", "-p", "<port>"],
        "server_policy": "next_16_webpack_build_start_v1",
    },
}


def detect_executor_capabilities(workspace: Path, *, profile: str) -> dict[str, Any]:
    package = workspace / "node_modules" / "next" / "package.json"
    if not package.is_file() and profile != "nextjs":
        return {}
    version = None
    reason = "next_package_missing"
    if package.is_file():
        try:
            value = json.loads(package.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError):
            reason = "next_package_invalid"
        else:
            if isinstance(value, dict) and isinstance(value.get("version"), str):
                version = value["version"]
                reason = "next_version_unregistered"
            else:
                reason = "next_version_missing"
    package_sha256 = (
        hashlib.sha256(package.read_bytes()).hexdigest() if package.is_file() else None
    )
    policy = _REGISTERED_NEXT_POLICIES.get(version)
    if policy is None:
        return {
            "next": {
                "status": "safe_execution_unavailable",
                "next_version": version,
                "reason": reason,
                "next_package_sha256": package_sha256,
                "safe_build_argv": None,
                "safe_start_argv_template": None,
            }
        }
    return {
        "next": {
            "status": "registered",
            "next_version": version,
            "next_package_sha256": package_sha256,
            **policy,
        }
    }


def candidate_visible_executor_capabilities(
    executor_capabilities: dict[str, Any],
) -> dict[str, Any]:
    next_capability = executor_capabilities.get("next")
    if not isinstance(next_capability, dict):
        return {}
    return {
        "next": {
            key: next_capability.get(key)
            for key in (
                "status",
                "next_version",
                "reason",
                "safe_build_argv",
                "safe_start_argv_template",
            )
            if key in next_capability
        }
    }


def executor_capabilities_match_workspace(
    workspace: Path, executor_capabilities: dict[str, Any]
) -> bool:
    profile = "nextjs" if "next" in executor_capabilities else "generic"
    return (
        detect_executor_capabilities(workspace, profile=profile)
        == executor_capabilities
    )


def validate_next_command_argv(
    argv: list[str], executor_capabilities: dict[str, Any] | None
) -> tuple[str, str | None]:
    operation = _next_operation(
        argv,
        next_workspace=isinstance(executor_capabilities, dict)
        and "next" in executor_capabilities,
    )
    if operation not in {"build", "start"}:
        return "not_applicable", None
    capability, reason = _registered_next_capability(executor_capabilities)
    if capability is None:
        return "executor_unavailable", reason
    if operation == "build":
        accepted = argv == capability["safe_build_argv"]
    else:
        template = capability["safe_start_argv_template"]
        accepted = len(argv) == len(template) and all(
            actual == expected
            if expected != "<port>"
            else actual.isdecimal() and 1 <= int(actual) <= 65535
            for actual, expected in zip(argv, template, strict=True)
        )
    if not accepted:
        return "policy_rejected", f"next_{operation}_argv_not_registered"
    return "accepted", None


def next_web_server_policy(
    *,
    candidate_argv: list[str],
    port: int,
    executor_capabilities: dict[str, Any] | None,
) -> dict[str, Any] | None:
    next_workspace = (
        isinstance(executor_capabilities, dict) and "next" in executor_capabilities
    )
    if (
        not next_workspace
        and _next_operation(candidate_argv, next_workspace=False) is None
    ):
        return None
    capability, reason = _registered_next_capability(executor_capabilities)
    if capability is None:
        return {"classification": "executor_unavailable", "reason": reason}
    start = [
        str(port) if value == "<port>" else value
        for value in capability["safe_start_argv_template"]
    ]
    if candidate_argv != start:
        return {
            "classification": "policy_rejected",
            "reason": "next_start_argv_not_registered",
        }
    return {
        "classification": "executable",
        "prepare_argv": list(capability["safe_build_argv"]),
        "server_argv": start,
        "server_policy": capability["server_policy"],
        "next_version": capability["next_version"],
    }


def _registered_next_capability(
    executor_capabilities: dict[str, Any] | None,
) -> tuple[dict[str, Any] | None, str]:
    next_capability = (
        executor_capabilities.get("next")
        if isinstance(executor_capabilities, dict)
        else None
    )
    if not isinstance(next_capability, dict):
        return None, "safe_execution_unavailable:next_capability_missing"
    if next_capability.get("status") != "registered":
        reason = next_capability.get("reason", "next_version_unregistered")
        return None, f"safe_execution_unavailable:{reason}"
    version = next_capability.get("next_version")
    policy = _REGISTERED_NEXT_POLICIES.get(version)
    if policy is None or any(
        next_capability.get(key) != policy[key]
        for key in ("safe_build_argv", "safe_start_argv_template", "server_policy")
    ):
        return None, "safe_execution_unavailable:next_capability_invalid"
    return next_capability, ""


def _next_operation(argv: list[str], *, next_workspace: bool) -> str | None:
    names = [Path(value).name for value in argv]
    if names and names[0] == "next":
        return argv[1] if len(argv) > 1 else None
    if len(names) >= 2 and names[:2] == ["npx", "next"]:
        return argv[2] if len(argv) > 2 else None
    if len(names) >= 3 and names[:3] in (
        ["npm", "exec", "next"],
        ["pnpm", "exec", "next"],
        ["yarn", "exec", "next"],
    ):
        return argv[3] if len(argv) > 3 else None
    if next_workspace and len(names) >= 3 and names[:2] == ["npm", "run"]:
        return argv[2] if argv[2] in {"build", "start"} else None
    if next_workspace and len(names) >= 2 and names[0] in {"pnpm", "yarn"}:
        return argv[1] if argv[1] in {"build", "start"} else None
    return None
