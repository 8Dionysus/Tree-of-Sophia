from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from tos_corpus_index_common import (  # noqa: E402
    TOS_CORPUS_INDEX_PATH,
    build_payload,
    render_payload,
    tracked_tos_paths,
)


class ToSCorpusIndexTest(unittest.TestCase):
    def test_generated_index_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_index_keeps_runtime_projection_subordinate(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertIn("runtime_projection", [entry["layer"] for entry in payload["authority_order"]])
        self.assertGreater(payload["counts"]["resources"], payload["counts"]["nodes"])

    def test_index_has_no_error_diagnostics(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        errors = [
            diagnostic
            for diagnostic in payload["diagnostics"]
            if diagnostic.get("level") == "error"
        ]
        self.assertEqual(errors, [])

    def test_source_navigation_joins_branch_work_item_file_and_links(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        navigation = payload["source_navigation"]
        nodes = {node["node_id"]: node for node in navigation["nodes"]}
        edges = {
            (edge["from_id"], edge["predicate_id"], edge["to_id"])
            for edge in navigation["edges"]
        }
        n16 = "tos.work.proto-cuneiform.n16-in-the-archaic-texts"
        self.assertIn(
            (
                "philosophy.eras.bronze-age.regions.west-asia.traditions.proto-cuneiform-accounting-ontologies",
                "has_source_planting",
                "tos.planting.a01.cdlb-2006-1-n16",
            ),
            edges,
        )
        self.assertIn(
            (n16, "downloadable_at", "tos.link.cdli.cdlb-2006-1.pdf"),
            edges,
        )
        item = (
            "tos.item.egyptian-scholarship.on-four-songs-contained-in-an-"
            "egyptian-papyrus-in-the-british-museum.en-goodwin-1874."
            "internet-archive-nls-scan-pdf"
        )
        file_id = "tos.file.sha256.71ca30507a61a791b503102b43c034aad08b45c5d78bbedf16043632add33675"
        self.assertIn((item, "has_file", file_id), edges)
        self.assertEqual(nodes[file_id]["properties"]["sha256"], file_id.removeprefix("tos.file.sha256."))
        self.assertEqual(nodes["tos.link.internet-archive.onfoursongsconta00good.pdf-download"]["properties"]["access_status"], "open_download")
        exact_scan_rights = next(
            right
            for right in navigation["rights"]
            if right["rights_id"].endswith("layer.exact-nls-digital-scan")
        )
        self.assertEqual(exact_scan_rights["review_status"], "unreviewed")

    def test_index_resources_are_owned_by_the_tracked_source_view(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        tracked_refs = {
            path.relative_to(REPO_ROOT).as_posix()
            for path in tracked_tos_paths()
        }
        resource_refs = {resource["path"] for resource in payload["resources"]}

        self.assertLessEqual(resource_refs, tracked_refs)
        self.assertFalse(
            any("payload" in Path(path_ref).parts for path_ref in resource_refs)
        )

    def test_authority_order_declares_all_emitted_layers(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        declared = {entry["layer"] for entry in payload["authority_order"]}
        emitted = set()
        for collection_name in (
            "branches",
            "manifests",
            "nodes",
            "relation_packs",
            "relation_edges",
            "resources",
        ):
            emitted.update(
                item["authority_layer"]
                for item in payload[collection_name]
                if "authority_layer" in item
            )
        self.assertEqual(sorted(emitted - declared), [])


if __name__ == "__main__":
    unittest.main()
