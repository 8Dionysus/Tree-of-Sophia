from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import validate_source_witness_foundation as foundation
from tos_corpus_index_common import project_source_item_file_memberships


class FileMembershipTests(unittest.TestCase):
    def test_identical_bytes_keep_one_file_identity_and_exact_item_pairs(self) -> None:
        payload = b"the same lawful source bytes\n"
        digest = hashlib.sha256(payload).hexdigest()
        file_id = f"tos.file.sha256.{digest}"
        index = foundation.SourceFileMembershipIndex()
        item_ids = ("tos.item.fixture.copy-a", "tos.item.fixture.copy-b")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            entries = []
            for item_id, directory_name in zip(item_ids, ("copy-a", "copy-b")):
                item_directory = root / directory_name
                relative_path = f"payload/{directory_name}.json"
                payload_path = item_directory / relative_path
                payload_path.parent.mkdir(parents=True)
                payload_path.write_bytes(payload)
                entry = {
                    "file_id": file_id,
                    "relative_path": relative_path,
                    "original_basename": f"{directory_name}.json",
                    "media_type": "application/json",
                    "byte_size": len(payload),
                    "sha256": digest,
                    "fixity_verified_at": f"2026-09-{22 if directory_name == 'copy-a' else 23:02d}T10:00:00Z",
                }
                entries.append(entry)
                self.assertEqual(
                    [],
                    foundation.validate_payload_file(
                        root,
                        item_directory,
                        entry,
                        require_local_payloads=True,
                    ),
                )
                self.assertEqual(
                    (),
                    index.add(
                        item_id=item_id,
                        file_id=file_id,
                        sha256=digest,
                        byte_size=len(payload),
                        media_type="application/json",
                    ),
                )

        self.assertTrue(index.contains(item_ids[0], file_id))
        self.assertTrue(index.contains(item_ids[1], file_id))
        self.assertFalse(index.contains("tos.item.fixture.unrelated", file_id))
        for item_id in item_ids:
            pair_anchor = {"item_id": item_id, "file_id": file_id, "file_sha256": digest}
            self.assertTrue(index.contains(pair_anchor["item_id"], pair_anchor["file_id"]))
            self.assertEqual(digest, index.sha256_for(pair_anchor["file_id"]))

        manifests = [
            (
                item_ids[0],
                "ToS/source-witnesses/fixture/copy-a/item.manifest.json",
                {
                    "acquisition_event_ref": "tos.event.acquisition.fixture.copy-a",
                    "rights_ref": "ToS/source-witnesses/fixture/copy-a/rights.json",
                    "payload_files": [entries[0]],
                },
            ),
            (
                item_ids[1],
                "ToS/source-witnesses/fixture/copy-b/item.manifest.json",
                {
                    "acquisition_event_ref": "tos.event.acquisition.fixture.copy-b",
                    "rights_ref": "ToS/source-witnesses/fixture/copy-b/rights.json",
                    "payload_files": [entries[1]],
                },
            ),
        ]
        nodes, edges, diagnostics = project_source_item_file_memberships(manifests)
        reversed_nodes, reversed_edges, reversed_diagnostics = project_source_item_file_memberships(
            list(reversed(manifests))
        )
        self.assertEqual([], diagnostics)
        self.assertEqual([], reversed_diagnostics)
        self.assertEqual(nodes, reversed_nodes)
        self.assertEqual(edges, reversed_edges)
        self.assertEqual(1, len(nodes))
        self.assertEqual(file_id, nodes[0]["node_id"])
        self.assertEqual(
            [manifest_ref for _, manifest_ref, _ in manifests],
            nodes[0]["source_refs"],
        )
        self.assertEqual(
            {"media_type": "application/json", "byte_size": len(payload), "sha256": digest},
            nodes[0]["properties"],
        )
        self.assertNotIn("rights_ref", nodes[0]["properties"])
        self.assertNotIn("original_basename", nodes[0]["properties"])
        self.assertNotIn("fixity_verified_at", nodes[0]["properties"])

        schema = json.loads(
            (REPO_ROOT / "ToS/contracts/tos-corpus-index.schema.json").read_text(
                encoding="utf-8"
            )
        )
        root_validator = Draft202012Validator(schema)
        root_validator.evolve(schema={"$ref": "#/$defs/sourceNavigationNode"}).validate(
            nodes[0]
        )

        self.assertEqual(set(item_ids), {edge["from_id"] for edge in edges})
        edge_by_item = {edge["from_id"]: edge for edge in edges}
        for item_id, manifest_ref, manifest in manifests:
            edge = edge_by_item[item_id]
            self.assertEqual(file_id, edge["to_id"])
            self.assertEqual([manifest_ref], edge["source_refs"])
            context = edge["properties"]["item_file_contexts"][0]
            self.assertEqual(manifest_ref, context["manifest_ref"])
            self.assertEqual(manifest["acquisition_event_ref"], context["acquisition_event_ref"])
            self.assertEqual(manifest["payload_files"][0]["original_basename"],
                             context["payload_entries"][0]["original_basename"])
            self.assertEqual(manifest["payload_files"][0]["fixity_verified_at"],
                             context["payload_entries"][0]["fixity_verified_at"])
            self.assertNotIn("rights_ref", context)
            root_validator.evolve(schema={"$ref": "#/$defs/sourceNavigationEdge"}).validate(
                edge
            )

    def test_same_file_id_rejects_digest_size_and_media_type_conflicts(self) -> None:
        first_bytes = b"immutable content"
        digest = hashlib.sha256(first_bytes).hexdigest()
        file_id = f"tos.file.sha256.{digest}"
        index = foundation.SourceFileMembershipIndex()
        self.assertEqual(
            (),
            index.add(
                item_id="tos.item.fixture.first",
                file_id=file_id,
                sha256=digest,
                byte_size=len(first_bytes),
                media_type="application/octet-stream",
            ),
        )
        self.assertIn(
            "sha256",
            index.add(
                item_id="tos.item.fixture.changed-digest",
                file_id=file_id,
                sha256=hashlib.sha256(b"different content").hexdigest(),
                byte_size=len(first_bytes),
                media_type="application/octet-stream",
            ),
        )
        self.assertIn(
            "byte_size",
            index.add(
                item_id="tos.item.fixture.changed-size",
                file_id=file_id,
                sha256=digest,
                byte_size=len(first_bytes) + 1,
                media_type="application/octet-stream",
            ),
        )
        self.assertIn(
            "media_type",
            index.add(
                item_id="tos.item.fixture.changed-media-type",
                file_id=file_id,
                sha256=digest,
                byte_size=len(first_bytes),
                media_type="text/plain",
            ),
        )
        self.assertEqual(digest, index.sha256_for(file_id))

    def test_projection_reports_conflicting_file_descriptor_without_membership(self) -> None:
        digest = hashlib.sha256(b"same bytes").hexdigest()
        file_id = f"tos.file.sha256.{digest}"
        manifests = [
            (
                "tos.item.fixture.first",
                "ToS/source-witnesses/fixture/first/item.manifest.json",
                {
                    "acquisition_event_ref": "tos.event.acquisition.fixture.first",
                    "payload_files": [{
                        "file_id": file_id,
                        "relative_path": "payload/source.bin",
                        "original_basename": "source.bin",
                        "media_type": "application/octet-stream",
                        "byte_size": 9,
                        "sha256": digest,
                        "fixity_verified_at": "2026-09-22T10:00:00Z",
                    }],
                },
            ),
            (
                "tos.item.fixture.second",
                "ToS/source-witnesses/fixture/second/item.manifest.json",
                {
                    "acquisition_event_ref": "tos.event.acquisition.fixture.second",
                    "payload_files": [{
                        "file_id": file_id,
                        "relative_path": "payload/source.bin",
                        "original_basename": "source.bin",
                        "media_type": "application/octet-stream",
                        "byte_size": 10,
                        "sha256": digest,
                        "fixity_verified_at": "2026-09-22T11:00:00Z",
                    }],
                },
            ),
        ]
        nodes, edges, diagnostics = project_source_item_file_memberships(manifests)
        reversed_nodes, reversed_edges, reversed_diagnostics = project_source_item_file_memberships(
            list(reversed(manifests))
        )
        self.assertEqual([], nodes)
        self.assertEqual([], edges)
        self.assertEqual(1, len(diagnostics))
        self.assertIn("conflicting byte_size", diagnostics[0]["message"])
        self.assertEqual(nodes, reversed_nodes)
        self.assertEqual(edges, reversed_edges)
        self.assertEqual(diagnostics, reversed_diagnostics)


if __name__ == "__main__":
    unittest.main()
