from __future__ import annotations

import json
from pathlib import Path
import unittest

from tos_access.source_navigation_query import source_dossier_query


FILE_ID = "tos.file.sha256.shared"
ITEM_A = "tos.item.copy.a"
ITEM_B = "tos.item.copy.b"
MANIFEST_A = "ToS/source-witnesses/fixture/copy-a/item.manifest.json"
MANIFEST_B = "ToS/source-witnesses/fixture/copy-b/item.manifest.json"
RIGHTS_A = "ToS/source-witnesses/fixture/copy-a/rights.json"
RIGHTS_B = "ToS/source-witnesses/fixture/copy-b/rights.json"
JENSEITS_MANIFEST = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/item.manifest.json"
JENSEITS_RIGHTS = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json"
JENSEITS_ITEM = "tos.item.friedrich-nietzsche.jenseits-von-gut-und-boese.de-naumann-1886.internet-archive-google-harvard-scan-pdf"
JENSEITS_OCR_FILE = "tos.file.sha256.ba8f4c91a317a3de03ab1f318860aaba6837d979e1ec99365e6d13def7db5a34"


def navigation_fixture(*, rights_b_positive: bool = False, legacy: bool = False):
    nodes = {
        FILE_ID: {"node_id": FILE_ID, "node_kind": "file"},
        ITEM_A: {"node_id": ITEM_A, "node_kind": "item"},
        ITEM_B: {"node_id": ITEM_B, "node_kind": "item"},
    }
    edges = [
        {
            "edge_id": f"{ITEM_A}:{FILE_ID}",
            "from_id": ITEM_A,
            "predicate_id": "has_file",
            "to_id": FILE_ID,
            "edge_kind": "authored_item_manifest",
            "source_refs": [MANIFEST_A],
            "properties": {"item_file_contexts": [{
                "manifest_ref": MANIFEST_A,
                "acquisition_event_ref": "tos.event.acquisition.copy.a",
                **({} if legacy else {"rights_ref": RIGHTS_A}),
                "payload_entries": [{"relative_path": "payload/a.bin", "original_basename": "a.bin", "fixity_verified_at": "2026-09-22T10:00:00Z", "container_member": False}],
            }]},
        },
        {
            "edge_id": f"{ITEM_B}:{FILE_ID}",
            "from_id": ITEM_B,
            "predicate_id": "has_file",
            "to_id": FILE_ID,
            "edge_kind": "authored_item_manifest",
            "source_refs": [MANIFEST_B],
            "properties": {"item_file_contexts": [{
                "manifest_ref": MANIFEST_B,
                "acquisition_event_ref": "tos.event.acquisition.copy.b",
                **({} if legacy else {"rights_ref": RIGHTS_B}),
                "payload_entries": [{"relative_path": "payload/b.bin", "original_basename": "b.bin", "fixity_verified_at": "2026-09-23T10:00:00Z", "container_member": False}],
            }]},
        },
    ]
    rights = [
        {"rights_id": "rights-a", "source_ref": RIGHTS_A, "scope_refs": [ITEM_A, FILE_ID], "assessment_status": "public_domain_reviewed", "redistribution_posture": "authorized", "review_status": "accepted"},
        {
            "rights_id": "rights-b",
            "source_ref": RIGHTS_B,
            "scope_refs": [ITEM_B, FILE_ID],
            "assessment_status": "licensed" if rights_b_positive else "copyright_undetermined",
            "redistribution_posture": "authorized" if rights_b_positive else "not_authorized",
            "review_status": "accepted" if rights_b_positive else "not_reviewed",
        },
    ]
    incoming = {FILE_ID: edges, ITEM_A: [], ITEM_B: []}
    outgoing = {ITEM_A: [edges[0]], ITEM_B: [edges[1]]}
    navigation = {"authority_boundary": "source-owned fixture navigation"}
    return navigation, nodes, incoming, outgoing, rights


def dossier(fixture, object_id: str, *, limit: int = 20):
    navigation, nodes, incoming, outgoing, rights = fixture
    return source_dossier_query(
        navigation,
        nodes,
        incoming,
        outgoing,
        lambda _ids: rights,
        object_id,
        limit=limit,
    )


