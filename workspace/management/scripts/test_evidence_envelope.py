import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from evidence_envelope import (
    CONSUMER_ADAPTERS,
    envelope_for,
    guard_errors,
    registered_families,
)


def enveloped(family):
    return {
        "legacy": "unchanged",
        "evidence_envelope": {
            "envelope_version": 1,
            "family": family,
            "kind": "guard_fixture",
            "epoch": 123,
            "claims": [],
            "nearest_miss": [],
            "source_refs": [],
        },
    }


class EvidenceEnvelopeGuardTests(unittest.TestCase):
    def test_every_registered_family_is_readable_by_all_consumers(self):
        self.assertEqual(guard_errors(), [])
        self.assertEqual(
            len(registered_families()) * 3,
            33,
            "promotion decisions expand the transverse family guard to 33/33",
        )
        for family in registered_families():
            for consumer in ("collector", "sheet", "classify"):
                self.assertEqual(
                    envelope_for(enveloped(family), consumer)["family"], family
                )

    def test_new_family_without_adapters_fails_all_three_directions(self):
        errors = guard_errors(registry=(*registered_families(), "future"))
        self.assertEqual(
            errors,
            [
                "collector: missing family adapters: future",
                "sheet: missing family adapters: future",
                "classify: missing family adapters: future",
            ],
        )

    def test_t1_family_would_fail_all_three_consumers_without_followup(self):
        adapters = {
            consumer: {
                family: mode for family, mode in mapping.items() if family != "T"
            }
            for consumer, mapping in CONSUMER_ADAPTERS.items()
        }
        self.assertEqual(
            guard_errors(adapters=adapters),
            [
                "collector: missing family adapters: T",
                "sheet: missing family adapters: T",
                "classify: missing family adapters: T",
            ],
        )

    def test_dead_adapter_is_also_reported(self):
        adapters = {
            consumer: {**mapping, "retired": "invalid"}
            for consumer, mapping in CONSUMER_ADAPTERS.items()
        }
        self.assertTrue(
            all("dead family adapters: retired" in error for error in guard_errors(adapters=adapters))
        )

    def test_historical_document_uses_legacy_fallback(self):
        for consumer in ("collector", "sheet", "classify"):
            self.assertIsNone(envelope_for({"legacy": "value"}, consumer))


if __name__ == "__main__":
    unittest.main()
