from __future__ import annotations

import math
from collections import Counter, defaultdict
from typing import Any


def build_resource_diagnostics(
    *, records: list[dict[str, Any]], lane_name: str
) -> dict[str, Any]:
    rows = [_diagnostic_row(record, lane_name) for record in records]
    phase_names = sorted({phase for row in rows for phase in row["phase_timings_ms"]})
    phase_timing_recorded = sum(bool(row["phase_timings_ms"]) for row in rows)
    provider_attempts = sum(row["provider_attempt_count"] for row in rows)
    provider_complete = sum(row["provider_complete_attempt_count"] for row in rows)
    evaluation_count = sum(row["evaluation_count"] for row in rows)
    runtime_recorded = sum(row["runtime_recorded_count"] for row in rows)
    executed_count = sum(row["executed_count"] for row in rows)
    executed_runtime_recorded = sum(
        row["executed_runtime_recorded_count"] for row in rows
    )
    unexecuted_count = sum(row["unexecuted_count"] for row in rows)
    input_tokens = [row["candidate_input_tokens"] for row in rows]
    total_tokens = [row["candidate_total_tokens"] for row in rows]
    input_sum = _sum_complete(input_tokens)
    total_sum = _sum_complete(total_tokens)
    overall_input_share = (
        _rounded(100.0 * input_sum / total_sum)
        if input_sum is not None and total_sum not in (None, 0)
        else None
    )
    return {
        "schema_version": "commandagent.goal_verify.resource_diagnostics.v4",
        "record_count": len(rows),
        "absolute": {
            "baseline_wall_ms": _distribution(
                [row["baseline_wall_ms"] for row in rows]
            ),
            "baseline_total_tokens": _distribution(
                [row["baseline_total_tokens"] for row in rows]
            ),
            "candidate_wall_ms": _distribution(
                [row["candidate_wall_ms"] for row in rows]
            ),
            "candidate_input_tokens": _distribution(input_tokens),
            "candidate_output_tokens": _distribution(
                [row["candidate_output_tokens"] for row in rows]
            ),
            "candidate_total_tokens": _distribution(total_tokens),
        },
        "provider_timing": {
            "attempt_count": provider_attempts,
            "complete_attempt_count": provider_complete,
            "complete": provider_attempts > 0
            and provider_complete == provider_attempts,
            "client_wall_ms": _distribution(
                [row["provider_client_wall_ms"] for row in rows]
            ),
            "prompt_eval_ms": _distribution(
                [row["provider_prompt_eval_ms"] for row in rows]
            ),
            "output_eval_ms": _distribution(
                [row["provider_output_eval_ms"] for row in rows]
            ),
        },
        "candidate_phase_timing": {
            "recorded_count": phase_timing_recorded,
            "missing_count": len(rows) - phase_timing_recorded,
            "complete": bool(rows) and phase_timing_recorded == len(rows),
            "phases_ms": {
                phase: _distribution(
                    [row["phase_timings_ms"].get(phase) for row in rows]
                )
                for phase in phase_names
            },
            "instrumentation_residual_ms": _distribution(
                [row["phase_timing_residual_ms"] for row in rows]
            ),
        },
        "oracle_runtime": {
            "evaluation_count": evaluation_count,
            "runtime_recorded_count": runtime_recorded,
            "runtime_missing_count": evaluation_count - runtime_recorded,
            "executed_count": executed_count,
            "executed_runtime_recorded_count": executed_runtime_recorded,
            "executed_runtime_missing_count": executed_count
            - executed_runtime_recorded,
            "unexecuted_count": unexecuted_count,
            "recorded_runtime_sum_ms": _distribution(
                [row["recorded_runtime_sum_ms"] for row in rows]
            ),
        },
        "attribution": {
            "residual_ms": _distribution([row["residual_ms"] for row in rows]),
            "provider_share_of_candidate_wall_pct": _distribution(
                [row["provider_wall_share_pct"] for row in rows]
            ),
            "output_share_of_provider_wall_pct": _distribution(
                [row["output_provider_share_pct"] for row in rows]
            ),
            "input_share_of_candidate_tokens_pct": _distribution(
                [row["input_token_share_pct"] for row in rows]
            ),
            "overall_input_share_of_candidate_tokens_pct": overall_input_share,
        },
        "cell_median_increase_pct": _cell_medians(rows),
        "tails": {
            "wall_increase_ratio_top_5pct": _tail_summary(
                rows,
                value_field="wall_increase_pct",
                baseline_field="baseline_wall_ms",
                candidate_field="candidate_wall_ms",
            ),
            "token_increase_ratio_top_5pct": _tail_summary(
                rows,
                value_field="token_increase_pct",
                baseline_field="baseline_total_tokens",
                candidate_field="candidate_total_tokens",
            ),
            "candidate_absolute_wall_top_5pct": _tail_summary(
                rows,
                value_field="candidate_wall_ms",
                baseline_field="baseline_wall_ms",
                candidate_field="candidate_wall_ms",
            ),
        },
    }


