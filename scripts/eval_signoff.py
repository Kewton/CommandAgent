#!/usr/bin/env python3
"""Evaluate broad CommandAgent eval roots against sign-off gates.

This script is eval-only. It reads existing summary artifacts and reports
ownership/diagnostic gaps; it never reruns cases or mutates workspaces.
"""

from __future__ import annotations

import argparse
import csv
import sys
from dataclasses import dataclass
from pathlib import Path


KNOWN_FAMILIES = {
    "smoke",
    "focused",
    "focused-fixture",
    "large",
    "supplemental",
}

BETTER_OWNER_TERMINAL_STATES = {
    "plan_parse_failed",
    "plan_schema_failed",
    "plan_lint_failed",
    "provider_transport_failed",
    "provider_parse_failed",
    "tool_protocol_failed",
    "step_policy_failed",
    "profile_contract_failed",
    "dependency_missing",
    "setup_failed",
    "port_in_use",
    "missing_deliverable",
    "missing_evidence",
    "evidence_binding_failed",
    "completion_evidence_failed",
    "stale_evidence",
}

BETTER_OWNER_CATEGORIES = {
    "planning",
    "provider_transport",
    "tool_protocol",
    "step_policy",
    "profile",
    "setup",
}

MISSING_VALUES = {"", "unknown", "none", "not_applicable"}

LARGE_DISPOSITIONS = {
    "closed_owned_failure",
    "implementation_blocker",
    "accepted_external_limitation",
    "split_forward",
}


@dataclass(frozen=True)
class RootSpec:
    family: str
    path: Path


@dataclass(frozen=True)
class FamilyRequirement:
    expected_count: int
    required: bool


@dataclass(frozen=True)
class AdmittedRoot:
    family: str
    path: Path
    case_count: int
    role: str


@dataclass(frozen=True)
class RootAdmission:
    findings: list["Finding"]
    admitted_roots: list[AdmittedRoot]
    family_case_counts: dict[str, int]
    current_case_coverage: int
    expected_current_case_coverage: int

    @property
    def status(self) -> str:
        return "fail" if self.findings else "pass"

    @property
    def reason(self) -> str:
        return "root_admission_failed" if self.findings else "current_roots_admitted"


@dataclass(frozen=True)
class Finding:
    family: str
    root: Path
    case_id: str
    run: str
    code: str
    detail: str


FINAL_CURRENT_REQUIREMENTS = {
    "smoke": FamilyRequirement(expected_count=3, required=True),
    "focused": FamilyRequirement(expected_count=82, required=True),
    "large": FamilyRequirement(expected_count=6, required=True),
    "small": FamilyRequirement(expected_count=0, required=False),
}

FINAL_CURRENT_FAMILIES = set(FINAL_CURRENT_REQUIREMENTS)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check CommandAgent eval roots against broad sign-off gates"
    )
    parser.add_argument(
        "--root",
        action="append",
        default=[],
        metavar="FAMILY=PATH",
        help=(
            "Eval root to check. FAMILY is one of: "
            + ", ".join(sorted(KNOWN_FAMILIES))
        ),
    )
    parser.add_argument(
        "--require-recheck",
        action="store_true",
        help="Require and evaluate recheck_summary.tsv for every root.",
    )
    parser.add_argument(
        "--summary-name",
        default=None,
        help="Override summary file name. Defaults to recheck_summary.tsv when "
        "--require-recheck is set, otherwise summary.tsv.",
    )
    return parser.parse_args()


def parse_root(value: str) -> RootSpec:
    if "=" not in value:
        raise SystemExit(f"--root must be FAMILY=PATH, got {value!r}")
    family, raw_path = value.split("=", 1)
    family = family.strip()
    if family not in KNOWN_FAMILIES:
        allowed = ", ".join(sorted(KNOWN_FAMILIES))
        raise SystemExit(f"unsupported root family {family!r}; expected {allowed}")
    path = Path(raw_path.strip())
    if not raw_path.strip():
        raise SystemExit(f"--root path is empty for family {family!r}")
    return RootSpec(family=family, path=path)


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def value(row: dict[str, str], key: str) -> str:
    return (row.get(key) or "").strip()


