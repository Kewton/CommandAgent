#!/usr/bin/env python3
"""CM-1b Community Mini App validator.

The S validator is deliberately independent of the adversarial fixture text.
It validates a pinned platform schema, a closed AppSpec vocabulary, and a
bounded statically typed computed-expression AST. Z/B and event emission are
added in the subsequent CM-1b commit.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml


SCHEMA_VERSION = "community.app-spec/v1"
ROOT_FIELDS = {
    "entities",
    "views",
    "actions",
    "validations",
    "computed",
    "permissions",
    "minIdentity",
}
MAX_AST_DEPTH = 12
MAX_AST_NODES = 64
TOKEN_RE = re.compile(
    r"\s*(?:(?P<number>\d+(?:\.\d+)?)|(?P<string>'[^']*'|\"[^\"]*\")|"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)|(?P<op>==|!=|<=|>=|&&|\|\||[()+\-*/%,?:<>!]))"
)
ALLOWED_FUNCTIONS = {"min", "max", "len"}


class ValidationError(ValueError):
    pass


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _kind(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "list"
    if isinstance(value, dict):
        return "mapping"
    return "unknown"


@dataclass(frozen=True)
class Token:
    kind: str
    value: str


def tokenize(expression: str) -> list[Token]:
    tokens: list[Token] = []
    position = 0
    while position < len(expression):
        match = TOKEN_RE.match(expression, position)
        if not match:
            raise ValidationError(f"computed token is not in the closed set at {position}")
        position = match.end()
        kind = match.lastgroup
        assert kind is not None
        tokens.append(Token(kind, match.group(kind)))
    tokens.append(Token("eof", ""))
    return tokens


class ExpressionParser:
    def __init__(self, expression: str, fields: dict[str, str]):
        self.tokens = tokenize(expression)
        self.index = 0
        self.fields = fields
        self.nodes = 0
        self.depth = 0

    def current(self) -> Token:
        return self.tokens[self.index]

    def take(self, value: str | None = None) -> Token:
        token = self.current()
        if value is not None and token.value != value:
            raise ValidationError(f"expected {value!r}, found {token.value!r}")
        self.index += 1
        return token

    def node(self, depth: int) -> None:
        self.nodes += 1
        self.depth = max(self.depth, depth)
        if self.nodes > MAX_AST_NODES:
            raise ValidationError("computed AST node limit exceeded")
        if depth > MAX_AST_DEPTH:
            raise ValidationError("computed AST depth limit exceeded")

    def parse(self) -> str:
        result = self.parse_conditional(1)
        if self.current().kind != "eof":
            raise ValidationError(f"unexpected computed token {self.current().value!r}")
        return result

    def parse_conditional(self, depth: int) -> str:
        left = self.parse_binary(0, depth)
        if self.current().value == "?":
            self.node(depth)
            self.take("?")
            true_type = self.parse_conditional(depth + 1)
            self.take(":")
            false_type = self.parse_conditional(depth + 1)
            if left != "boolean" or true_type != false_type:
                raise ValidationError("conditional requires boolean condition and equal branch types")
            return true_type
        return left

    def parse_binary(self, minimum: int, depth: int) -> str:
        left = self.parse_unary(depth)
        precedence = {"||": 1, "&&": 2, "==": 3, "!=": 3, "<": 4, ">": 4, "<=": 4, ">=": 4, "+": 5, "-": 5, "*": 6, "/": 6, "%": 6}
        while self.current().value in precedence and precedence[self.current().value] >= minimum:
            operator = self.take().value
            level = precedence[operator] + 1
            right = self.parse_binary(level, depth + 1)
            self.node(depth)
            if operator in {"&&", "||"}:
                if left != "boolean" or right != "boolean":
                    raise ValidationError(f"{operator} requires boolean operands")
                left = "boolean"
            elif operator in {"<", ">", "<=", ">=", "+", "-", "*", "/", "%"}:
                if left != "number" or right != "number":
                    raise ValidationError(f"{operator} requires number operands")
                left = "number"
            else:
                if left != right:
                    raise ValidationError(f"{operator} requires matching operand types")
                left = "boolean"
        return left

    def parse_unary(self, depth: int) -> str:
        if self.current().value == "!":
            self.node(depth)
            self.take("!")
            if self.parse_unary(depth + 1) != "boolean":
                raise ValidationError("! requires a boolean operand")
            return "boolean"
        if self.current().value == "-":
            self.node(depth)
            self.take("-")
            if self.parse_unary(depth + 1) != "number":
                raise ValidationError("unary - requires a number operand")
            return "number"
        return self.parse_primary(depth)

    def parse_primary(self, depth: int) -> str:
        token = self.current()
        if token.kind == "number":
            self.node(depth)
            self.take()
            return "number"
        if token.kind == "string":
            self.node(depth)
            self.take()
            return "string"
        if token.kind == "name":
            self.node(depth)
            name = self.take().value
            if name in {"true", "false"}:
                return "boolean"
            if self.current().value == "(":
                if name not in ALLOWED_FUNCTIONS:
                    raise ValidationError(f"unregistered computed function {name!r}")
                self.take("(")
                args: list[str] = []
                if self.current().value != ")":
                    args.append(self.parse_conditional(depth + 1))
                    while self.current().value == ",":
                        self.take(",")
                        args.append(self.parse_conditional(depth + 1))
                self.take(")")
                if name in {"min", "max"} and not args:
                    raise ValidationError(f"{name} requires arguments")
                if name in {"min", "max"} and any(arg != "number" for arg in args):
                    raise ValidationError(f"{name} requires number arguments")
                if name == "len" and len(args) != 1:
                    raise ValidationError("len requires one argument")
                if name == "len" and args[0] not in {"list", "string"}:
                    raise ValidationError("len requires a list or string")
                return "number"
            if name not in self.fields:
                raise ValidationError(f"unregistered computed field {name!r}")
            return self.fields[name]
        if token.value == "(":
            self.node(depth)
            self.take("(")
            result = self.parse_conditional(depth + 1)
            self.take(")")
            return result
        raise ValidationError(f"unexpected computed token {token.value!r}")


def load_yaml(path: Path) -> Any:
    try:
        return yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as exc:
        raise ValidationError(f"invalid YAML: {exc}") from exc


def validate_schema_pin(schema_path: Path, pin_path: Path) -> str:
    observed = sha256_file(schema_path)
    expected = pin_path.read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValidationError("schema pin is not a lowercase SHA-256")
    if observed != expected:
        raise ValidationError(f"schema pin mismatch: expected {expected}, observed {observed}")
    schema = load_yaml(schema_path)
    if not isinstance(schema, dict) or schema.get("schema_version") != SCHEMA_VERSION:
        raise ValidationError("unsupported platform schema version")
    if schema.get("fields") != {
        "entities": "list", "views": "list", "actions": "list",
        "validations": "list", "computed": "list", "permissions": "list",
        "minIdentity": "mapping",
    }:
        raise ValidationError("schema field vocabulary drift")
    return observed


def validate_spec(spec_path: Path, schema_path: Path, pin_path: Path) -> dict[str, Any]:
    schema_pin = validate_schema_pin(schema_path, pin_path)
    spec = load_yaml(spec_path)
    if not isinstance(spec, dict):
        raise ValidationError("app.spec.yaml must be a mapping")
    unknown = set(spec) - ROOT_FIELDS
    missing = ROOT_FIELDS - set(spec)
    if unknown or missing:
        raise ValidationError(f"closed AppSpec vocabulary mismatch: unknown={sorted(unknown)}, missing={sorted(missing)}")
    expected_types = {"entities": "list", "views": "list", "actions": "list", "validations": "list", "computed": "list", "permissions": "list", "minIdentity": "mapping"}
    for field, expected in expected_types.items():
        if _kind(spec[field]) != expected:
            raise ValidationError(f"{field} must be {expected}")
    fields: dict[str, str] = {}
    for entity in spec["entities"]:
        if not isinstance(entity, dict) or set(entity) - {"name", "fields"} or not isinstance(entity.get("name"), str):
            raise ValidationError("entity vocabulary mismatch")
        for field_name, field_type in (entity.get("fields") or {}).items():
            if field_type not in {"number", "string", "boolean", "list"}:
                raise ValidationError(f"unsupported entity field type {field_type!r}")
            fields[field_name] = field_type
    for item in spec["computed"]:
        if not isinstance(item, dict) or set(item) != {"name", "expression", "type"}:
            raise ValidationError("computed entry vocabulary mismatch")
        if item["type"] not in {"number", "string", "boolean", "list"}:
            raise ValidationError("computed type is outside the closed set")
        actual_type = ExpressionParser(str(item["expression"]), fields).parse()
        if actual_type != item["type"]:
            raise ValidationError(f"computed {item['name']!r} type mismatch: {actual_type} != {item['type']}")
        fields[str(item["name"])] = actual_type
    return {"family": "S", "verdict": "pass", "schema_pin_sha256": schema_pin, "spec_sha256": sha256_file(spec_path), "computed_fields": sorted(fields)}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("--schema-pin", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        print(json.dumps(validate_spec(args.spec, args.schema, args.schema_pin), sort_keys=True))
        return 0
    except (OSError, ValidationError, TypeError, ValueError) as exc:
        print(json.dumps({"family": "S", "verdict": "violation", "error": str(exc)}, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
