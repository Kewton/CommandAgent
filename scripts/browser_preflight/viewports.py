"""Explicit viewport requirements, independent of campaign-specific dimensions."""

import re


def requirements(value):
    if not isinstance(value, list) or not value:
        raise ValueError("Configure a nonempty required_viewports list")
    names = set()
    for item in value:
        if (
            not isinstance(item, dict)
            or set(item) != {"name", "width", "height"}
            or not isinstance(item["name"], str)
            or not re.fullmatch(r"[A-Za-z0-9_-]{1,64}", item["name"])
            or item["name"] in names
            or any(type(item[k]) is not int or item[k] < 2 for k in ("width", "height"))
        ):
            raise ValueError(
                "Each viewport requires a unique safe name and integer pixel dimensions"
            )
        names.add(item["name"])
    return value


def size(viewport):
    return {k: viewport[k] for k in ("width", "height")}
