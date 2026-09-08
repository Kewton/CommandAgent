"""Independently compile frozen R0 variants; never run against the original app."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

FIXTURE = Path(__file__).resolve().parents[1] / "tests/corpus/apps/issue448-nextjs-r0"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def files(root):
    return {str(p.relative_to(root)): sha256(p) for p in sorted(root.rglob("*")) if p.is_file()}


def assert_provenance():
    provenance = json.loads((FIXTURE / "provenance.json").read_text())
    assert {"original/" + p: h for p, h in files(FIXTURE / "original").items()} == provenance["original_sha256"]
    manifest = json.loads((FIXTURE / "fixture-sha256.json").read_text())
    assert {p: h for p, h in files(FIXTURE).items() if p != "fixture-sha256.json"} == manifest


def materialize(case, destination):
    # copytree refuses an existing destination, preventing accidental in-place repair.
    shutil.copytree(FIXTURE / "original", destination)
    for overlay in case["overlays"]:
        source = FIXTURE / "overlays" / overlay
        shutil.copytree(source, destination, dirs_exist_ok=True)
    return destination


def run(command, cwd, env):
    result = subprocess.run(command, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=240, check=False)
    return result.returncode, re.sub(r"\x1b\[[0-9;]*m", "", result.stdout)


def diagnostic_excerpt(output):
    """Keep compiler findings and locations, not raw process logs or local paths."""
    return "\n".join(line for line in output.splitlines() if
                     line.startswith(("./src/", "Type error:", "Attempted import error:",
                                      "  Type '", "    Type '", "Failed to compile.")))


def run_matrix():
    assert_provenance()
    before = files(FIXTURE)
    cases = json.loads((FIXTURE / "cases.json").read_text())
    env = dict(os.environ, NEXT_TELEMETRY_DISABLED="1", NO_COLOR="1", CI="1")
    results = []
    with tempfile.TemporaryDirectory(prefix="issue448-build-") as temporary:
        scratch = Path(temporary)
        dependencies = scratch / "dependencies"
        dependencies.mkdir()
        for name in ["package.json", "package-lock.json"]:
            shutil.copyfile(FIXTURE / "original" / name, dependencies / name)
        # Cache/log writes stay inside disposable storage as well.
        env["npm_config_cache"] = str(scratch / "npm-cache")
        print("Installing the frozen package-lock.json", flush=True)
        install_command = ["npm", "ci", "--include=dev", "--no-audit", "--no-fund"]
        install_exit, install_output = run(install_command, dependencies, env)
        assert install_exit == 0, install_output
        assert sha256(dependencies / "package-lock.json") == sha256(FIXTURE / "original/package-lock.json")
        versions = {package: json.loads((dependencies / "node_modules" / package / "package.json").read_text())["version"]
                    for package in ["next", "typescript", "react", "react-dom"]}
        assert versions["next"] == "14.2.35"
        for case in cases:
            workspace = materialize(case, scratch / case["name"])
            source_before = files(workspace)
            (workspace / "node_modules").symlink_to(dependencies / "node_modules", target_is_directory=True)
            print("Building " + case["name"], flush=True)
            exit_code, output = run(["npm", "run", "build"], workspace, env)
            assert exit_code == case["exit_code"], case["name"] + "\n" + output
            assert "Linting and checking validity of types" in output, output
            for expected in case["diagnostics"]:
                assert expected in output, case["name"] + " missing " + expected + "\n" + output
            for absent in case["absent"]:
                assert absent not in output, case["name"] + " unexpected " + absent + "\n" + output
            # The compiler may create .next output; every input must remain unchanged.
            assert {p: sha256(workspace / p) for p in source_before} == source_before
            if exit_code == 0:
                assert "Generating static pages (6/6)" in output, output
                for route in ["/api/projects", "/api/tasks", "/api/tasks/[id]"]:
                    assert route in output, output
                validate_status_helper(workspace, env)
            results.append({"case": case["name"], "command": "npm run build", "exit_code": exit_code,
                            "type_check_observed": True, "diagnostics": diagnostic_excerpt(output),
                            "source_sha256": source_before})
        node_exit, node_version = run(["node", "--version"], scratch, env)
        npm_exit, npm_version = run(["npm", "--version"], scratch, env)
        assert node_exit == npm_exit == 0
    assert files(FIXTURE) == before
    return {"schema_version": 1, "kind": "measured independent Next.js builds",
            "node": node_version.strip(), "npm": npm_version.strip(), "dependencies": versions,
            "install_command": " ".join(install_command), "install_exit_code": install_exit,
            "fixture_inputs_sha256": {p: h for p, h in before.items()
                                       if p.startswith(("original/", "overlays/")) or p == "cases.json"},
            "results": results, "status_helper_cases": 13,
            "limitations": "No model/browser campaign or concurrent writes. Deadlock and data loss unobserved; build success is not full R0 acceptance."}


def validate_status_helper(workspace, env):
    # Execute just the real typed helper after the complete, strict Next build.
    code = """
const ts = require('typescript');
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const source = fs.readFileSync('src/lib/types.ts', 'utf8');
const compiled = ts.transpileModule(source, {compilerOptions: {module: ts.ModuleKind.CommonJS}});
const context = {exports: {}};
vm.runInNewContext(compiled.outputText, context);
const validate = context.exports.isValidTaskStatus;
for (const value of ['not_started', 'in_progress', 'completed']) assert.equal(validate(value), true);
for (const value of ['', 'done', 'COMPLETED', null, undefined, 0, 1, true, {}, []]) assert.equal(validate(value), false);
"""
    exit_code, output = run(["node", "-e", code], workspace, env)
    assert exit_code == 0, output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists; choose a new evidence file")
    report = run_matrix()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
