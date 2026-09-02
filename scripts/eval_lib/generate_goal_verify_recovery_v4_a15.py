from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import (
    ROOT,
    _file_sha256,
    _json_sha256,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_1 import (
    _build_adapters as _build_a14_adapters,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_1 import (
    _build_tasks as _build_a14_tasks,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a14 import (
    _build_contract as _build_a14_contract,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    classify_case_recovery_eligibility,
)

EVAL = ROOT / "eval/goal_verify/v0"
FIXTURES = ROOT / "tests/fixtures/goal_verify_v4/a15"

BASE_CORPUS = EVAL / "phase6-main-corpus-v4.json"
BASE_WORKSPACES = EVAL / "phase6-real-workspaces-v4-main.json"
BASE_NEXT_WORKSPACES = EVAL / "phase6-real-workspaces-v3.json"
BASE_NEXT_REFERENCE = (
    ROOT
    / "tests/fixtures/goal_verify_v3/create-ui-copy-style-port-path/reference"
)

CORPUS_PATH = EVAL / "phase6-recovery-v4-a15-corpus.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a15.json"
ADAPTERS_PATH = EVAL / "phase6-command-adapters-v4-a15.json"
WORKSPACES_PATH = EVAL / "phase6-real-workspaces-v4-a15.json"
SMOKE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-smoke-contract.json"
FULL_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-full-contract.json"

SMOKE_CONTRACT_ID = "phase6-recovery-v4-20260831-a15-smoke-01"
FULL_CONTRACT_ID = "phase6-recovery-v4-20260831-a15-live-01"
DATA_WORKSPACE_ID = "a15-fix-data-reconciliation"
NEXT_WORKSPACE_ID = "a15-fix-nextjs-route-label"
NEXT_PORT = 4185
VARIANT_COUNT = 10

PROFILE_CELLS = {
    "cell-05": "cli",
    "cell-07": "generic",
    "cell-13": "data",
    "cell-14": "nextjs",
}


def _case_id(cell: int, task: int) -> str:
    return f"phase6-main-c{cell:02d}-task-{task:02d}"


def _source_task_id(cell: int, task: int) -> str:
    return f"phase6-cell-{cell:02d}-source-task-{task:02d}"


def _source_sha(goal: str) -> str:
    return hashlib.sha256(goal.encode("utf-8")).hexdigest()


def _fixture_sha(relative: str) -> str:
    return _file_sha256(FIXTURES / relative)


def _build_fixtures() -> None:
    _build_data_fixtures()
    _build_next_fixtures()


def _build_data_fixtures() -> None:
    root = FIXTURES / "fix-data-reconciliation"
    before_pipeline = _data_pipeline(used_rows_expression="len(rows)")
    after_pipeline = _data_pipeline(used_rows_expression="len(valid_rows)")
    for stage, pipeline in (("before", before_pipeline), ("after", after_pipeline)):
        stage_root = root / stage
        _write(stage_root / "pipeline/main.py", pipeline)
        _write(stage_root / "scripts/repro.py", _data_reproducer())
        _write(stage_root / "scripts/contract_check.py", _data_contract_check())
        _write(stage_root / "tests/test_pipeline.py", _data_regression_test())
        for task in range(1, VARIANT_COUNT + 1):
            _write(
                stage_root / f"data/task-{task:02d}.csv",
                (
                    "region,amount\n"
                    f"north,{task}\n"
                    f"south,{task + 1}\n"
                    "unknown,not-a-number\n"
                ),
            )
        _write(
            stage_root / "output/inspection.json",
            json.dumps(
                {
                    "column_names": ["region", "amount"],
                    "input_row_count": 3,
                    "type_summaries": {
                        "region": "string",
                        "amount": "numeric_with_invalid",
                    },
                    "distinct_values": {
                        "region": ["north", "south", "unknown"]
                    },
                    "sample_rows": [{"region": "north", "amount": "1"}],
                },
                ensure_ascii=False,
                sort_keys=True,
            )
            + "\n",
        )
        used_rows = 3 if stage == "before" else 2
        _write(
            stage_root / "output/results.json",
            json.dumps(
                {
                    "reconciliation": {
                        "input_rows": 3,
                        "used_rows": used_rows,
                        "excluded": [
                            {"reason": "non_numeric_amount", "rows": 1}
                        ],
                    },
                    "values": {"total": 3},
                },
                ensure_ascii=False,
                sort_keys=True,
            )
            + "\n",
        )
        _write(stage_root / "output/report.md", "# Data summary\n\nTotal: 3\n")


def _data_pipeline(*, used_rows_expression: str) -> str:
    return f'''#!/usr/bin/env python3
"""Deterministic CSV aggregation used by the A15 data Recovery experiment."""
import csv
import json
import sys
from pathlib import Path


def summarize(source: Path) -> dict:
    with source.open(encoding="utf-8", newline="") as handle:
        rows = list(csv.DictReader(handle))
    valid_rows = []
    excluded = 0
    for row in rows:
        try:
            amount = int(row["amount"])
        except (KeyError, TypeError, ValueError):
            excluded += 1
            continue
        valid_rows.append(amount)
    return {{
        "reconciliation": {{
            "input_rows": len(rows),
            "used_rows": {used_rows_expression},
            "excluded": [{{"reason": "non_numeric_amount", "rows": excluded}}],
        }},
        "values": {{"total": sum(valid_rows)}},
    }}


def write_outputs(source: Path) -> dict:
    result = summarize(source)
    output = Path("output")
    output.mkdir(exist_ok=True)
    inspection = {{
        "column_names": ["region", "amount"],
        "input_row_count": result["reconciliation"]["input_rows"],
        "type_summaries": {{"region": "string", "amount": "numeric_with_invalid"}},
        "distinct_values": {{"region": ["north", "south", "unknown"]}},
        "sample_rows": [{{"region": "north", "amount": "1"}}],
    }}
    (output / "inspection.json").write_text(
        json.dumps(inspection, ensure_ascii=False, sort_keys=True) + "\\n",
        encoding="utf-8",
    )
    (output / "results.json").write_text(
        json.dumps(result, ensure_ascii=False, sort_keys=True) + "\\n",
        encoding="utf-8",
    )
    (output / "report.md").write_text(
        f"# Data summary\\n\\nTotal: {{result['values']['total']}}\\n",
        encoding="utf-8",
    )
    return result


if __name__ == "__main__":
    source = Path(sys.argv[1]) if len(sys.argv) == 2 else Path("data/task-01.csv")
    write_outputs(source)
'''


def _data_reproducer() -> str:
    return '''#!/usr/bin/env python3
import importlib.util
import sys
from pathlib import Path


def load_pipeline():
    spec = importlib.util.spec_from_file_location("a15_data_pipeline", "pipeline/main.py")
    if spec is None or spec.loader is None:
        raise RuntimeError("pipeline loader unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main(argv):
    if len(argv) != 2:
        return 2
    source = Path(argv[1])
    if source.parent != Path("data") or not source.is_file():
        return 2
    result = load_pipeline().write_outputs(source)
    task = int(source.stem.removeprefix("task-"))
    expected = {
        "reconciliation": {
            "input_rows": 3,
            "used_rows": 2,
            "excluded": [{"reason": "non_numeric_amount", "rows": 1}],
        },
        "values": {"total": task * 2 + 1},
    }
    if result != expected:
        print(f"expected {expected!r}, got {result!r}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
'''


def _data_contract_check() -> str:
    return '''#!/usr/bin/env python3
import json
from pathlib import Path


required = [
    Path("pipeline/main.py"),
    Path("output/inspection.json"),
    Path("output/results.json"),
    Path("output/report.md"),
]
if not all(path.is_file() for path in required):
    raise SystemExit(1)
document = json.loads(Path("output/results.json").read_text(encoding="utf-8"))
raise SystemExit(0 if set(document) == {"reconciliation", "values"} else 1)
'''


def _data_regression_test() -> str:
    return '''import importlib.util
from pathlib import Path


def load_pipeline():
    spec = importlib.util.spec_from_file_location("a15_data_pipeline", "pipeline/main.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_all_valid_rows_remain_counted(tmp_path: Path):
    source = tmp_path / "valid.csv"
    source.write_text("region,amount\\nnorth,2\\nsouth,4\\n", encoding="utf-8")
    result = load_pipeline().summarize(source)
    assert result["reconciliation"] == {
        "input_rows": 2,
        "used_rows": 2,
        "excluded": [{"reason": "non_numeric_amount", "rows": 0}],
    }
    assert result["values"]["total"] == 6
'''


def _build_next_fixtures() -> None:
    root = FIXTURES / "fix-nextjs-route-label"
    package_json = (BASE_NEXT_REFERENCE / "package.json").read_text(encoding="utf-8")
    package_lock = (BASE_NEXT_REFERENCE / "package-lock.json").read_text(
        encoding="utf-8"
    )
    next_config = (BASE_NEXT_REFERENCE / "next.config.js").read_text(encoding="utf-8")
    for stage, prefix in (("before", "stale"), ("after", "ready")):
        stage_root = root / stage
        _write(stage_root / "package.json", package_json)
        _write(stage_root / "package-lock.json", package_lock)
        _write(stage_root / "next.config.js", next_config)
        _write(
            stage_root / "app/layout.js",
            "export default function Layout({ children }) {\n"
            "  return <html lang=\"en\"><body>{children}</body></html>;\n"
            "}\n",
        )
        _write(stage_root / "app/page.js", _next_page())
        _write(
            stage_root / "lib/label.mjs",
            (
                "export function formatTask(task) {\n"
                f"  return `{prefix}-${{task}}`;\n"
                "}\n\n"
                "export function stableLabel(value) {\n"
                "  return value;\n"
                "}\n"
            ),
        )
        _write(stage_root / "scripts/repro.mjs", _next_reproducer())
        _write(stage_root / "scripts/regression.mjs", _next_regression())
        for task in range(1, VARIANT_COUNT + 1):
            task_text = f"{task:02d}"
            _write(
                stage_root / f"fixture/task-{task_text}.json",
                json.dumps(
                    {"task": task_text, "expected": f"ready-{task_text}"},
                    separators=(",", ":"),
                    sort_keys=True,
                )
                + "\n",
            )


def _next_page() -> str:
    tasks = ", ".join(f'"{task:02d}"' for task in range(1, VARIANT_COUNT + 1))
    return f'''import {{ formatTask }} from "../lib/label.mjs";

const tasks = [{tasks}];

export default function Page() {{
  return (
    <main>
      <h1>Recovery profile fixture</h1>
      {{tasks.map((task) => (
        <p id={{`result-${{task}}`}} key={{task}}>{{formatTask(task)}}</p>
      ))}}
    </main>
  );
}}
'''


def _next_reproducer() -> str:
    return '''#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { formatTask } from "../lib/label.mjs";

if (process.argv.length !== 3 || !process.argv[2].startsWith("fixture/task-")) {
  process.exit(2);
}
const fixture = JSON.parse(await readFile(process.argv[2], "utf8"));
const actual = formatTask(fixture.task);
if (actual !== fixture.expected) {
  console.error(`expected ${fixture.expected}, got ${actual}`);
  process.exit(1);
}
'''


def _next_regression() -> str:
    return '''#!/usr/bin/env node
import { stableLabel } from "../lib/label.mjs";

if (stableLabel("health") !== "health") {
  process.exit(1);
}
'''


def _data_goal(task: int) -> str:
    task_text = f"{task:02d}"
    return (
        "Fix the existing deterministic data pipeline so "
        f"`python3 scripts/repro.py data/task-{task_text}.csv` changes from exit 1 "
        "to exit 0, with input_rows = used_rows + excluded rows. Preserve the "
        "frozen pytest and contract-check regressions and regenerate the standard "
        "data profile outputs."
    )


def _next_goal(task: int) -> str:
    task_text = f"{task:02d}"
    return (
        "Fix the existing offline Next.js App Router project so "
        f"`node scripts/repro.mjs fixture/task-{task_text}.json` changes from exit "
        f"1 to exit 0 and / renders #result-{task_text} as ready-{task_text}. "
        "Preserve the frozen Node regression and complete a production build."
    )


def _build_corpus() -> dict[str, Any]:
    corpus = copy.deepcopy(_load(BASE_CORPUS))
    corpus["schema_version"] = "commandagent.goal_verify.recovery_corpus.v4_a15"
    corpus["status"] = "draft"
    corpus["cases"] = [
        row
        for row in corpus["cases"]
        if row["case_id"].startswith(
            (
                "phase6-main-c05-",
                "phase6-main-c06-",
                "phase6-main-c07-",
                "phase6-main-c08-",
            )
        )
    ]
    for task in range(1, VARIANT_COUNT + 1):
        data_goal = _data_goal(task)
        next_goal = _next_goal(task)
        data_fixture = f"data/task-{task:02d}.csv"
        next_fixture = f"fixture/task-{task:02d}.json"
        corpus["cases"].append(
            {
                "allowed_verdicts": ["full"],
                "case_id": _case_id(13, task),
                "cell_id": "cell-13",
                "forbidden_verdicts": ["partial", "failed", "unverified"],
                "goal": data_goal,
                "intent": "fix",
                "language": "en",
                "optional_claims": [],
                "polarity": "positive",
                "product_stage": "before",
                "profile": "data",
                "required_claims": [
                    {
                        "id": "exact-reproducer",
                        "min_strength": "runtime",
                        "oracle": {
                            "kind": "exit_code",
                            "expected": "before 1, after 0",
                        },
                    },
                    {
                        "id": "regressions",
                        "min_strength": "runtime",
                        "oracle": {
                            "kind": "regression_set",
                            "expected": "pytest and contract-check both exit 0",
                        },
                    },
                ],
                "size": "small",
                "source_task_id": _source_task_id(13, task),
                "source_template_case_id": DATA_WORKSPACE_ID,
                "tags": ["real_data_profile", "reconciliation", "offline_executable"],
                "task_variant": {
                    "fixture": data_fixture,
                    "expected_total": task * 2 + 1,
                },
                "workspace_case_id": DATA_WORKSPACE_ID,
            }
        )
        corpus["cases"].append(
            {
                "allowed_verdicts": ["full"],
                "case_id": _case_id(14, task),
                "cell_id": "cell-14",
                "forbidden_verdicts": ["partial", "failed", "unverified"],
                "goal": next_goal,
                "intent": "fix",
                "language": "en",
                "optional_claims": [],
                "polarity": "positive",
                "product_stage": "before",
                "profile": "nextjs",
                "required_claims": [
                    {
                        "id": "exact-reproducer",
                        "min_strength": "runtime",
                        "oracle": {
                            "kind": "exit_code",
                            "expected": "before 1, after 0",
                        },
                    },
                    {
                        "id": "regressions",
                        "min_strength": "runtime",
                        "oracle": {
                            "kind": "regression_set",
                            "expected": "Node regression and production build exit 0",
                        },
                    },
                    {
                        "id": "route-render",
                        "min_strength": "runtime",
                        "oracle": {
                            "kind": "dom",
                            "expected": f"#result-{task:02d} text is ready-{task:02d}",
                        },
                    },
                ],
                "size": "small",
                "source_task_id": _source_task_id(14, task),
                "source_template_case_id": NEXT_WORKSPACE_ID,
                "tags": ["real_nextjs_profile", "production_build", "browser_oracle"],
                "task_variant": {
                    "fixture": next_fixture,
                    "selector": f"#result-{task:02d}",
                    "expected_text": f"ready-{task:02d}",
                },
                "workspace_case_id": NEXT_WORKSPACE_ID,
            }
        )
    return corpus


def _base_task_row(case: dict[str, Any]) -> dict[str, Any]:
    return {
        "case_id": case["case_id"],
        "source_goal_sha256": _source_sha(case["goal"]),
        "goal": case["goal"],
        "goal_role": "shared semantic goal for both Recovery arms",
        "registered_claims": [row["id"] for row in case["required_claims"]],
        "dropped_from_a5_execution_goal": [],
        "decision": (
            "A15 adds a real, offline-executable profile task to the eligible "
            "Recovery population; it is not a dependency or profile-mismatch sentinel"
        ),
    }


def _build_tasks(*, status: str) -> dict[str, Any]:
    registry = copy.deepcopy(_build_a14_tasks())
    registry["cases"] = [
        row
        for row in registry["cases"]
        if row["case_id"].startswith(
            (
                "phase6-main-c05-",
                "phase6-main-c06-",
                "phase6-main-c07-",
                "phase6-main-c08-",
            )
        )
    ]
    corpus_by_id = {row["case_id"]: row for row in _build_corpus()["cases"]}
    registry["status"] = status
    registry["supersedes"] = str(
        Path("eval/goal_verify/v0/phase6-task-contracts-v4-a14-a13-1.json")
    )
    registry["policy"]["a15_real_profiles"] = (
        "data and nextjs eligible tasks must use their actual admitted profile "
        "artifacts and frozen executable host oracles; sentinels remain separate"
    )
    for task in range(1, VARIANT_COUNT + 1):
        registry["cases"].append(_data_task(corpus_by_id[_case_id(13, task)], task))
        registry["cases"].append(_next_task(corpus_by_id[_case_id(14, task)], task))
    return registry


def _data_task(case: dict[str, Any], task: int) -> dict[str, Any]:
    row = _base_task_row(case)
    fixture = f"data/task-{task:02d}.csv"
    reproducer = ["python3", "scripts/repro.py", fixture]
    regressions = [
        {"id": "pytest-tests", "argv": ["python3", "-m", "pytest", "-q", "tests"]},
        {"id": "contract-check", "argv": ["python3", "scripts/contract_check.py"]},
    ]
    verify_commands = [" ".join(reproducer), *[" ".join(r["argv"]) for r in regressions]]
    row.update(
        {
            "offline_dependencies": ["python3-stdlib", "pytest"],
            "operational_constraints": {
                "allowed_dependencies": ["python3-stdlib", "pytest"],
                "entry_paths": ["pipeline/main.py"],
                "extra_tools": ["pytest"],
                "frozen_regression_set": regressions,
                "install_from_network": "forbidden",
                "language": "python3",
                "network": "denied",
                "registered_reproducer_fixture": fixture,
                "reproducer": {
                    "argv": reproducer,
                    "expected_exit_before": 1,
                    "expected_exit_after": 0,
                    "stage_before": "before",
                },
                "scored": False,
                "shell": "forbidden",
            },
            "completion_contract": {
                "deferred_verify_requirements": [],
                "deterministic_oracles": [],
                "evidence_hint_tokens": [fixture, "data_reconciliation", "contract-check"],
                "fix_reproducer_command": " ".join(reproducer),
                "goal": case["goal"],
                "profile": "data",
                "required_capabilities": [
                    "data_reconciliation",
                    "data_claims_binding",
                    "data_rerun_consistency",
                    "data_results_schema",
                ],
                "required_evidence": [
                    "implementation_artifact",
                    "test_artifact",
                    "bound_verify_command",
                    "non_zero_test_or_assertion_evidence",
                ],
                "required_obligations": [
                    "implementation",
                    "verification",
                    "acceptance_evidence",
                ],
                "required_paths": [
                    "pipeline/main.py",
                    fixture,
                    "scripts/repro.py",
                    "scripts/contract_check.py",
                    "tests/test_pipeline.py",
                    "output/inspection.json",
                    "output/results.json",
                    "output/report.md",
                ],
                "verify_commands": verify_commands,
                "verify_repair_cap": 1,
            },
        }
    )
    return row


def _next_task(case: dict[str, Any], task: int) -> dict[str, Any]:
    row = _base_task_row(case)
    fixture = f"fixture/task-{task:02d}.json"
    reproducer = ["node", "scripts/repro.mjs", fixture]
    regressions = [
        {"id": "node-regression", "argv": ["node", "scripts/regression.mjs"]},
        {"id": "production-build", "argv": ["npx", "next", "build", "--webpack"]},
    ]
    verify_commands = [" ".join(reproducer), *[" ".join(r["argv"]) for r in regressions]]
    dependencies = [
        "next@16.3.1",
        "react@19.2.8",
        "react-dom@19.2.8",
    ]
    row.update(
        {
            "offline_dependencies": [*dependencies, "playwright-core@1.62.1 (host-owned)"],
            "operational_constraints": {
                "allowed_dependencies": dependencies,
                "entry_paths": ["app/page.js"],
                "framework": "nextjs-app-router",
                "frozen_regression_set": regressions,
                "install_from_network": "forbidden",
                "language": "javascript",
                "network": "denied",
                "provisioning": "hash-bound vendored node_modules restored before each arm",
                "registered_reproducer_fixture": fixture,
                "reproducer": {
                    "argv": reproducer,
                    "expected_exit_before": 1,
                    "expected_exit_after": 0,
                    "stage_before": "before",
                },
                "reserved_port": NEXT_PORT,
                "scored": False,
                "shell": "forbidden",
            },
            "completion_contract": {
                "deferred_verify_requirements": [],
                "deterministic_oracles": [],
                "evidence_hint_tokens": [fixture, f"#result-{task:02d}", "production-build"],
                "fix_reproducer_command": " ".join(reproducer),
                "goal": case["goal"],
                "profile": "nextjs",
                "required_capabilities": [],
                "required_evidence": [
                    "implementation_artifact",
                    "test_artifact",
                    "bound_verify_command",
                    "nextjs_route_evidence",
                    "build_command_or_dependency_missing_boundary",
                ],
                "required_obligations": [
                    "implementation",
                    "verification",
                    "acceptance_evidence",
                ],
                "required_paths": [
                    "package.json",
                    "package-lock.json",
                    "next.config.js",
                    "app/layout.js",
                    "app/page.js",
                    "lib/label.mjs",
                    fixture,
                    "scripts/repro.mjs",
                    "scripts/regression.mjs",
                ],
                "verify_commands": verify_commands,
                "verify_repair_cap": 1,
            },
        }
    )
    return row


def _build_adapters() -> dict[str, Any]:
    registry = copy.deepcopy(_build_a14_adapters())
    registry["adapters"] = [
        row
        for row in registry["adapters"]
        if row["case_id"].startswith(
            (
                "phase6-main-c05-",
                "phase6-main-c06-",
                "phase6-main-c07-",
                "phase6-main-c08-",
            )
        )
    ]
    registry["schema_version"] = "commandagent.goal_verify.adapters.v4_a15"
    for task in range(1, VARIANT_COUNT + 1):
        registry["adapters"].extend(_data_adapters(task))
        registry["adapters"].extend(_next_adapters(task))
    return registry


def _command_adapter(
    *,
    case_id: str,
    adapter_id: str,
    claim_id: str,
    role: str,
    argv: list[str],
    workspace: str,
    stage: str,
    expected: int,
    fixture: str,
    fixture_sha256: str,
    timeout_ms: int = 10_000,
) -> dict[str, Any]:
    return {
        "a14_role": role,
        "adapter_id": adapter_id,
        "case_id": case_id,
        "claim_id": claim_id,
        "executor": {
            "argv": argv,
            "executor_status": "existing",
            "kind": "fixture_hash_command",
            "observation": {"expected": expected, "kind": "exit_code"},
            "observed_strength": "runtime",
            "registered_fixture": {"path": fixture, "sha256": fixture_sha256},
            "stage": stage,
            "timeout_ms": timeout_ms,
            "workspace": workspace,
        },
        "proposal": {
            "input_binding": {
                "argv": argv,
                "kind": "fixture_command",
                "path": fixture,
                "sha256": fixture_sha256,
                "strategies": ["command", "fixture"],
            },
            "observation_kinds": ["existing_binding", "file", "exit_code"],
            "polarities": ["failure" if expected else "success"],
            "strategies": ["existing_fix_evidence", "fixture", "command"],
        },
    }


def _data_adapters(task: int) -> list[dict[str, Any]]:
    case_id = _case_id(13, task)
    fixture = f"data/task-{task:02d}.csv"
    fixture_sha = _fixture_sha(f"fix-data-reconciliation/before/{fixture}")
    argv = ["python3", "scripts/repro.py", fixture]
    return [
        _command_adapter(
            case_id=case_id,
            adapter_id=f"a15-data-before--{case_id}",
            claim_id="exact-reproducer",
            role="precondition",
            argv=argv,
            workspace=DATA_WORKSPACE_ID,
            stage="before",
            expected=1,
            fixture=fixture,
            fixture_sha256=fixture_sha,
        ),
        _command_adapter(
            case_id=case_id,
            adapter_id=f"a15-data-after--{case_id}",
            claim_id="exact-reproducer",
            role="final_success",
            argv=argv,
            workspace=DATA_WORKSPACE_ID,
            stage="after",
            expected=0,
            fixture=fixture,
            fixture_sha256=fixture_sha,
        ),
        {
            "a14_role": "final_success",
            "adapter_id": f"a15-data-regressions--{case_id}",
            "case_id": case_id,
            "claim_id": "regressions",
            "executor": {
                "executor_status": "existing",
                "kind": "regression_set",
                "observation": {"expected": 0, "kind": "exit_code"},
                "observed_strength": "runtime",
                "registered": [
                    {"id": "pytest-tests", "argv": ["python3", "-m", "pytest", "-q", "tests"]},
                    {"id": "contract-check", "argv": ["python3", "scripts/contract_check.py"]},
                ],
                "stage": "after",
                "timeout_ms": 60_000,
                "workspace": DATA_WORKSPACE_ID,
            },
            "proposal": {
                "observation_kinds": ["existing_binding", "exit_code"],
                "polarities": ["success"],
                "strategies": ["existing_fix_evidence", "command"],
            },
        },
    ]


def _next_adapters(task: int) -> list[dict[str, Any]]:
    case_id = _case_id(14, task)
    fixture = f"fixture/task-{task:02d}.json"
    fixture_sha = _fixture_sha(f"fix-nextjs-route-label/before/{fixture}")
    argv = ["node", "scripts/repro.mjs", fixture]
    browser = _next_browser_tool()
    server = {
        "argv": ["npx", "next", "start", "-p", str(NEXT_PORT)],
        "loopback_only": True,
        "port": NEXT_PORT,
        "ready_path": "/",
    }
    return [
        _command_adapter(
            case_id=case_id,
            adapter_id=f"a15-next-before--{case_id}",
            claim_id="exact-reproducer",
            role="precondition",
            argv=argv,
            workspace=NEXT_WORKSPACE_ID,
            stage="before",
            expected=1,
            fixture=fixture,
            fixture_sha256=fixture_sha,
        ),
        _command_adapter(
            case_id=case_id,
            adapter_id=f"a15-next-after--{case_id}",
            claim_id="exact-reproducer",
            role="final_success",
            argv=argv,
            workspace=NEXT_WORKSPACE_ID,
            stage="after",
            expected=0,
            fixture=fixture,
            fixture_sha256=fixture_sha,
            timeout_ms=30_000,
        ),
        {
            "a14_role": "final_success",
            "adapter_id": f"a15-next-regressions--{case_id}",
            "case_id": case_id,
            "claim_id": "regressions",
            "executor": {
                "executor_status": "existing",
                "kind": "regression_set",
                "observation": {"expected": 0, "kind": "exit_code"},
                "observed_strength": "runtime",
                "registered": [
                    {"id": "node-regression", "argv": ["node", "scripts/regression.mjs"]},
                    {"id": "production-build", "argv": ["npx", "next", "build", "--webpack"]},
                ],
                "stage": "after",
                "timeout_ms": 240_000,
                "workspace": NEXT_WORKSPACE_ID,
            },
            "proposal": {
                "observation_kinds": ["existing_binding", "exit_code"],
                "polarities": ["success"],
                "strategies": ["existing_fix_evidence", "command"],
            },
        },
        {
            "a14_role": "final_success",
            "adapter_id": f"a15-next-route--{case_id}",
            "case_id": case_id,
            "claim_id": "route-render",
            "executor": {
                "browser_executable": ".goal-verify-tools/chromium/headless_shell",
                "browser_sha256": browser["files_sha256"]["headless_shell"],
                "executor_status": "existing",
                "kind": "playwright_script",
                "observation": {
                    "checks": [
                        {"selector": f"#result-{task:02d}", "text": f"ready-{task:02d}"}
                    ],
                    "kind": "dom",
                },
                "observed_strength": "runtime",
                "script": [
                    {"goto": "/"},
                    {"read_text": f"#result-{task:02d}"},
                ],
                "server": server,
                "stage": "after",
                "timeout_ms": 60_000,
                "workspace": NEXT_WORKSPACE_ID,
            },
            "proposal": {
                "observation_kinds": ["existing_binding", "dom"],
                "polarities": ["success"],
                "strategies": ["existing_fix_evidence", "browser"],
            },
        },
    ]


def _next_browser_tool() -> dict[str, Any]:
    registry = _load(BASE_NEXT_WORKSPACES)
    workspace = next(
        row
        for row in registry["workspaces"]
        if row["case_id"] == "create-ui-copy-style-port-path"
    )
    return copy.deepcopy(workspace["provisioning"]["browser_tool"])


def _tracked_hashes(root: Path, stages: tuple[str, ...]) -> tuple[list[str], dict[str, str]]:
    tracked = []
    hashes = {}
    for stage in stages:
        for path in sorted((root / stage).rglob("*")):
            if (
                not path.is_file()
                or path.suffix == ".pyc"
                or any(
                    part in {"__pycache__", ".next", "node_modules"}
                    for part in path.parts
                )
            ):
                continue
            relative = path.relative_to(root).as_posix()
            tracked.append(relative)
            hashes[relative] = _file_sha256(path)
    return tracked, hashes


def _build_workspaces(*, status: str) -> dict[str, Any]:
    registry = copy.deepcopy(_load(BASE_WORKSPACES))
    registry["schema_version"] = "commandagent.goal_verify.real_workspace_additions.v4_a15"
    registry["status"] = status
    data_root = FIXTURES / "fix-data-reconciliation"
    next_root = FIXTURES / "fix-nextjs-route-label"
    data_tracked, data_hashes = _tracked_hashes(data_root, ("before", "after"))
    next_tracked, next_hashes = _tracked_hashes(next_root, ("before", "after"))
    next_source = next(
        row
        for row in _load(BASE_NEXT_WORKSPACES)["workspaces"]
        if row["case_id"] == "create-ui-copy-style-port-path"
    )
    registry["workspaces"].extend(
        [
            {
                "candidate_oracle_execution_stage": "before",
                "case_id": DATA_WORKSPACE_ID,
                "frozen_file_sha256": data_hashes,
                "goal": "A15 real data-profile reconciliation fix template",
                "intent": "fix",
                "must_contain": [
                    "before: exact fixture fails only the row reconciliation invariant",
                    "after: exact fixture and frozen regressions pass offline",
                ],
                "product_run": {
                    "initial_stage": "before",
                    "network": "denied",
                    "toolchain": "python3 stdlib and preinstalled pytest",
                    "workspace_copy": "fresh copy into execution_root per paired run",
                },
                "profile": "data",
                "root": str(data_root.relative_to(ROOT)) + "/",
                "stages": {
                    "before": "deterministic data pipeline with incorrect used_rows accounting",
                    "after": "reference repair with valid reconciliation and unchanged regressions",
                },
                "status": status,
                "tracked_files": data_tracked,
            },
            {
                "candidate_oracle_execution_stage": "before",
                "case_id": NEXT_WORKSPACE_ID,
                "frozen_file_sha256": next_hashes,
                "goal": "A15 real Next.js App Router route-label fix template",
                "intent": "fix",
                "must_contain": [
                    "before: exact Node fixture and route text expose the same stale label",
                    "after: exact fixture, production build, and browser DOM oracle pass offline",
                ],
                "product_run": {
                    "initial_stage": "before",
                    "network": "denied",
                    "toolchain": "hash-bound offline Next.js and Chromium provisioning",
                    "workspace_copy": "fresh copy into execution_root per paired run",
                },
                "profile": "nextjs",
                "provisioning": copy.deepcopy(next_source["provisioning"]),
                "root": str(next_root.relative_to(ROOT)) + "/",
                "stages": {
                    "before": "runnable Next.js app with stale task labels",
                    "after": "reference repair with ready labels",
                },
                "status": status,
                "tracked_files": next_tracked,
            },
        ]
    )
    return registry


def _eligible_case_ids() -> list[str]:
    return [
        _case_id(cell, task)
        for cell in (5, 7, 13, 14)
        for task in range(1, VARIANT_COUNT + 1)
    ]


def _sentinel_case_ids() -> list[str]:
    return [
        _case_id(cell, task)
        for cell in (6, 8)
        for task in range(1, VARIANT_COUNT + 1)
    ]


def _full_pair_ids(case_ids: list[str], repeats: int) -> list[str]:
    return [
        f"{case_id}--pair-{sample:02d}"
        for case_id in case_ids
        for sample in range(1, repeats + 1)
    ]


def _smoke_pair_ids() -> list[str]:
    representative = [
        _case_id(5, 5),
        _case_id(7, 1),
        _case_id(13, 1),
        _case_id(14, 1),
    ]
    sentinels = [_case_id(6, 1), _case_id(8, 1)]
    return [*_full_pair_ids(representative, 3), *_full_pair_ids(sentinels, 1)]


def _base_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    authorized: bool,
    tasks: dict[str, Any],
    adapters: dict[str, Any],
    workspaces: dict[str, Any],
) -> dict[str, Any]:
    contract = _build_a14_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=authorized,
    )
    contract.update(
        {
            "schema_version": "commandagent.goal_verify.recovery_experiment.v4_a15",
            "corpus": str(CORPUS_PATH.relative_to(ROOT)),
            "task_contract_registry": str(TASKS_PATH.relative_to(ROOT)),
            "frozen_external_oracles": str(ADAPTERS_PATH.relative_to(ROOT)),
            "workspace_registry_additions": str(WORKSPACES_PATH.relative_to(ROOT)),
            "supersedes_contract": "phase6-recovery-v4-20260830-a14-a14-live-01",
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15",
            "reason": (
                "replace sentinel-only coverage with eligible, offline-executable "
                "real data and Next.js fix tasks while preserving sentinel roles"
            ),
            "historical_run_policy": (
                "A14-A14 remains immutable and is not pooled because its eligible "
                "population contains only cli and generic profiles"
            ),
            "inference_role": (
                "four-profile Recovery 0-vs-1 estimate with profile-specific claims"
            ),
        }
    )
    contract["smoke"].update(
        {
            "browser_oracle_gate_source": (
                "separate frozen-reference production-build and Chromium preflight"
            ),
            "require_browser_oracle_executability": True,
            "require_separate_browser_oracle_preflight": True,
        }
    )
    task_by_id = {row["case_id"]: row for row in tasks["cases"]}
    all_case_ids = [*_eligible_case_ids(), *_sentinel_case_ids()]
    contract["recovery_eligibility"]["preregistered_smoke_cases"] = {
        case_id: classify_case_recovery_eligibility(
            task_contract=task_by_id[case_id], adapters=adapters["adapters"]
        )
        for case_id in all_case_ids
    }
    contract.setdefault("oracle_executability_preflight", {}).setdefault(
        "reference_overrides", {}
    )
    contract["frozen_input_sha256"] = {
        contract["corpus"]: _json_sha256(
            {**_build_corpus(), "status": tasks["status"]}
        ),
        contract["task_contract_registry"]: _json_sha256(tasks),
        contract["frozen_external_oracles"]: _json_sha256(adapters),
        contract["workspace_registry"]: _file_sha256(ROOT / contract["workspace_registry"]),
        contract["workspace_registry_additions"]: _json_sha256(workspaces),
        contract["resource_budget_config"]: _file_sha256(
            ROOT / contract["resource_budget_config"]
        ),
    }
    contract["runner_sources"].extend(
        [
            "scripts/eval_lib/generate_goal_verify_recovery_v4_a15.py",
            "scripts/eval_lib/goal_verify_recovery_a15_report.py",
            "scripts/eval-goal-verify-recovery-a15-report.py",
        ]
    )
    contract["runner_sources"] = list(dict.fromkeys(contract["runner_sources"]))
    return contract


def _build_smoke_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    authorized: bool,
    tasks: dict[str, Any],
    adapters: dict[str, Any],
    workspaces: dict[str, Any],
) -> dict[str, Any]:
    contract = _base_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        authorized=authorized,
        tasks=tasks,
        adapters=adapters,
        workspaces=workspaces,
    )
    pairs = _smoke_pair_ids()
    task_by_id = {row["case_id"]: row for row in tasks["cases"]}
    contract.pop("full_experiment", None)
    contract.update(
        {
            "contract_id": SMOKE_CONTRACT_ID,
            "smoke_run_id": SMOKE_CONTRACT_ID,
        }
    )
    contract["smoke"].update(
        {
            "selected_pair_ids": pairs,
            "expected_pair_count": len(pairs),
            "minimum_executed_recovery_pairs": 2,
            "minimum_executed_recovery_pairs_per_real_profile": 1,
            "minimum_pairs_per_real_profile": 3,
            "required_real_profiles": ["cli", "generic", "data", "nextjs"],
            "typed_fix_reproducer_commands": {
                pair_id: task_by_id[pair_id.split("--pair-", 1)[0]][
                    "completion_contract"
                ]["fix_reproducer_command"]
                for pair_id in pairs
                if pair_id.split("--pair-", 1)[0] in set(_eligible_case_ids())
            },
            "inference_role": (
                "instrument smoke across real cli, generic, data, and Next.js "
                "tasks plus dependency/profile sentinels"
            ),
            "effect_claim_allowed": False,
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": False,
            "approved_at": "2026-08-31" if authorized else None,
        }
    )
    return contract


