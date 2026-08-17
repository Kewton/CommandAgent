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
import shutil
import subprocess
import tempfile
import threading
from dataclasses import dataclass
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
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


FORBIDDEN_PATTERNS = {
    "process.env": re.compile(r"\bprocess\s*\.\s*env\b"),
    "eval": re.compile(r"\beval\s*\("),
    "child_process": re.compile(r"\b(?:require\s*\(\s*['\"]child_process|from\s+['\"]child_process)"),
    "raw_fetch": re.compile(r"\bfetch\s*\("),
    "dynamic_import": re.compile(r"\bimport\s*\("),
}


def _iter_source_files(root: Path) -> list[Path]:
    suffixes = {".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs"}
    return sorted(path for path in root.rglob("*") if path.is_file() and path.suffix in suffixes and "node_modules" not in path.parts)


def validate_core_snapshot(root: Path, manifest_path: Path) -> None:
    rows = [line.split(None, 1) for line in manifest_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if not rows:
        raise ValidationError("core snapshot manifest is empty")
    expected = {relative: digest for digest, relative in rows}
    observed: dict[str, str] = {}
    for path in sorted((root / "core").rglob("*")):
        if path.is_file():
            observed[str(path.relative_to(root))] = sha256_file(path)
    if observed != expected:
        raise ValidationError(f"core snapshot changed: expected={sorted(expected)}, observed={sorted(observed)}")


def validate_lockfile(root: Path) -> None:
    package_path = root / "package.json"
    lock_path = root / "package-lock.json"
    if not package_path.is_file() or not lock_path.is_file():
        raise ValidationError("package.json and package-lock.json are required")
    package = json.loads(package_path.read_text(encoding="utf-8"))
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    dependencies = dict(package.get("dependencies", {}))
    if dependencies:
        raise ValidationError("dependency is not in the empty initial allowlist")
    packages = lock.get("packages")
    if not isinstance(packages, dict) or "" not in packages:
        raise ValidationError("lockfile root package is missing")
    for name, entry in packages.items():
        if name == "":
            continue
        if not isinstance(entry, dict) or not entry.get("integrity"):
            raise ValidationError(f"lockfile hash is missing for {name}")


def validate_zone(root: Path, core_manifest: Path, changed_paths: list[str]) -> dict[str, Any]:
    validate_core_snapshot(root, core_manifest)
    core_changes = [path for path in changed_paths if path == "core" or path.startswith("core/")]
    if core_changes:
        raise ValidationError(f"core diff is non-empty: {core_changes}")
    findings: list[str] = []
    for source in _iter_source_files(root):
        text = source.read_text(encoding="utf-8")
        for name, pattern in FORBIDDEN_PATTERNS.items():
            if pattern.search(text):
                findings.append(f"{name}:{source.relative_to(root)}")
    if findings:
        raise ValidationError("forbidden API detected: " + ", ".join(sorted(findings)))
    validate_lockfile(root)
    return {"family": "Z", "verdict": "pass", "core_snapshot_sha256": sha256_file(core_manifest), "changed_paths": changed_paths, "dependency_allowlist": []}


def _find_esbuild(explicit: str | None) -> str | None:
    if explicit:
        return explicit if Path(explicit).is_file() else None
    return shutil.which("esbuild")


def _serve(directory: Path) -> tuple[ThreadingHTTPServer, threading.Thread, int]:
    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args: Any, **kwargs: Any) -> None:
            super().__init__(*args, directory=str(directory), **kwargs)

        def log_message(self, format: str, *args: Any) -> None:
            return

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread, server.server_port


def derive_smoke_assertions(spec: dict[str, Any]) -> dict[str, Any]:
    views = [str(item["name"]) for item in spec["views"] if isinstance(item, dict) and "name" in item]
    actions = [str(item["name"]) for item in spec["actions"] if isinstance(item, dict) and "name" in item]
    if not views or not actions:
        raise ValidationError("AppSpec does not provide smoke-derivable views/actions")
    return {"view_selectors": [f'[data-testid="{name}"]' for name in views], "action_selectors": [f'[data-action="{name}"]' for name in actions]}


def validate_build_and_smoke(root: Path, spec_path: Path, esbuild: str | None) -> dict[str, Any]:
    binary = _find_esbuild(esbuild)
    if binary is None:
        raise ValidationError("esbuild is unavailable")
    spec = load_yaml(spec_path)
    assertions = derive_smoke_assertions(spec)
    app_source = root / "src/app-zone/app.ts"
    html_source = root / "src/app-zone/index.html"
    if not app_source.is_file() or not html_source.is_file():
        raise ValidationError("synthetic Community build inputs are incomplete")
    with tempfile.TemporaryDirectory(prefix="cm1b-build-") as output:
        outdir = Path(output)
        bundle = outdir / "app.js"
        subprocess.run([binary, str(app_source), "--bundle", "--format=esm", "--platform=browser", f"--outfile={bundle}"], check=True, capture_output=True, text=True)
        (outdir / "index.html").write_text(html_source.read_text(encoding="utf-8").replace("./app.ts", "./app.js"), encoding="utf-8")
        try:
            from playwright.sync_api import sync_playwright
        except ImportError as exc:
            raise ValidationError("Playwright is unavailable") from exc
        server, thread, port = _serve(outdir)
        try:
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(headless=True)
                page = browser.new_page()
                page.goto(f"http://127.0.0.1:{port}/index.html", wait_until="networkidle")
                for selector in assertions["view_selectors"]:
                    page.locator(selector).wait_for()
                for selector in assertions["action_selectors"]:
                    page.locator(selector).wait_for()
                if "[data-action=\"increment\"]" in assertions["action_selectors"]:
                    page.locator('[data-action="increment"]').click()
                    if page.locator(assertions["view_selectors"][0]).inner_text() != "1":
                        raise ValidationError("AppSpec-derived increment smoke assertion failed")
                if "[data-action=\"reset\"]" in assertions["action_selectors"]:
                    page.locator('[data-action="reset"]').click()
                    if page.locator(assertions["view_selectors"][0]).inner_text() != "0":
                        raise ValidationError("AppSpec-derived reset smoke assertion failed")
                browser.close()
        finally:
            server.shutdown()
            thread.join(timeout=2)
    return {"family": "B", "verdict": "pass", "esbuild": binary, "smoke": "playwright", "assertions": assertions}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("--schema-pin", type=Path, required=True)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--core-manifest", type=Path)
    parser.add_argument("--changed-path", action="append", default=[])
    parser.add_argument("--esbuild")
    parser.add_argument("--build-smoke", action="store_true")
    args = parser.parse_args(argv)
    try:
        result: dict[str, Any] = validate_spec(args.spec, args.schema, args.schema_pin)
        if args.root is not None:
            if args.core_manifest is None:
                raise ValidationError("--core-manifest is required with --root")
            result["zone"] = validate_zone(args.root, args.core_manifest, args.changed_path)
            if args.build_smoke:
                result["build"] = validate_build_and_smoke(args.root, args.spec, args.esbuild)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (OSError, ValidationError, TypeError, ValueError) as exc:
        print(json.dumps({"family": "S/Z/B", "verdict": "violation", "error": str(exc)}, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
