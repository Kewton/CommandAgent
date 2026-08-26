from __future__ import annotations

import math
import random
from collections import defaultdict
from collections.abc import Callable
from typing import Any


def validate_cluster_design(
    rows: list[dict[str, Any]],
    *,
    minimum_clusters_per_cell: int,
    minimum_pairs_per_cluster: int,
) -> list[str]:
    errors = []
    grouped: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    seen_pairs = set()
    for index, row in enumerate(rows):
        cell_id = row.get("cell_id")
        cluster_id = row.get("source_task_id")
        pair_id = row.get("pair_id")
        if not all(
            isinstance(value, str) and value for value in (cell_id, cluster_id, pair_id)
        ):
            errors.append(f"rows[{index}] lacks cell_id/source_task_id/pair_id")
            continue
        if pair_id in seen_pairs:
            errors.append(f"duplicate pair_id: {pair_id}")
        seen_pairs.add(pair_id)
        grouped[cell_id][cluster_id] += 1
    for cell_id, clusters in sorted(grouped.items()):
        if len(clusters) < minimum_clusters_per_cell:
            errors.append(
                f"cell {cell_id} has {len(clusters)} clusters; "
                f"requires {minimum_clusters_per_cell}"
            )
        for cluster_id, count in sorted(clusters.items()):
            if count < minimum_pairs_per_cluster:
                errors.append(
                    f"cluster {cell_id}/{cluster_id} has {count} pairs; "
                    f"requires {minimum_pairs_per_cluster}"
                )
    return errors


def cluster_paired_bootstrap_interval(
    rows: list[dict[str, Any]],
    *,
    delta: Callable[[dict[str, Any]], float],
    samples: int,
    seed: int,
    hierarchical: bool = True,
) -> dict[str, Any]:
    """Resample source-task clusters equally, then optionally rows within cluster."""
    if samples < 1:
        raise ValueError("bootstrap samples must be positive")
    clusters: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        cluster_id = row.get("source_task_id")
        if not isinstance(cluster_id, str) or not cluster_id:
            raise ValueError("every row must have a source_task_id")
        clusters[cluster_id].append(row)
    cluster_ids = sorted(clusters)
    if len(cluster_ids) < 2:
        return {
            "status": "insufficient_evidence",
            "cluster_count": len(cluster_ids),
            "pair_count": len(rows),
            "lower": None,
            "upper": None,
        }
    rng = random.Random(seed)
    estimates = []
    for _ in range(samples):
        sampled_clusters = [
            cluster_ids[rng.randrange(len(cluster_ids))] for _ in cluster_ids
        ]
        cluster_estimates = []
        for cluster_id in sampled_clusters:
            members = clusters[cluster_id]
            sampled_members = (
                [members[rng.randrange(len(members))] for _ in members]
                if hierarchical
                else members
            )
            cluster_estimates.append(
                sum(delta(row) for row in sampled_members) / len(sampled_members)
            )
        estimates.append(sum(cluster_estimates) / len(cluster_estimates))
    estimates.sort()
    lower_index = max(0, math.floor(0.025 * (len(estimates) - 1)))
    upper_index = min(len(estimates) - 1, math.ceil(0.975 * (len(estimates) - 1)))
    return {
        "status": "estimated",
        "method": "hierarchical_cluster_paired_percentile"
        if hierarchical
        else "cluster_paired_percentile",
        "cluster_count": len(cluster_ids),
        "pair_count": len(rows),
        "samples": samples,
        "seed": seed,
        "lower": round(estimates[lower_index], 6),
        "upper": round(estimates[upper_index], 6),
    }
