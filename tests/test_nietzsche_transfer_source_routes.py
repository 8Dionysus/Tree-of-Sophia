from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import build_nietzsche_transfer_source_routes as builder
import validate_nietzsche_transfer_source_routes as validator


class NietzscheTransferSourceRouteTests(unittest.TestCase):
    def test_tracked_source_maps_pairings_and_routes_close(self) -> None:
        self.assertEqual([], validator.validate())

    def test_generation_is_deterministic_from_tracked_maps(self) -> None:
        for config in builder.CONFIGS:
            outputs = builder.build_one(config, "2026-08-08T21:15:00-06:00")
            for relative, rendered in outputs.items():
                self.assertEqual((REPO_ROOT / relative).read_text(encoding="utf-8"), rendered)

    def test_route_contract_rejects_alignment_promotion(self) -> None:
        schema_validator = validator._schema(validator.ROUTE_SCHEMA)
        config = builder.CONFIGS[0]
        route_path = Path("ToS/source-witnesses/works/friedrich-nietzsche") / config["slug"] / "alignments/structure" / config["pair_slug"] / "transfer-candidate-source-structural-route.v1.json"
        payload = json.loads((REPO_ROOT / route_path).read_text(encoding="utf-8"))
        payload["candidates"][0]["possible_source_structural_routes"][0]["translation_alignment_claimed"] = True
        self.assertTrue(list(schema_validator.iter_errors(payload)))

    def test_pair_contract_rejects_human_review_promotion(self) -> None:
        schema_validator = validator._schema(validator.PAIR_SCHEMA)
        config = builder.CONFIGS[1]
        pair_path = Path("ToS/source-witnesses/works/friedrich-nietzsche") / config["slug"] / "alignments/structure" / config["pair_slug"] / "hierarchical-numbered-unit-label-correspondence.json"
        payload = json.loads((REPO_ROOT / pair_path).read_text(encoding="utf-8"))
        payload["pairings"][0]["human_review_performed"] = True
        self.assertTrue(list(schema_validator.iter_errors(payload)))


if __name__ == "__main__":
    unittest.main()
