from __future__ import annotations

import importlib.util
import json
import os
import sys
from collections import deque
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any


INDEX_RELATIVE_PATH = Path("ToS/derived-exports/tos_corpus_index.min.json")
PHILOSOPHY_PROJECTION_RELATIVE_PATH = Path("ToS/derived-exports/philosophy_graph_projection.min.json")
PHILOSOPHY_AUDIT_RELATIVE_PATH = Path("ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json")
EVIDENCE_PROJECTION_RELATIVE_PATH = Path("ToS/derived-exports/epistemic_evidence_projection.min.json")
WORD_ANALYSIS_PROVIDER_RELATIVE_PATH = Path("scripts/prepare_zarathustra_word_analysis_v1.py")
SOURCE_GAP_LEDGER_RELATIVE_PATH = Path("ToS/source-witnesses/access-requests/public-ledger")
SUPPORTED_CORPUS_VIEW_IDS = {
    "corpus-topology",
    "route-graph",
    "promotion-flow",
}
PHILOSOPHY_PATH_STATE_LIMIT = 50_000
PHILOSOPHY_PATH_FRONTIER_LIMIT = 5_000
PHILOSOPHY_CHALLENGE_PREDICATES = {
    "contested_by",
    "uncertain_relation",
    "polemicizes_with",
}


