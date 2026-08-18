#!/usr/bin/env python3
"""Create and deterministically reverify a Community Mini App delivery bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

MANIFEST_SCHEMA = "commandagent.community-delivery-bundle/v1"
REVERIFY_SCHEMA = "commandagent.community-reverification/v1"
PROMOTION_SCHEMA = "commandagent.community-promotion-record/v1"


class BundleError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def artifact_level(root: Path) -> str:
    zone = root / "src/app-zone"
    return (
        "L3"
        if zone.is_dir() and any(path.is_file() for path in zone.rglob("*"))
        else "L2"
    )


def promotion_record(root: Path, level: str) -> dict[str, Any]:
    evidence = root / "evidence/promotion-decision.json"
    if level == "L2":
        if evidence.exists():
            raise BundleError("L2 artifact unexpectedly contains promotion evidence")
        return {
            "schema_version": PROMOTION_SCHEMA,
            "artifact_level": "L2",
            "status": "not_applicable_l2",
            "app_zone_present": False,
            "promotion_evidence_path": None,
            "evidence_claim": False,
        }
    if not evidence.is_file():
        raise BundleError("L3 artifact is missing evidence/promotion-decision.json")
    return {
        "schema_version": PROMOTION_SCHEMA,
        "artifact_level": level,
        "status": "present",
        "app_zone_present": True,
        "promotion_evidence_path": "artifacts/evidence/promotion-decision.json",
        "evidence_claim": True,
        "evidence_sha256": sha256_file(evidence),
    }


def inventory(bundle: Path) -> list[dict[str, Any]]:
    excluded = {"bundle-manifest.json"}
    return [
        {
            "path": path.relative_to(bundle).as_posix(),
            "size_bytes": path.stat().st_size,
            "sha256": sha256_file(path),
        }
        for path in sorted(bundle.rglob("*"))
        if path.is_file() and path.name not in excluded
    ]


def write_manifest(
    bundle: Path,
    *,
    source_run: str,
    level: str,
    verdict: str,
    binary_sha256: str,
) -> dict[str, Any]:
    manifest = {
        "schema_version": MANIFEST_SCHEMA,
        "storage_unit": "R2_delivery_unit",
        "source_run": source_run,
        "artifact_level": level,
        "expected_verdict": verdict,
        "instrument": {
            "binary_sha256": binary_sha256,
            "verification_profile": "community-mini-app",
        },
        "files": inventory(bundle),
    }
    write_json(bundle / "bundle-manifest.json", manifest)
    return manifest


def verify_manifest(bundle: Path) -> dict[str, Any]:
    manifest = read_json(bundle / "bundle-manifest.json")
    if manifest.get("schema_version") != MANIFEST_SCHEMA:
        raise BundleError("unsupported bundle manifest schema")
    declared = manifest.get("files")
    if not isinstance(declared, list):
        raise BundleError("bundle manifest files must be a list")
    expected_paths: set[str] = set()
    for item in declared:
        relative = item.get("path") if isinstance(item, dict) else None
        if (
            not isinstance(relative, str)
            or relative.startswith("/")
            or ".." in Path(relative).parts
        ):
            raise BundleError(f"unsafe bundle manifest path: {relative!r}")
        path = bundle / relative
        expected_paths.add(relative)
        if not path.is_file():
            raise BundleError(f"bundle file is missing: {relative}")
        if path.stat().st_size != item.get("size_bytes"):
            raise BundleError(f"bundle size mismatch: {relative}")
        observed = sha256_file(path)
        if observed != item.get("sha256"):
            raise BundleError(f"bundle SHA-256 mismatch: {relative}")
    observed_paths = {
        path.relative_to(bundle).as_posix()
        for path in bundle.rglob("*")
        if path.is_file() and path.name != "bundle-manifest.json"
    }
    if observed_paths != expected_paths:
        extra = sorted(observed_paths - expected_paths)
        missing = sorted(expected_paths - observed_paths)
        raise BundleError(
            f"bundle inventory mismatch: extra={extra}, missing={missing}"
        )
    return manifest


def copy_artifacts(source: Path, target: Path, level: str) -> None:
    required_files = [
        "app.spec.yaml",
        "core.sha256sums",
        "core/README.md",
        "schema/app-spec.schema.yaml",
        "schema/app-spec.schema.sha256",
    ]
    if level != "L2":
        required_files.extend(
            path.relative_to(source).as_posix()
            for path in (source / "src/app-zone").rglob("*")
            if path.is_file()
        )
        required_files.append("evidence/promotion-decision.json")
    for relative in required_files:
        source_path = source / relative
        if not source_path.is_file():
            raise BundleError(f"required delivery artifact is missing: {relative}")
        target_path = target / relative
        target_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, target_path)


def latest_community_verdict(events: Path) -> str:
    verdict: str | None = None
    for line in events.read_text(encoding="utf-8").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError as exc:
            raise BundleError(f"invalid event JSON: {exc}") from exc
        if event.get("event") == "community_profile_verification":
            candidate = event.get("verdict")
            if isinstance(candidate, str):
                verdict = candidate
    if verdict is None:
        raise BundleError("community_profile_verification event is missing")
    return verdict


def project_summary(
    binary: Path,
    events: Path,
    model_metadata: dict[str, Any],
) -> dict[str, Any]:
    command = [
        str(binary),
        "--runs",
        "--summary-json",
        "--provider",
        model_metadata["executor_provider"],
        "--model",
        model_metadata["executor_model"],
        "--planner-provider",
        model_metadata["planner_provider"],
        "--planner-model",
        model_metadata["planner_model"],
    ]
    think = model_metadata.get("ollama_think")
    if think is not None:
        command.append(f"--think={think}")
    environment = os.environ.copy()
    environment["COMMANDAGENT_EVAL_EVENTS"] = str(events)
    completed = subprocess.run(
        command,
        check=True,
        capture_output=True,
        text=True,
        env=environment,
    )
    for line in reversed(completed.stdout.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "commandagent.headless-summary/v1":
            return value
    raise BundleError("commandagent did not emit a headless summary")


def portable_summary(summary: dict[str, Any]) -> dict[str, Any]:
    projected = dict(summary)
    projected["acceptance_sheet_path"] = "acceptance-sheet.md"
    projected["artifacts_dir"] = "artifacts"
    projected["events_path"] = "verification-events.jsonl"
    return projected


def create_bundle(args: argparse.Namespace) -> int:
    if args.output.exists():
        raise BundleError(f"output already exists: {args.output}")
    binary_sha = sha256_file(args.binary)
    declared_sha = args.binary_sha256.lower()
    if binary_sha != declared_sha:
        raise BundleError(
            f"instrument SHA-256 mismatch: expected {declared_sha}, observed {binary_sha}"
        )
    campaign = read_json(args.campaign_summary)
    selected = next(
        (
            item
            for item in campaign.get("runs", [])
            if item.get("id") == args.source_run
        ),
        None,
    )
    if selected is None:
        raise BundleError(
            f"source run is absent from campaign summary: {args.source_run}"
        )
    source_meta = read_json(args.campaign_meta)
    run_meta = next(
        (
            item
            for item in source_meta.get("runs", [])
            if item.get("name") == args.source_run
        ),
        None,
    )
    if run_meta is None:
        raise BundleError(
            f"source run is absent from campaign metadata: {args.source_run}"
        )
    level = artifact_level(args.source_artifacts)
    if level != selected.get("level"):
        raise BundleError(
            f"artifact level mismatch: measured {selected.get('level')}, observed {level}"
        )
    expected_verdict = latest_community_verdict(args.source_events)
    if expected_verdict != "full" or selected.get("full") is not True:
        raise BundleError("delivery source must have a measured full verdict")

    args.output.mkdir(parents=True)
    copy_artifacts(args.source_artifacts, args.output / "artifacts", level)
    shutil.copy2(
        args.source_artifacts / "acceptance-sheet.md",
        args.output / "acceptance-sheet.md",
    )
    shutil.copy2(args.source_events, args.output / "verification-events.jsonl")
    model_metadata = {
        "executor_provider": "openai",
        "executor_model": selected["model_returns"][0]["requested"],
        "planner_provider": "ollama",
        "planner_model": run_meta["command_argv"][
            run_meta["command_argv"].index("--planner-model") + 1
        ],
        "ollama_think": selected.get("think"),
    }
    summary = portable_summary(
        project_summary(args.binary, args.source_events, model_metadata)
    )
    write_json(args.output / "summary.json", summary)
    write_json(
        args.output / "promotion-record.json",
        promotion_record(args.source_artifacts, level),
    )
    write_json(
        args.output / "source-generation.json",
        {
            "schema_version": "commandagent.community-source-generation/v1",
            "source_run": args.source_run,
            "campaign": campaign["series_id"],
            "execution_revision": campaign["execution_revision"],
            "binary_sha256": campaign["binary_sha256"],
            "command_argv": run_meta["command_argv"],
            "duration_secs": selected["duration_secs"],
            "provider_cost_usd": selected["cost_usd"],
            "artifact_level": selected["level"],
            "verdict": expected_verdict,
            "measurement_full": selected["full"],
            "repair_cycles": selected["repair_cycles"],
            "model_metadata": model_metadata,
            "verification_events_sha256": sha256_file(args.source_events),
        },
    )
    write_manifest(
        args.output,
        source_run=args.source_run,
        level=level,
        verdict=expected_verdict,
        binary_sha256=binary_sha,
    )
    return 0


def reference_command(bundle: Path, validator: Path, level: str) -> list[str]:
    artifacts = bundle / "artifacts"
    command = [
        sys.executable,
        str(validator),
        "--spec",
        str(artifacts / "app.spec.yaml"),
        "--schema",
        str(artifacts / "schema/app-spec.schema.yaml"),
        "--schema-pin",
        str(artifacts / "schema/app-spec.schema.sha256"),
        "--root",
        str(artifacts),
        "--core-manifest",
        str(artifacts / "core.sha256sums"),
        "--changed-path",
        "app.spec.yaml",
    ]
    if level != "L2":
        command.append("--build-smoke")
    return command


def reverify_bundle(args: argparse.Namespace) -> int:
    manifest = verify_manifest(args.bundle)
    observed_binary_sha = sha256_file(args.binary)
    expected_binary_sha = manifest["instrument"]["binary_sha256"]
    if observed_binary_sha != expected_binary_sha:
        raise BundleError(
            f"instrument SHA-256 mismatch: expected {expected_binary_sha}, observed {observed_binary_sha}"
        )
    level = manifest["artifact_level"]
    with tempfile.TemporaryDirectory(prefix="cm4-community-reverify-") as directory:
        temporary = Path(directory)
        events = temporary / "events.jsonl"
        environment = os.environ.copy()
        environment["COMMANDAGENT_EVAL_EVENTS"] = str(events)
        product = subprocess.run(
            [
                str(args.binary),
                "--offline",
                "--profile",
                "community-mini-app",
                "--prompt",
                "Validate app.spec.yaml against the pinned schema; fail on violation.",
                "--cwd",
                str(args.bundle / "artifacts"),
                "--state-dir",
                str(temporary / "state"),
                "--no-footer",
                "--summary-json",
            ],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )
        product_summary: dict[str, Any] | None = None
        for line in reversed(product.stdout.splitlines()):
            try:
                candidate = json.loads(line)
            except json.JSONDecodeError:
                continue
            if candidate.get("schema_version") == "commandagent.headless-summary/v1":
                product_summary = candidate
                break
        if product.returncode != 0 or product_summary is None:
            raise BundleError(
                f"product reverification failed: exit={product.returncode}, stderr={product.stderr.strip()}"
            )
        product_verdict = latest_community_verdict(events)
        reference = subprocess.run(
            reference_command(args.bundle, args.reference_validator, level),
            check=False,
            capture_output=True,
            text=True,
        )
        try:
            reference_result = json.loads(reference.stdout.strip())
        except json.JSONDecodeError as exc:
            raise BundleError("reference validator did not emit JSON") from exc
        reference_ok = (
            reference.returncode == 0
            and reference_result.get("verdict") == "pass"
            and reference_result.get("zone", {}).get("verdict") == "pass"
        )
        families = {
            "S": "pass" if reference_result.get("verdict") == "pass" else "violation",
            "Z": reference_result.get("zone", {}).get("verdict", "violation"),
            "B": (
                "not_applicable_l2"
                if level == "L2"
                else reference_result.get("build", {}).get("verdict", "violation")
            ),
        }
        result = {
            "schema_version": REVERIFY_SCHEMA,
            "manifest_verified": True,
            "instrument_sha256_verified": True,
            "artifact_level": level,
            "applicability": "S+Z" if level == "L2" else "S+Z+B",
            "families": families,
            "product_exit_code": product.returncode,
            "product_verdict": product_verdict,
            "reference_exit_code": reference.returncode,
            "reference_verdict": "full" if reference_ok else "violation",
            "expected_verdict": manifest["expected_verdict"],
            "verdict_equal": (
                reference_ok
                and product_verdict == manifest["expected_verdict"]
                and product_summary.get("verdict") == manifest["expected_verdict"]
            ),
        }
        if not result["verdict_equal"]:
            raise BundleError(f"reverification verdict mismatch: {result}")
        output = args.bundle / "reverification.json"
        previous = output.read_bytes() if output.exists() else None
        write_json(output, result)
        current = output.read_bytes()
        if previous is not None and previous != current:
            raise BundleError("reverification output is not byte-deterministic")
        write_manifest(
            args.bundle,
            source_run=manifest["source_run"],
            level=level,
            verdict=manifest["expected_verdict"],
            binary_sha256=expected_binary_sha,
        )
        print(json.dumps(result, sort_keys=True))
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    subparsers = root.add_subparsers(dest="command", required=True)
    create = subparsers.add_parser("create")
    create.add_argument("--source-artifacts", type=Path, required=True)
    create.add_argument("--source-events", type=Path, required=True)
    create.add_argument("--source-run", required=True)
    create.add_argument("--campaign-summary", type=Path, required=True)
    create.add_argument("--campaign-meta", type=Path, required=True)
    create.add_argument("--binary", type=Path, required=True)
    create.add_argument("--binary-sha256", required=True)
    create.add_argument("--output", type=Path, required=True)
    create.set_defaults(func=create_bundle)

    reverify = subparsers.add_parser("reverify")
    reverify.add_argument("--bundle", type=Path, required=True)
    reverify.add_argument("--binary", type=Path, required=True)
    reverify.add_argument("--reference-validator", type=Path, required=True)
    reverify.set_defaults(func=reverify_bundle)
    return root


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        return args.func(args)
    except (BundleError, OSError, KeyError, ValueError) as exc:
        print(json.dumps({"verdict": "violation", "error": str(exc)}, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