def is_missing(raw: str) -> bool:
    return raw.strip().casefold() in MISSING_VALUES


def is_missing_for(row: dict[str, str], field: str, raw: str) -> bool:
    cleaned = raw.strip().casefold()
    if cleaned == "not_applicable" and not_applicable_allowed(row, field):
        return False
    return cleaned in MISSING_VALUES


def not_applicable_allowed(row: dict[str, str], field: str) -> bool:
    if field == "target":
        return target_not_applicable_allowed(row)
    if field in {"evidence_binding", "completion_evidence"}:
        return evidence_not_applicable_allowed(row)
    return False


def target_not_applicable_allowed(row: dict[str, str]) -> bool:
    return target_optional_context(row) and has_owner_action_attempt(row)


def evidence_not_applicable_allowed(row: dict[str, str]) -> bool:
    if not provider_boundary_context(row):
        return False
    if not has_owner_action_attempt(row):
        return False
    return value(row, "attempt_outcome").casefold() in {
        "blocked_external",
        "explicit_stop",
        "stopped_external",
    }


def target_optional_context(row: dict[str, str]) -> bool:
    return value(row, "terminal_state") in {
        "explicit_stop",
        "provider_transport_failed",
        "provider_parse_failed",
    } or provider_boundary_context(row)


def provider_boundary_context(row: dict[str, str]) -> bool:
    return (
        value(row, "terminal_state")
        in {"provider_transport_failed", "provider_parse_failed"}
        or value(row, "failure_category") == "provider_transport"
        or value(row, "diagnostic_code").startswith("provider_transport:")
    )


def has_owner_action_attempt(row: dict[str, str]) -> bool:
    active_job = value(row, "active_job")
    owner = value(row, "recovery_owner") or value(row, "active_owner")
    action = value(row, "repair_action") or value(row, "selected_action")
    attempt_outcome = value(row, "attempt_outcome")
    return not (
        is_missing(active_job)
        or is_missing(owner)
        or is_missing(action)
        or is_missing(attempt_outcome)
    )


def is_success(row: dict[str, str]) -> bool:
    return value(row, "success").casefold() == "true"


def row_id(row: dict[str, str]) -> tuple[str, str]:
    return value(row, "case_id") or "<unknown>", value(row, "run") or "?"


def finding(spec: RootSpec, row: dict[str, str], code: str, detail: str) -> Finding:
    case_id, run = row_id(row)
    return Finding(
        family=spec.family,
        root=spec.path,
        case_id=case_id,
        run=run,
        code=code,
        detail=detail,
    )


def root_finding(spec: RootSpec, code: str, detail: str) -> Finding:
    return Finding(
        family=spec.family,
        root=spec.path,
        case_id="<root_admission>",
        run="?",
        code=code,
        detail=f"{detail}; root={spec.path}",
    )


def normalized_root_path(path: Path) -> Path:
    return path.expanduser().resolve(strict=False)


def row_case_ids(rows: list[dict[str, str]]) -> set[str]:
    return {value(row, "case_id") for row in rows if value(row, "case_id")}


def root_matches_family(spec: RootSpec, rows: list[dict[str, str]]) -> bool:
    case_ids = row_case_ids(rows)
    path_hint = str(spec.path).replace("\\", "/").casefold()
    if spec.family == "small":
        return not case_ids or "small" in path_hint
    if not case_ids:
        return False
    if spec.family == "smoke":
        return all(case_id.startswith("smoke-") for case_id in case_ids)
    if spec.family == "large":
        return all(case_id.startswith("large-") for case_id in case_ids)
    if spec.family == "focused":
        if any(case_id.startswith(("smoke-", "large-")) for case_id in case_ids):
            return False
        return "focused" in path_hint or any(
            case_id.startswith(("focused-", "phase")) for case_id in case_ids
        )
    if spec.family in {"focused-fixture", "supplemental"}:
        return True
    return False