def _build_full_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    authorized: bool,
    tasks: dict[str, Any],
    adapters: dict[str, Any],
    workspaces: dict[str, Any],
) -> dict[str, Any]:
    contract = _base_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        authorized=authorized,
        tasks=tasks,
        adapters=adapters,
        workspaces=workspaces,
    )
    eligible_cases = _eligible_case_ids()
    sentinel_cases = _sentinel_case_ids()
    eligible_pairs = _full_pair_ids(eligible_cases, 3)
    sentinel_pairs = _full_pair_ids(sentinel_cases, 1)
    selected_pairs = [*eligible_pairs, *sentinel_pairs]
    task_by_id = {row["case_id"]: row for row in tasks["cases"]}
    contract.update(
        {
            "contract_id": FULL_CONTRACT_ID,
            "smoke_run_id": FULL_CONTRACT_ID,
        }
    )
    contract["smoke"].update(
        {
            "selected_pair_ids": selected_pairs,
            "expected_pair_count": len(selected_pairs),
            "minimum_executed_recovery_pairs": 40,
            "typed_fix_reproducer_commands": {
                pair_id: task_by_id[pair_id.split("--pair-", 1)[0]][
                    "completion_contract"
                ]["fix_reproducer_command"]
                for pair_id in eligible_pairs
            },
            "inference_role": "full preregistered four-profile Recovery experiment",
            "effect_claim_allowed": False,
        }
    )
    contract["full_experiment"].update(
        {
            "eligible_cell_ids": list(PROFILE_CELLS),
            "eligible_case_ids": eligible_cases,
            "sentinel_case_ids": sentinel_cases,
            "eligible_pair_ids": eligible_pairs,
            "sentinel_pair_ids": sentinel_pairs,
            "eligible_pair_count": len(eligible_pairs),
            "sentinel_pair_count": len(sentinel_pairs),
            "minimum_executed_recovery_pairs": 40,
            "minimum_executed_recovery_pairs_per_profile": 5,
            "profile_cells": PROFILE_CELLS,
            "primary_estimand": (
                "equally weighted four-profile mean of +1 frozen external fail-to-pass, "
                "-1 pass-to-non-pass, and 0 otherwise after task-cluster resampling"
            ),
            "go_rule": (
                "all instrument gates; each of cli, generic, data, and nextjs has at "
                "least five executed Recoveries and a 2,000-sample profile-specific "
                "95% CI lower bound above zero; pooled CI lower above zero; zero harm, "
                "regression, instrumentation-unusable, and sentinel Recovery; all "
                "profile resource budgets met"
            ),
            "stopping_rule": (
                "collect exactly 120 eligible pairs (10 tasks x 3 in each of four "
                "profiles) and 20 sentinels; never replace or post-hoc exclude pairs"
            ),
            "full_freeze_prerequisites": (
                "A15 smoke GO on all four real profiles, exact-SHA CI, and four "
                "wall/token budgets fixed from smoke without inspecting full outcomes"
            ),
            "resource_budget_basis": (
                "draft placeholder inherited from A14; before full freeze replace with "
                "four budgets fixed from A15 smoke without observing full outcomes"
            ),
        }
    )
    contract["analysis"].update(
        {
            "primary_population": (
                "real offline-executable fix tasks in cli, generic, data, and nextjs; "
                "dependency and explicit profile-contract cases remain sentinels"
            ),
            "profile_effect_claim": (
                "the phrase all profiles improve is forbidden unless every frozen "
                "profile-specific lower confidence bound is above zero"
            ),
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": authorized,
            "approved_at": "2026-08-31" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A15 real data/Next.js Recovery experiment inputs"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    parser.add_argument("--full-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if (args.smoke_collection_authorized or args.full_collection_authorized) and not args.code_sha:
        parser.error("collection authorization requires exact-SHA inputs")
    if args.full_collection_authorized:
        parser.error(
            "full collection cannot freeze before A15 smoke fixes profile-specific budgets"
        )
    _build_fixtures()
    asset_status = "frozen" if args.code_sha else "draft"
    smoke_status = asset_status
    full_status = "frozen" if args.full_collection_authorized else "draft"
    corpus = _build_corpus()
    corpus["status"] = asset_status
    tasks = _build_tasks(status=asset_status)
    adapters = _build_adapters()
    workspaces = _build_workspaces(status=asset_status)
    _write_json(CORPUS_PATH, corpus)
    _write_json(TASKS_PATH, tasks)
    _write_json(ADAPTERS_PATH, adapters)
    _write_json(WORKSPACES_PATH, workspaces)
    _write_json(
        SMOKE_CONTRACT_PATH,
        _build_smoke_contract(
            status=smoke_status,
            code_sha=args.code_sha or "",
            exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
            authorized=args.smoke_collection_authorized,
            tasks=tasks,
            adapters=adapters,
            workspaces=workspaces,
        ),
    )
    _write_json(
        FULL_CONTRACT_PATH,
        _build_full_contract(
            status=full_status,
            code_sha=args.code_sha or "",
            exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
            authorized=args.full_collection_authorized,
            tasks=tasks,
            adapters=adapters,
            workspaces=workspaces,
        ),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
