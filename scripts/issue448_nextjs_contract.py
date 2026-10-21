"""Run real locked builds through Recovery finish with the unchanged R0 contract.

HTTP and browser interaction responses are explicitly scripted test inputs.
This does not run a browser, model, or live campaign.
"""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

from issue448_nextjs_r0 import FIXTURE, assert_provenance, files, run, sha256

ROOT = Path(__file__).resolve().parents[1]
COMMAND = ["cargo", "test", "--lib", "issue448_original_contract_finish_matrix", "--", "--nocapture"]
SCENARIOS = ["all_original_observations_pass", "build_failure", "failed_interaction",
             "missing_interaction", "http_failure", "missing_implementation"]


def parse_results(output):
    marker = "ISSUE448_ORIGINAL_CONTRACT "
    results = [json.loads(line.split(marker, 1)[1]) for line in output.splitlines() if marker in line]
    assert [row["scenario"] for row in results] == SCENARIOS, "incomplete or reordered matrix"
    for index, row in enumerate(results):
        assert row["build_mode"] == "real"
        assert row["build_exit_code"] == (1 if row["scenario"] == "build_failure" else 0)
        assert row["decision"]["decision"] == ("promoted" if index == 0 else "rejected")
        assert (row["control_before_sha256"] != row["control_after_sha256"]) == (index == 0)
    return results


def run_matrix():
    assert_provenance()
    before = files(FIXTURE)
    env = dict(os.environ, NEXT_TELEMETRY_DISABLED="1", CI="1", NO_COLOR="1")
    with tempfile.TemporaryDirectory(prefix="issue448-contract-") as temporary:
        scratch = Path(temporary)
        dependencies = scratch / "dependencies"
        dependencies.mkdir()
        for name in ["package.json", "package-lock.json"]:
            shutil.copyfile(FIXTURE / "original" / name, dependencies / name)
        env["npm_config_cache"] = str(scratch / "npm-cache")
        install = ["npm", "ci", "--include=dev", "--no-audit", "--no-fund"]
        print("Installing the original dependency lock", flush=True)
        code, output = run(install, dependencies, env)
        assert code == 0, output
        assert sha256(dependencies / "package-lock.json") == sha256(FIXTURE / "original/package-lock.json")
        versions = {p: json.loads((dependencies / "node_modules" / p / "package.json").read_text())["version"]
                    for p in ["next", "typescript", "react", "react-dom"]}
        assert versions["next"] == "14.2.35"
        assert versions["typescript"] == "5.9.3"
        env["ISSUE448_REAL_NODE_MODULES"] = str(dependencies / "node_modules")
        env["ISSUE448_REAL_NPM"] = shutil.which("npm")
        print("Running six original-contract finish cases with real builds and scripted HTTP/interaction", flush=True)
        completed = subprocess.run(COMMAND, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, timeout=900, check=False)
        assert completed.returncode == 0, completed.stdout
        results = parse_results(completed.stdout)
        node_code, node = run(["node", "--version"], scratch, env)
        npm_code, npm = run(["npm", "--version"], scratch, env)
        assert node_code == npm_code == 0
    assert files(FIXTURE) == before
    return {"schema_version": 1, "kind": "real locked builds; scripted HTTP/interaction inputs; normal Next.js finish/promotion",
            "command": " ".join(COMMAND), "node": node.strip(), "npm": npm.strip(), "dependencies": versions,
            "install_command": " ".join(install), "install_exit_code": code,
            "original_corpus_sha256": before, "results": results,
            "limitations": "No real browser/model acceptance. No concurrent lock test; deadlock and data loss unobserved."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists; choose a new evidence file")
    report = run_matrix()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print("Passed all six original-contract decisions", flush=True)


if __name__ == "__main__":
    main()