def admit_roots(
    specs: list[RootSpec], *, require_recheck: bool, summary_name: str | None
) -> RootAdmission:
    findings: list[Finding] = []
    admitted_roots: list[AdmittedRoot] = []
    family_case_counts = {
        family: 0 for family in FINAL_CURRENT_REQUIREMENTS
    }
    labels: dict[str, RootSpec] = {}
    paths: dict[Path, RootSpec] = {}
    selected_rows: dict[str, list[dict[str, str]]] = {}

    for spec in specs:
        if spec.family in labels:
            findings.append(
                root_finding(
                    spec,
                    "duplicate_root_label",
                    f"duplicate root label {spec.family!r}",
                )
            )
        else:
            labels[spec.family] = spec
        normalized = normalized_root_path(spec.path)
        if normalized in paths:
            previous = paths[normalized]
            findings.append(
                root_finding(
                    spec,
                    "duplicate_root_path",
                    "root path is already registered "
                    f"as {previous.family!r}: {normalized}",
                )
            )
        else:
            paths[normalized] = spec

    for family, requirement in FINAL_CURRENT_REQUIREMENTS.items():
        if requirement.required and family not in labels:
            findings.append(
                Finding(
                    family=family,
                    root=Path("<missing>"),
                    case_id="<root_admission>",
                    run="?",
                    code="missing_required_root",
                    detail=f"required root {family!r} is missing",
                )
            )

    for spec in specs:
        root_findings, summary_path = root_summary_path(
            spec,
            require_recheck=require_recheck,
            summary_name=summary_name,
        )
        findings.extend(root_findings)
        if summary_path is None:
            continue
        rows = read_rows(summary_path)
        selected_rows[spec.family] = rows
        case_count = len(row_case_ids(rows))
        role = "current" if spec.family in FINAL_CURRENT_FAMILIES else "supplemental"
        admitted_roots.append(
            AdmittedRoot(
                family=spec.family,
                path=spec.path,
                case_count=case_count,
                role=role,
            )
        )
        if spec.family in FINAL_CURRENT_REQUIREMENTS:
            family_case_counts[spec.family] = case_count
            requirement = FINAL_CURRENT_REQUIREMENTS[spec.family]
            if case_count != requirement.expected_count:
                findings.append(
                    root_finding(
                        spec,
                        "root_case_count_mismatch",
                        "expected "
                        f"{requirement.expected_count} {spec.family} cases; "
                        f"observed {case_count}",
                    )
                )
        if not root_matches_family(spec, rows):
            findings.append(
                root_finding(
                    spec,
                    "root_family_mismatch",
                    f"root contents do not match family {spec.family!r}",
                )
            )

    current_case_coverage = sum(
        family_case_counts[family]
        for family, requirement in FINAL_CURRENT_REQUIREMENTS.items()
        if requirement.required
    )
    expected_current_case_coverage = sum(
        requirement.expected_count
        for requirement in FINAL_CURRENT_REQUIREMENTS.values()
        if requirement.required
    )
    if current_case_coverage != expected_current_case_coverage:
        findings.append(
            Finding(
                family="current",
                root=Path("<current-roots>"),
                case_id="<root_admission>",
                run="?",
                code="current_case_coverage_mismatch",
                detail=(
                    f"expected {expected_current_case_coverage} current cases; "
                    f"observed {current_case_coverage}"
                ),
            )
        )
    for family, rows in selected_rows.items():
        if family not in FINAL_CURRENT_REQUIREMENTS:
            continue
        if not rows and FINAL_CURRENT_REQUIREMENTS[family].expected_count == 0:
            continue
        if not row_case_ids(rows):
            findings.append(
                root_finding(
                    labels[family],
                    "missing_case_ids",
                    f"root {family!r} has no case_id values",
                )
            )

    return RootAdmission(
        findings=findings,
        admitted_roots=admitted_roots,
        family_case_counts=family_case_counts,
        current_case_coverage=current_case_coverage,
        expected_current_case_coverage=expected_current_case_coverage,
    )


