from __future__ import annotations

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "validate_lived_witness_route.py"
SCHEMA_PATH = REPO_ROOT / "ToS" / "contracts" / "lived-witness-packet.schema.json"


def load_module() -> object:
    spec = importlib.util.spec_from_file_location("validate_lived_witness_route", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {SCRIPT_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


route = load_module()
schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
validator = Draft202012Validator(schema, format_checker=FormatChecker())


class LivedWitnessRouteTests(unittest.TestCase):
    def test_current_route_validates_without_claiming_human_truth(self) -> None:
        self.assertEqual([], route.run_validation(REPO_ROOT))

    def test_synthetic_draft_satisfies_contract(self) -> None:
        errors = list(validator.iter_errors(route.synthetic_draft_packet()))
        self.assertEqual([], errors)
        self.assertEqual(
            [], route.validate_packet_mechanics(route.synthetic_draft_packet(), schema)
        )

    def test_body_digest_must_match_exact_utf8_body(self) -> None:
        packet = deepcopy(route.synthetic_draft_packet())
        packet["testimony"]["body"] += " Changed after hashing."
        issues = route.validate_packet_mechanics(packet, schema)
        self.assertTrue(any("does not match the exact UTF-8 body" in issue for issue in issues))

    def test_ai_cannot_be_the_first_person_author(self) -> None:
        packet = deepcopy(route.synthetic_draft_packet())
        packet["author"]["agent_ref"] = "model:synthetic"
        packet["author"]["agent_kind"] = "model"
        errors = list(validator.iter_errors(packet))
        self.assertTrue(errors)

    def test_silent_transformation_is_rejected(self) -> None:
        packet = deepcopy(route.synthetic_draft_packet())
        packet["capture"]["silent_transformation_allowed"] = True
        errors = list(validator.iter_errors(packet))
        self.assertTrue(errors)

    def test_confirmed_packet_requires_exact_author_review(self) -> None:
        packet = deepcopy(route.synthetic_draft_packet())
        packet["record_status"] = "author-confirmed"
        errors = list(validator.iter_errors(packet))
        self.assertTrue(errors)

        digest = packet["testimony"]["body_sha256"]
        packet["author_review"] = {
            "author_confirmed": True,
            "confirmed_at": "2026-08-08T00:05:00Z",
            "confirmation_method": "explicit-text-confirmation",
            "reviewed_body_sha256": digest,
            "review_note": None,
        }
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual([], route.validate_packet_mechanics(packet, schema))

        packet["author_review"]["reviewed_body_sha256"] = "0" * 64
        self.assertEqual([], list(validator.iter_errors(packet)))
        issues = route.validate_packet_mechanics(packet, schema)
        self.assertTrue(any("does not bind the exact testimony body" in issue for issue in issues))

    def test_permissions_are_independent_required_decisions(self) -> None:
        packet = deepcopy(route.synthetic_draft_packet())
        del packet["use_permissions"]["model_training"]
        errors = list(validator.iter_errors(packet))
        self.assertTrue(errors)

    def test_cli_reports_structural_scope(self) -> None:
        completed = subprocess.run(
            (sys.executable, str(SCRIPT_PATH)),
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(0, completed.returncode, msg=completed.stdout + completed.stderr)
        self.assertIn("structure and private boundary only", completed.stdout)
        self.assertIn("remain unvalidated", completed.stdout)


if __name__ == "__main__":
    unittest.main()
