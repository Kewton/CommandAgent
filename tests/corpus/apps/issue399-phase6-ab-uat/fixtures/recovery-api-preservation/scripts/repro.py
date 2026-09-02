import importlib.util
from pathlib import Path


def load_pipeline():
    spec = importlib.util.spec_from_file_location(
        "recovery_fixture_pipeline", "pipeline/main.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    result = load_pipeline().write_outputs(Path("data/input.csv"))
    return 0 if result["used_rows"] == 2 else 1


if __name__ == "__main__":
    raise SystemExit(main())
