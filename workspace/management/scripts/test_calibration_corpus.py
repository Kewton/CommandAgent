import json
import tempfile
import unittest
from pathlib import Path

import calibration_corpus

MEASURED_ELEV_003 = (
    Path(__file__).resolve().parents[3]
    / "tests/corpus/apps/test0725_cli_elev_003/fixtures"
)


class CalibrationCorpusTests(unittest.TestCase):
    def test_measured_c2_nearest_miss_is_retroactively_collected_once(self):
        rows = list(calibration_corpus.records(MEASURED_ELEV_003))

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["kind"], "c2")
        self.assertEqual(rows[0]["judgement"], "violation")
        self.assertEqual(rows[0]["claim"], "--anvil-invalid-probe")
        self.assertEqual(
            rows[0]["nearest_miss"],
            {"candidate": "--pattern", "edit_distance": 17},
        )

        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp)
            self.assertEqual(
                calibration_corpus.append([MEASURED_ELEV_003], store=store), 1
            )
            self.assertEqual(
                calibration_corpus.append([MEASURED_ELEV_003], store=store), 0
            )
            saved = [
                json.loads(line)
                for line in (store / "c2/records.jsonl").read_text().splitlines()
            ]
            self.assertEqual(saved, rows)


if __name__ == "__main__":
    unittest.main()
