from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquire_registry_sources as acquisition


class RegistrySourceAcquisitionTests(unittest.TestCase):
    def test_repository_paths_reject_escape_absolute_and_symlink_routes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for ref in ("../outside", "/outside", "a/../outside", "a\\outside", "a//b", "a/./b", "a\0b"):
                with self.subTest(ref=ref), self.assertRaises(ValueError):
                    acquisition.safe_path(root, ref)
            (root / "link").symlink_to(root.parent, target_is_directory=True)
            with self.assertRaises(ValueError):
                acquisition.safe_path(root, "link/outside")
            self.assertEqual(acquisition.safe_path(root, "ToS/source-witnesses/work.json"), root / "ToS/source-witnesses/work.json")

    def test_payload_fixity_checks_size_and_git_blob_identity(self) -> None:
        body = b"exact\r\n"
        entry = {"basename": "source.txt", "byte_size": len(body),
                 "git_blob_sha1": hashlib.sha1(b"blob 7\0" + body).hexdigest()}
        self.assertEqual(acquisition.check_file(body, entry), hashlib.sha256(body).hexdigest())
        for altered in (b"exact\n", b"other\r\n"):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                acquisition.check_file(altered, entry)

    def test_json_rejects_duplicate_keys_and_nonstandard_numbers(self) -> None:
        for body in (b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}', b'{"x":-Infinity}'):
            with self.subTest(body=body), self.assertRaises(ValueError):
                acquisition.strict_json(body)
        decomposed = "e\u0301"
        self.assertEqual(acquisition.strict_json(json.dumps({"x": decomposed}).encode()), {"x": decomposed})

    def test_preparation_receipt_requires_the_exact_digest_commit_and_checks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "manifest.json"
            manifest.write_text('{"version":1}\n')
            receipt_path = root / "checkpoint.json"
            receipt = {"manifest_sha256": acquisition.sha256(manifest.read_bytes()), "commit": "a" * 40,
                "runtime_session_id": "test-session", "checkpoint_review_ref": "review:test",
                "passed_checks": ["prepared source closure"], "status": "passed"}
            with patch.object(acquisition.subprocess, "check_output", return_value="a" * 40 + "\n"):
                receipt_path.write_text(json.dumps(receipt))
                acquisition.check_preparation_receipt(root, manifest, receipt_path)
                for field, value in (("manifest_sha256", "b" * 64), ("commit", "b" * 40),
                                     ("runtime_session_id", ""), ("checkpoint_review_ref", ""),
                                     ("passed_checks", []), ("passed_checks", [True]), ("status", "pending")):
                    altered = {**receipt, field: value}
                    receipt_path.write_text(json.dumps(altered))
                    with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                        acquisition.check_preparation_receipt(root, manifest, receipt_path)

    def test_pinned_manifest_rejects_an_unbound_payload_url(self) -> None:
        manifest_path = ROOT / "ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/manifest.json"
        manifest = json.loads(manifest_path.read_bytes())
        manifest["targets"][0]["files"][0]["url"] = "https://example.invalid/unbound-source.xml"
        with tempfile.TemporaryDirectory() as directory:
            changed = Path(directory) / "manifest.json"
            changed.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "unbound source URL"):
                acquisition.load_preparation(ROOT, changed)

    def test_osis_validates_real_book_addresses_and_retains_source_characters(self) -> None:
        target = {"slug": "example", "coverage": {"kind": "osis-book", "book": "Prov", "chapter_count": 1}}
        body = ('<osis xmlns="http://www.bibletechnologies.net/2003/OSIS/namespace">'
                '<chapter osisID="Prov.1"><verse osisID="Prov.1.1"><w>א\u05b7</w></verse></chapter></osis>').encode()
        report = acquisition.inspect_payloads(target, [({"basename": "Prov.xml"}, body)])
        self.assertFalse(report["source_bytes_changed"])
        self.assertEqual(report["files"][0]["chapter_ids"], ["Prov.1"])
        self.assertEqual(report["files"][0]["word_count"], 1)
        for altered in (body.replace(b"Prov.1.1", b"Job.1.1"), body.replace(b"Prov.1\"", b"Prov.2\"")):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, [({"basename": "Prov.xml"}, altered)])

    def test_bilara_refuses_wrong_work_and_missing_required_unit(self) -> None:
        target = {"slug": "khuddakapatha", "coverage": {"kind": "bilara-root", "uid": "kp", "file_count": 2}}
        bodies = [({"basename": f"kp{number}_root-pli-ms.json"}, json.dumps({f"kp{number}:1": "source"}).encode()) for number in (1, 2)]
        self.assertEqual(acquisition.inspect_payloads(target, bodies)["unique_segment_count"], 2)
        for altered in (bodies[:1], [bodies[0], ({"basename": "kp2_root-pli-ms.json"}, b'{"mn9:1":"source"}')]):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, altered)

    def test_oraec_components_cannot_be_flattened_into_one_source_language(self) -> None:
        target = {"slug": "egyptian-example", "coverage": {"kind": "oraec-composition"},
                  "ids": {"expression": "tos.expression.example.egy", "translation_expression": "tos.expression.example.de"}}
        parsed = {"sentences": [{"translation": "German translation", "words": [{"written_form": "Egyptian form"}]}]}
        body = json.dumps(parsed).encode()
        result = acquisition.inspect_payloads(target, [({"basename": "source.json"}, body)])
        components = result["component_witnesses"]
        self.assertEqual([component["language"] for component in components], ["egy", "de"])
        self.assertEqual(len({component["file_id"] for component in components}), 1)
        self.assertEqual(components[0]["selectors"][0]["json_pointer_pattern"], "/sentences/*/words/*/written_form")
        self.assertNotIn("German translation", json.dumps(result))
        without_translation = copy.deepcopy(parsed)
        del without_translation["sentences"][0]["translation"]
        with self.assertRaisesRegex(ValueError, "separate German"):
            acquisition.inspect_payloads(target, [({"basename": "source.json"}, json.dumps(without_translation).encode())])


if __name__ == "__main__":
    unittest.main()