def _diagnostic_row(record: dict[str, Any], lane_name: str) -> dict[str, Any]:
    lane = record.get("lanes", {}).get(lane_name, {})
    usage = lane.get("resource_usage", {})
    baseline = record.get("baseline", {}).get("resource_usage", {})
    attempts = lane.get("attempts", [])
    attempts = attempts if isinstance(attempts, list) else []
    provider_rows = [_provider_timing(attempt) for attempt in attempts]
    provider_complete = [row for row in provider_rows if row["complete"]]
    provider_client_wall_ms = _sum_optional(
        [row["client_wall_ms"] for row in provider_rows]
    )
    provider_prompt_eval_ms = _sum_optional(
        [row["prompt_eval_ms"] for row in provider_rows]
    )
    provider_output_eval_ms = _sum_optional(
        [row["output_eval_ms"] for row in provider_rows]
    )
    evaluations = lane.get("execution", {}).get("evaluations", [])
    evaluations = evaluations if isinstance(evaluations, list) else []
    recorded_runtimes = [
        float(evaluation["runtime_ms"])
        for evaluation in evaluations
        if _nonnegative_number(evaluation.get("runtime_ms"))
    ]
    executed = [evaluation for evaluation in evaluations if evaluation.get("executed")]
    executed_with_runtime = [
        evaluation
        for evaluation in executed
        if _nonnegative_number(evaluation.get("runtime_ms"))
    ]
    candidate_wall_ms = _optional_number(usage.get("wall_time_ms"))
    candidate_input_tokens = _optional_number(usage.get("input_tokens"))
    candidate_output_tokens = _optional_number(usage.get("output_tokens"))
    candidate_total_tokens = _optional_number(usage.get("total_tokens"))
    baseline_wall_ms = _optional_number(baseline.get("wall_time_ms"))
    baseline_total_tokens = _optional_number(baseline.get("total_tokens"))
    phase_timings = usage.get("phase_timings_ms")
    phase_timings = (
        {
            phase: float(value)
            for phase, value in phase_timings.items()
            if isinstance(phase, str) and _nonnegative_number(value)
        }
        if isinstance(phase_timings, dict)
        else {}
    )
    runtime_sum = sum(recorded_runtimes)
    residual_ms = (
        candidate_wall_ms - provider_client_wall_ms - runtime_sum
        if candidate_wall_ms is not None and provider_client_wall_ms is not None
        else None
    )
    return {
        "pair_id": record.get("pair_id"),
        "cell_id": record.get("cell_id"),
        "baseline_wall_ms": baseline_wall_ms,
        "baseline_total_tokens": baseline_total_tokens,
        "candidate_wall_ms": candidate_wall_ms,
        "candidate_input_tokens": candidate_input_tokens,
        "candidate_output_tokens": candidate_output_tokens,
        "candidate_total_tokens": candidate_total_tokens,
        "provider_attempt_count": len(provider_rows),
        "provider_complete_attempt_count": len(provider_complete),
        "provider_client_wall_ms": provider_client_wall_ms,
        "provider_prompt_eval_ms": provider_prompt_eval_ms,
        "provider_output_eval_ms": provider_output_eval_ms,
        "evaluation_count": len(evaluations),
        "runtime_recorded_count": len(recorded_runtimes),
        "executed_count": len(executed),
        "executed_runtime_recorded_count": len(executed_with_runtime),
        "unexecuted_count": len(evaluations) - len(executed),
        "recorded_runtime_sum_ms": runtime_sum,
        "phase_timings_ms": phase_timings,
        "phase_timing_residual_ms": _optional_number(
            usage.get("phase_timing_residual_ms")
        ),
        "residual_ms": residual_ms,
        "wall_increase_pct": _ratio(candidate_wall_ms, baseline_wall_ms),
        "token_increase_pct": _ratio(candidate_total_tokens, baseline_total_tokens),
        "provider_wall_share_pct": _ratio(provider_client_wall_ms, candidate_wall_ms),
        "output_provider_share_pct": _ratio(
            provider_output_eval_ms, provider_client_wall_ms
        ),
        "input_token_share_pct": _ratio(candidate_input_tokens, candidate_total_tokens),
    }


