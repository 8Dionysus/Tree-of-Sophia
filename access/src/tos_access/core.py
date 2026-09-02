from __future__ import annotations

import json
import os
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any


INDEX_RELATIVE_PATH = Path("ToS/derived-exports/tos_corpus_index.min.json")
PHILOSOPHY_PROJECTION_RELATIVE_PATH = Path("ToS/derived-exports/philosophy_graph_projection.min.json")
PHILOSOPHY_AUDIT_RELATIVE_PATH = Path("ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json")
SUPPORTED_CORPUS_VIEW_IDS = {
    "corpus-topology",
    "route-graph",
    "promotion-flow",
}


@lru_cache(maxsize=8)
def _read_json_version(path_text: str, mtime_ns: int, size: int) -> dict[str, Any]:
    del mtime_ns, size
    path = Path(path_text)
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError(f"ToS corpus index is not a JSON object: {path}")
    return payload


def _read_json(path: Path) -> dict[str, Any]:
    stat = path.stat()
    return _read_json_version(path.resolve().as_posix(), stat.st_mtime_ns, stat.st_size)


def _contains(value: Any, needle: str) -> bool:
    if isinstance(value, str):
        return needle in value.lower()
    if isinstance(value, dict):
        return any(_contains(item, needle) for item in value.values())
    if isinstance(value, list):
        return any(_contains(item, needle) for item in value)
    return False


def _source_refs(items: list[dict[str, Any]]) -> list[str]:
    refs = {
        str(item.get("source_ref"))
        for item in items
        if isinstance(item.get("source_ref"), str) and item.get("source_ref")
    }
    for item in items:
        if isinstance(item.get("source_refs"), list):
            refs.update(str(ref) for ref in item["source_refs"] if isinstance(ref, str) and ref)
    return sorted(refs)


def _supported_corpus_views(payload: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        view
        for view in payload.get("graph_views", [])
        if isinstance(view, dict) and view.get("view_id") in SUPPORTED_CORPUS_VIEW_IDS
    ]


def _relation_pack_paths(payload: dict[str, Any]) -> dict[str, str]:
    return {
        str(pack["pack_id"]): str(pack["path"])
        for pack in payload.get("relation_packs", [])
        if isinstance(pack, dict)
        and isinstance(pack.get("pack_id"), str)
        and isinstance(pack.get("path"), str)
        and pack.get("path")
    }


def _relation_edge_with_source_ref(edge: dict[str, Any], pack_paths: dict[str, str]) -> dict[str, Any]:
    item = dict(edge)
    if not item.get("source_ref"):
        source_ref = pack_paths.get(str(item.get("pack_id") or ""))
        if source_ref:
            item["source_ref"] = source_ref
    return item


def _string_list(value: Any) -> list[str]:
    return [str(item) for item in value] if isinstance(value, list) else []


def _layer_allowed(item: dict[str, Any], layers: set[str]) -> bool:
    if not layers:
        return True
    item_layers = item.get("graph_layers")
    if not isinstance(item_layers, list):
        return False
    return bool(set(str(layer) for layer in item_layers) & layers)


def _predicate_allowed(item: dict[str, Any], predicates: set[str]) -> bool:
    if not predicates:
        return True
    predicate = item.get("predicate_id")
    return isinstance(predicate, str) and predicate in predicates


def _unique_values(items: list[dict[str, Any]], key: str) -> list[str]:
    return sorted({str(item[key]) for item in items if isinstance(item.get(key), str) and item.get(key)})


def _bounded_int(value: Any, default: int, minimum: int, maximum: int) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        parsed = default
    return max(minimum, min(maximum, parsed))


