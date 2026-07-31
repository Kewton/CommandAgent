#!/usr/bin/env python3
"""Create deliberately incomplete profile/intent admission scaffolds."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
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
STRUCTURE_LITERAL_GUIDANCE_CHECKLIST_ITEM = "every structure-gate required shape has prior literal-example guidance and a fixture"
MACHINE_PLAN_PRESET_CHECKLIST_ITEM = "create planning is machine-synthesized/profile-preset and planner free composition is disabled"
SOURCE_MATERIAL_INJECTION_CHECKLIST_ITEM = "inputs that generation must read are placed in bounded machine-injected guidance with a measured fixture"
MEASUREMENT_ASSET_DESIGN_CHECKLIST_ITEM = "intentionally incomplete measurement candidates are mechanically unextractable, not semantically ambiguous"


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
        f"- [ ] {MACHINE_PLAN_PRESET_CHECKLIST_ITEM}\n"
        f"- [ ] {SOURCE_MATERIAL_INJECTION_CHECKLIST_ITEM}\n"
        "- [ ] conformance negative tests green\n"
        "- [ ] corpus fixture is an archived real run\n"
        "- [ ] reviewer explicitly changes `off` to `admitted`\n"
        "\n## Measurement asset design\n\n"
        f"- [ ] {MEASUREMENT_ASSET_DESIGN_CHECKLIST_ITEM}\n",
        encoding="utf-8",
    )
    return base


def generate_pack(ident: str, source_version: str, version: str) -> Path:
    """Clone one reviewed pack as an unpinned local experiment scaffold."""
    pack_id_pattern = r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*"
    semver_pattern = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    if re.fullmatch(pack_id_pattern, ident) is None:
        raise ValueError("pack id must use registered lowercase pack-id syntax")
    for label, value in (("source version", source_version), ("version", version)):
        if re.fullmatch(semver_pattern, value) is None:
            raise ValueError(f"{label} must be a SemVer core version")
    if source_version == version:
        raise ValueError("pack scaffold version must differ from the source version")

    pack_root = REPOSITORY_ROOT / "packs" / ident
    source = pack_root / source_version
    destination = pack_root / version
    if not source.is_dir():
        raise ValueError(f"source pack does not exist: {source}")
    if destination.exists():
        raise ValueError(f"destination pack already exists: {destination}")

    source_files = [
        name for name in ("assist.yaml", "eval.yaml") if (source / name).is_file()
    ]
    if not source_files:
        raise ValueError("source pack must contain assist.yaml or eval.yaml")
    rendered: dict[str, str] = {}
    old_version = f"  version: {source_version}"
    new_version = f"  version: {version}"
    for name in source_files:
        text = (source / name).read_text(encoding="utf-8")
        if text.count(f"  id: {ident}") != 1 or text.count(old_version) != 1:
            raise ValueError(
                f"{source / name} identity does not match the requested source"
            )
        rendered[name] = text.replace(old_version, new_version, 1)

    destination.mkdir(parents=True)
    for name, text in rendered.items():
        (destination / name).write_text(text, encoding="utf-8")
    return destination


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="kind", required=True)
    for kind in ("profile", "intent"):
        p = sub.add_parser(kind)
        p.add_argument("id")
    pack = sub.add_parser("pack")
    pack.add_argument("id")
    pack.add_argument("--from-version", required=True)
    pack.add_argument("--version", required=True)
    a = ap.parse_args()
    if a.kind == "pack":
        print(generate_pack(a.id, a.from_version, a.version))
    else:
        print(generate(a.kind, a.id))


if __name__ == "__main__":
    main()
