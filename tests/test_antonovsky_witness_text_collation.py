from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from jsonschema import FormatChecker
from jsonschema.validators import validator_for


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import build_antonovsky_2007_1911_collation as builder  # noqa: E402
import validate_source_witness_foundation as foundation  # noqa: E402


OWNER_ROOT = Path("/srv/AbyssOS/Tree-of-Sophia")
ARTIFACT_ROOT = Path("/srv/abyss-machine/storage/artifacts")
PLAN_PATH = REPO_ROOT / foundation.ANTONOVSKY_2007_1911_COLLATION_PLAN


def _load(path: Path) -> dict:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise TypeError(f"expected object: {path}")
    return payload


class AntonovskyWitnessTextCollationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = _load(PLAN_PATH)
        cls.outputs = cls.plan["outputs"]
        cls.collation_path = REPO_ROOT / cls.outputs["collation_packet_ref"]
        cls.layer_path = REPO_ROOT / cls.outputs["source_text_layer_ref"]
        cls.unit_path = REPO_ROOT / cls.outputs["source_text_unit_packet_ref"]
        cls.event_path = REPO_ROOT / cls.outputs["provenance_event_ref"]
        cls.collation = _load(cls.collation_path)
        schema = _load(REPO_ROOT / foundation.WITNESS_TEXT_COLLATION_V1_SCHEMA)
        validator_type = validator_for(schema)
        validator_type.check_schema(schema)
        cls.schema_validator = validator_type(
            schema, format_checker=FormatChecker()
        )

    def _validate_with_override(self, ref: str, payload: dict) -> list[tuple[str, str]]:
        original = foundation._load_json
        target = (REPO_ROOT / ref).resolve()

        def substitute(path: Path, repo_root: Path, issues: list[tuple[str, str]]):
            if path.resolve() == target:
                return copy.deepcopy(payload)
            return original(path, repo_root, issues)

        with mock.patch.object(foundation, "_load_json", side_effect=substitute):
            return foundation.validate_antonovsky_2007_1911_collation(REPO_ROOT)

    def assert_issue_contains(self, issues: list[tuple[str, str]], fragment: str) -> None:
        self.assertTrue(
            any(fragment in message for _, message in issues),
            f"missing issue fragment {fragment!r}: {issues}",
        )

    def test_current_schema_and_release_safe_closure(self) -> None:
        self.assertEqual(list(self.schema_validator.iter_errors(self.collation)), [])
        self.assertEqual(
            foundation._witness_text_collation_v1_issues(self.collation), []
        )
        self.assertEqual(
            foundation.validate_antonovsky_2007_1911_collation(REPO_ROOT), []
        )

    def test_acceptance_without_review_is_rejected(self) -> None:
        mutated = copy.deepcopy(self.collation)
        mutated["collations"][0]["status"] = "accepted"
        schema_messages = [error.message for error in self.schema_validator.iter_errors(mutated)]
        semantic_messages = foundation._witness_text_collation_v1_issues(mutated)
        self.assertTrue(schema_messages)
        self.assertIn("decided collation lacks human review", semantic_messages)
        self.assertIn("non-human collation acceptance", semantic_messages)

    def test_fabricated_human_review_gold_and_source_acceptance_are_rejected(self) -> None:
        mutated = copy.deepcopy(self.plan)
        mutated["authority_boundary"]["human_review_performed"] = True
        mutated["authority_boundary"]["gold_created"] = True
        mutated["authority_boundary"]["source_text_accepted"] = True
        mutated["human_observation"]["source_review_created"] = True
        issues = self._validate_with_override(
            foundation.ANTONOVSKY_2007_1911_COLLATION_PLAN.as_posix(), mutated
        )
        self.assert_issue_contains(issues, "plan authority widened")
        self.assert_issue_contains(issues, "Workbench observation was promoted")

    def test_equivalence_derivation_translation_and_semantics_are_rejected(self) -> None:
        mutated = copy.deepcopy(self.collation)
        boundary = mutated["collations"][0]["interpretive_boundary"]
        boundary["textual_equivalence_established"] = True
        boundary["expression_derivation_established"] = True
        boundary["translation_relation_inferred"] = True
        boundary["semantic_relation_inferred"] = True
        self.assertIn(
            "collation interpretive boundary widened",
            foundation._witness_text_collation_v1_issues(mutated),
        )
        issues = self._validate_with_override(self.outputs["collation_packet_ref"], mutated)
        self.assert_issue_contains(issues, "non-promotion boundary drifted")

    def test_projection_of_proposed_collation_is_rejected(self) -> None:
        mutated = copy.deepcopy(self.collation)
        mutated["projections"] = [
            {
                "projection_id": "tos.witness-text-collation-projection.sid-0123456789abcdef0123456789abcdef",
                "projection_kind": "variant_graph",
                "artifact_ref": "ToS/derived-exports/synthetic-negative.json",
                "artifact_sha256": "0" * 64,
                "source_collation_refs": [mutated["collations"][0]["collation_id"]],
                "authority": "derived_non_authoritative_view",
                "visibility": "local_only",
                "publication_authorized": False,
            }
        ]
        self.assertEqual(list(self.schema_validator.iter_errors(mutated)), [])
        self.assertIn(
            "projection uses a collation that is not accepted",
            foundation._witness_text_collation_v1_issues(mutated),
        )

    def test_witness_selector_digest_and_reference_drift_are_rejected(self) -> None:
        mutated = copy.deepcopy(self.collation)
        witness = mutated["witnesses"][0]
        witness["selector"]["start"] += 1
        witness["exact_sha256"] = "0" * 64
        witness["text_unit_packet"]["ref"] = "ToS/invalid-unit.json"
        issues = self._validate_with_override(self.outputs["collation_packet_ref"], mutated)
        self.assert_issue_contains(issues, "collation witness binding drifted")

    def test_metric_and_private_detail_drift_are_rejected(self) -> None:
        mutated = copy.deepcopy(self.collation)
        mutated["collations"][0]["comparison_views"][0]["similarity_score_ppm"] += 1
        mutated["collations"][0]["comparison_views"][0]["private_edit_script_sha256"] = "0" * 64
        mutated["collations"][0]["private_detail"]["tracked"] = True
        mutated["collations"][0]["private_detail"]["visibility"] = "public"
        issues = self._validate_with_override(self.outputs["collation_packet_ref"], mutated)
        self.assert_issue_contains(issues, "metrics, or non-promotion boundary drifted")

    def test_rights_and_publication_widening_are_rejected(self) -> None:
        mutated = copy.deepcopy(self.collation)
        mutated["rights_and_visibility"]["publication_authorized"] = True
        mutated["rights_and_visibility"]["effective_visibility"] = "public"
        self.assertIn(
            "private-source collation authorizes publication",
            foundation._witness_text_collation_v1_issues(mutated),
        )
        issues = self._validate_with_override(self.outputs["collation_packet_ref"], mutated)
        self.assert_issue_contains(issues, "rights or visibility drifted")

    def test_tracked_records_have_no_private_content_fields(self) -> None:
        forbidden = {"diplomatic_transcription", "opcodes", "source_text", "target_text"}
        for path in (PLAN_PATH, self.collation_path, self.layer_path, self.unit_path, self.event_path):
            payload = _load(path)
            stack = [payload]
            while stack:
                value = stack.pop()
                if isinstance(value, dict):
                    self.assertFalse(forbidden & set(value), path.as_posix())
                    stack.extend(value.values())
                elif isinstance(value, list):
                    stack.extend(value)

    @unittest.skipUnless(
        (ARTIFACT_ROOT / "tree-of-sophia-foundation-lab/human-review/zarathustra-15-page-gold-pass-1-20260723/human-review-workbench.pass-1.autosave.json").is_file()
        and OWNER_ROOT.is_dir(),
        "owner-local observation and witness bytes are unavailable",
    )
    def test_owner_local_replay_guards_and_mode(self) -> None:
        completed = subprocess.run(
            [
                sys.executable,
                str(SCRIPTS_ROOT / "build_antonovsky_2007_1911_collation.py"),
                "--check",
                "--repo-root",
                str(REPO_ROOT),
                "--owner-root",
                str(OWNER_ROOT),
                "--artifact-root",
                str(ARTIFACT_ROOT),
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        for field in ("private_text_ref", "private_detail_ref"):
            path = OWNER_ROOT / self.outputs[field]
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)

        autosave_drift = copy.deepcopy(self.plan)
        autosave_drift["human_observation"]["autosave_sha256"] = "0" * 64
        with self.assertRaises(builder.AntonovskyCollationError):
            builder._read_human_observation(
                artifact_root=ARTIFACT_ROOT, plan=autosave_drift
            )

        text, _, _ = builder._read_human_observation(
            artifact_root=ARTIFACT_ROOT, plan=self.plan
        )
        wrong_boundary = copy.deepcopy(self.plan)
        wrong_boundary["witness_2007"]["sentence_start"] = wrong_boundary["witness_2007"]["heading_end"]
        with self.assertRaises(builder.AntonovskyCollationError):
            builder._select_2007_sentence(text, wrong_boundary)

        target_text_path = OWNER_ROOT / self.plan["witness_1911"]["text_layer_ref"]
        target_text = target_text_path.read_text(encoding="utf-8")
        source_sentence = text[
            self.plan["witness_2007"]["sentence_start"] : self.plan["witness_2007"]["sentence_end"]
        ]
        target_sentence = target_text[
            self.plan["witness_1911"]["sentence_start"] : self.plan["witness_1911"]["sentence_end"]
        ]
        metric_drift = copy.deepcopy(self.plan)
        metric_drift["comparison_method"]["views"][0]["opcode_count"] += 1
        with self.assertRaises(builder.AntonovskyCollationError):
            builder._comparison_views(source_sentence, target_sentence, metric_drift)

        tracked_bytes = b"\n".join(
            (REPO_ROOT / self.outputs[field]).read_bytes()
            for field in (
                "source_anchor_ref",
                "source_text_layer_ref",
                "source_text_unit_packet_ref",
                "collation_packet_ref",
                "provenance_event_ref",
            )
        ) + PLAN_PATH.read_bytes()
        self.assertNotIn(text.encode("utf-8"), tracked_bytes)
        self.assertNotIn(source_sentence.encode("utf-8"), tracked_bytes)
        self.assertNotIn(target_sentence.encode("utf-8"), tracked_bytes)

    @unittest.skipUnless(
        (ARTIFACT_ROOT / "tree-of-sophia-foundation-lab/human-review/zarathustra-15-page-gold-pass-1-20260723/human-review-workbench.sparse-calibration-closure/closure.json").is_file(),
        "owner-local closure is unavailable",
    )
    def test_attestation_and_promotion_flip_are_rejected(self) -> None:
        observation = self.plan["human_observation"]
        with tempfile.TemporaryDirectory() as directory:
            artifact_root = Path(directory)
            for field in (
                "autosave_relative_path",
                "closure_relative_path",
                "closure_receipt_relative_path",
            ):
                source = ARTIFACT_ROOT / observation[field]
                destination = artifact_root / observation[field]
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source.read_bytes())

            for field, value in (
                ("attestation_status", "attested"),
                ("promotion_authorized", True),
            ):
                mutated_plan = copy.deepcopy(self.plan)
                closure_path = artifact_root / observation["closure_relative_path"]
                closure = _load(closure_path)
                closure[field] = value
                closure_bytes = (
                    json.dumps(closure, ensure_ascii=False, indent=2, sort_keys=True)
                    + "\n"
                ).encode("utf-8")
                closure_path.write_bytes(closure_bytes)
                mutated_plan["human_observation"]["closure_sha256"] = hashlib.sha256(
                    closure_bytes
                ).hexdigest()
                with self.assertRaises(builder.AntonovskyCollationError):
                    builder._read_human_observation(
                        artifact_root=artifact_root, plan=mutated_plan
                    )
                # Restore the frozen input before exercising the second mutation.
                closure_path.write_bytes(
                    (ARTIFACT_ROOT / observation["closure_relative_path"]).read_bytes()
                )


if __name__ == "__main__":
    unittest.main()
