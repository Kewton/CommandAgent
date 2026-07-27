#!/usr/bin/env python3
"""Create deliberately incomplete profile/intent admission scaffolds."""

from __future__ import annotations

import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHAPTERS = [
    "1. スコープ",
    "2. full の意味（最重要・不変条件）",
    "3. 要求 evidence（full の必須ゲート）",
    "4. assurance 階層",
    "5. 実行プローブ",
    "6. 偽装耐性（conformance ネガティブテストの要求）",
    "7. スコープ外（明示）",
    "8. 生成側への制約",
]
PROJECTION_CHECKLIST_ITEM = (
    "completion assurance projection mapping and a measured fixture are implemented"
)
PRODUCTION_ACTIVATION_CHECKLIST_ITEM = (
    "every verification component has a production acceptance-path activation test"
)
STRUCTURE_LITERAL_GUIDANCE_CHECKLIST_ITEM = (
    "every structure-gate required shape has prior literal-example guidance and a fixture"
)


def generate(kind, ident):
    base = ROOT / "scaffolds" / kind / ident
    base.mkdir(parents=True, exist_ok=True)
    title = f"{kind.title()} Contract Scaffold: {ident}"
    text = [
        f"# {title} (draft — off until admitted)",
        "",
        "状態: **off until admitted**。空欄は埋めるべき判断である。",
        "",
    ]
    for ch in CHAPTERS:
        text += [
            f"## {ch}",
            "",
            "<!-- TODO: 既存契約から転記し、固有判断を記入する。 -->",
            "",
        ]
    (base / "contract.md").write_text("\n".join(text), encoding="utf-8")
    (base / "manifest.toml").write_text(
        f'[manifest]\nid = "{ident}"\nkind = "{kind}"\nadmission = "off"\nprofile = ""\nintent = ""\n',
        encoding="utf-8",
    )
    (base / "conformance.md").write_text(
        "# Conformance skeleton\n\n- evidence label only without file: reject\n- altered evidence: reject\n- missing required evidence: reject\n- epoch/order violation: reject\n- lineage or carry discontinuity: reject\n",
        encoding="utf-8",
    )
    (base / "corpus").mkdir(exist_ok=True)
    (base / "ADMISSION.md").write_text(
        "# Admission checklist\n\n"
        "- [ ] contract chapters filled and reviewed\n"
        "- [ ] manifest fields and profile/intent are real\n"
        "- [ ] required evidence mapped to implementation\n"
        f"- [ ] {PROJECTION_CHECKLIST_ITEM}\n"
        f"- [ ] {PRODUCTION_ACTIVATION_CHECKLIST_ITEM}\n"
        f"- [ ] {STRUCTURE_LITERAL_GUIDANCE_CHECKLIST_ITEM}\n"
        "- [ ] conformance negative tests green\n"
        "- [ ] corpus fixture is an archived real run\n"
        "- [ ] reviewer explicitly changes `off` to `admitted`\n",
        encoding="utf-8",
    )
    return base


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="kind", required=True)
    for kind in ("profile", "intent"):
        p = sub.add_parser(kind)
        p.add_argument("id")
    a = ap.parse_args()
    print(generate(a.kind, a.id))


if __name__ == "__main__":
    main()
