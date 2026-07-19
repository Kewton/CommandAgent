from __future__ import annotations

import os
import sys
from collections.abc import Callable, Mapping

CURRENT_PREFIX = "COMMANDAGENT_"
LEGACY_PREFIX = "ANVIL_"

_warned_legacy_names: set[str] = set()


def getenv(name: str, default: str | None = None) -> str | None:
    value = _resolve(name, os.environ, _warned_legacy_names, _warn)
    return default if value is None else value


def _resolve(
    name: str,
    environ: Mapping[str, str],
    warned_names: set[str],
    warn: Callable[[str, str], None],
) -> str | None:
    if name in environ:
        return environ[name]
    if not name.startswith(CURRENT_PREFIX):
        return None
    legacy_name = f"{LEGACY_PREFIX}{name.removeprefix(CURRENT_PREFIX)}"
    if legacy_name not in environ:
        return None
    if legacy_name not in warned_names:
        warned_names.add(legacy_name)
        warn(legacy_name, name)
    return environ[legacy_name]


def _warn(legacy_name: str, current_name: str) -> None:
    print(
        f"warning: {legacy_name} is deprecated; use {current_name} instead",
        file=sys.stderr,
    )