def _provider_timing(attempt: dict[str, Any]) -> dict[str, Any]:
    response = attempt.get("response", {})
    response = response.get("response", {}) if isinstance(response, dict) else {}
    response = response if isinstance(response, dict) else {}
    client = _nanoseconds_to_ms(response.get("client_wall_time_ns"))
    prompt = _nanoseconds_to_ms(response.get("prompt_eval_duration"))
    output = _nanoseconds_to_ms(response.get("eval_duration"))
    return {
        "client_wall_ms": client,
        "prompt_eval_ms": prompt,
        "output_eval_ms": output,
        "complete": client is not None and prompt is not None and output is not None,
    }


def _cell_medians(rows: list[dict[str, Any]]) -> dict[str, dict[str, float | None]]:
    cells: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        cell_id = row.get("cell_id")
        if isinstance(cell_id, str) and cell_id:
            cells[cell_id].append(row)
    return {
        cell_id: {
            "wall_time_increase_pct": _percentile_optional(
                [row["wall_increase_pct"] for row in cell_rows], 0.50
            ),
            "total_tokens_increase_pct": _percentile_optional(
                [row["token_increase_pct"] for row in cell_rows], 0.50
            ),
        }
        for cell_id, cell_rows in sorted(cells.items())
    }


def _tail_summary(
    rows: list[dict[str, Any]],
    *,
    value_field: str,
    baseline_field: str,
    candidate_field: str,
) -> dict[str, Any]:
    eligible = [row for row in rows if _nonnegative_number(row.get(value_field))]
    count = math.ceil(0.05 * len(eligible)) if eligible else 0
    selected = sorted(eligible, key=lambda row: row[value_field], reverse=True)[:count]
    cells = Counter(
        row["cell_id"]
        for row in selected
        if isinstance(row.get("cell_id"), str) and row["cell_id"]
    )
    return {
        "fraction": 0.05,
        "eligible_count": len(eligible),
        "selected_count": len(selected),
        "cell_counts": dict(sorted(cells.items())),
        "value": _distribution([row[value_field] for row in selected]),
        "baseline": _distribution([row[baseline_field] for row in selected]),
        "candidate": _distribution([row[candidate_field] for row in selected]),
    }


def _distribution(values: list[Any]) -> dict[str, float | int | None]:
    numeric = [float(value) for value in values if _nonnegative_number(value)]
    if not numeric:
        return {
            "count": 0,
            "min": None,
            "p50": None,
            "p95": None,
            "max": None,
            "mean": None,
        }
    return {
        "count": len(numeric),
        "min": _rounded(min(numeric)),
        "p50": _percentile(numeric, 0.50),
        "p95": _percentile(numeric, 0.95),
        "max": _rounded(max(numeric)),
        "mean": _rounded(sum(numeric) / len(numeric)),
    }


def _percentile_optional(values: list[Any], quantile: float) -> float | None:
    numeric = [float(value) for value in values if _nonnegative_number(value)]
    return _percentile(numeric, quantile) if numeric else None


def _percentile(values: list[float], quantile: float) -> float:
    ordered = sorted(values)
    index = max(0, math.ceil(quantile * len(ordered)) - 1)
    return _rounded(ordered[index])


def _sum_optional(values: list[float | None]) -> float | None:
    if not values or any(value is None for value in values):
        return None
    return sum(float(value) for value in values if value is not None)


def _sum_complete(values: list[float | None]) -> float | None:
    if not values or any(value is None for value in values):
        return None
    return sum(float(value) for value in values if value is not None)


def _ratio(numerator: float | None, denominator: float | None) -> float | None:
    if numerator is None or denominator is None or denominator <= 0:
        return None
    return 100.0 * numerator / denominator


def _nanoseconds_to_ms(value: Any) -> float | None:
    number = _optional_number(value)
    return number / 1_000_000.0 if number is not None else None


def _optional_number(value: Any) -> float | None:
    return float(value) if _nonnegative_number(value) else None


def _nonnegative_number(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(float(value))
        and value >= 0
    )


def _rounded(value: float) -> float:
    return round(value, 6)
