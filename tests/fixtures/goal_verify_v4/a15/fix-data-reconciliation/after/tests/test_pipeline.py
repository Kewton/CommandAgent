import importlib.util
from pathlib import Path


def load_pipeline():
    spec = importlib.util.spec_from_file_location("a15_data_pipeline", "pipeline/main.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_all_valid_rows_remain_counted(tmp_path: Path):
    source = tmp_path / "valid.csv"
    source.write_text("region,amount\nnorth,2\nsouth,4\n", encoding="utf-8")
    result = load_pipeline().summarize(source)
    assert result["reconciliation"] == {
        "input_rows": 2,
        "used_rows": 2,
        "excluded": [{"reason": "non_numeric_amount", "rows": 0}],
    }
    assert result["values"]["total"] == 6
