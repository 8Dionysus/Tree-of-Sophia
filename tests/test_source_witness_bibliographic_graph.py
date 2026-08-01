from __future__ import annotations

import copy
import hashlib
import json
import subprocess
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
    load_verified_projection,
    query_projection,
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
        self.assertEqual(counts["source_claims"], 101)
        self.assertEqual(counts["claim_traces"], 101)
        self.assertEqual(counts["nodes"], 312)
        self.assertEqual(counts["edges"], 645)
        self.assertEqual(counts["direct_subject_object_edges"], 0)
        self.assertFalse(payload["relation_model"]["direct_subject_object_edges"])
        self.assertEqual(payload["graph_layers"], ["bibliographic"])
        self.assertEqual(payload["review_counts"], {"unreviewed": 101})
        self.assertEqual(payload["visibility_counts"], {"public_metadata_only": 101})
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

    def test_cross_reference_guard_closes_normalized_provision_routes(self) -> None:
        payload = build_payload()
        provision_trace = next(
            trace
            for trace in payload["claim_traces"]
            if trace["predicate"] == "provision_activity"
        )

        wrong_kind = copy.deepcopy(payload)
        place_edge = next(
            edge
            for edge in wrong_kind["edges"]
            if edge["claim_ref"] == provision_trace["claim_ref"]
            and edge["edge_kind"] == "has_normalized_place"
        )
        place_edge["to_id"] = provision_trace["subject_node_id"]
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "normalized place route must end at a Place identity",
        ):
            _validate_cross_references(wrong_kind)

        missing_trace_ref = copy.deepcopy(payload)
        mutated_trace = next(
            trace
            for trace in missing_trace_ref["claim_traces"]
            if trace["claim_ref"] == provision_trace["claim_ref"]
        )
        mutated_trace["normalized_identity_node_ids"] = []
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "normalized identity trace differs from normalized edges",
        ):
            _validate_cross_references(missing_trace_ref)

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

    def test_exact_claim_query_returns_complete_source_bundle(self) -> None:
        payload = load_verified_projection()
        claim_ref = (
            "tos.claim.edition.ecce-homo.insel-1908.edited-by-raoul-richter"
        )
        result = query_projection(payload, claim_ref=claim_ref)
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 1)
        match = result["matches"][0]
        self.assertEqual(match["claim_ref"], claim_ref)
        self.assertEqual(
            match["source_return"]["source_claim"]["claim_id"],
            claim_ref,
        )
        self.assertEqual(
            match["source_return"]["canonical_sha256"],
            match["claim_sha256"],
        )
        self.assertEqual(match["subject_node"]["node_kind"], "identity")
        self.assertEqual(match["object_node"]["node_kind"], "identity")
        self.assertTrue(match["evidence_nodes"])
        self.assertEqual(match["maker_node"]["node_kind"], "maker")
        self.assertEqual(
            match["provenance_event_node"]["node_kind"],
            "provenance_event",
        )
        self.assertEqual(match["review_nodes"], [])
        self.assertTrue(match["edges"])

    def test_query_uses_exact_and_semantics(self) -> None:
        payload = load_verified_projection()
        subject_ref = (
            "tos.collection.friedrich-nietzsche."
            "works-in-two-volumes-volume-2-mysl-1996"
        )
        result = query_projection(
            payload,
            subject_ref=subject_ref,
            predicate="contains_work",
        )
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 7)
        self.assertEqual(
            [match["claim_ref"] for match in result["matches"]],
            sorted(match["claim_ref"] for match in result["matches"]),
        )
        for match in result["matches"]:
            self.assertEqual(match["predicate"], "contains_work")
            self.assertEqual(
                match["subject_node"]["properties"]["identity_ref"],
                subject_ref,
            )

    def test_first_publication_chronology_remains_claim_scoped_literal(self) -> None:
        payload = load_verified_projection()
        result = query_projection(
            payload,
            predicate="first_publication_chronology",
        )
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 7)
        self.assertTrue(
            all(match["object_node"]["node_kind"] == "literal" for match in result["matches"])
        )
        self.assertTrue(
            all(
                match["object_node"]["properties"]["value"]["ordering_warning"]
                for match in result["matches"]
            )
        )
        self.assertTrue(
            all(
                match["source_return"]["file_ref"].endswith(
                    "/work-chronology-claims.jsonl"
                )
                for match in result["matches"]
            )
        )

    def test_provision_activity_query_preserves_literal_and_normalized_routes(
        self,
    ) -> None:
        payload = load_verified_projection()
        leipzig_ref = "tos.place.leipzig"
        result = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref=leipzig_ref,
        )
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 2)
        self.assertEqual(
            {
                "tos.organization.c-g-naumann-verlag-leipzig",
                "tos.organization.insel-verlag-anton-kippenberg-leipzig",
            },
            {
                node["properties"]["identity_ref"]
                for match in result["matches"]
                for node in match["normalized_identity_nodes"]
                if node["properties"]["identity_kind"] == "organization"
            },
        )
        for match in result["matches"]:
            self.assertEqual(match["object_node"]["node_kind"], "literal")
            self.assertEqual(
                match["object_node"]["properties"]["value"]["temporal"]["role"],
                "statement_date",
            )
            self.assertIn(
                "has_normalized_place",
                {edge["edge_kind"] for edge in match["edges"]},
            )
            self.assertTrue(
                all(
                    edge["from_id"] == match["claim_node"]["node_id"]
                    for edge in match["edges"]
                )
            )

        modern_successor = query_projection(
            payload,
            normalized_ref="tos.organization.insel-verlag-berlin",
        )
        self.assertEqual(modern_successor["status"], "no_match")
        self.assertEqual(modern_successor["matches"], [])

    def test_foundation_topology_queries_return_all_three_relation_families(self) -> None:
        payload = load_verified_projection()
        work_ref = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        work_result = query_projection(
            payload,
            subject_ref=work_ref,
            predicate="has_expression",
        )
        self.assertEqual(work_result["result_count"], 8)
        self.assertTrue(
            all(
                match["source_return"]["file_ref"].endswith(
                    "/work-expression-claims.jsonl"
                )
                for match in work_result["matches"]
            )
        )

        expression_ref = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-mysl-1996"
        )
        edition_ref = (
            "tos.edition.friedrich-nietzsche.works-in-two-volumes."
            "moscow-mysl-1996-volume-2"
        )
        embodiment_result = query_projection(
            payload,
            subject_ref=expression_ref,
            object_ref=edition_ref,
            predicate="embodied_by",
        )
        self.assertEqual(embodiment_result["result_count"], 1)
        embodiment = embodiment_result["matches"][0]
        self.assertEqual(
            embodiment["claim_node"]["properties"]["assertion_layer"],
            "bibliographic_assertion",
        )
        self.assertEqual(
            embodiment["claim_node"]["properties"]["review_status"],
            "unreviewed",
        )
        self.assertEqual(
            embodiment["source_return"]["source_claim"]["predicate"],
            "embodied_by",
        )

        edition_with_two_items = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "leipzig-c-g-naumann-1893"
        )
        exemplar_result = query_projection(
            payload,
            subject_ref=edition_with_two_items,
            predicate="exemplified_by",
        )
        self.assertEqual(exemplar_result["result_count"], 2)
        self.assertTrue(
            all(len(match["evidence_nodes"]) == 3 for match in exemplar_result["matches"])
        )

    def test_embodiment_topology_does_not_assert_textual_equivalence(self) -> None:
        payload = load_verified_projection()
        result = query_projection(payload, predicate="embodied_by", limit=20)
        self.assertEqual(result["result_count"], 20)
        for match in result["matches"]:
            source_claim = match["source_return"]["source_claim"]
            self.assertEqual(source_claim["claim_type"], "bibliographic")
            self.assertEqual(source_claim["epistemic_status"], "observed")
            self.assertNotIn("same_as", source_claim["predicate"])
            self.assertNotIn("textual", json.dumps(source_claim, ensure_ascii=False))

    def test_query_no_match_is_explicit_and_deterministic(self) -> None:
        payload = load_verified_projection()
        first = query_projection(payload, claim_ref="tos.claim.missing")
        second = query_projection(payload, claim_ref="tos.claim.missing")
        self.assertEqual(first, second)
        self.assertEqual(first["status"], "no_match")
        self.assertEqual(first["result_count"], 0)
        self.assertEqual(first["matches"], [])

    def test_query_requires_selector_and_rejects_silent_truncation(self) -> None:
        payload = load_verified_projection()
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "at least one exact query selector",
        ):
            query_projection(payload)
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "exceeding explicit limit 20",
        ):
            query_projection(payload, review_status="unreviewed")

    def test_verified_loader_rejects_projection_fingerprint_drift(self) -> None:
        payload = self.load_projection()
        payload["claim_traces"][0]["predicate"] = "tampered_predicate"
        with tempfile.TemporaryDirectory() as temporary:
            graph_path = Path(temporary) / "graph.json"
            graph_path.write_text(
                json.dumps(payload, ensure_ascii=False),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                BibliographicGraphBuildError,
                "projection fingerprint does not match",
            ):
                load_verified_projection(graph_path=graph_path)

    def test_query_cli_emits_json_and_rejects_unbounded_dump(self) -> None:
        script = REPO_ROOT / "scripts" / "query_source_witness_bibliographic_graph.py"
        claim_ref = (
            "tos.claim.edition.der-fall-wagner.naumann-1888."
            "nominal-later-issue-state"
        )
        completed = subprocess.run(
            [sys.executable, str(script), "--claim-ref", claim_ref],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result["result_count"], 1)
        self.assertEqual(result["matches"][0]["claim_ref"], claim_ref)
        self.assertNotIn("/srv/", completed.stdout)
        self.assertNotIn("/home/", completed.stdout)

        rejected = subprocess.run(
            [sys.executable, str(script)],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(rejected.returncode, 2)
        self.assertEqual(rejected.stdout, "")
        self.assertIn("at least one exact query selector", rejected.stderr)


if __name__ == "__main__":
    unittest.main()