def _bounded_graph(
    nodes: list[dict[str, Any]],
    edges: list[dict[str, Any]],
    limit: int,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    nodes_by_id = {
        str(node.get("node_id")): node
        for node in nodes
        if isinstance(node.get("node_id"), str)
    }
    selected_node_ids: set[str] = set()
    selected_edges: list[dict[str, Any]] = []
    for edge in edges:
        left = str(edge.get("from_id") or "")
        right = str(edge.get("to_id") or "")
        if left not in nodes_by_id or right not in nodes_by_id:
            continue
        additions = {left, right} - selected_node_ids
        if len(selected_node_ids) + len(additions) > limit:
            continue
        selected_node_ids.update(additions)
        selected_edges.append(edge)
        if len(selected_edges) >= limit:
            break
    for node in nodes:
        node_id = str(node.get("node_id") or "")
        if len(selected_node_ids) >= limit:
            break
        if node_id:
            selected_node_ids.add(node_id)
    selected_nodes = [
        node for node in nodes if str(node.get("node_id") or "") in selected_node_ids
    ]
    return selected_nodes, selected_edges


def _bounded_clusters(
    clusters: list[dict[str, Any]],
    nodes: list[dict[str, Any]],
    edges: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    node_ids = {str(node.get("node_id") or "") for node in nodes}
    edge_ids = {str(edge.get("edge_id") or "") for edge in edges}
    bounded: list[dict[str, Any]] = []
    for cluster in clusters:
        original_node_ids = _string_list(cluster.get("member_node_ids"))
        original_edge_ids = _string_list(cluster.get("member_edge_ids"))
        member_node_ids = [node_id for node_id in original_node_ids if node_id in node_ids]
        member_edge_ids = [edge_id for edge_id in original_edge_ids if edge_id in edge_ids]
        if not member_node_ids and not member_edge_ids:
            continue
        item = dict(cluster)
        item["member_node_ids"] = member_node_ids
        item["member_edge_ids"] = member_edge_ids
        item["available_member_node_count"] = len(original_node_ids)
        item["available_member_edge_count"] = len(original_edge_ids)
        properties = dict(item.get("properties") or {})
        if "member_count" in properties:
            properties["member_count"] = len(member_node_ids)
        if "edge_count" in properties:
            properties["edge_count"] = len(member_edge_ids)
        item["properties"] = properties
        bounded.append(item)
    return bounded


def _discover_root(explicit: str | Path | None = None) -> Path:
    if explicit:
        return Path(explicit).expanduser().resolve()
    configured = os.environ.get("TOS_ROOT") or os.environ.get("AOA_TOS_ROOT")
    if configured:
        return Path(configured).expanduser().resolve()

    package_runtime_data = Path(__file__).resolve().parent / "runtime_data"
    candidates = [Path.cwd(), *Path.cwd().parents, package_runtime_data]
    candidates.extend(Path(__file__).resolve().parents)
    seen: set[Path] = set()
    for candidate in candidates:
        resolved = candidate.resolve()
        if resolved in seen:
            continue
        seen.add(resolved)
        if (resolved / INDEX_RELATIVE_PATH).is_file():
            return resolved
    return Path.cwd().resolve()


def _view_nodes_edges(
    payload: dict[str, Any],
    view: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    inline_nodes = [item for item in view.get("nodes", []) if isinstance(item, dict)]
    inline_edges = [item for item in view.get("edges", []) if isinstance(item, dict)]
    if inline_nodes or inline_edges:
        return inline_nodes, inline_edges

    node_ids = {str(item) for item in view.get("node_ids", []) if isinstance(item, str)}
    edge_ids = {str(item) for item in view.get("edge_ids", []) if isinstance(item, str)}
    nodes = [
        item
        for item in payload.get("nodes", [])
        if isinstance(item, dict) and str(item.get("node_id") or "") in node_ids
    ]
    edges = [
        item
        for item in payload.get("edges", [])
        if isinstance(item, dict) and str(item.get("edge_id") or "") in edge_ids
    ]
    return nodes, edges


def _projection_nodes_edges(payload: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    nodes: list[dict[str, Any]] = []
    edges: list[dict[str, Any]] = []
    seen_node_ids: set[str] = set()
    seen_edge_ids: set[str] = set()

    def append_unique(items: list[dict[str, Any]], seen: set[str], candidate: dict[str, Any], key: str) -> None:
        identity = str(candidate.get(key) or "")
        if not identity or identity in seen:
            return
        seen.add(identity)
        items.append(candidate)

    for node in payload.get("nodes", []):
        if isinstance(node, dict):
            append_unique(nodes, seen_node_ids, node, "node_id")
    for edge in payload.get("edges", []):
        if isinstance(edge, dict):
            append_unique(edges, seen_edge_ids, edge, "edge_id")
    for view in payload.get("views", []):
        if not isinstance(view, dict):
            continue
        view_nodes, view_edges = _view_nodes_edges(payload, view)
        for node in view_nodes:
            append_unique(nodes, seen_node_ids, node, "node_id")
        for edge in view_edges:
            append_unique(edges, seen_edge_ids, edge, "edge_id")
    return nodes, edges


@dataclass(slots=True)
class ToSAccessCore:
    tos_root: Path
    index_path: Path
    philosophy_graph_projection_path: Path
    philosophy_post_planting_audit_path: Path

    @classmethod
    def discover(
        cls,
        tos_root: str | Path | None = None,
        index_path: str | Path | None = None,
        philosophy_graph_projection_path: str | Path | None = None,
        philosophy_post_planting_audit_path: str | Path | None = None,
    ) -> "ToSAccessCore":
        root = _discover_root(tos_root)
        index = Path(
            index_path
            or os.environ.get("TOS_CORPUS_INDEX_PATH")
            or root / INDEX_RELATIVE_PATH
        ).expanduser()
        if not index.is_absolute():
            index = root / index
        philosophy_projection = Path(
            philosophy_graph_projection_path
            or os.environ.get("TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH")
            or root / PHILOSOPHY_PROJECTION_RELATIVE_PATH
        ).expanduser()
        if not philosophy_projection.is_absolute():
            philosophy_projection = root / philosophy_projection
        philosophy_audit = Path(
            philosophy_post_planting_audit_path
            or os.environ.get("TOS_PHILOSOPHY_POST_PLANTING_AUDIT_PATH")
            or root / PHILOSOPHY_AUDIT_RELATIVE_PATH
        ).expanduser()
        if not philosophy_audit.is_absolute():
            philosophy_audit = root / philosophy_audit
        return cls(
            tos_root=root,
            index_path=index.resolve(),
            philosophy_graph_projection_path=philosophy_projection.resolve(),
            philosophy_post_planting_audit_path=philosophy_audit.resolve(),
        )

    def index_exists(self) -> bool:
        return self.index_path.is_file()

    def index(self) -> dict[str, Any]:
        return _read_json(self.index_path)

    def philosophy_projection_exists(self) -> bool:
        return self.philosophy_graph_projection_path.is_file()

    def philosophy_projection(self) -> dict[str, Any]:
        payload = _read_json(self.philosophy_graph_projection_path)
        supported = {"tos_philosophy_graph_projection_v1", "tos_philosophy_graph_projection_v2"}
        if payload.get("schema_version") not in supported:
            raise RuntimeError(
                "ToS philosophy graph projection schema_version must be one of "
                + ", ".join(sorted(supported))
            )
        return payload

    def philosophy_audit_exists(self) -> bool:
        return self.philosophy_post_planting_audit_path.is_file()

    def philosophy_audit_payload(self) -> dict[str, Any]:
        payload = _read_json(self.philosophy_post_planting_audit_path)
        if payload.get("schema_version") != "tos_philosophy_post_planting_audit_v1":
            raise RuntimeError("ToS philosophy post-planting audit schema_version must be tos_philosophy_post_planting_audit_v1")
        return payload

    def status(self) -> dict[str, Any]:
        exists = self.index_exists()
        payload = self.index() if exists else {}
        return {
            "schema": "tos_corpus_mcp_status_v1",
            "index_exists": exists,
            "tos_root": self.tos_root.as_posix(),
            "index_path": self.index_path.as_posix(),
            "owner_repo": payload.get("owner_repo"),
            "surface_kind": payload.get("surface_kind"),
            "counts": payload.get("counts", {}),
            "graph_views": [view.get("view_id") for view in _supported_corpus_views(payload)],
            "authority_order": payload.get("authority_order", []),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    def summary(self) -> dict[str, Any]:
        payload = self.index()
        return {
            "schema": "tos_corpus_mcp_summary_v1",
            "status": self.status(),
            "counts": payload.get("counts", {}),
            "branches": payload.get("branches", []),
            "graph_views": _supported_corpus_views(payload),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_order": payload.get("authority_order", []),
        }

    def search(self, query: str, limit: int = 20, resource_kind: str | None = None) -> dict[str, Any]:
        payload = self.index()
        needle = query.lower().strip()
        limit = _bounded_int(limit, 20, 1, 100)
        results: list[dict[str, Any]] = []
        for collection_name in ("nodes", "resources", "manifests", "branches", "graph_views"):
            for item in payload.get(collection_name, []):
                if not isinstance(item, dict):
                    continue
                if resource_kind and item.get("resource_kind") != resource_kind:
                    continue
                if needle and not _contains(item, needle):
                    continue
                results.append({"collection": collection_name, "item": item})
                if len(results) >= limit:
                    return self._search_payload(query, resource_kind, results)
        return self._search_payload(query, resource_kind, results)

    def _search_payload(
        self,
        query: str,
        resource_kind: str | None,
        results: list[dict[str, Any]],
    ) -> dict[str, Any]:
        return {
            "schema": "tos_corpus_mcp_search_v1",
            "query": query,
            "resource_kind": resource_kind,
            "result_count": len(results),
            "results": results,
            "authority_note": "Tree-of-Sophia owns corpus meaning; this MCP packet is an abyss-stack access-plane view.",
        }

    def resources(
        self,
        resource_kind: str | None = None,
        owner_branch: str | None = None,
        limit: int = 100,
    ) -> dict[str, Any]:
        payload = self.index()
        limit = _bounded_int(limit, 100, 1, 1000)
        items = []
        for resource in payload.get("resources", []):
            if not isinstance(resource, dict):
                continue
            if resource_kind and resource.get("resource_kind") != resource_kind:
                continue
            if owner_branch and resource.get("owner_branch") != owner_branch:
                continue
            items.append(resource)
            if len(items) >= limit:
                break
        return {
            "schema": "tos_corpus_mcp_resources_v1",
            "resource_kind": resource_kind,
            "owner_branch": owner_branch,
            "count": len(items),
            "resources": items,
            "authority_order": payload.get("authority_order", []),
        }

    def node(self, node_id: str) -> dict[str, Any]:
        payload = self.index()
        pack_paths = _relation_pack_paths(payload)
        matches = [
            node
            for node in payload.get("nodes", [])
            if isinstance(node, dict) and node.get("node_id") == node_id
        ]
        related_edges = [
            _relation_edge_with_source_ref(edge, pack_paths)
            for edge in payload.get("relation_edges", [])
            if isinstance(edge, dict) and (edge.get("from_id") == node_id or edge.get("to_id") == node_id)
        ]
        if not matches and related_edges:
            owner_branches = sorted(
                {
                    str(edge["owner_branch"])
                    for edge in related_edges
                    if isinstance(edge.get("owner_branch"), str) and edge.get("owner_branch")
                }
            )
            matches = [
                {
                    "node_id": node_id,
                    "label": node_id,
                    "node_type": "relation-endpoint",
                    "owner_branches": owner_branches,
                    "source_refs": _source_refs(related_edges),
                    "projection_posture": "identity materialized from indexed relation endpoints",
                }
            ]
        if not matches:
            raise KeyError(f"unknown ToS corpus node: {node_id}")
        return {
            "schema": "tos_corpus_mcp_node_v1",
            "node_id": node_id,
            "matches": matches,
            "related_edges": related_edges,
            "authority_note": "Node authority stays in the source_path named by the index.",
        }

    def relation_pack(self, pack_id: str) -> dict[str, Any]:
        payload = self.index()
        pack_paths = _relation_pack_paths(payload)
        packs = [
            pack
            for pack in payload.get("relation_packs", [])
            if isinstance(pack, dict) and pack.get("pack_id") == pack_id
        ]
        if not packs:
            raise KeyError(f"unknown ToS corpus relation pack: {pack_id}")
        edges = [
            _relation_edge_with_source_ref(edge, pack_paths)
            for edge in payload.get("relation_edges", [])
            if isinstance(edge, dict) and edge.get("pack_id") == pack_id
        ]
        return {
            "schema": "tos_corpus_mcp_relation_pack_v1",
            "pack_id": pack_id,
            "packs": packs,
            "edges": edges,
            "authority_note": "Relation-pack authority stays in the ToS path named by the pack.",
        }

    def graph_view(self, view_id: str, limit: int = 100) -> dict[str, Any]:
        payload = self.index()
        limit = _bounded_int(limit, 100, 1, 1000)
        view = next(
            (item for item in payload.get("graph_views", []) if isinstance(item, dict) and item.get("view_id") == view_id),
            None,
        )
        if view is None:
            raise KeyError(f"unknown ToS graph view: {view_id}")
        if view_id not in SUPPORTED_CORPUS_VIEW_IDS:
            raise KeyError(f"unsupported standalone ToS graph view: {view_id}")
        graph_nodes: list[dict[str, Any]] = []
        graph_edges: list[dict[str, Any]] = []
        if view_id == "corpus-topology":
            items = payload.get("branches", [])[:limit]
            root_id = f"view:{view_id}"
            graph_nodes = [
                {
                    "node_id": root_id,
                    "label": view.get("title") or view_id,
                    "node_type": "corpus-root",
                    "source_ref": view.get("entry_surface"),
                }
            ]
            for branch in items:
                if not isinstance(branch, dict) or not branch.get("id"):
                    continue
                branch_id = str(branch["id"])
                source_ref = branch.get("owner_surface") or branch.get("path")
                graph_nodes.append(
                    {
                        **branch,
                        "node_id": branch_id,
                        "label": branch_id,
                        "node_type": "corpus-branch",
                        "source_ref": source_ref,
                    }
                )
                graph_edges.append(
                    {
                        "edge_id": f"corpus-edge:{root_id}:{branch_id}",
                        "from_id": root_id,
                        "to_id": branch_id,
                        "predicate_id": "contains",
                        "source_ref": source_ref,
                    }
                )
        elif view_id == "route-graph":
            packs_by_id = {
                str(pack.get("pack_id")): pack
                for pack in payload.get("relation_packs", [])
                if isinstance(pack, dict)
                and pack.get("owner_branch") == "ToS/canon"
                and pack.get("pack_id")
            }
            graph_edges = []
            pack_paths = _relation_pack_paths(payload)
            for edge in payload.get("relation_edges", []):
                if not isinstance(edge, dict) or str(edge.get("pack_id") or "") not in packs_by_id:
                    continue
                item = _relation_edge_with_source_ref(edge, pack_paths)
                graph_edges.append(item)
                if len(graph_edges) >= limit:
                    break
            endpoint_ids = {
                str(endpoint)
                for edge in graph_edges
                for endpoint in (edge.get("from_id"), edge.get("to_id"))
                if endpoint
            }
            graph_nodes = [
                node
                for node in payload.get("nodes", [])
                if isinstance(node, dict) and str(node.get("node_id") or "") in endpoint_ids
            ]
            items = graph_edges
        elif view_id == "promotion-flow":
            items = []
            pack_paths = _relation_pack_paths(payload)
            for edge in payload.get("relation_edges", []):
                if not isinstance(edge, dict) or edge.get("owner_branch") != "ToS/candidate-intake":
                    continue
                item = _relation_edge_with_source_ref(edge, pack_paths)
                items.append(item)
                if len(items) >= limit:
                    break
            graph_edges = items
            endpoint_ids = {
                str(endpoint)
                for edge in graph_edges
                for endpoint in (edge.get("from_id"), edge.get("to_id"))
                if endpoint
            }
            indexed_nodes = {
                str(node.get("node_id")): node
                for node in payload.get("nodes", [])
                if isinstance(node, dict) and node.get("node_id")
            }
            endpoint_source_refs: dict[str, set[str]] = {node_id: set() for node_id in endpoint_ids}
            for edge in graph_edges:
                source_ref = edge.get("source_ref")
                if not isinstance(source_ref, str) or not source_ref:
                    continue
                for endpoint in (edge.get("from_id"), edge.get("to_id")):
                    endpoint_id = str(endpoint or "")
                    if endpoint_id in endpoint_source_refs:
                        endpoint_source_refs[endpoint_id].add(source_ref)
            graph_nodes = [
                indexed_nodes.get(node_id)
                or {
                    "node_id": node_id,
                    "label": node_id,
                    "node_type": "candidate-endpoint",
                    "authority_layer": "candidate_intake",
                    "owner_branch": "ToS/candidate-intake",
                    "source_refs": sorted(endpoint_source_refs[node_id]),
                }
                for node_id in sorted(endpoint_ids)
            ]
        return {
            "schema": "tos_corpus_mcp_graph_view_v1",
            "view": view,
            "item_count": len(items),
            "items": items,
            "node_count": len(graph_nodes),
            "edge_count": len(graph_edges),
            "nodes": graph_nodes,
            "edges": graph_edges,
            "counts": payload.get("counts", {}),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    def packet(self, query: str = "", view_id: str | None = None, limit: int = 20) -> dict[str, Any]:
        payload = self.index()
        limit = _bounded_int(limit, 20, 1, 100)
        search = self.search(query=query, limit=limit) if query else {"result_count": 0, "results": []}
        view_packet = self.graph_view(view_id, limit=limit) if view_id else None
        return {
            "schema": "tos_corpus_mcp_packet_v1",
            "query": query,
            "view_id": view_id,
            "result_count": search["result_count"],
            "results": search["results"],
            "view": view_packet,
            "counts": payload.get("counts", {}),
            "authority_order": payload.get("authority_order", []),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    def philosophy_status(self) -> dict[str, Any]:
        exists = self.philosophy_projection_exists()
        payload = self.philosophy_projection() if exists else {}
        return {
            "schema": "tos_philosophy_mcp_status_v1",
            "projection_exists": exists,
            "tos_root": self.tos_root.as_posix(),
            "projection_path": self.philosophy_graph_projection_path.as_posix(),
            "owner_repo": payload.get("owner_repo"),
            "surface_kind": payload.get("surface_kind"),
            "counts": payload.get("counts", {}),
            "views": [view.get("view_id") for view in payload.get("views", []) if isinstance(view, dict)],
            "graph_layers": [
                layer.get("layer_id")
                for layer in payload.get("graph_layers", [])
                if isinstance(layer, dict) and layer.get("layer_id")
            ],
            "visibility_model": payload.get("visibility_model", {}),
            "snapshot_review": payload.get("snapshot_review", {}),
            "runtime_projection_boundary": payload.get(
                "runtime_projection_boundary",
                {
                    "runtime_owner": "abyss-stack",
                    "missing_state": "ToS philosophy graph projection is not present at this MCP path",
                },
            ),
            "authority_note": "Tree-of-Sophia owns philosophy meaning; this MCP packet is a Tree-of-Sophia standalone access aid.",
        }

    def philosophy_views(self) -> dict[str, Any]:
        payload = self.philosophy_projection()
        clusters_by_view: dict[str, int] = {}
        for cluster in payload.get("clusters", []):
            if not isinstance(cluster, dict):
                continue
            for view_id in cluster.get("view_ids", []):
                clusters_by_view[str(view_id)] = clusters_by_view.get(str(view_id), 0) + 1
        views = []
        for view in payload.get("views", []):
            if not isinstance(view, dict):
                continue
            views.append(
                {
                    "view_id": view.get("view_id"),
                    "title": view.get("title"),
                    "layout_hint": view.get("layout_hint"),
                    "graph_layers": view.get("graph_layers", []),
                    "node_count": len(_view_nodes_edges(payload, view)[0]),
                    "edge_count": len(_view_nodes_edges(payload, view)[1]),
                    "cluster_count": clusters_by_view.get(str(view.get("view_id")), 0),
                    "review_intent": view.get("review_intent"),
                    "collapse_rule": view.get("collapse_rule", {}),
                    "source_ref": view.get("source_ref"),
                    "route_card": view.get("route_card"),
                }
            )
        return {
            "schema": "tos_philosophy_mcp_views_v1",
            "views": views,
            "counts": payload.get("counts", {}),
            "graph_layers": payload.get("graph_layers", []),
            "layer_counts": payload.get("layer_counts", []),
            "visibility_model": payload.get("visibility_model", {}),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    @staticmethod
    def _philosophy_view_contract(
        payload: dict[str, Any],
        view: dict[str, Any],
        clusters: list[dict[str, Any]],
    ) -> dict[str, Any]:
        nodes, edges = _view_nodes_edges(payload, view)
        source_refs = payload.get("source_refs", {}) if isinstance(payload.get("source_refs"), dict) else {}
        return {
            "schema": "tos_philosophy_mcp_view_contract_v1",
            "view_id": view.get("view_id"),
            "route_card": view.get("route_card"),
            "layout_hint": view.get("layout_hint"),
            "graph_layers": [str(layer) for layer in view.get("graph_layers", []) if layer],
            "node_kinds": _unique_values(nodes, "node_type"),
            "edge_predicates": _unique_values(edges, "predicate_id"),
            "cluster_kinds": _unique_values(clusters, "cluster_kind"),
            "node_count": len(nodes),
            "edge_count": len(edges),
            "cluster_count": len(clusters),
            "source_view_contract_ref": source_refs.get("source_view_contract_ref"),
        }

    def philosophy_contracts(self) -> dict[str, Any]:
        payload = self.philosophy_projection()
        views = [view for view in payload.get("views", []) if isinstance(view, dict)]
        nodes, edges = _projection_nodes_edges(payload)
        clusters = [cluster for cluster in payload.get("clusters", []) if isinstance(cluster, dict)]
        source_refs = payload.get("source_refs", {}) if isinstance(payload.get("source_refs"), dict) else {}
        return {
            "schema": "tos_philosophy_mcp_contracts_v1",
            "source_contract_refs": {
                key: value for key, value in source_refs.items() if isinstance(value, str) and value
            },
            "runtime_contract": {
                "runtime_owner": "Tree-of-Sophia",
                "source_owner": "Tree-of-Sophia",
                "packet_shape": "bounded MCP resources and tools over ToS derived exports",
                "limits": [
                    "no writeback",
                    "no canon promotion",
                    "MCP packets are access aids, not source authority",
                ],
            },
            "views": [
                self._philosophy_view_contract(
                    payload,
                    view,
                    self._philosophy_clusters_for_payload(
                        payload,
                        view_id=str(view.get("view_id") or ""),
                        limit=1_000_000,
                    ),
                )
                for view in views
            ],
            "node_kinds": _unique_values(nodes, "node_type"),
            "edge_predicates": _unique_values(edges, "predicate_id"),
            "graph_layers": _unique_values(
                [layer for layer in payload.get("graph_layers", []) if isinstance(layer, dict)],
                "layer_id",
            ),
            "cluster_kinds": _unique_values(clusters, "cluster_kind"),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_note": "Tree-of-Sophia owns graph meaning; MCP exposes the access-plane contract only.",
        }

    def philosophy_view(self, view_id: str, limit: int = 1000) -> dict[str, Any]:
        payload = self.philosophy_projection()
        limit = _bounded_int(limit, 1000, 1, 1000)
        view = next(
            (item for item in payload.get("views", []) if isinstance(item, dict) and item.get("view_id") == view_id),
            None,
        )
        if view is None:
            raise KeyError(f"unknown ToS philosophy graph view: {view_id}")
        all_nodes, all_edges = _view_nodes_edges(payload, view)
        nodes, edges = _bounded_graph(all_nodes, all_edges, limit)
        clusters = _bounded_clusters(
            self._philosophy_clusters_for_payload(payload, view_id=view_id, limit=1_000_000),
            nodes,
            edges,
        )[:limit]
        bounded_view = dict(view)
        bounded_view.pop("nodes", None)
        bounded_view.pop("edges", None)
        bounded_view["node_ids"] = [str(node.get("node_id")) for node in nodes if node.get("node_id")]
        bounded_view["edge_ids"] = [str(edge.get("edge_id")) for edge in edges if edge.get("edge_id")]
        return {
            "schema": "tos_philosophy_mcp_view_v1",
            "view": bounded_view,
            "node_count": len(nodes),
            "edge_count": len(edges),
            "available_node_count": len(all_nodes),
            "available_edge_count": len(all_edges),
            "limit": limit,
            "nodes": nodes,
            "edges": edges,
            "clusters": clusters,
            "review_packet": self.philosophy_review_packet(view_id),
            "source_refs": view.get("source_refs", []),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    @staticmethod
    def _philosophy_clusters_for_payload(
        payload: dict[str, Any],
        *,
        view_id: str | None = None,
        cluster_kind: str | None = None,
        limit: int = 80,
    ) -> list[dict[str, Any]]:
        clusters: list[dict[str, Any]] = []
        for cluster in payload.get("clusters", []):
            if not isinstance(cluster, dict):
                continue
            if view_id and view_id not in set(cluster.get("view_ids", [])):
                continue
            if cluster_kind and cluster.get("cluster_kind") != cluster_kind:
                continue
            clusters.append(cluster)
        clusters.sort(key=lambda item: (str(item.get("cluster_kind") or ""), str(item.get("label") or "")))
        return clusters[: max(limit, 0)]

    def philosophy_layers(self) -> dict[str, Any]:
        payload = self.philosophy_projection()
        return {
            "schema": "tos_philosophy_mcp_layers_v1",
            "graph_layers": payload.get("graph_layers", []),
            "layer_counts": payload.get("layer_counts", []),
            "visibility_model": payload.get("visibility_model", {}),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    def philosophy_clusters(
        self,
        view_id: str | None = None,
        cluster_kind: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        payload = self.philosophy_projection()
        limit = _bounded_int(limit, 80, 1, 1000)
        clusters = self._philosophy_clusters_for_payload(
            payload,
            view_id=view_id,
            cluster_kind=cluster_kind,
            limit=limit,
        )
        return {
            "schema": "tos_philosophy_mcp_clusters_v1",
            "view_id": view_id,
            "cluster_kind": cluster_kind,
            "clusters": clusters,
            "cluster_count": len(clusters),
            "counts": payload.get("counts", {}),
            "source_refs": _source_refs(clusters),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    def philosophy_scale_manifest(self, view_id: str | None = None, layers: list[str] | None = None) -> dict[str, Any]:
        payload = self.philosophy_projection()
        layer_filter = set(layers or [])
        rows = {
            table: self.philosophy_scale_rows(table, view_id=view_id, layers=layers)
            for table in (
                "nodes",
                "edges",
                "clusters",
                "cluster-node-memberships",
                "cluster-edge-memberships",
            )
        }
        def table_descriptor(table: str) -> dict[str, Any]:
            return {
                "row_count": len(rows[table]),
                "packet_route": "tos_philosophy_graph_scale_rows",
                "packet_route_args": {
                    "table": table,
                    "view_id": view_id,
                    "layers": sorted(layer_filter),
                },
            }
        return {
            "schema": "tos_philosophy_mcp_scale_manifest_v1",
            "view_id": view_id,
            "layers": sorted(layer_filter),
            "tables": {
                table: table_descriptor(table)
                for table in rows
            },
            "source_projection_ref": self.philosophy_graph_projection_path.as_posix(),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_note": "Scale manifests are MCP navigation packets; ToS derived exports remain authoritative.",
        }

    def philosophy_scale_rows(
        self,
        table: str,
        view_id: str | None = None,
        layers: list[str] | None = None,
    ) -> list[dict[str, Any]]:
        payload = self.philosophy_projection()
        layer_filter = set(layers or [])
        if view_id:
            view = next(
                (item for item in payload.get("views", []) if isinstance(item, dict) and item.get("view_id") == view_id),
                None,
            )
            if view is None:
                raise KeyError(f"unknown ToS philosophy graph view: {view_id}")
            nodes, edges = _view_nodes_edges(payload, view)
            clusters = self._philosophy_clusters_for_payload(payload, view_id=view_id, limit=1_000_000)
        else:
            nodes, edges = _projection_nodes_edges(payload)
            clusters = [item for item in payload.get("clusters", []) if isinstance(item, dict)]

        nodes = [node for node in nodes if _layer_allowed(node, layer_filter)]
        node_ids = {str(node.get("node_id")) for node in nodes if isinstance(node.get("node_id"), str)}
        edges = [
            edge
            for edge in edges
            if _layer_allowed(edge, layer_filter)
            and str(edge.get("from_id") or "") in node_ids
            and str(edge.get("to_id") or "") in node_ids
        ]
        edge_ids = {str(edge.get("edge_id")) for edge in edges if isinstance(edge.get("edge_id"), str)}
        clusters = _bounded_clusters(
            [cluster for cluster in clusters if _layer_allowed(cluster, layer_filter)],
            nodes,
            edges,
        )

        if table == "nodes":
            return nodes
        if table == "edges":
            return edges
        if table == "clusters":
            return clusters
        if table == "cluster-node-memberships":
            return [
                {
                    "cluster_id": cluster.get("cluster_id"),
                    "node_id": node_id,
                    "source_ref": cluster.get("source_ref"),
                    "source_refs": cluster.get("source_refs", []),
                }
                for cluster in clusters
                for node_id in _string_list(cluster.get("member_node_ids"))
                if node_id in node_ids
            ]
        if table == "cluster-edge-memberships":
            return [
                {
                    "cluster_id": cluster.get("cluster_id"),
                    "edge_id": edge_id,
                    "source_ref": cluster.get("source_ref"),
                    "source_refs": cluster.get("source_refs", []),
                }
                for cluster in clusters
                for edge_id in _string_list(cluster.get("member_edge_ids"))
                if edge_id in edge_ids
            ]
        raise KeyError(f"unknown scale export table: {table}")

    def philosophy_scale_packet(
        self,
        table: str,
        view_id: str | None = None,
        layers: list[str] | None = None,
        offset: int = 0,
        limit: int = 1000,
    ) -> dict[str, Any]:
        all_rows = self.philosophy_scale_rows(table, view_id=view_id, layers=layers)
        offset = _bounded_int(offset, 0, 0, 10_000_000)
        limit = _bounded_int(limit, 1000, 1, 10_000)
        rows = all_rows[offset : offset + limit]
        next_offset = offset + len(rows)
        return {
            "schema": "tos_philosophy_mcp_scale_rows_v1",
            "table": table,
            "view_id": view_id,
            "layers": sorted(set(layers or [])),
            "offset": offset,
            "limit": limit,
            "row_count": len(rows),
            "total_row_count": len(all_rows),
            "next_offset": next_offset if next_offset < len(all_rows) else None,
            "rows": rows,
            "source_projection_ref": self.philosophy_graph_projection_path.as_posix(),
            "authority_note": "Scale rows are MCP navigation packets; ToS derived exports remain authoritative.",
        }

    def philosophy_review_packet(self, view_id: str = "chronology") -> dict[str, Any]:
        payload = self.philosophy_projection()
        packet = next(
            (
                item
                for item in payload.get("review_packets", [])
                if isinstance(item, dict) and item.get("view_id") == view_id
            ),
            None,
        )
        if packet is None:
            raise KeyError(f"unknown ToS philosophy review packet view: {view_id}")
        return {
            "schema": "tos_philosophy_mcp_review_packet_v1",
            "packet": packet,
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_note": "Tree-of-Sophia owns review packet semantics; MCP serves the compact access packet.",
        }

    def philosophy_snapshot(self) -> dict[str, Any]:
        payload = self.philosophy_projection()
        return {
            "schema": "tos_philosophy_mcp_snapshot_v1",
            "snapshot_review": payload.get("snapshot_review", {}),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_note": "Tree-of-Sophia owns snapshot semantics; MCP serves fingerprints for review and diff routing.",
        }

    def philosophy_audit(self) -> dict[str, Any]:
        if not self.philosophy_audit_exists():
            return {
                "schema": "tos_philosophy_mcp_audit_v1",
                "audit_exists": False,
                "audit_path": self.philosophy_post_planting_audit_path.as_posix(),
                "audit": {},
                "authority_note": "Tree-of-Sophia has not published the post-planting audit at this MCP path.",
            }
        return {
            "schema": "tos_philosophy_mcp_audit_v1",
            "audit_exists": True,
            "audit_path": self.philosophy_post_planting_audit_path.as_posix(),
            "audit": self.philosophy_audit_payload(),
            "authority_note": "Tree-of-Sophia owns the audit; MCP serves it as an access packet.",
        }

    def philosophy_unresolved(self, view_id: str | None = None) -> dict[str, Any]:
        payload = self.philosophy_projection()
        surfaces = [item for item in payload.get("unresolved_review_surfaces", []) if isinstance(item, dict)]
        if view_id:
            surfaces = [
                item
                for item in self.philosophy_review_packet(view_id)["packet"].get("unresolved_diagnostics", [])
                if isinstance(item, dict)
            ]
        return {
            "schema": "tos_philosophy_mcp_unresolved_v1",
            "view_id": view_id,
            "unresolved": surfaces,
            "unresolved_count": len(surfaces),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    def philosophy_node(self, node_id: str) -> dict[str, Any]:
        payload = self.philosophy_projection()
        nodes, edges = _projection_nodes_edges(payload)
        node = next(
            (item for item in nodes if item.get("node_id") == node_id),
            None,
        )
        if node is None:
            raise KeyError(f"unknown ToS philosophy node: {node_id}")
        related_edges = [
            edge
            for edge in edges
            if edge.get("from_id") == node_id or edge.get("to_id") == node_id
        ]
        return {
            "schema": "tos_philosophy_mcp_node_v1",
            "node_id": node_id,
            "node": node,
            "related_edges": related_edges,
            "source_refs": _source_refs([node] + related_edges),
            "authority_note": "Node source_ref stays authoritative in Tree-of-Sophia; MCP exposes an access packet only.",
        }

    def philosophy_edge(self, edge_id: str) -> dict[str, Any]:
        payload = self.philosophy_projection()
        nodes, edges = _projection_nodes_edges(payload)
        edge = next(
            (item for item in edges if item.get("edge_id") == edge_id),
            None,
        )
        if edge is None:
            raise KeyError(f"unknown ToS philosophy edge: {edge_id}")
        endpoint_ids = {str(edge.get("from_id") or ""), str(edge.get("to_id") or "")}
        endpoints = [
            node
            for node in nodes
            if str(node.get("node_id") or "") in endpoint_ids
        ]
        return {
            "schema": "tos_philosophy_mcp_edge_v1",
            "edge_id": edge_id,
            "edge": edge,
            "endpoints": endpoints,
            "source_refs": _source_refs([edge] + endpoints),
            "authority_note": "Edge source_ref stays authoritative in Tree-of-Sophia; MCP exposes an access packet only.",
        }

    def philosophy_neighborhood(
        self,
        node_id: str,
        depth: int = 1,
        layers: list[str] | None = None,
        predicates: list[str] | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        node_packet = self.philosophy_node(node_id)
        payload = self.philosophy_projection()
        depth = _bounded_int(depth, 1, 1, 3)
        limit = _bounded_int(limit, 80, 1, 300)
        layer_filter = set(layers or [])
        predicate_filter = set(predicates or [])
        nodes, projection_edges = _projection_nodes_edges(payload)
        nodes_by_id = {
            str(node.get("node_id")): node
            for node in nodes
            if node.get("node_id")
        }
        node_order = {node_id: index for index, node_id in enumerate(nodes_by_id)}
        allowed_node_ids = {
            node_key
            for node_key, candidate in nodes_by_id.items()
            if node_key == node_id or _layer_allowed(candidate, layer_filter)
        }
        all_edges = [
            edge
            for edge in projection_edges
            if _layer_allowed(edge, layer_filter)
            and _predicate_allowed(edge, predicate_filter)
            and str(edge.get("from_id") or "") in allowed_node_ids
            and str(edge.get("to_id") or "") in allowed_node_ids
        ]
        selected_ids = {node_id}
        discovery_order = [node_id]
        frontier = [node_id]
        traversal_edges: list[dict[str, Any]] = []
        selected_edge_ids: set[str] = set()
        for _ in range(depth):
            candidates: dict[str, dict[str, Any]] = {}
            for edge in all_edges:
                from_id = str(edge.get("from_id") or "")
                to_id = str(edge.get("to_id") or "")
                if from_id in frontier and to_id not in selected_ids:
                    candidates.setdefault(to_id, edge)
                if to_id in frontier and from_id not in selected_ids:
                    candidates.setdefault(from_id, edge)
            next_frontier: list[str] = []
            for candidate_id in sorted(candidates, key=lambda item: (node_order.get(item, len(node_order)), item)):
                if len(discovery_order) - 1 >= limit:
                    break
                selected_ids.add(candidate_id)
                discovery_order.append(candidate_id)
                next_frontier.append(candidate_id)
                edge = candidates[candidate_id]
                edge_id = str(edge.get("edge_id") or "")
                if edge_id not in selected_edge_ids:
                    traversal_edges.append(edge)
                    selected_edge_ids.add(edge_id)
            frontier = next_frontier
            if not frontier or len(discovery_order) - 1 >= limit:
                break
        neighbors = [nodes_by_id[item] for item in discovery_order[1:] if item in nodes_by_id]
        retained_edges = list(traversal_edges)
        for edge in all_edges:
            if len(retained_edges) >= limit:
                break
            edge_id = str(edge.get("edge_id") or "")
            if edge_id in selected_edge_ids:
                continue
            if str(edge.get("from_id") or "") in selected_ids and str(edge.get("to_id") or "") in selected_ids:
                retained_edges.append(edge)
                selected_edge_ids.add(edge_id)
        return {
            "schema": "tos_philosophy_mcp_neighborhood_v1",
            "node": node_packet["node"],
            "neighbors": neighbors,
            "edges": retained_edges,
            "depth": depth,
            "layers": sorted(layer_filter),
            "predicates": sorted(predicate_filter),
            "limit": limit,
            "source_refs": _source_refs([node_packet["node"]] + neighbors + retained_edges),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
        }

    def philosophy_path_between(
        self,
        from_id: str,
        to_id: str,
        layers: list[str] | None = None,
        predicates: list[str] | None = None,
        max_depth: int = 6,
    ) -> dict[str, Any]:
        payload = self.philosophy_projection()
        nodes, edges = _projection_nodes_edges(payload)
        nodes_by_id = {
            str(node.get("node_id")): node
            for node in nodes
            if isinstance(node.get("node_id"), str)
        }
        if from_id not in nodes_by_id:
            raise KeyError(f"unknown ToS philosophy node: {from_id}")
        if to_id not in nodes_by_id:
            raise KeyError(f"unknown ToS philosophy node: {to_id}")
        layer_filter = set(layers or [])
        predicate_filter = set(predicates or [])
        adjacency: dict[str, list[tuple[str, dict[str, Any]]]] = {}
        for edge in edges:
            if (
                not _layer_allowed(edge, layer_filter)
                or not _predicate_allowed(edge, predicate_filter)
            ):
                continue
            left = str(edge.get("from_id") or "")
            right = str(edge.get("to_id") or "")
            adjacency.setdefault(left, []).append((right, edge))

        queue: list[tuple[str, list[str], list[dict[str, Any]]]] = [(from_id, [from_id], [])]
        seen = {from_id}
        found_nodes: list[str] = []
        found_edges: list[dict[str, Any]] = []
        depth_limit = max(1, min(8, int(max_depth)))
        while queue:
            current, path_nodes, path_edges = queue.pop(0)
            if current == to_id:
                found_nodes = path_nodes
                found_edges = path_edges
                break
            if len(path_edges) >= depth_limit:
                continue
            for neighbor, edge in adjacency.get(current, []):
                if neighbor in seen:
                    continue
                seen.add(neighbor)
                queue.append((neighbor, [*path_nodes, neighbor], [*path_edges, edge]))
        path_nodes_payload = [nodes_by_id[node_id] for node_id in found_nodes]
        return {
            "schema": "tos_philosophy_mcp_path_v1",
            "from_id": from_id,
            "to_id": to_id,
            "found": bool(found_nodes),
            "nodes": path_nodes_payload,
            "edges": found_edges,
            "max_depth": depth_limit,
            "layers": sorted(layer_filter),
            "predicates": sorted(predicate_filter),
            "source_refs": _source_refs(path_nodes_payload + found_edges),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_note": "Tree-of-Sophia owns graph meaning; MCP serves a bounded path packet.",
        }

    def philosophy_search(self, query: str, limit: int = 20) -> dict[str, Any]:
        payload = self.philosophy_projection()
        nodes, edges = _projection_nodes_edges(payload)
        needle = query.lower().strip()
        limit = _bounded_int(limit, 20, 1, 100)
        results: list[dict[str, Any]] = []
        collections = {
            "views": payload.get("views", []),
            "nodes": nodes,
            "edges": edges,
            "clusters": payload.get("clusters", []),
            "review_packets": payload.get("review_packets", []),
            "graph_layers": payload.get("graph_layers", []),
        }
        for collection_name, collection in collections.items():
            for item in collection:
                if not isinstance(item, dict):
                    continue
                if needle and not _contains(item, needle):
                    continue
                compact_item = dict(item)
                if collection_name == "views":
                    view_nodes, view_edges = _view_nodes_edges(payload, item)
                    for key in ("nodes", "edges", "node_ids", "edge_ids"):
                        compact_item.pop(key, None)
                    compact_item["node_count"] = len(view_nodes)
                    compact_item["edge_count"] = len(view_edges)
                elif collection_name == "clusters":
                    member_node_ids = _string_list(compact_item.pop("member_node_ids", []))
                    member_edge_ids = _string_list(compact_item.pop("member_edge_ids", []))
                    compact_item["member_node_count"] = len(member_node_ids)
                    compact_item["member_edge_count"] = len(member_edge_ids)
                elif collection_name == "review_packets":
                    diagnostics = compact_item.pop("unresolved_diagnostics", [])
                    compact_item["unresolved_diagnostic_count"] = len(diagnostics) if isinstance(diagnostics, list) else 0
                results.append({"collection": collection_name, "item": compact_item})
                if len(results) >= limit:
                    return self._philosophy_search_payload(query, results)
        return self._philosophy_search_payload(query, results)

    @staticmethod
    def _philosophy_search_payload(query: str, results: list[dict[str, Any]]) -> dict[str, Any]:
        return {
            "schema": "tos_philosophy_mcp_search_v1",
            "query": query,
            "result_count": len(results),
            "results": results,
            "authority_note": "Tree-of-Sophia owns philosophy meaning; this MCP search result is an access-plane packet.",
        }

    def philosophy_packet(self, query: str = "", view_id: str | None = None, limit: int = 20) -> dict[str, Any]:
        payload = self.philosophy_projection()
        limit = _bounded_int(limit, 20, 1, 100)
        search = self.philosophy_search(query=query, limit=limit) if query else {"result_count": 0, "results": []}
        view_packet = self.philosophy_view(view_id, limit=limit) if view_id else None
        compact_view = None
        if view_packet:
            compact_view = {
                "view": view_packet["view"],
                "nodes": view_packet["nodes"],
                "edges": view_packet["edges"],
                "clusters": view_packet.get("clusters", [])[:limit],
                "review_packet": view_packet.get("review_packet"),
                "source_refs": view_packet["source_refs"],
            }
        return {
            "schema": "tos_philosophy_mcp_packet_v1",
            "query": query,
            "view_id": view_id,
            "result_count": search["result_count"],
            "results": search["results"],
            "view": compact_view,
            "counts": payload.get("counts", {}),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_note": "Packets are access aids; ToS owns meaning and Neo4j/UI/MCP remain projections.",
        }

    def philosophy_lens_packet(self, view_id: str, limit: int = 20) -> dict[str, Any]:
        packet = self.philosophy_packet(view_id=view_id, limit=limit)
        review = self.philosophy_review_packet(view_id)
        return {
            "schema": "tos_philosophy_mcp_lens_packet_v1",
            "view_id": view_id,
            "packet": packet,
            "review_packet": review["packet"],
            "authority_note": "Lens packets are compact review slices; ToS source_ref surfaces remain authoritative.",
        }

    def read_resource(self, uri: str) -> dict[str, Any]:
        if uri == "tos-corpus://status":
            return self.status()
        if uri == "tos-corpus://summary":
            return self.summary()
        if uri == "tos-corpus://graph-views":
            payload = self.index()
            return {"schema": "tos_corpus_mcp_graph_views_v1", "graph_views": _supported_corpus_views(payload)}
        prefix = "tos-corpus://graph-view/"
        if uri.startswith(prefix):
            return self.graph_view(uri.removeprefix(prefix))
        if uri == "tos-philosophy://status":
            return self.philosophy_status()
        if uri == "tos-philosophy://views":
            return self.philosophy_views()
        if uri == "tos-philosophy://layers":
            return self.philosophy_layers()
        if uri == "tos-philosophy://contracts":
            return self.philosophy_contracts()
        if uri == "tos-philosophy://scale-manifest":
            return self.philosophy_scale_manifest()
        if uri == "tos-philosophy://snapshot":
            return self.philosophy_snapshot()
        if uri == "tos-philosophy://audit":
            return self.philosophy_audit()
        if uri == "tos-philosophy://clusters":
            return self.philosophy_clusters()
        if uri == "tos-philosophy://unresolved":
            return self.philosophy_unresolved()
        philosophy_view_prefix = "tos-philosophy://view/"
        if uri.startswith(philosophy_view_prefix):
            return self.philosophy_view(uri.removeprefix(philosophy_view_prefix))
        philosophy_review_prefix = "tos-philosophy://review-packet/"
        if uri.startswith(philosophy_review_prefix):
            return self.philosophy_review_packet(uri.removeprefix(philosophy_review_prefix))
        philosophy_edge_prefix = "tos-philosophy://edge/"
        if uri.startswith(philosophy_edge_prefix):
            return self.philosophy_edge(uri.removeprefix(philosophy_edge_prefix))
        philosophy_lens_prefix = "tos-philosophy://lens/"
        if uri.startswith(philosophy_lens_prefix):
            return self.philosophy_lens_packet(uri.removeprefix(philosophy_lens_prefix))
        raise KeyError(f"unknown ToS corpus resource URI: {uri}")

    def render_resource(self, uri: str) -> str:
        return json.dumps(self.read_resource(uri), ensure_ascii=False, indent=2, sort_keys=True)
