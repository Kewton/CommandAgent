from pathlib import Path
raise SystemExit(0 if Path('repro.py').is_file() else 1)
