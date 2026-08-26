from __future__ import annotations

import copy
import hashlib
import json
import random
import urllib.request
from collections import Counter
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_live import (
    _atomic_json,
    load_json,
    request_ollama,
    sha256_file,
)

REVIEW_SCHEMA: dict[str, Any] = {
    "type": "object",
    "additionalProperties": False,
    "required": ["reviews"],
    "properties": {
        "reviews": {
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": False,
                "required": ["pair_id", "preferred_variant", "reason_codes"],
                "properties": {
                    "pair_id": {"type": "string"},
                    "preferred_variant": {"enum": ["A", "B", "tie"]},
                    "reason_codes": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 4,
                        "items": {"type": "string"},
                    },
                },
            },
        }
    },
}


def _card(case: dict[str, Any]) -> dict[str, Any]:
    observation = case["observation"]
    return {
        "claimed_claim_ids": observation["claimed_claim_ids"],
        "verified_claims": observation["verified_claims"],
        "verdict": observation["verdict"],
        "schema_valid": observation["schema_valid"],
    }


def prepare_items(
    baseline: dict[str, Any], candidate: dict[str, Any], *, seed: int
) -> tuple[list[dict[str, Any]], dict[str, dict[str, str]]]:
    baseline_by_id = {case["case_id"]: case for case in baseline["cases"]}
    candidate_by_id = {case["case_id"]: case for case in candidate["cases"]}
    if baseline_by_id.keys() != candidate_by_id.keys():
        raise ValueError("blind review requires identical paired case IDs")
    rng = random.Random(seed)
    items = []
    mapping: dict[str, dict[str, str]] = {}
    for pair_id in sorted(baseline_by_id):
        base = baseline_by_id[pair_id]
        cand = candidate_by_id[pair_id]
        swapped = bool(rng.getrandbits(1))
        variants = [_card(cand), _card(base)] if swapped else [_card(base), _card(cand)]
        mapping[pair_id] = {
            "A": "candidate" if swapped else "baseline",
            "B": "baseline" if swapped else "candidate",
        }
        items.append(
            {
                "pair_id": pair_id,
                "intent": base["intent"],
                "profile": base["profile"],
                "language": base["language"],
                "size": base["size"],
                "required_claims": base["required_claims"],
                "variant_A": variants[0],
                "variant_B": variants[1],
            }
        )
    return items, mapping


def _review_prompt(contract: dict[str, Any], items: list[dict[str, Any]]) -> str:
    criteria = "\n".join(f"- {criterion}" for criterion in contract["criteria"])
    return (
        "You are reviewing anonymous verification variants. Variant identity is intentionally "
        "hidden. Judge only the fields shown. For every pair_id return A, B, or tie and 1-4 short "
        "reason codes. Do not infer identity from formatting and do not emit prose.\n\n"
        f"Criteria:\n{criteria}\n\n"
        f"ITEMS JSON:\n{json.dumps(items, ensure_ascii=False)}"
    )


def _parse_reviews(raw: str, expected: set[str]) -> list[dict[str, Any]]:
    value = json.loads(raw)
    reviews = value.get("reviews") if isinstance(value, dict) else None
    if not isinstance(reviews, list):
        raise TypeError("review response lacks reviews array")
    ids = [review.get("pair_id") for review in reviews if isinstance(review, dict)]
    if len(ids) != len(set(ids)) or set(ids) != expected:
        raise ValueError("review response pair IDs differ from requested batch")
    for review in reviews:
        if review.get("preferred_variant") not in {"A", "B", "tie"}:
            raise ValueError("review response has invalid preference")
        reasons = review.get("reason_codes")
        if not isinstance(reasons, list) or not reasons:
            raise ValueError("review response lacks reason codes")
    return reviews