def _unavailable_word_analysis_capability(reason: str) -> dict[str, Any]:
    return {
        "schema": "tos_zarathustra_word_analysis_capability_v1",
        "available": False,
        "reason": reason,
        "provider_ref": WORD_ANALYSIS_PROVIDER_RELATIVE_PATH.as_posix(),
        "publication_posture": "excluded_from_public_bundle",
        "task": None,
        "authority": {
            "source_owner": "Tree-of-Sophia",
            "access_plane_is_source": False,
            "is_semantic_truth": False,
            "writes_to_tree": False,
            "reviewed": False,
            "canon": False,
        },
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
    evidence_projection_path: Path

    @classmethod
    def discover(
        cls,
        tos_root: str | Path | None = None,
        index_path: str | Path | None = None,
        philosophy_graph_projection_path: str | Path | None = None,
        philosophy_post_planting_audit_path: str | Path | None = None,
        evidence_projection_path: str | Path | None = None,
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
        evidence_projection = Path(
            evidence_projection_path
            or os.environ.get("TOS_EVIDENCE_PROJECTION_PATH")
            or root / EVIDENCE_PROJECTION_RELATIVE_PATH
        ).expanduser()
        if not evidence_projection.is_absolute():
            evidence_projection = root / evidence_projection
        return cls(
            tos_root=root,
            index_path=index.resolve(),
            philosophy_graph_projection_path=philosophy_projection.resolve(),
            philosophy_post_planting_audit_path=philosophy_audit.resolve(),
            evidence_projection_path=evidence_projection.resolve(),
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

    def evidence_projection_exists(self) -> bool:
        return self.evidence_projection_path.is_file()

    def evidence_projection(self) -> dict[str, Any]:
        payload = _read_json(self.evidence_projection_path)
        if payload.get("schema_version") != "tos_epistemic_evidence_projection_v1":
            raise RuntimeError(
                "ToS Evidence Lens projection schema_version must be "
                "tos_epistemic_evidence_projection_v1"
            )
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

    def zarathustra_word_analysis_task(
        self,
        query: str,
        language: str = "ru",
        rank: int = 1,
        include_semantic_neighbors: bool = False,
    ) -> dict[str, Any]:
        normalized_query = str(query).strip()
        if not normalized_query:
            raise ValueError("word-analysis query is required")
        if len(normalized_query) > 256:
            raise ValueError("word-analysis query exceeds 256 characters")
        normalized_language = str(language).strip().lower()
        if normalized_language not in {"de", "ru", "en"}:
            raise ValueError(f"unsupported word-analysis language: {normalized_language}")
        bounded_rank = _bounded_int(rank, 1, 1, 100)
        provider_candidate = self.tos_root / WORD_ANALYSIS_PROVIDER_RELATIVE_PATH
        root = self.tos_root.resolve()
        authority = {
            "source_owner": "Tree-of-Sophia",
            "access_plane_is_source": False,
            "is_semantic_truth": False,
            "writes_to_tree": False,
            "reviewed": False,
            "canon": False,
        }
        if provider_candidate.is_symlink() or not provider_candidate.is_file():
            return _unavailable_word_analysis_capability(
                "local source-bound word-analysis provider is not installed"
            )
        provider_path = provider_candidate.resolve()
        if root not in provider_path.parents:
            raise RuntimeError("local word-analysis provider escapes the configured ToS root")
        stat = provider_path.stat()
        module_name = f"tos_local_word_analysis_{stat.st_mtime_ns}_{stat.st_size}"
        spec = importlib.util.spec_from_file_location(module_name, provider_path)
        if spec is None or spec.loader is None:
            raise RuntimeError(f"cannot load local word-analysis provider: {provider_path}")
        module = importlib.util.module_from_spec(spec)
        sys.modules[module_name] = module
        try:
            spec.loader.exec_module(module)
            build_task = getattr(module, "build_task", None)
            if not callable(build_task):
                raise RuntimeError("local word-analysis provider has no callable build_task")
            try:
                task = build_task(
                    normalized_query,
                    normalized_language,
                    rank=bounded_rank,
                    include_semantic_neighbors=bool(include_semantic_neighbors),
                )
            except RuntimeError as exc:
                if not str(exc).startswith(
                    "private source-return artifact must be a regular non-symlink:"
                ):
                    raise
                return _unavailable_word_analysis_capability(
                    "private source-return artifacts are not installed"
                )
        finally:
            sys.modules.pop(module_name, None)
        task_authority = task.get("authority") if isinstance(task, dict) else None
        task_source = task.get("source") if isinstance(task, dict) else None
        safe_authority = isinstance(task_authority, dict) and all(
            task_authority.get(field) is False
            for field in ("accepted", "semantic_fact_asserted", "canon_effect")
        )
        source_bound = (
            isinstance(task_source, dict)
            and task_source.get("language") == "de"
            and isinstance(task_source.get("exact_context"), str)
            and bool(task_source["exact_context"])
        )
        if (
            not isinstance(task, dict)
            or task.get("schema_version") != "tos_zarathustra_word_analysis_task_v1"
            or not safe_authority
            or not source_bound
        ):
            raise RuntimeError("local word-analysis provider returned an unsupported task contract")
        return {
            "schema": "tos_zarathustra_word_analysis_capability_v1",
            "available": True,
            "reason": None,
            "provider_ref": WORD_ANALYSIS_PROVIDER_RELATIVE_PATH.as_posix(),
            "publication_posture": "local_full_tree_only",
            "task": task,
            "authority": authority,
        }

    def zarathustra_word_analysis_public_capability(self) -> dict[str, Any]:
        """Describe the public bundle posture without loading the local provider."""
        return _unavailable_word_analysis_capability(
            "local source-bound word-analysis provider is excluded from the public bundle"
        )

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

    def source_gap_search(self, query: str, limit: int = 20) -> dict[str, Any]:
        """Search a bounded public set of authored source-access gap records."""
        normalized_query = str(query).strip()
        if len(normalized_query) > 256:
            raise ValueError("source-gap query exceeds 256 characters")
        needle = normalized_query.casefold()
        bounded_limit = _bounded_int(limit, 20, 1, 100)
        root = self.tos_root.resolve()
        ranked_gaps: list[tuple[int, dict[str, Any]]] = []
        ledger = self.tos_root / SOURCE_GAP_LEDGER_RELATIVE_PATH
        candidates = sorted(ledger.glob("*.access-request.json")) if ledger.is_dir() else []
        for candidate in candidates[:100]:
            relative_path = candidate.relative_to(self.tos_root)
            if candidate.is_symlink():
                raise RuntimeError(f"unsafe public source-gap record: {relative_path.as_posix()}")
            resolved = candidate.resolve()
            if root not in resolved.parents or resolved.stat().st_size > 256_000:
                raise RuntimeError(f"unsafe public source-gap record: {relative_path.as_posix()}")
            record = _read_json(resolved)
            if record.get("schema_version") != "tos_access_request_v1":
                raise RuntimeError(f"unsupported source-gap record: {relative_path.as_posix()}")
            if record.get("personal_or_confidential_data_committed") is True:
                raise RuntimeError(f"source-gap record is not public-safe: {relative_path.as_posix()}")
            if needle and not _contains(record, needle):
                continue
            material = record.get("material") if isinstance(record.get("material"), dict) else {}
            response = record.get("response") if isinstance(record.get("response"), dict) else {}
            tos_refs = _string_list(material.get("tos_refs"))
            request_id = str(record.get("request_id") or relative_path.stem)
            title = str(material.get("title") or request_id)
            source_ref = relative_path.as_posix()
            refs = [
                source_ref,
                *_string_list(material.get("discovery_refs")),
                *_string_list(record.get("rights_record_refs")),
                *_string_list(response.get("safe_evidence_refs")),
            ]
            gap = {
                    "edge_id": f"cluster-relation:source-gap:{request_id}",
                    "from_id": tos_refs[0] if tos_refs else "tos.subject.friedrich-nietzsche",
                    "to_id": request_id,
                    "from_label": str(material.get("edition_or_resource") or (tos_refs[0] if tos_refs else "Tree of Sophia")),
                    "to_label": title,
                    "predicate_id": "source_access_gap",
                    "source_refs": list(dict.fromkeys(refs)),
                    "access_status": str(record.get("access_status") or "unknown"),
                    "request_status": str(record.get("request_status") or "unknown"),
                    "response_state": str(response.get("state") or "none"),
                    "request_sent": bool(record.get("sent_at")),
                    "authority_posture": "source_witness_public_ledger",
                    "review_posture": str(record.get("request_status") or "unknown"),
                    "canon_status": "not_applicable",
                    "confidence": "recorded_status",
                    "properties": {
                        "public_summary_en": (
                            f"ToS records {title} as {record.get('access_status') or 'unknown'}; "
                            f"request status is {record.get('request_status') or 'unknown'} and response state is "
                            f"{response.get('state') or 'none'}."
                        ),
                        "research_purpose": str(record.get("research_purpose") or ""),
                    },
                }
            title_haystack = title.casefold()
            identity_haystack = f"{request_id} {material.get('responsibility') or ''}".casefold()
            score = (8 if needle and needle in title_haystack else 0) + (4 if needle and needle in identity_haystack else 0)
            ranked_gaps.append((score, gap))
        ranked_gaps.sort(key=lambda item: (-item[0], str(item[1].get("to_label") or "").casefold()))
        gaps = [gap for _, gap in ranked_gaps[:bounded_limit]]
        return {
            "schema": "tos_source_gap_search_v1",
            "query": normalized_query,
            "result_count": len(gaps),
            "gaps": gaps,
            "authority_note": (
                "These are recorded source-access gaps in a bounded public runtime set; this is not a "
                "corpus-completeness or legal conclusion. No request is sent and no source or canon is changed."
            ),
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

    def philosophy_epistemic_packet(
        self,
        item_id: str,
        view_id: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return a projection-bounded evidence and challenge field for one selected item."""
        payload = self.philosophy_projection()
        all_nodes, all_edges = _projection_nodes_edges(payload)
        nodes_by_id = {
            str(node.get("node_id")): node
            for node in all_nodes
            if isinstance(node.get("node_id"), str)
        }
        edges_by_id = {
            str(edge.get("edge_id")): edge
            for edge in all_edges
            if isinstance(edge.get("edge_id"), str)
        }
        selection = nodes_by_id.get(item_id) or edges_by_id.get(item_id)
        if selection is None:
            raise KeyError(f"unknown ToS philosophy projection item: {item_id}")

        available_nodes = all_nodes
        available_edges = all_edges
        if view_id:
            view = next(
                (
                    item
                    for item in payload.get("views", [])
                    if isinstance(item, dict) and item.get("view_id") == view_id
                ),
                None,
            )
            if view is None:
                raise KeyError(f"unknown ToS philosophy graph view: {view_id}")
            available_nodes, available_edges = _view_nodes_edges(payload, view)
            available_ids = {
                str(item.get("node_id") or item.get("edge_id") or "")
                for item in [*available_nodes, *available_edges]
            }
            if item_id not in available_ids:
                raise KeyError(f"ToS philosophy projection item is not present in view {view_id}: {item_id}")

        bounded_limit = _bounded_int(limit, 80, 1, 200)
        if item_id in nodes_by_id:
            selected_node_ids = {item_id}
            relation_candidates = [
                edge
                for edge in available_edges
                if edge.get("from_id") == item_id or edge.get("to_id") == item_id
            ]
        else:
            selected_node_ids = {
                str(selection.get("from_id") or ""),
                str(selection.get("to_id") or ""),
            }
            relation_candidates = [
                edge
                for edge in available_edges
                if edge.get("edge_id") == item_id
                or edge.get("from_id") in selected_node_ids
                or edge.get("to_id") in selected_node_ids
            ]

        relation_candidates.sort(
            key=lambda item: (
                str(item.get("edge_id") or "") != item_id,
                str(item.get("edge_id") or ""),
            )
        )
        selected_edge_is_context = (
            item_id in edges_by_id
            and selection.get("predicate_id") not in PHILOSOPHY_CHALLENGE_PREDICATES
        )
        challenge_capacity = bounded_limit - 1 if selected_edge_is_context else bounded_limit
        available_challenge_relations = [
            edge
            for edge in relation_candidates
            if edge.get("predicate_id") in PHILOSOPHY_CHALLENGE_PREDICATES
        ]
        challenge_relations = available_challenge_relations[:challenge_capacity]
        challenge_relations_truncated = len(challenge_relations) < len(available_challenge_relations)
        challenge_ids = {str(edge.get("edge_id") or "") for edge in challenge_relations}
        remaining = max(0, bounded_limit - len(challenge_relations))
        context_relations = [
            edge
            for edge in relation_candidates
            if str(edge.get("edge_id") or "") not in challenge_ids
        ][:remaining]
        selected_relations = [*challenge_relations, *context_relations]

        related_node_ids = {
            str(endpoint)
            for edge in selected_relations
            for endpoint in (edge.get("from_id"), edge.get("to_id"))
            if endpoint
        }
        if item_id in nodes_by_id:
            related_node_ids.discard(item_id)
        neighbor_nodes = [
            node
            for node in available_nodes
            if str(node.get("node_id") or "") in related_node_ids
        ]

        surrounding_items = [*challenge_relations, *context_relations, *neighbor_nodes]
        field_items = [selection, *surrounding_items]
        field_posture_items = [
            item
            for item in surrounding_items
            if str(item.get("node_id") or item.get("edge_id") or "") != item_id
        ]
        selection_properties = (
            selection.get("properties")
            if isinstance(selection.get("properties"), dict)
            else {}
        )
        properties = [
            item.get("properties")
            for item in field_posture_items
            if isinstance(item.get("properties"), dict)
        ]
        authority_postures = sorted({
            str(item.get("authority_posture"))
            for item in properties
            if isinstance(item.get("authority_posture"), str) and item.get("authority_posture")
        })
        canon_statuses = sorted({
            str(item.get("canon_status"))
            for item in properties
            if isinstance(item.get("canon_status"), str) and item.get("canon_status")
        })
        review_postures = sorted({
            str(item.get("review_posture"))
            for item in properties
            if isinstance(item.get("review_posture"), str) and item.get("review_posture")
        })
        confidence_values = sorted({
            str(item.get("confidence") or item.get("master_confidence"))
            for item in properties
            if item.get("confidence") or item.get("master_confidence")
        })
        selection_posture = {
            "authority_posture": selection_properties.get("authority_posture"),
            "canon_status": selection_properties.get("canon_status"),
            "review_posture": selection_properties.get("review_posture"),
            "confidence": selection_properties.get("confidence")
            or selection_properties.get("master_confidence"),
            "priority": selection_properties.get("priority"),
            # Never infer closure from the coincidental presence of IDs or refs.
            "claim_evidence_closed": selection_properties.get("claim_evidence_closed") is True,
        }

        return {
            "schema": "tos_philosophy_epistemic_packet_v1",
            "item_id": item_id,
            "view_id": view_id,
            "selection": selection,
            "challenge_relations": challenge_relations,
            "context_relations": context_relations,
            "neighbor_nodes": neighbor_nodes,
            "selection_posture": selection_posture,
            "field_posture": {
                "authority_postures": authority_postures,
                "canon_statuses": canon_statuses,
                "review_postures": review_postures,
                "confidence_values": confidence_values,
            },
            "coverage": {
                "posture": "partial",
                "challenge_state": (
                    "projected_signals_truncated"
                    if challenge_relations_truncated
                    else "projected_signals"
                    if available_challenge_relations
                    else "none_in_projection_scope"
                ),
                "available_challenge_relations": len(available_challenge_relations),
                "returned_challenge_relations": len(challenge_relations),
                "missing_surfaces": [
                    "claim-level support and counterevidence",
                    "source-visible review decisions",
                    "rights and publication decisions",
                ],
            },
            "authority_boundary": {
                "is_source": False,
                "is_canon": False,
                "is_semantic_truth": False,
                "is_rights_clearance": False,
            },
            "counts": {
                "challenge_relations": len(challenge_relations),
                "available_challenge_relations": len(available_challenge_relations),
                "context_relations": len(context_relations),
                "neighbor_nodes": len(neighbor_nodes),
                "source_refs": len(_source_refs(field_items)),
            },
            "source_refs": _source_refs(field_items),
            "challenge_predicates": sorted(PHILOSOPHY_CHALLENGE_PREDICATES),
            "runtime_projection_boundary": payload.get("runtime_projection_boundary", {}),
            "authority_note": (
                "This packet exposes projected challenge signals and source-return routes. "
                "A contested_by, uncertain_relation, or polemicizes_with candidate is not adjudicated counterevidence; "
                "ToS source, claim, review, rights, and canon owners remain authoritative."
            ),
        }

    def evidence_lens_packet(
        self,
        mode: str,
        item_id: str,
        view_id: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Join a selected graph item to explicit public-safe evidence routes."""
        if mode not in {"philosophy", "corpus"}:
            raise KeyError(f"unsupported ToS Evidence Lens mode: {mode}")
        bounded_limit = _bounded_int(limit, 80, 1, 200)
        if mode == "philosophy":
            context = self.philosophy_epistemic_packet(item_id, view_id=view_id, limit=bounded_limit)
            selection = context["selection"]
            challenge_relations = context["challenge_relations"]
            context_relations = context["context_relations"]
            neighbor_nodes = context["neighbor_nodes"]
            selection_posture = context["selection_posture"]
            field_posture = context["field_posture"]
            coverage = dict(context["coverage"])
            projection_refs = context["source_refs"]
        else:
            selected_view_id = view_id or "route-graph"
            if selected_view_id != "route-graph":
                raise KeyError("corpus Evidence Lens currently supports the route-graph view")
            graph = self.graph_view(selected_view_id, limit=1000)
            nodes = [item for item in graph.get("nodes", []) if isinstance(item, dict)]
            edges = [item for item in graph.get("edges", []) if isinstance(item, dict)]
            selection = next(
                (
                    item
                    for item in [*nodes, *edges]
                    if str(item.get("node_id") or item.get("edge_id") or "") == item_id
                ),
                None,
            )
            if selection is None:
                raise KeyError(f"unknown ToS corpus route-graph item: {item_id}")
            if selection.get("node_id"):
                selected_node_ids = {item_id}
                relation_candidates = [
                    edge
                    for edge in edges
                    if edge.get("from_id") == item_id or edge.get("to_id") == item_id
                ]
            else:
                selected_node_ids = {
                    str(selection.get("from_id") or ""),
                    str(selection.get("to_id") or ""),
                }
                relation_candidates = [
                    edge
                    for edge in edges
                    if edge.get("edge_id") == item_id
                    or edge.get("from_id") in selected_node_ids
                    or edge.get("to_id") in selected_node_ids
                ]
            relation_candidates.sort(
                key=lambda item: (
                    str(item.get("edge_id") or "") != item_id,
                    str(item.get("edge_id") or ""),
                )
            )
            context_relations = relation_candidates[:bounded_limit]
            related_node_ids = {
                str(endpoint)
                for edge in context_relations
                for endpoint in (edge.get("from_id"), edge.get("to_id"))
                if endpoint
            }
            if selection.get("node_id"):
                related_node_ids.discard(item_id)
            neighbor_nodes = [
                node for node in nodes if str(node.get("node_id") or "") in related_node_ids
            ]
            challenge_relations = []
            selection_posture = {
                "authority_posture": selection.get("authority_layer"),
                "canon_status": selection.get("status"),
                "review_posture": None,
                "confidence": selection.get("confidence"),
                "priority": None,
                "claim_evidence_closed": False,
            }
            field_posture = {
                "authority_postures": _unique_values(context_relations, "authority_layer"),
                "canon_statuses": _unique_values(context_relations, "status"),
                "review_postures": [],
                "confidence_values": _unique_values(context_relations, "confidence"),
            }
            coverage = {
                "posture": "partial",
                "challenge_state": "none_in_projection_scope",
                "available_challenge_relations": 0,
                "returned_challenge_relations": 0,
                "missing_surfaces": ["curated Evidence Lens scene lookup pending"],
            }
            projection_refs = _source_refs([selection, *context_relations, *neighbor_nodes])

        evidence = self.evidence_projection()
        scene = next(
            (
                candidate
                for candidate in evidence.get("scenes", [])
                if isinstance(candidate, dict)
                and any(
                    isinstance(route, dict)
                    and route.get("mode") == mode
                    and item_id in route.get("item_ids", [])
                    for route in candidate.get("selections", [])
                )
            ),
            None,
        )
        if scene is None:
            finding = "No curated Evidence Lens route is published for this selection."
            finding_ru = "Для выбранного объекта ещё не опубликован курируемый маршрут Evidence Lens."
            posture = "projection-only"
            conclusion = {
                "can_conclude": False,
                "canon_membership": selection.get("authority_layer") == "canon",
                "claim_evidence_closed": False,
                "allowed": ["inspect the projection context and its source-return references"],
                "allowed_ru": ["исследовать контекст проекции и её ссылки возврата к источникам"],
                "not_allowed": ["infer evidence closure from projection membership"],
                "not_allowed_ru": ["выводить доказательную замкнутость из присутствия в проекции"],
            }
            routes: list[dict[str, Any]] = []
            gaps = ["curated source, review, rights, and claim/evidence routes"]
            gaps_ru = ["курируемые маршруты к source, review, rights и claim/evidence"]
            source_anchors: list[dict[str, Any]] = []
        else:
            finding = str(scene.get("finding") or "")
            finding_ru = str(scene.get("finding_ru") or finding)
            posture = str(scene.get("posture") or "")
            conclusion = dict(scene.get("conclusion") or {})
            routes = [dict(route) for route in scene.get("routes", []) if isinstance(route, dict)]
            gaps = [str(gap) for gap in scene.get("gaps", [])]
            gaps_ru = [str(gap) for gap in scene.get("gaps_ru", gaps)]
            source_anchors = [
                dict(anchor) for anchor in scene.get("source_anchors", []) if isinstance(anchor, dict)
            ]
            coverage["missing_surfaces"] = gaps
            coverage["posture"] = "curated-route"

        route_counts: dict[str, int] = {}
        for route in routes:
            route_kind = str(route.get("route_kind") or "other")
            route_counts[route_kind] = route_counts.get(route_kind, 0) + 1
        source_refs = sorted(
            {
                *projection_refs,
                *(str(ref) for ref in (scene or {}).get("source_refs", []) if isinstance(ref, str)),
            }
        )
        agent_summary = {
            "selection": item_id,
            "finding": finding,
            "finding_ru": finding_ru,
            "posture": posture,
            "can_conclude": conclusion.get("can_conclude") is True,
            "canon_membership": conclusion.get("canon_membership") is True,
            "claim_evidence_closed": conclusion.get("claim_evidence_closed") is True,
            "route_counts": route_counts,
            "gap_count": len(gaps),
            "page_updated": True,
            "next_actions": [
                "inspect the full route cards on the page",
                "open the referenced owner surface before making a stronger claim",
            ],
        }
        return {
            "schema": "tos_evidence_lens_packet_v1",
            "mode": mode,
            "item_id": item_id,
            "view_id": view_id,
            "selection": selection,
            "scene": scene,
            "finding": finding,
            "finding_ru": finding_ru,
            "posture": posture,
            "conclusion": conclusion,
            "source_anchors": source_anchors,
            "routes": routes,
            "gaps": gaps,
            "gaps_ru": gaps_ru,
            "challenge_relations": challenge_relations,
            "context_relations": context_relations,
            "neighbor_nodes": neighbor_nodes,
            "selection_posture": selection_posture,
            "field_posture": field_posture,
            "coverage": coverage,
            "counts": {
                "routes": len(routes),
                "source_anchors": len(source_anchors),
                "gaps": len(gaps),
                "challenge_relations": len(challenge_relations),
                "context_relations": len(context_relations),
            },
            "source_refs": source_refs,
            "authority_boundary": evidence.get("authority_boundary", {}),
            "authority_note": evidence.get("authority_boundary", {}).get("note", ""),
            "agent_summary": agent_summary,
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
        direction: str = "outgoing",
        view_id: str | None = None,
        excluded_edge_ids: list[str] | None = None,
        alternative_limit: int = 1,
    ) -> dict[str, Any]:
        payload = self.philosophy_projection()
        nodes, edges = _projection_nodes_edges(payload)
        all_nodes_by_id = {
            str(node.get("node_id")): node
            for node in nodes
            if isinstance(node.get("node_id"), str)
        }
        if from_id not in all_nodes_by_id:
            raise KeyError(f"unknown ToS philosophy node: {from_id}")
        if to_id not in all_nodes_by_id:
            raise KeyError(f"unknown ToS philosophy node: {to_id}")
        direction = str(direction or "outgoing").strip().lower()
        if direction not in {"outgoing", "incoming", "either"}:
            raise ValueError("direction must be outgoing, incoming, or either")
        depth_limit = _bounded_int(max_depth, 6, 1, 8)
        path_limit = _bounded_int(alternative_limit, 1, 1, 5)
        layer_filter = set(layers or [])
        predicate_filter = set(predicates or [])
        excluded_edges = {str(edge_id) for edge_id in (excluded_edge_ids or []) if str(edge_id)}

        available_nodes = nodes
        available_edges = edges
        if view_id:
            view = next(
                (item for item in payload.get("views", []) if isinstance(item, dict) and item.get("view_id") == view_id),
                None,
            )
            if view is None:
                raise KeyError(f"unknown ToS philosophy graph view: {view_id}")
            available_nodes, available_edges = _view_nodes_edges(payload, view)
        available_node_ids = {
            str(node.get("node_id"))
            for node in available_nodes
            if isinstance(node.get("node_id"), str)
        }

        adjacency: dict[str, list[tuple[str, dict[str, Any], str]]] = {}
        for edge in sorted(
            available_edges,
            key=lambda item: (
                str(item.get("edge_id") or ""),
                str(item.get("from_id") or ""),
                str(item.get("to_id") or ""),
            ),
        ):
            if (
                not _layer_allowed(edge, layer_filter)
                or not _predicate_allowed(edge, predicate_filter)
            ):
                continue
            left = str(edge.get("from_id") or "")
            right = str(edge.get("to_id") or "")
            edge_id = str(edge.get("edge_id") or "")
            if edge_id in excluded_edges or left not in available_node_ids or right not in available_node_ids:
                continue
            if direction in {"outgoing", "either"}:
                adjacency.setdefault(left, []).append((right, edge, "forward"))
            if direction in {"incoming", "either"} and (left != right or direction == "incoming"):
                adjacency.setdefault(right, []).append((left, edge, "reverse"))

        queue = deque([(from_id, [from_id], [], [])])
        paths: list[dict[str, Any]] = []
        explored_states = 0
        enqueued_states = 1
        max_frontier_size = 1
        exploration_truncated = False
        while queue and len(paths) < path_limit:
            if explored_states >= PHILOSOPHY_PATH_STATE_LIMIT:
                exploration_truncated = True
                break
            current, path_node_ids, path_edges, traversal = queue.popleft()
            explored_states += 1
            if current == to_id:
                path_nodes = [all_nodes_by_id[node_id] for node_id in path_node_ids]
                paths.append(
                    {
                        "path_index": len(paths),
                        "node_ids": path_node_ids,
                        "edge_ids": [str(edge.get("edge_id") or "") for edge in path_edges],
                        "nodes": path_nodes,
                        "edges": path_edges,
                        "traversal": traversal,
                        "source_refs": _source_refs(path_nodes + path_edges),
                    }
                )
                continue
            if len(path_edges) >= depth_limit:
                continue
            for neighbor, edge, traversal_direction in adjacency.get(current, []):
                if neighbor in path_node_ids:
                    continue
                if (
                    enqueued_states >= PHILOSOPHY_PATH_STATE_LIMIT
                    or len(queue) >= PHILOSOPHY_PATH_FRONTIER_LIMIT
                ):
                    exploration_truncated = True
                    break
                queue.append(
                    (
                        neighbor,
                        [*path_node_ids, neighbor],
                        [*path_edges, edge],
                        [
                            *traversal,
                            {
                                "edge_id": edge.get("edge_id"),
                                "from_node_id": current,
                                "to_node_id": neighbor,
                                "edge_direction": traversal_direction,
                            },
                        ],
                    )
                )
                enqueued_states += 1
                max_frontier_size = max(max_frontier_size, len(queue))

        primary = paths[0] if paths else {"nodes": [], "edges": []}
        source_items = [
            item
            for path in paths
            for collection in (path["nodes"], path["edges"])
            for item in collection
        ]
        return {
            "schema": "tos_philosophy_mcp_path_v2",
            "from_id": from_id,
            "to_id": to_id,
            "found": bool(paths),
            "path_count": len(paths),
            "paths": paths,
            "nodes": primary["nodes"],
            "edges": primary["edges"],
            "max_depth": depth_limit,
            "direction": direction,
            "view_id": view_id,
            "excluded_edge_ids": sorted(excluded_edges),
            "alternative_limit": path_limit,
            "exploration_truncated": exploration_truncated,
            "explored_state_count": explored_states,
            "enqueued_state_count": enqueued_states,
            "frontier_limit": PHILOSOPHY_PATH_FRONTIER_LIMIT,
            "max_frontier_size": max_frontier_size,
            "layers": sorted(layer_filter),
            "predicates": sorted(predicate_filter),
            "source_refs": _source_refs(source_items),
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