def observation_findings(spec: RootSpec, row: dict[str, str]) -> list[Finding]:
    if is_success(row):
        return []
    terminal_state = value(row, "terminal_state")
    contract_layer = value(row, "contract_layer")
    diagnostic_code = value(row, "diagnostic_code")
    reason = value(row, "reason")
    explicit_stop_reason = value(row, "explicit_stop_reason")
    findings: list[Finding] = []

    if terminal_state in {"", "unknown"}:
        findings.append(
            finding(spec, row, "unknown_terminal_state", "terminal_state is unknown")
        )
    if contract_layer == "unknown_contract" and not (
        terminal_state == "explicit_stop" and explicit_stop_reason
    ):
        findings.append(
            finding(
                spec,
                row,
                "unknown_contract_layer",
                "contract_layer is unknown_contract without explicit stop reason",
            )
        )
    if reason.startswith("rc:") and (
        not diagnostic_code
        or diagnostic_code == "unknown"
        or diagnostic_code.startswith("rc_")
    ):
        findings.append(
            finding(
                spec,
                row,
                "raw_undiagnostic_rc",
                f"reason={reason} diagnostic_code={diagnostic_code or '<empty>'}",
            )
        )
    return findings


def focused_findings(spec: RootSpec, row: dict[str, str]) -> list[Finding]:
    status = value(row, "expected_assertion_status")
    if status == "failed":
        failures = value(row, "expected_assertion_failures")
        return [
            finding(
                spec,
                row,
                "focused_assertion_failed",
                failures or "expected assertion failed",
            )
        ]
    return []


def generic_source_fallback_findings(
    spec: RootSpec, row: dict[str, str]
) -> list[Finding]:
    if is_success(row):
        return []
    active_job = value(row, "active_job")
    recovery_owner = value(row, "recovery_owner")
    terminal_state = value(row, "terminal_state")
    failure_category = value(row, "failure_category")
    if active_job != "source_implementation_repair" and recovery_owner != "source":
        return []
    if (
        terminal_state in BETTER_OWNER_TERMINAL_STATES
        or failure_category in BETTER_OWNER_CATEGORIES
    ):
        return [
            finding(
                spec,
                row,
                "generic_source_fallback",
                f"terminal_state={terminal_state} failure_category={failure_category}",
            )
        ]
    return []


def large_ownership_findings(spec: RootSpec, row: dict[str, str]) -> list[Finding]:
    if is_success(row):
        return []
    findings: list[Finding] = []
    active_job = value(row, "active_job")
    owner = value(row, "recovery_owner") or value(row, "active_owner")
    action = value(row, "repair_action") or value(row, "selected_action")
    target = value(row, "target_path") or value(row, "selected_target")
    evidence_binding = value(row, "evidence_binding_status")
    completion_evidence = value(row, "completion_evidence_status")
    attempt_outcome = value(row, "attempt_outcome")
    target_optional = target_optional_context(row)
    required = [
        ("missing_active_job", "active_job", active_job),
        ("missing_owner", "owner", owner),
        ("missing_action", "action", action),
        ("missing_evidence_binding", "evidence_binding", evidence_binding),
        (
            "missing_completion_evidence",
            "completion_evidence",
            completion_evidence,
        ),
        ("missing_attempt_outcome", "attempt_outcome", attempt_outcome),
    ]
    if not target_optional:
        required.append(("missing_target", "target", target))
    for code, field, raw in required:
        if is_missing_for(row, field, raw):
            findings.append(finding(spec, row, code, f"{code} for failed large row"))
    return findings


