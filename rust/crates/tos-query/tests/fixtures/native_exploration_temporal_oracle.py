"""Independent maintained Python rules over the exact emitted native CMP graph.

Opaque checkpoint tokens and snapshot hashes are adapter-specific. Only those
fields are removed for packet parity; counts, order, scene, and all carriers stay.
"""
import copy
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[5] / "access" / "src"))
from tos_access.exploration import ExplorationService
from tos_access.temporal_comparison import compare_temporal_operands

raw = sys.stdin.buffer.read()
graph = json.loads(raw)
nodes = graph["nodes"]
relations = graph["relations"]
sources = sorted({item["source_graph"] for item in [*nodes, *relations]})

def comparable_packet(packet):
    packet = copy.deepcopy(packet)
    packet.pop("snapshot_revision")
    packet["page"]["next_cursor"] = None
    return packet

requests = []
for profile in ("all", "overview"):
    for direction in ("incoming", "outgoing", "either"):
        # Discover exact IDs from producer bytes, preserving no fixture ID ABI.
        for node in nodes:
            requests.append({"focus_node_id": node["id"], "sources": sources,
                             "profile": profile, "direction": direction,
                             "max_depth": 2, "page_nodes": 1, "page_relations": 1})
for kind, rows in (("node", nodes), ("relation", relations)):
    if rows:
        item = rows[0]
        requests.append({"schema_version": "tos_exploration_request_v2",
                         "source_revision": graph["source_revision"],
                         "origin": {"kind": kind, "id": item["id"],
                                    "content_revision": item["content_revision"]},
                         "sources": sources, "profile": "all", "direction": "either",
                         "max_depth": 2, "page_nodes": 1, "page_relations": 1})
exploration = []
for work in (2, 512):
    for request in requests:
        service = ExplorationService(lambda: graph, work_limit=work)
        pages = []
        value = request
        for _ in range(256):
            packet = service.explore(value)
            pages.append(comparable_packet(packet))
            cursor = packet["page"]["next_cursor"]
            if cursor is None:
                break
            value = {"cursor": cursor}
        else:
            raise AssertionError("bounded fixture exploration failed to finish")
        exploration.append({"request": request, "work": work, "pages": pages})
claims = [n for n in nodes if n["kind_id"] == "claim" and n["type_id"] == "tos.entity.claim"]
assert claims, "genuine native Claim fixture required"
lookup = lambda identifier: [n for n in nodes if n["id"] == identifier]
temporal = []
for left, right in [(c, c) for c in claims] + [(nodes[0], claims[0])]:
    request = {"schema_version": "tos_temporal_comparison_request_v1",
               "source_revision": graph["source_revision"],
               "left": {"node_id": left["id"], "content_revision": left["content_revision"]},
               "right": {"node_id": right["id"], "content_revision": right["content_revision"]}}
    temporal.append({"request": request,
                     "packet": compare_temporal_operands(graph["source_revision"], request, lookup)})
json.dump({"input_sha256": hashlib.sha256(raw).hexdigest(),
           "exploration": exploration, "temporal": temporal}, sys.stdout, ensure_ascii=False)
