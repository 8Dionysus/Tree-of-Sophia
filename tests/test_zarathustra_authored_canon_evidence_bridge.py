#!/usr/bin/env python3
"""Protect the authored Zarathustra route to exact-source evidence bridge."""

from __future__ import annotations

import copy
import json
import stat
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

from jsonschema import FormatChecker
from jsonschema.validators import validator_for


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
OWNER_ROOT = Path("/srv/AbyssOS/Tree-of-Sophia")
sys.path.insert(0, str(SCRIPTS_ROOT))

import build_zarathustra_authored_canon_evidence_bridge as builder  # noqa: E402
import validate_source_witness_foundation as foundation  # noqa: E402


PLAN_PATH = REPO_ROOT / foundation.ZARATHUSTRA_AUTHORED_CANON_EVIDENCE_BRIDGE_PLAN


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


class ZarathustraAuthoredCanonEvidenceBridgeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = _load(PLAN_PATH)
        cls.outputs = cls.plan["outputs"]
        cls.bridge_path = REPO_ROOT / cls.outputs["bridge_ref"]
        cls.unit_path = REPO_ROOT / cls.outputs["unit_packet_ref"]
        cls.event_path = REPO_ROOT / cls.outputs["provenance_event_ref"]
        cls.bridge = _load(cls.bridge_path)
        cls.unit = _load(cls.unit_path)
        cls.event = _load(cls.event_path)
        bridge_schema = _load(
            REPO_ROOT / foundation.AUTHORED_ROUTE_EVIDENCE_BRIDGE_V1_SCHEMA
        )
        bridge_type = validator_for(bridge_schema)
        bridge_type.check_schema(bridge_schema)
        cls.bridge_validator = bridge_type(
            bridge_schema, format_checker=FormatChecker()
        )

    def _validate_with_override(
        self, ref: str | Path, payload: dict
    ) -> list[tuple[str, str]]:
        original = foundation._load_json
        target = (REPO_ROOT / ref).resolve()

        def substitute(path: Path, repo_root: Path, issues: list[tuple[str, str]]):
            if path.resolve() == target:
                return copy.deepcopy(payload)
            return original(path, repo_root, issues)

        with mock.patch.object(foundation, "_load_json", side_effect=substitute):
            return foundation.validate_zarathustra_authored_canon_evidence_bridge(
                REPO_ROOT
            )

    def assert_issue_contains(
        self, issues: list[tuple[str, str]], fragment: str
    ) -> None:
        self.assertTrue(
            any(fragment in message for _, message in issues),
            f"missing issue fragment {fragment!r}: {issues}",
        )

    def test_current_schema_and_foundation_closure(self) -> None:
        self.assertEqual(list(self.bridge_validator.iter_errors(self.bridge)), [])
        self.assertEqual(
            foundation.validate_zarathustra_authored_canon_evidence_bridge(
                REPO_ROOT
            ),
            [],
        )

    def test_twelve_segment_order_match_and_digest_drift_are_rejected(self) -> None:
        mutated = copy.deepcopy(self.bridge)
        mutated["segment_crosswalk"][0]["legacy_segment_id"] = "seg.1.1.1.2"
        mutated["segment_crosswalk"][1]["dta_normalized_sha256"] = "0" * 64
        issues = self._validate_with_override(self.outputs["bridge_ref"], mutated)
        self.assert_issue_contains(issues, "twelve-segment mechanical crosswalk drifted")

    def test_fabricated_human_attestation_and_claim_closure_are_rejected(self) -> None:
        mutated = copy.deepcopy(self.bridge)
        mutated["canon_inventory"]["machine_readable_human_attestation_count"] = 1
        mutated["canon_inventory"]["relations_with_modern_claim_refs"] = 1
        mutated["canon_inventory"]["relations_with_modern_evidence_refs"] = 1
        mutated["assurance"]["claim_evidence_closure"] = True
        schema_messages = [
            error.message for error in self.bridge_validator.iter_errors(mutated)
        ]
        self.assertTrue(schema_messages)
        issues = self._validate_with_override(self.outputs["bridge_ref"], mutated)
        self.assert_issue_contains(issues, "bridge authority widened")
        self.assert_issue_contains(issues, "modern closure drifted")

    def test_source_translation_semantic_graph_and_canon_promotion_are_rejected(
        self,
    ) -> None:
        mutated = copy.deepcopy(self.bridge)
        mutated["effects"].update(
            {
                "source_text_accepted": True,
                "german_competence_attested": True,
                "translation_accepted": True,
                "sign_or_concept_promoted": True,
                "legacy_relation_migrated": True,
                "graph_projection_admitted": True,
                "canon_revised": True,
            }
        )
        issues = self._validate_with_override(self.outputs["bridge_ref"], mutated)
        self.assert_issue_contains(issues, "bridge authority widened")

    def test_publication_server_transfer_and_routine_human_task_are_rejected(
        self,
    ) -> None:
        mutated = copy.deepcopy(self.bridge)
        mutated["effects"]["human_task_created"] = True
        mutated["effects"]["new_publication_authorized"] = True
        mutated["effects"]["server_transfer_authorized"] = True
        issues = self._validate_with_override(self.outputs["bridge_ref"], mutated)
        self.assert_issue_contains(issues, "bridge authority widened")

    def test_legacy_roles_cannot_be_recast_as_modern_review(self) -> None:
        mutated = copy.deepcopy(self.bridge)
        mutated["witness_roles"][0]["current_assurance"] = (
            "legacy-authored-translation-not-modernly-reviewed"
        )
        mutated["witness_roles"][1]["modern_review_refs"] = [
            "tos.review.synthetic-fabrication"
        ]
        issues = self._validate_with_override(self.outputs["bridge_ref"], mutated)
        self.assert_issue_contains(issues, "authored witness role or digest drifted")

    def test_competing_source_and_authored_segmentations_cannot_be_collapsed(
        self,
    ) -> None:
        mutated = copy.deepcopy(self.unit)
        mutated["segmentations"][0]["competing_segmentation_refs"] = []
        mutated["segmentations"][1]["status"] = "accepted"
        mutated["segmentations"][1]["source_text_authority"] = True
        issues = self._validate_with_override(self.outputs["unit_packet_ref"], mutated)
        self.assert_issue_contains(issues, "competing segmentation posture widened")

    def test_authored_surface_fixity_drift_is_rejected(self) -> None:
        mutated = copy.deepcopy(self.bridge)
        mutated["authored_surfaces"]["source_node"]["sha256"] = "0" * 64
        mutated["authored_surfaces"]["relation_pack"]["sha256"] = "1" * 64
        issues = self._validate_with_override(self.outputs["bridge_ref"], mutated)
        self.assert_issue_contains(issues, "authored surface binding drifted")

    def test_node_and_relation_inventory_digest_drift_is_rejected(self) -> None:
        mutated = copy.deepcopy(self.bridge)
        mutated["canon_inventory"]["node_records_sha256"] = "0" * 64
        mutated["canon_inventory"]["relation_ids_sha256"] = "1" * 64
        mutated["canon_inventory"]["node_kind_counts"]["support"] -= 1
        issues = self._validate_with_override(self.outputs["bridge_ref"], mutated)
        self.assert_issue_contains(issues, "authored canon inventory or modern closure drifted")

    def test_provenance_cannot_claim_review_or_publication(self) -> None:
        mutated = copy.deepcopy(self.event)
        mutated["review_and_authority"].update(
            {
                "human_review_status": "performed",
                "accepted_uses": ["publication"],
                "promotion_authorized": True,
            }
        )
        mutated["rights_and_visibility"].update(
            {"content_visibility": "public_content", "publication_authorized": True}
        )
        issues = self._validate_with_override(self.outputs["provenance_event_ref"], mutated)
        self.assert_issue_contains(issues, "provenance closure or authority drifted")

    def test_provenance_requires_every_authored_and_review_input(self) -> None:
        mutated = copy.deepcopy(self.event)
        source_node_ref = self.plan["authored_surfaces"]["source_node_ref"]
        mutated["entities"]["inputs"] = [
            row
            for row in mutated["entities"]["inputs"]
            if row["entity_ref"] != source_node_ref
        ]
        issues = self._validate_with_override(self.outputs["provenance_event_ref"], mutated)
        self.assert_issue_contains(issues, "provenance closure or authority drifted")

    def test_plan_boundary_widening_is_rejected(self) -> None:
        mutated = copy.deepcopy(self.plan)
        mutated["authority_boundary"]["accepted_translation"] = True
        mutated["authority_boundary"]["human_task_created"] = True
        issues = self._validate_with_override(
            foundation.ZARATHUSTRA_AUTHORED_CANON_EVIDENCE_BRIDGE_PLAN,
            mutated,
        )
        self.assert_issue_contains(issues, "bridge plan authority widened")

    def test_tracked_records_do_not_expose_private_text_fields(self) -> None:
        forbidden = {
            "text",
            "source_text",
            "target_text",
            "diplomatic_transcription",
            "dta_normalized",
            "legacy_normalized",
            "operations",
            "opcodes",
        }
        tracked_refs = (
            "anchor_ref",
            "raw_layer_ref",
            "normalized_layer_ref",
            "unit_packet_ref",
            "bridge_ref",
            "provenance_event_ref",
        )
        for field in tracked_refs:
            payload = _load(REPO_ROOT / self.outputs[field])
            stack = [payload]
            while stack:
                value = stack.pop()
                if isinstance(value, dict):
                    self.assertFalse(forbidden & set(value), field)
                    stack.extend(value.values())
                elif isinstance(value, list):
                    stack.extend(value)

    def test_tracked_record_rejects_absolute_owner_local_path(self) -> None:
        mutated = copy.deepcopy(
            _load(REPO_ROOT / self.outputs["provenance_event_ref"])
        )
        mutated["reproducibility"]["known_gaps"][0] = "/srv/private-owner-root"
        issues = self._validate_with_override(
            self.outputs["provenance_event_ref"], mutated
        )
        self.assert_issue_contains(
            issues, "tracked bridge record exposes an absolute owner-local path"
        )

    @unittest.skipUnless(
        OWNER_ROOT.is_dir(),
        "owner-local exact source and private evidence store are unavailable",
    )
    def test_owner_local_replay_fixity_modes_and_text_nonleakage(self) -> None:
        completed = subprocess.run(
            [
                sys.executable,
                str(SCRIPTS_ROOT / "build_zarathustra_authored_canon_evidence_bridge.py"),
                "--check",
                "--repo-root",
                str(REPO_ROOT),
                "--local-input-root",
                str(OWNER_ROOT),
                "--local-output-root",
                str(OWNER_ROOT),
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        for field in (
            "private_raw_content_ref",
            "private_normalized_content_ref",
            "private_operations_ref",
            "private_comparison_ref",
        ):
            path = OWNER_ROOT / self.outputs[field]
            self.assertTrue(path.is_file())
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

        raw = (OWNER_ROOT / self.outputs["private_raw_content_ref"]).read_bytes()
        normalized = (
            OWNER_ROOT / self.outputs["private_normalized_content_ref"]
        ).read_bytes()
        tracked = b"\n".join(
            (REPO_ROOT / self.outputs[field]).read_bytes()
            for field in (
                "anchor_ref",
                "raw_layer_ref",
                "normalized_layer_ref",
                "unit_packet_ref",
                "bridge_ref",
                "provenance_event_ref",
            )
        ) + PLAN_PATH.read_bytes()
        self.assertNotIn(raw, tracked)
        self.assertNotIn(normalized, tracked)

    def test_builder_rejects_incomplete_or_reordered_complete_sequence(self) -> None:
        complete = "alpha beta gamma"
        self.assertEqual(
            builder._spans(["alpha", "beta", "gamma"], complete),
            ([(0, 5), (6, 10), (11, 16)], [(5, 6), (10, 11)]),
        )
        with self.assertRaises(builder.EvidenceBridgeError):
            builder._spans(["alpha", "gamma", "beta"], complete)
        with self.assertRaises(builder.EvidenceBridgeError):
            builder._spans(["alpha", "beta"], complete)


if __name__ == "__main__":
    unittest.main()
