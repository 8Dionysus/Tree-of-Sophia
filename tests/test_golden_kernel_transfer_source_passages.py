from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import validate_golden_kernel_transfer_source_passages as validator


class GoldenKernelTransferSourcePassageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.payload = json.loads(
            (REPO_ROOT / validator.OUTPUT_PATH).read_text(encoding="utf-8")
        )
        cls.schema = json.loads(
            (REPO_ROOT / validator.SCHEMA_PATH).read_text(encoding="utf-8")
        )

    def test_release_safe_tracked_closure_passes(self) -> None:
        with patch.object(validator, "_validate_local_content", side_effect=AssertionError("no implicit private root")):
            self.assertEqual(0, validator.main(["--repo-root", str(REPO_ROOT)]))

    def private_git_fixture(self) -> tuple[Path, Path, dict]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        subprocess.run(["git", "init", "-q", str(root)], check=True, capture_output=True)
        subprocess.run(["git", "config", "core.excludesFile", "/dev/null"],
                       cwd=root, check=True, capture_output=True)
        local_ref = validator.GOLD_ROOT / "local-content"
        local = root / local_ref
        candidate = local / "transfer-source-passages/v1/synthetic-candidate.json"
        candidate.parent.mkdir(parents=True)
        candidate.write_text('{"synthetic": "No source witness content."}\n', encoding="utf-8")
        candidate.chmod(0o600)
        (local / "README.md").write_text("Synthetic private custody route.\n", encoding="utf-8")
        (root / ".gitignore").write_text(
            f"/{local_ref.as_posix()}/*\n!/{local_ref.as_posix()}/README.md\n", encoding="utf-8")
        subprocess.run(["git", "add", ".gitignore", (local_ref / "README.md").as_posix()],
                       cwd=root, check=True, capture_output=True)
        payload = {"passage_candidates": [
            {"private_content_ref": candidate.relative_to(root).as_posix()},
            {"private_content_ref": None},
        ]}
        return root, candidate, payload

    def test_ignored_private_candidate_may_exist_without_being_read(self) -> None:
        root, candidate, payload = self.private_git_fixture()
        self.assertTrue(candidate.is_file())
        with (patch.object(Path, "open", side_effect=AssertionError("no private bytes in Git boundary check")),
              patch.object(validator, "_read_json", side_effect=AssertionError("no private JSON read")),
              patch.object(validator, "_sha256_path", side_effect=AssertionError("no private digest read"))):
            validator._validate_no_tracked_private_content(root, payload)

    def test_force_tracked_private_candidate_is_rejected(self) -> None:
        root, candidate, payload = self.private_git_fixture()
        subprocess.run(["git", "add", "-f", candidate.relative_to(root).as_posix()],
                       cwd=root, check=True, capture_output=True)
        with self.assertRaisesRegex(validator.ValidationFailure, "private local content entered Git"):
            validator._validate_no_tracked_private_content(root, payload)

    def test_removed_private_ignore_rule_is_rejected(self) -> None:
        root, _candidate, payload = self.private_git_fixture()
        (root / ".gitignore").write_text("", encoding="utf-8")
        with self.assertRaisesRegex(validator.ValidationFailure, "private content ref is not ignored"):
            validator._validate_no_tracked_private_content(root, payload)

    def test_exact_counts_layers_and_unresolved_boundaries_are_preserved(self) -> None:
        self.assertEqual(validator.EXPECTED_SUMMARY, self.payload["summary"])
        actual_unresolved = {
            (candidate["work_ref"], candidate["qualified_unit_key"]): tuple(
                candidate["unresolved_boundaries"]
            )
            for candidate in self.payload["passage_candidates"]
            if candidate["status"] == "boundary-unresolved"
        }
        self.assertEqual(validator.EXPECTED_UNRESOLVED, actual_unresolved)
        actual_layers: dict[str, int] = {}
        for candidate in self.payload["passage_candidates"]:
            if candidate["status"] != "materialized-layer-exact-candidate":
                continue
            layer = candidate["boundary_layer"]
            actual_layers[layer] = actual_layers.get(layer, 0) + 1
        self.assertEqual(validator.EXPECTED_LAYER_COUNTS, actual_layers)

    def test_contract_rejects_false_acceptance_alignment_or_gold(self) -> None:
        schema_validator = Draft202012Validator(self.schema)
        for mutation in ("accepted", "alignment", "gold"):
            with self.subTest(mutation=mutation):
                payload = copy.deepcopy(self.payload)
                if mutation == "accepted":
                    payload["passage_candidates"][0]["accepted_source_text"] = True
                elif mutation == "alignment":
                    payload["effects"]["source_to_target_passage_alignment_created"] = (
                        True
                    )
                else:
                    payload["summary"]["target_gold_count"] = 1
                self.assertTrue(list(schema_validator.iter_errors(payload)))

    def test_antichrist_navigation_layer_does_not_assert_textual_identity(self) -> None:
        candidates = [
            candidate
            for candidate in self.payload["passage_candidates"]
            if candidate["work_ref"].endswith(".der-antichrist")
            and candidate["status"] == "materialized-layer-exact-candidate"
        ]
        self.assertTrue(candidates)
        for candidate in candidates:
            self.assertEqual(
                "bounded-source-visible-two-page-offset-only-no-textual-identity",
                candidate["navigation_relation"],
            )
            self.assertNotEqual(
                candidate["content_witness"]["item_ref"],
                candidate["address_witness"]["item_ref"],
            )
            self.assertEqual(
                [page - 2 for page in candidate["navigation_page_span"]],
                candidate["address_page_span"],
            )

    def test_jp2_marker_returns_are_bounded_and_text_free(self) -> None:
        composite = [
            candidate
            for candidate in self.payload["passage_candidates"]
            if candidate["boundary_layer"] == "jp2-visible-marker-plus-djvu-xml-line"
        ]
        self.assertEqual(
            {"main:8", "main:9", "main:43", "main:44"},
            {candidate["qualified_unit_key"] for candidate in composite},
        )
        for candidate in composite:
            evidence = candidate["boundary_evidence"]
            self.assertEqual(
                "same-Item-scandata-leaf-to-jp2-member-and-djvu-xml-object-order",
                evidence["relation"],
            )
            for marker in evidence["marker_returns"]:
                self.assertEqual(
                    marker["leaf_number"] + 1,
                    marker["navigation_page"],
                )
                self.assertFalse(marker["human_review_performed"])
                self.assertNotIn("text", marker)

    def test_validator_rejects_jp2_marker_coordinate_drift(self) -> None:
        payload = copy.deepcopy(self.payload)
        candidate = next(
            candidate
            for candidate in payload["passage_candidates"]
            if candidate["boundary_layer"] == "jp2-visible-marker-plus-djvu-xml-line"
        )
        candidate["boundary_evidence"]["marker_returns"][0]["pixel_bbox"]["x"] += 1
        target_payload = json.loads(
            (REPO_ROOT / validator.TARGET_PATH).read_text(encoding="utf-8")
        )
        with self.assertRaisesRegex(
            validator.ValidationFailure, "exact JP2 marker return set drifted"
        ):
            validator._validate_candidates(payload, target_payload)

    def test_pdf_marker_returns_are_bounded_and_text_free(self) -> None:
        composite = [
            candidate
            for candidate in self.payload["passage_candidates"]
            if candidate["boundary_layer"]
            == "pdf-visible-marker-plus-poppler-pdf-bbox-line"
        ]
        self.assertEqual(
            {"32", "essay-1:10"},
            {candidate["qualified_unit_key"] for candidate in composite},
        )
        for candidate in composite:
            evidence = candidate["boundary_evidence"]
            self.assertEqual(candidate["content_witness"], evidence["pdf_witness"])
            self.assertEqual(
                "same-PDF-page-image-mask-to-first-following-poppler-bbox-order",
                evidence["relation"],
            )
            for marker in evidence["marker_returns"]:
                self.assertEqual(
                    f"pdf-page-{marker['pdf_page']:04d}",
                    marker["pdf_resource_id"],
                )
                self.assertFalse(marker["human_review_performed"])
                self.assertNotIn("text", marker)

    def test_validator_rejects_pdf_marker_coordinate_drift(self) -> None:
        payload = copy.deepcopy(self.payload)
        candidate = next(
            candidate
            for candidate in payload["passage_candidates"]
            if candidate["boundary_layer"]
            == "pdf-visible-marker-plus-poppler-pdf-bbox-line"
        )
        candidate["boundary_evidence"]["marker_returns"][0]["pixel_bbox"]["x"] += 1
        target_payload = json.loads(
            (REPO_ROOT / validator.TARGET_PATH).read_text(encoding="utf-8")
        )
        with self.assertRaisesRegex(
            validator.ValidationFailure, "PDF marker boundary evidence drifted"
        ):
            validator._validate_candidates(payload, target_payload)

    def test_tracked_manifest_contains_no_private_source_text(self) -> None:
        rendered = json.dumps(self.payload, ensure_ascii=False)
        for forbidden in (
            '"automatic_candidate_text"',
            '"records"',
            '"words"',
        ):
            self.assertNotIn(forbidden, rendered)
        for candidate in self.payload["passage_candidates"]:
            private_ref = candidate["private_content_ref"]
            if candidate["status"] == "boundary-unresolved":
                self.assertIsNone(private_ref)
                continue
            self.assertIn("/local-content/transfer-source-passages/v1/", private_ref)


if __name__ == "__main__":
    unittest.main()
