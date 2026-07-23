# ruff: noqa: E701,E702
import sys
import tempfile
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from acceptance_sheet import generate

class AcceptanceSheetTests(unittest.TestCase):
    def make(self, text, extra=None):
        d=Path(tempfile.mkdtemp()); (d/'workflow-events.jsonl').write_text(text)
        if extra: (d/'evidence').mkdir(); (d/'evidence'/'check.json').write_text(extra)
        return d
    def test_full_shape(self):
        s=generate(self.make('{"event":"run_stop","status":"full"}\n','{"check_id":"pipeline_probe"}'))
        self.assertIn('定義された検証を全て実行し成立',s); self.assertIn('パイプラインを実行',s)
    def test_failed_missing_is_recorded(self):
        s=generate(self.make('{"event":"workflow_adjudicated","verdict":"circle_failed","reason":"node_failed:fix"}\n'))
        self.assertIn('未完了',s); self.assertIn('記録なし',s)
    def test_unknown_check_passthrough(self):
        s=generate(self.make('{"event":"run_stop","status":"full"}\n','{"check_id":"new_check"}'))
        self.assertIn('`new_check`',s)

if __name__ == '__main__': unittest.main()
