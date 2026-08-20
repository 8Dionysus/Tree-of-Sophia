from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import validate_golden_kernel_transfer_route_readiness as validator


class GoldenKernelTransferRouteReadinessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.payload = json.loads(
            (REPO_ROOT / validator.OUTPUT_PATH).read_text(encoding="utf-8")
        )
        cls.target_payload = json.loads(
            (REPO_ROOT / validator.TARGET_PATH).read_text(encoding="utf-8")
        )
        cls.source_payload = json.loads(
            (REPO_ROOT / validator.SOURCE_PATH).read_text(encoding="utf-8")
        )
        cls.schema = json.loads(
            (REPO_ROOT / validator.SCHEMA_PATH).read_text(encoding="utf-8")
        )
        cls.provenance_schema = json.loads(
            (REPO_ROOT / validator.PROVENANCE_SCHEMA_PATH).read_text(encoding="utf-8")
        )

    def test_release_safe_readiness_closure_passes(self) -> None:
        self.assertEqual(0, validator.main(["--repo-root", str(REPO_ROOT)]))

    def test_raw_candidate_statuses_recompute_projection_counts(self) -> None:
        target_by_id = {
            candidate["passage_candidate_id"]: candidate
            for candidate in self.target_payload["passage_candidates"]
        }
        source_by_target = {
            candidate["target_passage_candidate_id"]: candidate
            for candidate in self.source_payload["passage_candidates"]
        }
        self.assertEqual(set(target_by_id), set(source_by_target))
        self.assertEqual(
            35,
            sum(
                source["status"] == "materialized-layer-exact-candidate"
                for source in source_by_target.values()
            ),
        )
        intersecting = [
            target
            for target in target_by_id.values()
            if target["status"] == "proposed-intersecting-layer-exact"
        ]
        nonintersecting = [
            target
            for target in target_by_id.values()
            if target["status"] == "rejected-nonintersecting-layer-exact"
        ]
        self.assertEqual(32, len(intersecting))
        self.assertEqual(3, len(nonintersecting))
        self.assertEqual(
            20,
            len({target["frozen_page_candidate_id"] for target in intersecting}),
        )
        self.assertEqual(validator.EXPECTED_SUMMARY, self.payload["summary"])

    def test_contract_rejects_alignment_eligibility_and_gold(self) -> None:
        schema_validator = Draft202012Validator(self.schema)
        for field in (
            "source_to_target_passage_alignment",
            "eligible_for_variant_execution",
            "target_gold",
        ):
            with self.subTest(field=field):
                payload = copy.deepcopy(self.payload)
                payload["routes"][0][field] = True
                self.assertTrue(list(schema_validator.iter_errors(payload)))

    def test_projection_exposes_no_private_content_fields(self) -> None:
        rendered = json.dumps(self.payload, ensure_ascii=False)
        for forbidden in (
            '"text"',
            '"words"',
            '"records"',
            '"private_content_ref"',
            '"private_content_sha256"',
        ):
            self.assertNotIn(forbidden, rendered)

    def test_readiness_provenance_uses_owner_vocabulary(self) -> None:
        events = [
            json.loads(line)
            for line in (REPO_ROOT / validator.PROVENANCE_PATH)
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        event = next(
            event for event in events if event.get("event_id") == validator.EVENT_ID
        )
        self.assertEqual(
            [], list(Draft202012Validator(self.provenance_schema).iter_errors(event))
        )
        self.assertEqual("export", event["event_type"])
        self.assertEqual("software", event["method"]["maker_type"])

    def test_validator_rejects_frozen_page_intersection_drift(self) -> None:
        payload = copy.deepcopy(self.payload)
        payload["routes"][0]["frozen_page_intersection"] = False
        with self.assertRaisesRegex(
            validator.ReadinessValidationFailure, "derived route drifted"
        ):
            validator._validate_routes(
                payload, self.target_payload, self.source_payload
            )


if __name__ == "__main__":
    unittest.main()