class SharedFileRightsTests(unittest.TestCase):
    def test_shared_file_does_not_promote_one_items_rights_to_the_other(self) -> None:
        fixture = navigation_fixture()
        fixture[4].append({
            "rights_id": "rights-a-file-layer",
            "source_ref": RIGHTS_A,
            "scope_refs": [FILE_ID],
            "assessment_status": "copyright_undetermined",
            "redistribution_posture": "not_authorized",
            "review_status": "unreviewed",
        })
        file_dossier = dossier(fixture, FILE_ID)
        self.assertFalse(file_dossier["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual("membership_scoped_review_required", file_dossier["agent_summary"]["rights_posture"])
        self.assertEqual([FILE_ID, ITEM_A, ITEM_B], file_dossier["agent_summary"]["rights_scope_refs"])
        self.assertEqual({"rights-a", "rights-a-file-layer", "rights-b"}, {row["rights_id"] for row in file_dossier["rights"]})

        item_a = dossier(fixture, ITEM_A)
        self.assertTrue(item_a["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual(["rights-a"], [row["rights_id"] for row in item_a["rights"]])
        item_b = dossier(fixture, ITEM_B)
        self.assertFalse(item_b["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual(["rights-b"], [row["rights_id"] for row in item_b["rights"]])

    def test_file_only_candidate_from_exact_source_stays_review_required(self) -> None:
        fixture = navigation_fixture()
        navigation, nodes, incoming, outgoing, _rights = fixture
        incoming[FILE_ID] = incoming[FILE_ID][:1]
        outgoing = {ITEM_A: outgoing[ITEM_A]}
        nodes.pop(ITEM_B)
        rights = [{
            "rights_id": "rights-a-file-candidate",
            "source_ref": RIGHTS_A,
            "scope_refs": [FILE_ID],
            "assessment_status": "licensed",
            "redistribution_posture": "authorized",
            "review_status": "not_reviewed",
        }]
        result = source_dossier_query(navigation, nodes, incoming, outgoing, lambda _ids: rights, FILE_ID, limit=20)
        self.assertEqual(["rights-a-file-candidate"], [row["rights_id"] for row in result["rights"]])
        self.assertEqual("candidate_requires_human_review", result["agent_summary"]["rights_posture"])
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])

    def test_file_aggregate_requires_context_for_every_edge_source_ref(self) -> None:
        fixture = navigation_fixture(rights_b_positive=True)
        edge = fixture[2][FILE_ID][0]
        edge["source_refs"].append("ToS/source-witnesses/fixture/copy-a/superseded-item.manifest.json")
        result = dossier(fixture, FILE_ID)
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual("membership_scoped_review_required", result["agent_summary"]["rights_posture"])

    def test_file_aggregate_rejects_duplicate_manifest_contexts(self) -> None:
        fixture = navigation_fixture(rights_b_positive=True)
        edge = fixture[2][FILE_ID][0]
        contexts = edge["properties"]["item_file_contexts"]
        contexts.append(dict(contexts[0]))
        result = dossier(fixture, FILE_ID)
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual("membership_scoped_review_required", result["agent_summary"]["rights_posture"])

    def test_legacy_single_manifest_edge_without_context_keeps_unique_item_scoped_rights(self) -> None:
        fixture = navigation_fixture(legacy=True)
        navigation, nodes, incoming, outgoing, rights = fixture
        edge = incoming[FILE_ID][0]
        edge.pop("properties")
        incoming[FILE_ID] = [edge]
        outgoing = {ITEM_A: [edge]}
        nodes.pop(ITEM_B)
        rights = rights[:1]
        result = source_dossier_query(
            navigation, nodes, incoming, outgoing, lambda _ids: rights, FILE_ID, limit=20
        )
        self.assertTrue(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual(["rights-a"], [row["rights_id"] for row in result["rights"]])

    def test_actual_jenseits_ocr_layer_is_retained_for_its_exact_file_membership(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        manifest = json.loads((repo_root / JENSEITS_MANIFEST).read_text(encoding="utf-8"))
        rights_record = json.loads((repo_root / JENSEITS_RIGHTS).read_text(encoding="utf-8"))
        layer = next(
            entry for entry in rights_record["layer_assessments"]
            if entry.get("layer_id") == (
                "tos.rights.jenseits-naumann-1886.internet-archive-google-harvard-scan-pdf"
                ".layer.ocr-coordinate-xml"
            )
        )
        self.assertEqual("local_only", rights_record["visibility"])
        self.assertEqual(JENSEITS_RIGHTS, manifest["rights_ref"])
        self.assertIn(JENSEITS_OCR_FILE, layer["scope_refs"])
        self.assertNotIn(JENSEITS_ITEM, layer["scope_refs"])

        # Match the bounded fields emitted by source-navigation projection. The
        # local_only source remains excluded from public projection; this only
        # verifies exact-source Item/File consumer binding.
        projected_layer = {
            "rights_id": layer["layer_id"],
            "source_ref": manifest["rights_ref"],
            "scope_refs": layer["scope_refs"],
            "assessment_status": layer["assessment_status"],
            "redistribution_posture": layer["redistribution_posture"],
            "review_status": layer["review_status"],
        }
        edge = {
            "edge_id": f"{JENSEITS_ITEM}:{JENSEITS_OCR_FILE}",
            "from_id": JENSEITS_ITEM,
            "predicate_id": "has_file",
            "to_id": JENSEITS_OCR_FILE,
            "edge_kind": "authored_item_manifest",
            "source_refs": [JENSEITS_MANIFEST],
            "properties": {"item_file_contexts": [{
                "manifest_ref": JENSEITS_MANIFEST,
                "rights_ref": manifest["rights_ref"],
                "acquisition_event_ref": manifest["acquisition_event_ref"],
                "payload_entries": [],
            }]},
        }
        result = source_dossier_query(
            {"authority_boundary": "source-owned fixture navigation"},
            {
                JENSEITS_ITEM: {"node_id": JENSEITS_ITEM, "node_kind": "item"},
                JENSEITS_OCR_FILE: {"node_id": JENSEITS_OCR_FILE, "node_kind": "file"},
            },
            {JENSEITS_ITEM: [], JENSEITS_OCR_FILE: [edge]},
            {JENSEITS_ITEM: [edge]},
            lambda _ids: [projected_layer],
            JENSEITS_OCR_FILE,
            limit=20,
        )
        self.assertEqual([layer["layer_id"]], [row["rights_id"] for row in result["rights"]])
        self.assertEqual("not_cleared", result["agent_summary"]["rights_posture"])
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])

    def test_file_conclusion_requires_every_complete_membership_to_be_reviewed_positive(self) -> None:
        fixture = navigation_fixture(rights_b_positive=True)
        self.assertTrue(dossier(fixture, FILE_ID)["agent_summary"]["can_conclude_legal_openness"])

        truncated = dossier(fixture, FILE_ID, limit=2)
        self.assertTrue(truncated["truncated"])
        self.assertFalse(truncated["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual("membership_scoped_review_required", truncated["agent_summary"]["rights_posture"])

    def test_legacy_single_item_file_keeps_its_exact_item_scoped_rights(self) -> None:
        fixture = navigation_fixture(legacy=True)
        navigation, nodes, incoming, outgoing, rights = fixture
        incoming[FILE_ID] = incoming[FILE_ID][:1]
        outgoing = {ITEM_A: outgoing[ITEM_A]}
        nodes.pop(ITEM_B)
        rights = rights[:1]
        result = source_dossier_query(
            navigation,
            nodes,
            incoming,
            outgoing,
            lambda _ids: rights,
            FILE_ID,
            limit=20,
        )
        self.assertTrue(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual(["rights-a"], [row["rights_id"] for row in result["rights"]])

    def test_legacy_single_item_file_rejects_conflicting_unbound_rights_sources(self) -> None:
        fixture = navigation_fixture(legacy=True)
        navigation, nodes, incoming, outgoing, rights = fixture
        incoming[FILE_ID] = incoming[FILE_ID][:1]
        outgoing = {ITEM_A: outgoing[ITEM_A]}
        nodes.pop(ITEM_B)
        rights = [
            rights[0],
            {
                **rights[0],
                "rights_id": "rights-a-conflict",
                "source_ref": "ToS/source-witnesses/fixture/alternate-rights.json",
                "redistribution_posture": "not_authorized",
            },
        ]
        result = source_dossier_query(
            navigation,
            nodes,
            incoming,
            outgoing,
            lambda _ids: rights,
            FILE_ID,
            limit=20,
        )
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual("membership_scoped_review_required", result["agent_summary"]["rights_posture"])
        self.assertEqual([], result["rights"])

    def test_legacy_file_only_rights_scope_stays_unbound(self) -> None:
        fixture = navigation_fixture(legacy=True)
        navigation, nodes, incoming, outgoing, _rights = fixture
        incoming[FILE_ID] = incoming[FILE_ID][:1]
        outgoing = {ITEM_A: outgoing[ITEM_A]}
        nodes.pop(ITEM_B)
        rights = [{
            "rights_id": "legacy-file-only",
            "source_ref": RIGHTS_A,
            "scope_refs": [FILE_ID],
            "assessment_status": "licensed",
            "redistribution_posture": "authorized",
            "review_status": "accepted",
        }]
        result = source_dossier_query(navigation, nodes, incoming, outgoing, lambda _ids: rights, FILE_ID, limit=20)
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual("membership_scoped_review_required", result["agent_summary"]["rights_posture"])
        self.assertEqual([], result["rights"])

    def test_legacy_shared_file_without_rights_refs_fails_closed(self) -> None:
        fixture = navigation_fixture(legacy=True)
        result = dossier(fixture, FILE_ID)
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual([], result["rights"])
        self.assertEqual("membership_scoped_review_required", result["agent_summary"]["rights_posture"])


if __name__ == "__main__":
    unittest.main()