def large_disposition_findings(spec: RootSpec, row: dict[str, str]) -> list[Finding]:
    if is_success(row):
        return []
    findings: list[Finding] = []
    disposition = value(row, "large_disposition")
    reason = value(row, "large_disposition_reason")
    owner_action_status = value(row, "large_disposition_owner_action_status")
    evidence = value(row, "large_disposition_evidence")
    owner = value(row, "recovery_owner") or value(row, "active_owner")
    action = value(row, "repair_action") or value(row, "selected_action")
    active_job = value(row, "active_job") or value(row, "runtime_job_kind")

    if is_missing(disposition):
        findings.append(
            finding(
                spec,
                row,
                "missing_large_disposition",
                "failed large row has no large_disposition",
            )
        )
        return findings
    if disposition not in LARGE_DISPOSITIONS:
        findings.append(
            finding(
                spec,
                row,
                "invalid_large_disposition",
                f"large_disposition={disposition}",
            )
        )
        return findings
    if is_missing(reason):
        findings.append(
            finding(
                spec,
                row,
                "missing_large_disposition_reason",
                f"large_disposition={disposition}",
            )
        )
    if is_missing(evidence):
        findings.append(
            finding(
                spec,
                row,
                "missing_large_disposition_evidence",
                f"large_disposition={disposition}",
            )
        )
    if owner_action_status and owner_action_status != "consistent":
        findings.append(
            finding(
                spec,
                row,
                "inconsistent_large_owner_action",
                owner_action_status,
            )
        )
    if owner == "source" and action == "correct_tool_protocol":
        findings.append(
            finding(
                spec,
                row,
                "inconsistent_large_owner_action",
                "source owner cannot carry correct_tool_protocol action",
            )
        )
    if active_job == "tool_protocol_correction" and owner != "tool_protocol":
        findings.append(
            finding(
                spec,
                row,
                "inconsistent_large_owner_action",
                f"tool_protocol_correction owner={owner or '<empty>'}",
            )
        )
    if disposition == "implementation_blocker":
        findings.append(
            finding(
                spec,
                row,
                "large_implementation_blocker",
                reason or "large row remains an implementation blocker",
            )
        )
    if disposition == "split_forward":
        findings.append(
            finding(
                spec,
                row,
                "large_split_forward_open",
                reason or "large row was split forward",
            )
        )
    if disposition == "accepted_external_limitation" and not provider_boundary_context(row):
        findings.append(
            finding(
                spec,
                row,
                "invalid_external_limitation",
                "accepted external limitation without provider boundary evidence",
            )
        )
    return findings


def classify(spec: RootSpec, rows: list[dict[str, str]]) -> list[Finding]:
    findings: list[Finding] = []
    for row in rows:
        findings.extend(observation_findings(spec, row))
        if spec.family in {"focused", "focused-fixture"}:
            findings.extend(focused_findings(spec, row))
        if spec.family == "large":
            findings.extend(generic_source_fallback_findings(spec, row))
            findings.extend(large_ownership_findings(spec, row))
            findings.extend(large_disposition_findings(spec, row))
    return findings


def recheck_row_keys(rows: list[dict[str, str]]) -> set[tuple[str, str]]:
    return {
        (value(row, "case_id"), value(row, "run"))
        for row in rows
        if value(row, "case_id") or value(row, "run")
    }


def focused_summary_findings(
    spec: RootSpec,
    rows: list[dict[str, str]],
    *,
    authoritative_recheck_rows: list[dict[str, str]] | None = None,
) -> list[Finding]:
    findings: list[Finding] = []
    if spec.family not in {"focused", "focused-fixture"}:
        return findings
    rechecked = recheck_row_keys(authoritative_recheck_rows or [])
    for row in rows:
        key = (value(row, "case_id"), value(row, "run"))
        if key in rechecked:
            continue
        findings.extend(focused_findings(spec, row))
    return findings


