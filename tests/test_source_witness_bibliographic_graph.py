from __future__ import annotations

import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from source_witness_bibliographic_graph_common import (  # noqa: E402
    CLAIM_CATALOG_REF,
    GRAPH_PATH,
    BibliographicGraphBuildError,
    _load_claim_catalog,
    _projection_fingerprint,
    _validate_cross_references,
    build_payload,
    render_payload,
)


class SourceWitnessBibliographicGraphTest(unittest.TestCase):
    def load_projection(self) -> dict[str, object]:
        return json.loads(GRAPH_PATH.read_text(encoding="utf-8"))

    def test_generated_projection_matches_builder(self) -> None:
        self.assertEqual(
            GRAPH_PATH.read_text(encoding="utf-8"),
            render_payload(build_payload()),
        )

    def test_projection_is_claim_reified_and_complete(self) -> None:
        payload = self.load_projection()
        counts = payload["counts"]
        self.assertEqual(counts["source_claims"], 31)
        self.assertEqual(counts["claim_traces"], 31)
        self.assertEqual(counts["direct_subject_object_edges"], 0)
        self.assertFalse(payload["relation_model"]["direct_subject_object_edges"])
        self.assertEqual(payload["graph_layers"], ["bibliographic"])
        self.assertEqual(payload["review_counts"], {"unreviewed": 31})
        self.assertEqual(payload["visibility_counts"], {"public_metadata_only": 31})
        self.assertEqual(
            payload["projection_fingerprint"],
            _projection_fingerprint(payload),
        )

    def test_every_edge_returns_to_claim_evidence_maker_event_and_review(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        traces = {trace["claim_ref"]: trace for trace in payload["claim_traces"]}
        claim_nodes = {
            node["properties"]["claim_ref"]: node
            for node in payload["nodes"]
            if node["node_kind"] == "claim"
        }
        self.assertEqual(set(traces), set(claim_nodes))

        for edge in payload["edges"]:
            trace = traces[edge["claim_ref"]]
            self.assertEqual(edge["from_id"], trace["claim_node_id"])
            self.assertEqual(edge["claim_sha256"], trace["claim_sha256"])
            self.assertEqual(edge["evidence_node_ids"], trace["evidence_node_ids"])
            self.assertEqual(edge["maker_node_id"], trace["maker_node_id"])
            self.assertEqual(
                edge["provenance_event_node_id"],
                trace["provenance_event_node_id"],
            )
            self.assertEqual(edge["review_status"], trace["review_status"])
            self.assertIn(edge["to_id"], nodes)
            self.assertTrue(edge["evidence_node_ids"])
            self.assertTrue(
                all(nodes[node_id]["node_kind"] == "evidence" for node_id in edge["evidence_node_ids"])
            )

        for trace in traces.values():
            event = nodes[trace["provenance_event_node_id"]]
            maker = nodes[trace["maker_node_id"]]
            self.assertEqual(event["node_kind"], "provenance_event")
            self.assertTrue(event["properties"]["started_at"])
            self.assertTrue(event["properties"]["ended_at"])
            self.assertTrue(event["properties"]["method"]["name"])
            self.assertEqual(maker["node_kind"], "maker")
            self.assertTrue(maker["properties"]["agent_ref"])

    def test_claim_source_return_uses_independent_canonical_digest(self) -> None:
        payload = self.load_projection()
        for trace in payload["claim_traces"]:
            source_path = REPO_ROOT / trace["source_claim_file_ref"]
            raw_line = source_path.read_text(encoding="utf-8").splitlines()[
                trace["source_claim_line"] - 1
            ]
            source_claim = json.loads(raw_line)
            canonical = json.dumps(
                source_claim,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            )
            digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
            self.assertEqual(source_claim["claim_id"], trace["claim_ref"])
            self.assertEqual(digest, trace["source_claim_sha256"])
            self.assertEqual(digest, trace["claim_sha256"])

    def test_literal_objects_remain_literals_not_false_identities(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        traces = {trace["claim_ref"]: trace for trace in payload["claim_traces"]}
        issue_trace = traces[
            "tos.claim.edition.der-fall-wagner.naumann-1888.nominal-later-issue-state"
        ]
        issue_object = nodes[issue_trace["object_node_id"]]
        self.assertEqual(issue_object["node_kind"], "literal")
        self.assertEqual(
            issue_object["properties"]["value"]["textual_identity_status"],
            "unresolved",
        )
        self.assertEqual(
            issue_object["properties"]["value"]["textual_difference_status"],
            "unresolved",
        )
        direct_assertions = [
            edge
            for edge in payload["edges"]
            if nodes[edge["from_id"]]["node_kind"] != "claim"
        ]
        self.assertEqual(direct_assertions, [])

    def test_projection_contains_no_local_payload_route(self) -> None:
        payload = self.load_projection()
        serialized = json.dumps(payload, ensure_ascii=False)
        self.assertNotIn("/srv/", serialized)
        self.assertNotIn("/home/", serialized)
        for node in payload["nodes"]:
            self.assertNotIn("/payload/", node["source_ref"])

    def test_cross_reference_guard_rejects_direct_subject_object_edge(self) -> None:
        payload = build_payload()
        mutated = copy.deepcopy(payload)
        first_trace = mutated["claim_traces"][0]
        first_edge = next(
            edge
            for edge in mutated["edges"]
            if edge["claim_ref"] == first_trace["claim_ref"]
        )
        first_edge["from_id"] = first_trace["subject_node_id"]
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "every edge must start at its reified claim node",
        ):
            _validate_cross_references(mutated)

    def test_catalog_loader_rejects_nonpublic_claim(self) -> None:
        source_entry = json.loads(
            (REPO_ROOT / CLAIM_CATALOG_REF).read_text(encoding="utf-8").splitlines()[0]
        )
        source_entry["visibility"] = "local_only"
        with tempfile.TemporaryDirectory() as temporary:
            temp_root = Path(temporary)
            claim_path = temp_root / CLAIM_CATALOG_REF
            claim_path.parent.mkdir(parents=True)
            claim_path.write_text(
                json.dumps(source_entry, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                BibliographicGraphBuildError,
                "visibility is not safe",
            ):
                _load_claim_catalog(temp_root)


if __name__ == "__main__":
    unittest.main()