def run_blind_review(
    *,
    root: Path,
    baseline_path: Path,
    candidate_draft_path: Path,
    contract_path: Path,
    run_dir: Path,
) -> dict[str, Any]:
    baseline = load_json(baseline_path)
    candidate = load_json(candidate_draft_path)
    contract = load_json(contract_path)
    tags_endpoint = contract["endpoint"].removesuffix("/api/generate") + "/api/tags"
    with urllib.request.urlopen(tags_endpoint, timeout=30) as response:
        tags = json.loads(response.read().decode())
    matching = [
        model for model in tags.get("models", []) if model.get("name") == contract["reviewer_model"]
    ]
    if len(matching) != 1 or matching[0].get("digest") != contract["reviewer_model_digest"]:
        raise ValueError("blind reviewer model digest differs from frozen contract")
    items, mapping = prepare_items(
        baseline, candidate, seed=int(contract["randomization_seed"])
    )
    items_sha256 = hashlib.sha256(
        json.dumps(items, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
    ).hexdigest()
    mapping_sha256 = hashlib.sha256(
        json.dumps(mapping, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
    ).hexdigest()
    run_dir.mkdir(parents=True, exist_ok=True)
    frozen = {
        "schema_version": "commandagent.goal_verify.phase6_blind_manifest.v0",
        "contract": str(contract_path.relative_to(root)),
        "contract_sha256": sha256_file(contract_path),
        "baseline_sha256": sha256_file(baseline_path),
        "candidate_draft_sha256": sha256_file(candidate_draft_path),
        "reviewer_model": contract["reviewer_model"],
        "reviewer_model_digest": contract["reviewer_model_digest"],
        "review_schema_sha256": hashlib.sha256(
            json.dumps(REVIEW_SCHEMA, sort_keys=True).encode()
        ).hexdigest(),
        "reviewer_source_sha256": {
            "scripts/eval-goal-verify-blind.py": sha256_file(
                root / "scripts/eval-goal-verify-blind.py"
            ),
            "scripts/eval_lib/goal_verify_blind.py": sha256_file(
                root / "scripts/eval_lib/goal_verify_blind.py"
            ),
        },
        "item_count": len(items),
        "blind_items_sha256": items_sha256,
        "variant_mapping_sha256": mapping_sha256,
    }
    manifest_path = run_dir / "blind-review-manifest.json"
    if manifest_path.exists() and load_json(manifest_path) != frozen:
        raise ValueError("blind review inputs differ from frozen manifest")
    _atomic_json(manifest_path, frozen)
    items_path = run_dir / "blind-items.json"
    mapping_path = run_dir / "variant-mapping.json"
    _atomic_json(items_path, items)
    _atomic_json(mapping_path, mapping)

    batch_size = int(contract["batch_size"])
    all_reviews: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    batch_record_sha256: dict[str, str] = {}
    for offset in range(0, len(items), batch_size):
        batch = items[offset : offset + batch_size]
        batch_id = offset // batch_size + 1
        record_path = run_dir / "raw" / f"batch-{batch_id:02d}.json"
        if record_path.exists():
            record = load_json(record_path)
        else:
            record = {}
            for attempt in range(int(contract["max_retries"]) + 1):
                response = request_ollama(
                    endpoint=contract["endpoint"],
                    model=contract["reviewer_model"],
                    prompt=_review_prompt(contract, batch),
                    schema=REVIEW_SCHEMA,
                    seed=int(contract["randomization_seed"]) + batch_id + attempt,
                    temperature=float(contract["generation"]["temperature"]),
                    num_predict=int(contract["generation"]["num_predict"]),
                    timeout_sec=int(contract["generation"]["request_timeout_sec"]),
                    keep_alive=str(contract["generation"]["keep_alive"]),
                    think=bool(contract["generation"]["think"]),
                )
                error = None
                reviews = None
                if response["status"] == "completed":
                    try:
                        reviews = _parse_reviews(
                            response["response"].get("response", ""),
                            {item["pair_id"] for item in batch},
                        )
                    except (TypeError, ValueError, json.JSONDecodeError) as caught:
                        error = str(caught)
                else:
                    error = response.get("error", "provider_error")
                record = {
                    "batch_id": batch_id,
                    "attempt": attempt,
                    "response": response,
                    "reviews": reviews,
                    "error": error,
                }
                if reviews is not None:
                    break
            _atomic_json(record_path, record)
        if record.get("batch_id") != batch_id:
            raise ValueError(f"blind record batch ID mismatch: {batch_id}")
        expected_ids = {item["pair_id"] for item in batch}
        reparsed = None
        if record.get("response", {}).get("status") == "completed":
            try:
                reparsed = _parse_reviews(
                    record["response"]["response"].get("response", ""), expected_ids
                )
            except (TypeError, ValueError, json.JSONDecodeError):
                reparsed = None
        if reparsed != record.get("reviews"):
            raise ValueError(f"blind record stored reviews differ from raw response: {batch_id}")
        batch_record_sha256[f"batch-{batch_id:02d}"] = sha256_file(record_path)
        if record.get("reviews") is None:
            failures.append({"batch_id": batch_id, "error": record.get("error")})
        else:
            all_reviews.extend(record["reviews"])

    decoded = []
    counts: Counter[str] = Counter()
    for review in all_reviews:
        preference = review["preferred_variant"]
        decoded_preference = "tie" if preference == "tie" else mapping[review["pair_id"]][preference]
        counts[decoded_preference] += 1
        decoded.append({**review, "decoded_preference": decoded_preference})
    complete = not failures and len(decoded) == len(items)
    candidate_corpus_sha256 = None
    if complete:
        final_candidate = copy.deepcopy(candidate)
        final_candidate["annotation_protocol"] = {
            "method": "variant-hidden local-provider review of normalized evidence cards",
            "label_author": "phase6-live-runner",
            "reviewer": contract["reviewer_model"],
            "reviewed_at": contract["frozen_at"],
            "status": "reviewed",
            "disagreements": [],
        }
        candidate_path = run_dir / "candidate-corpus.json"
        _atomic_json(candidate_path, final_candidate)
        candidate_corpus_sha256 = sha256_file(candidate_path)
    report = {
        "schema_version": "commandagent.goal_verify.phase6_blind_report.v0",
        "complete": complete,
        "reviewed_pairs": len(decoded),
        "expected_pairs": len(items),
        "blind_items_sha256": items_sha256,
        "variant_mapping_sha256": mapping_sha256,
        "blind_items_file_sha256": sha256_file(items_path),
        "variant_mapping_file_sha256": sha256_file(mapping_path),
        "batch_record_sha256": batch_record_sha256,
        "candidate_corpus_sha256": candidate_corpus_sha256,
        "preference_counts": dict(sorted(counts.items())),
        "failures": failures,
        "reviews": decoded,
    }
    _atomic_json(run_dir / "blind-review-report.json", report)
    return report


def validate_blind_evidence(
    *,
    root: Path,
    baseline_path: Path,
    candidate_draft_path: Path,
    contract_path: Path,
    run_dir: Path,
) -> dict[str, Any]:
    baseline = load_json(baseline_path)
    candidate_draft = load_json(candidate_draft_path)
    contract = load_json(contract_path)
    manifest = load_json(run_dir / "blind-review-manifest.json")
    report = load_json(run_dir / "blind-review-report.json")
    expected_items, expected_mapping = prepare_items(
        baseline, candidate_draft, seed=int(contract["randomization_seed"])
    )
    items = json.loads((run_dir / "blind-items.json").read_text(encoding="utf-8"))
    mapping = load_json(run_dir / "variant-mapping.json")
    if items != expected_items or mapping != expected_mapping:
        raise ValueError("blind items or mapping differ from deterministic reconstruction")
    canonical_items_hash = hashlib.sha256(
        json.dumps(items, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
    ).hexdigest()
    canonical_mapping_hash = hashlib.sha256(
        json.dumps(mapping, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
    ).hexdigest()
    expected_manifest_fields = {
        "contract_sha256": sha256_file(contract_path),
        "baseline_sha256": sha256_file(baseline_path),
        "candidate_draft_sha256": sha256_file(candidate_draft_path),
        "item_count": len(items),
        "blind_items_sha256": canonical_items_hash,
        "variant_mapping_sha256": canonical_mapping_hash,
    }
    if any(manifest.get(key) != value for key, value in expected_manifest_fields.items()):
        raise ValueError("blind manifest differs from reconstructed inputs")
    for source_path, expected_hash in manifest.get("reviewer_source_sha256", {}).items():
        if sha256_file(root / source_path) != expected_hash:
            raise ValueError(f"blind reviewer source hash mismatch: {source_path}")
    if report.get("blind_items_sha256") != canonical_items_hash or report.get(
        "variant_mapping_sha256"
    ) != canonical_mapping_hash:
        raise ValueError("blind report canonical item or mapping hash mismatch")
    if report.get("blind_items_file_sha256") != sha256_file(run_dir / "blind-items.json"):
        raise ValueError("blind item file hash mismatch")
    if report.get("variant_mapping_file_sha256") != sha256_file(
        run_dir / "variant-mapping.json"
    ):
        raise ValueError("blind mapping file hash mismatch")

    batch_size = int(contract["batch_size"])
    expected_batch_count = (len(items) + batch_size - 1) // batch_size
    expected_batch_names = {f"batch-{batch_id:02d}" for batch_id in range(1, expected_batch_count + 1)}
    if set(report.get("batch_record_sha256", {})) != expected_batch_names:
        raise ValueError("blind report batch set is incomplete or has extras")
    actual_batch_names = {path.stem for path in (run_dir / "raw").glob("batch-*.json")}
    if actual_batch_names != expected_batch_names:
        raise ValueError("blind raw batch set is incomplete or has extras")

    all_reviews: list[dict[str, Any]] = []
    for batch_index in range(expected_batch_count):
        batch_id = batch_index + 1
        batch_name = f"batch-{batch_id:02d}"
        batch = items[batch_index * batch_size : (batch_index + 1) * batch_size]
        record_path = run_dir / "raw" / f"{batch_name}.json"
        if sha256_file(record_path) != report["batch_record_sha256"][batch_name]:
            raise ValueError(f"blind batch hash mismatch: {batch_name}")
        record = load_json(record_path)
        if record.get("batch_id") != batch_id:
            raise ValueError(f"blind batch ID mismatch: {batch_name}")
        parsed = _parse_reviews(
            record.get("response", {}).get("response", {}).get("response", ""),
            {item["pair_id"] for item in batch},
        )
        if parsed != record.get("reviews"):
            raise ValueError(f"blind stored reviews differ from raw response: {batch_name}")
        all_reviews.extend(parsed)

    decoded = []
    counts: Counter[str] = Counter()
    for review in all_reviews:
        preference = review["preferred_variant"]
        decoded_preference = (
            "tie" if preference == "tie" else mapping[review["pair_id"]][preference]
        )
        counts[decoded_preference] += 1
        decoded.append({**review, "decoded_preference": decoded_preference})
    if report.get("reviews") != decoded or report.get("preference_counts") != dict(
        sorted(counts.items())
    ):
        raise ValueError("blind report decisions differ from raw batch reconstruction")
    if not report.get("complete") or report.get("reviewed_pairs") != len(items):
        raise ValueError("blind report is incomplete")

    candidate_path = run_dir / "candidate-corpus.json"
    final_candidate = load_json(candidate_path)
    if final_candidate.get("cases") != candidate_draft.get("cases"):
        raise ValueError("blind-reviewed candidate cases differ from frozen draft")
    if final_candidate.get("annotation_protocol", {}).get("status") != "reviewed":
        raise ValueError("blind-reviewed candidate annotation is incomplete")
    if report.get("candidate_corpus_sha256") != sha256_file(candidate_path):
        raise ValueError("blind-reviewed candidate corpus hash mismatch")
    return report