def root_summary_path(
    spec: RootSpec, *, require_recheck: bool, summary_name: str | None
) -> tuple[list[Finding], Path | None]:
    missing: list[Finding] = []
    if not spec.path.exists():
        missing.append(
            Finding(spec.family, spec.path, "<root>", "?", "missing_root", "root missing")
        )
        return missing, None
    normal = spec.path / "summary.tsv"
    recheck = spec.path / "recheck_summary.tsv"
    if not normal.exists():
        missing.append(
            Finding(
                spec.family,
                spec.path,
                "<root>",
                "?",
                "missing_summary",
                "summary.tsv missing",
            )
        )
    if require_recheck and not recheck.exists():
        missing.append(
            Finding(
                spec.family,
                spec.path,
                "<root>",
                "?",
                "missing_recheck_summary",
                "recheck_summary.tsv missing",
            )
        )
    selected_name = summary_name or ("recheck_summary.tsv" if require_recheck else "summary.tsv")
    selected = spec.path / selected_name
    if not selected.exists():
        selected = None
    return missing, selected


def render(findings: list[Finding], admission: RootAdmission | None = None) -> str:
    lines = ["# Eval Sign-off", ""]
    if admission is not None:
        lines.append(f"root_admission_status: {admission.status}")
        lines.append(f"root_admission_reason: {admission.reason}")
        admitted = ", ".join(
            f"{root.family}={root.path}({root.case_count},{root.role})"
            for root in admission.admitted_roots
        )
        lines.append(f"admitted_roots: {admitted or 'none'}")
        family_counts = ", ".join(
            f"{family}={admission.family_case_counts[family]}"
            for family in sorted(admission.family_case_counts)
        )
        lines.append(f"family_case_counts: {family_counts}")
        lines.append(
            "current_case_coverage: "
            f"{admission.current_case_coverage}/"
            f"{admission.expected_current_case_coverage}"
        )
        lines.append("")
    if not findings:
        lines.append("status: pass")
        return "\n".join(lines) + "\n"
    lines.append("status: fail")
    lines.append("")
    lines.append("| family | case | run | code | detail |")
    lines.append("| --- | --- | --- | --- | --- |")
    for item in findings:
        lines.append(
            "| "
            + " | ".join(
                [
                    escape(item.family),
                    escape(item.case_id),
                    escape(item.run),
                    escape(item.code),
                    escape(item.detail),
                ]
            )
            + " |"
        )
    return "\n".join(lines) + "\n"


def escape(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def main() -> int:
    args = parse_args()
    if not args.root:
        raise SystemExit("at least one --root FAMILY=PATH is required")
    specs = [parse_root(item) for item in args.root]
    admission = admit_roots(
        specs,
        require_recheck=args.require_recheck,
        summary_name=args.summary_name,
    )
    all_findings: list[Finding] = []
    if admission.findings:
        all_findings.extend(admission.findings)
        sys.stdout.write(render(all_findings, admission))
        return 1
    for spec in specs:
        root_findings, summary_path = root_summary_path(
            spec,
            require_recheck=args.require_recheck,
            summary_name=args.summary_name,
        )
        all_findings.extend(root_findings)
        if summary_path is None:
            continue
        if args.require_recheck and spec.family in {"focused", "focused-fixture"}:
            normal_summary = spec.path / "summary.tsv"
            if normal_summary.exists():
                selected_rows = read_rows(summary_path)
                all_findings.extend(
                    focused_summary_findings(
                        spec,
                        read_rows(normal_summary),
                        authoritative_recheck_rows=selected_rows,
                    )
                )
                all_findings.extend(classify(spec, selected_rows))
                continue
        all_findings.extend(classify(spec, read_rows(summary_path)))
    sys.stdout.write(render(all_findings, admission))
    return 1 if all_findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
