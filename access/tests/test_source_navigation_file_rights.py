from __future__ import annotations

import unittest

from tos_access.source_navigation_query import source_dossier_query


FILE_ID = "tos.file.sha256.shared"
ITEM_A = "tos.item.copy.a"
ITEM_B = "tos.item.copy.b"
MANIFEST_A = "ToS/source-witnesses/fixture/copy-a/item.manifest.json"
MANIFEST_B = "ToS/source-witnesses/fixture/copy-b/item.manifest.json"
RIGHTS_A = "ToS/source-witnesses/fixture/copy-a/rights.json"
RIGHTS_B = "ToS/source-witnesses/fixture/copy-b/rights.json"


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
        file_dossier = dossier(fixture, FILE_ID)
        self.assertFalse(file_dossier["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual("membership_scoped_review_required", file_dossier["agent_summary"]["rights_posture"])
        self.assertEqual([FILE_ID, ITEM_A, ITEM_B], file_dossier["agent_summary"]["rights_scope_refs"])
        self.assertEqual({"rights-a", "rights-b"}, {row["rights_id"] for row in file_dossier["rights"]})

        item_a = dossier(fixture, ITEM_A)
        self.assertTrue(item_a["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual(["rights-a"], [row["rights_id"] for row in item_a["rights"]])
        item_b = dossier(fixture, ITEM_B)
        self.assertFalse(item_b["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual(["rights-b"], [row["rights_id"] for row in item_b["rights"]])

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

    def test_legacy_shared_file_without_rights_refs_fails_closed(self) -> None:
        fixture = navigation_fixture(legacy=True)
        result = dossier(fixture, FILE_ID)
        self.assertFalse(result["agent_summary"]["can_conclude_legal_openness"])
        self.assertEqual([], result["rights"])
        self.assertEqual("membership_scoped_review_required", result["agent_summary"]["rights_posture"])


if __name__ == "__main__":
    unittest.main()
