from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ACCESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS_ROOT / "src"))

from tos_access.knowledge import build_knowledge_graph  # noqa: E402


def _schemas():
    contracts = ACCESS_ROOT / "contracts"
    graph = json.loads((contracts / "knowledge-graph.v1.schema.json").read_text())
    indexed = json.loads((contracts / "knowledge-search-indexed.v2.schema.json").read_text())
    registry = Registry().with_resource(graph["$id"], Resource.from_contents(graph))
    return graph, Draft202012Validator(indexed, registry=registry)


def _packet():
    graph = build_knowledge_graph(
        {"source_navigation": {"nodes": [
            {"node_id": "tos.synthetic.work", "node_kind": "work",
             "source_ref": "fixture:source", "properties": {}},
            {"node_id": "tos.synthetic.edition", "node_kind": "edition",
             "source_ref": "fixture:source", "properties": {}},
        ], "edges": [
            {"edge_id": "fixture:edge", "from_id": "tos.synthetic.work",
             "to_id": "tos.synthetic.edition", "predicate_id": "has_edition",
             "source_refs": ["fixture:source"]},
        ]}},
        {},
    )
    return graph, {
        "schema": "tos_knowledge_search_indexed_v2",
        "source_revision": graph["source_revision"],
        "query": "work",
        "filters": {"sources": ["source-navigation"], "kind_ids": [], "predicate_ids": []},
        "page": {"cursor": None, "next_cursor": None, "limit_per_kind": 40,
                 "ordering_scope": "global-rank", "has_more": False},
        "counts": {"matching_nodes": 2, "matching_relations": 1,
                   "returned_nodes": 2, "returned_relations": 1,
                   "scope": "exact-if-kind-exhausted-without-continuation"},
        "nodes": graph["nodes"], "relations": graph["relations"],
        "authority_boundary": graph["authority_boundary"],
        "work": {"nodes": {}, "relations": {}},
    }


def test_indexed_v2_structural_source_extension_keeps_graph_v1_frozen():
    graph_schema, indexed = _schemas()
    graph, packet = _packet()
    Draft202012Validator(graph_schema).validate(graph)
    indexed.validate(packet)

    extended_graph = copy.deepcopy(graph)
    extended_packet = copy.deepcopy(packet)
    extended_graph["nodes"][0]["source_graph"] = "owner-registered-eighth"
    extended_packet["nodes"][0]["source_graph"] = "owner-registered-eighth"
    extended_packet["relations"][0]["source_graph"] = "owner-registered-eighth"
    extended_packet["filters"]["sources"] = ["owner-registered-eighth"]
    assert not Draft202012Validator(graph_schema).is_valid(extended_graph)
    indexed.validate(extended_packet)

    extended_packet["nodes"][0]["source_graph"] = ""
    assert not indexed.is_valid(extended_packet)
