from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from tos_corpus_index_common import (  # noqa: E402
    TOS_CORPUS_INDEX_PATH,
    build_payload,
    build_relations,
    render_payload,
    tracked_tos_paths,
    project_text_packet,
)
from partitioned_projection_common import (  # noqa: E402
    build_storage,
    check_partitioned_payload,
    disk_payload,
    is_partitioned,
    ProjectionReader,
)


class ToSCorpusIndexTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        super().setUpClass()
        cls._projection_storage_cm = None
        cls._projection_storage = None
        cls._real_projection_partitioned = is_partitioned(TOS_CORPUS_INDEX_PATH)
        if cls._real_projection_partitioned:
            cls._projection_storage_cm = build_storage()
            cls._projection_storage = cls._projection_storage_cm.__enter__()
            try:
                reader = ProjectionReader(TOS_CORPUS_INDEX_PATH)
                cls._real_payload = disk_payload(reader, cls._projection_storage)
                expected = build_payload(storage=cls._projection_storage)
                check_partitioned_payload(TOS_CORPUS_INDEX_PATH, expected)
                cls._real_parity_verified = True
            except BaseException:
                cls._projection_storage_cm.__exit__(*sys.exc_info())
                cls._projection_storage_cm = None
                cls._projection_storage = None
                raise
        else:
            cls._real_payload = json.loads(
                TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8")
            )
            cls._real_parity_verified = False

    @classmethod
    def tearDownClass(cls) -> None:
        try:
            if cls._projection_storage_cm is not None:
                cls._projection_storage_cm.__exit__(None, None, None)
        finally:
            cls._projection_storage_cm = None
            cls._projection_storage = None
            super().tearDownClass()

    def load_projection(self) -> dict[str, object]:
        return self._real_payload

    def test_text_packet_projection_is_versioned_and_visibility_bounded(self):
        path = REPO_ROOT / "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-a-source-layout-observation.json"
        packet = json.loads(path.read_text())
        nodes, edges = project_text_packet(packet, path.relative_to(REPO_ROOT).as_posix())
        self.assertTrue({"text-layer", "anchor", "text-unit", "annotation"}.issubset({n['node_kind'] for n in nodes}))
        self.assertTrue(any(e['predicate_id'] == 'has_text_layer' for e in edges))
        for node in nodes:
            self.assertIn('packet_version', node['properties'])
        packet['rights_and_visibility']['packet_visibility'] = 'restricted'
        self.assertEqual(project_text_packet(packet, 'ToS/private.json'), ([], []))

    def test_private_text_metadata_does_not_publish_word_hashes(self):
        path = REPO_ROOT / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-layout.v1.json"
        packet = json.loads(path.read_text())
        nodes, edges = project_text_packet(packet, path.relative_to(REPO_ROOT).as_posix())
        self.assertGreater(len(nodes), 0)
        rendered = json.dumps(nodes)
        self.assertNotIn('exact_sha256', rendered)
        self.assertTrue(all(n['properties']['content_available'] is False for n in nodes))

    def test_generated_index_matches_builder(self) -> None:
        if self._real_projection_partitioned:
            self.assertTrue(self._real_parity_verified)
        else:
            expected = render_payload(build_payload())
            current = TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8")
            self.assertEqual(current, expected)

    def test_index_keeps_runtime_projection_subordinate(self) -> None:
        payload = self.load_projection()
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertIn("runtime_projection", [entry["layer"] for entry in payload["authority_order"]])
        self.assertGreater(payload["counts"]["resources"], payload["counts"]["nodes"])

    def test_index_has_no_error_diagnostics(self) -> None:
        payload = self.load_projection()
        errors = [
            diagnostic
            for diagnostic in payload["diagnostics"]
            if diagnostic.get("level") == "error"
        ]
        self.assertEqual(errors, [])

    def test_authored_node_and_source_record_fields_survive_projection_losslessly(self) -> None:
        payload = self.load_projection()
        indexed_nodes = {node["node_id"]: node for node in payload["nodes"]}
        source_nodes = {
            node["node_id"]: node for node in payload["source_navigation"]["nodes"]
        }

        authored_path = REPO_ROOT / (
            "ToS/canon/synthesis/friedrich-nietzsche/thus-spoke-zarathustra/"
            "prologue-1/departure-from-reflective-origin/node.json"
        )
        authored = json.loads(authored_path.read_text(encoding="utf-8"))
        projected = indexed_nodes[authored["node_id"]]
        self.assertEqual(projected["properties"], authored)
        self.assertNotEqual(projected["label"], authored["source_anchor"])
        self.assertIn("departure", projected["label"].casefold())
        self.assertEqual(projected["properties"]["key_terms"], authored["key_terms"])
        self.assertEqual(projected["properties"]["relations"], authored["relations"])

        record_path = REPO_ROOT / (
            "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra/work.json"
        )
        record = json.loads(record_path.read_text(encoding="utf-8"))
        source_node = source_nodes[record["record_id"]]
        self.assertEqual(source_node["properties"]["source_record"], record)
        self.assertEqual(source_node["properties"]["record_version"], 10)
        self.assertEqual(
            source_node["properties"]["same_as_posture"],
            "no_equivalence_claim",
        )

    def test_source_navigation_joins_branch_work_item_file_and_links(self) -> None:
        payload = self.load_projection()
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
        self.assertIn(
            {
                "value": "Фридрих Ницше",
                "language": "ru",
                "source_ref": "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json",
                "status": "verified",
            },
            nodes["tos.agent.friedrich-nietzsche"]["properties"]["variant_labels"],
        )
        self.assertIn(
            "Так говорил Заратустра",
            {
                entry["value"]
                for entry in nodes["tos.work.friedrich-nietzsche.also-sprach-zarathustra"]["properties"]["variant_labels"]
            },
        )
        self.assertIn(
            (
                "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
                "authored_by",
                "tos.agent.friedrich-nietzsche",
            ),
            edges,
        )
        authorship = next(
            edge
            for edge in navigation["edges"]
            if edge["from_id"] == "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
            and edge["predicate_id"] == "authored_by"
            and edge["to_id"] == "tos.agent.friedrich-nietzsche"
        )
        self.assertEqual(authorship["review_status"], "unreviewed")
        self.assertTrue(authorship["claim_ref"].endswith("authored-by-friedrich-nietzsche"))
        self.assertIn(
            "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/responsibility-claims.jsonl",
            authorship["source_refs"],
        )
        exact_scan_rights = next(
            right
            for right in navigation["rights"]
            if right["rights_id"].endswith("layer.exact-nls-digital-scan")
        )
        self.assertEqual(exact_scan_rights["review_status"], "unreviewed")

    def test_index_resources_are_owned_by_the_tracked_source_view(self) -> None:
        payload = self.load_projection()
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
        payload = self.load_projection()
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

    def test_relation_edges_memory_and_disk_builds_share_declared_composite_order(self) -> None:
        csv_header = "edge_id,from_id,predicate_id,to_id\n"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = {
                "canon": root / "ToS/canon/example/edges.csv",
                "candidate": root / "ToS/candidate-intake/example/edges.csv",
            }
            paths["canon"].parent.mkdir(parents=True)
            paths["candidate"].parent.mkdir(parents=True)
            paths["canon"].write_text(
                csv_header
                + "m002,canon.from,p,canon.to\n"
                + "m001,canon.from,p,canon.to\n",
                encoding="utf-8",
            )
            paths["candidate"].write_text(
                csv_header
                + "m003,candidate.from,p,candidate.to\n",
                encoding="utf-8",
            )
            with patch("tos_corpus_index_common.REPO_ROOT", root):
                memory_packs, memory_edges = build_relations([], tuple(reversed(tuple(paths.values()))))
                with build_storage() as storage:
                    disk_packs, disk_edges = build_relations(
                        [], tuple(reversed(tuple(paths.values()))), storage=storage
                    )
                    self.assertEqual(memory_packs, list(disk_packs))
                    self.assertEqual(memory_edges, list(disk_edges))
            self.assertEqual(
                [(row["pack_id"], row["edge_id"]) for row in memory_edges],
                [
                    ("candidate-intake/example", "m003"),
                    ("canon/example", "m001"),
                    ("canon/example", "m002"),
                ],
            )


if __name__ == "__main__":
    unittest.main()
