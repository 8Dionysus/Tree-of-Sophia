from __future__ import annotations

import copy
import base64
import binascii
import hashlib
import importlib.util
import json
import os
import sys
import tempfile
from collections import OrderedDict, deque
from dataclasses import dataclass, field
from threading import Lock, RLock
from pathlib import Path
from typing import Any

from .knowledge import (
    AddressedUpdateError,
    KnowledgeGraphIndex,
    KnowledgeSearchIndex,
    KNOWLEDGE_SOURCES,
    addressed_update_knowledge_graph,
    build_knowledge_graph,
    execute_knowledge_lens,
    focus_knowledge_node,
    inspect_knowledge_node,
    inspect_knowledge_relation,
    knowledge_catalog as build_knowledge_catalog,
    knowledge_source_revision,
    search_knowledge_graph,
)
from .search_read_model import (
    SEARCH_READ_MODEL_DEFAULT_BYTES,
    SEARCH_READ_MODEL_MAX_POSTINGS,
    SEARCH_READ_MODEL_MAX_VERIFY_CHARS,
    SEARCH_READ_MODEL_PAGE_SIZE,
    SearchReadModelError,
    SearchReadModelPage,
    SearchReadModelSnapshotError,
    SQLiteKnowledgeSearchReadModel,
    normalize_search_query,
)
from .exploration import ExplorationService, exploration_capabilities
from .temporal_comparison import compare_temporal_claims
from .published_read_model import PublishedKnowledgeReadModel, PublishedReadModelError
from .published_exploration import PublishedExplorationService
from .published_lens import PublishedLensService
from .source_read import SourceReadError, SourceReadService, contract_summary, unavailable_capabilities
from .source_read_owner import SelectedSourceReadService
from .query_store import QueryStore, QueryStoreRequired, DEFAULT_RELATIVE_PATH
from .projection_store import load_projection
from .locations import data_root, program_path


INDEX_RELATIVE_PATH = Path("ToS/derived-exports/tos_corpus_index.min.json")
PHILOSOPHY_PROJECTION_RELATIVE_PATH = Path("ToS/derived-exports/philosophy_graph_projection.min.json")
BIBLIOGRAPHIC_GRAPH_RELATIVE_PATH = Path(
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"
)
ENTITY_TYPE_REGISTRY_RELATIVE_PATH = Path(
    "ToS/doctrine/semantic-interchange/entity-types.v1.json"
)
RELATION_TYPE_REGISTRY_RELATIVE_PATH = Path(
    "ToS/doctrine/semantic-interchange/relation-types.v1.json"
)
PHILOSOPHY_AUDIT_RELATIVE_PATH = Path("ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json")
EVIDENCE_PROJECTION_RELATIVE_PATH = Path("ToS/derived-exports/epistemic_evidence_projection.min.json")
WORD_ANALYSIS_PROVIDER_RELATIVE_PATH = Path("scripts/prepare_zarathustra_word_analysis_v1.py")
SOURCE_GAP_LEDGER_RELATIVE_PATH = Path("ToS/source-witnesses/access-requests/public-ledger")
SOURCE_READ_CONTRACT_RELATIVE_PATH = Path("access/contracts/source-read.v1.schema.json")
KNOWLEDGE_CONTRACT_RELATIVE_PATHS = {
    "api": Path("access/contracts/knowledge-api.v1.json"),
    "knowledge_graph": Path("access/contracts/knowledge-graph.v1.schema.json"),
    "readable_context": Path("access/contracts/readable-context.v1.schema.json"),
    "lens_spec": Path("access/contracts/lens-spec.v1.schema.json"),
    "lens_result": Path("access/contracts/lens-result.v1.schema.json"),
    "temporal_comparison_request": Path("access/contracts/temporal-comparison-request.v1.schema.json"),
    "temporal_comparison_result": Path("access/contracts/temporal-comparison-result.v1.schema.json"),
    "source_read": SOURCE_READ_CONTRACT_RELATIVE_PATH,
    "entity_type_registry_schema": Path(
        "ToS/contracts/semantic-entity-type-registry.schema.json"
    ),
    "relation_type_registry_schema": Path(
        "ToS/contracts/semantic-relation-type-registry.schema.json"
    ),
    "entity_type_registry": ENTITY_TYPE_REGISTRY_RELATIVE_PATH,
    "relation_type_registry": RELATION_TYPE_REGISTRY_RELATIVE_PATH,
}
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
JSON_VERSION_CACHE_MAX_PATHS = 32
_KNOWLEDGE_SCHEMA_RULES = {
    "philosophy": (
        {"tos_philosophy_graph_projection_v1", "tos_philosophy_graph_projection_v2"},
        "ToS philosophy graph projection schema_version must be one of "
        "tos_philosophy_graph_projection_v1, tos_philosophy_graph_projection_v2",
    ),
    "entity_type_registry": ({"tos_semantic_entity_type_registry_v1"},
        "ToS entity type registry schema_version must be tos_semantic_entity_type_registry_v1"),
    "relation_type_registry": ({"tos_semantic_relation_type_registry_v1"},
        "ToS relation type registry schema_version must be tos_semantic_relation_type_registry_v1"),
}
SEARCH_READ_MODEL_DEFAULT_MAX_BYTES = 512 * 1024 * 1024
SEARCH_READ_MODEL_CURSOR_SCHEMA = "tos_knowledge_search_indexed_cursor_v1"
_json_version_cache_lock = Lock()
_json_version_cache: OrderedDict[
    str, tuple[tuple[int, int, int, int], dict[str, Any]]
] = OrderedDict()


def _default_search_read_model_path(root: Path) -> Path:
    """Choose a restart-surviving cache path without writing into the repo."""
    root_digest = hashlib.sha256(root.resolve().as_posix().encode("utf-8")).hexdigest()[:24]
    return Path(tempfile.gettempdir()) / "tos-access-search" / f"{root_digest}.sqlite"


def _indexed_search_cursor_encode(payload: dict[str, Any]) -> str:
    raw = json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return base64.urlsafe_b64encode(raw).decode("ascii").rstrip("=")


def _indexed_search_cursor_decode(value: str) -> dict[str, Any]:
    if not isinstance(value, str) or not value or len(value) > 8192:
        raise SearchReadModelError("invalid indexed knowledge search cursor")
    try:
        padded = value + "=" * (-len(value) % 4)
        decoded = json.loads(base64.urlsafe_b64decode(padded.encode("ascii")))
    except (ValueError, UnicodeError, binascii.Error, json.JSONDecodeError) as error:
        raise SearchReadModelError("invalid indexed knowledge search cursor") from error
    if not isinstance(decoded, dict) or decoded.get("schema") != SEARCH_READ_MODEL_CURSOR_SCHEMA:
        raise SearchReadModelError("invalid indexed knowledge search cursor")
    return decoded


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


def _read_json_file(path: Path) -> dict[str, Any]:
    """Read a selected carrier without registering or evicting process caches."""
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError(f"ToS corpus index is not a JSON object: {path}")
    return payload


def _checked_knowledge_schema(payload: dict[str, Any], name: str) -> dict[str, Any]:
    allowed, message = _KNOWLEDGE_SCHEMA_RULES[name]
    if payload.get("schema_version") not in allowed:
        raise RuntimeError(message)
    return payload


def _read_json_version(
    path_text: str, mtime_ns: int, size: int, inode: int, ctime_ns: int
) -> dict[str, Any]:
    """Read one latest version of a JSON carrier without retaining history.

    The graph itself is retained by ``ToSAccessCore`` only as the currently
    published snapshot.  A process-wide multi-version LRU here would keep old
    raw carriers alive after supersession and multiply the graph's memory
    footprint across source edits.  Keep at most one parsed payload per path;
    a source-state change replaces that entry atomically.  The state key keeps
    same-size/same-mtime rewrites honest through inode and ctime changes.
    """
    state = (mtime_ns, size, inode, ctime_ns)
    with _json_version_cache_lock:
        cached = _json_version_cache.get(path_text)
        if cached is not None and cached[0] == state:
            _json_version_cache.move_to_end(path_text)
            return cached[1]
    path = Path(path_text)
    payload = _read_json_file(path)
    with _json_version_cache_lock:
        _json_version_cache[path_text] = (state, payload)
        _json_version_cache.move_to_end(path_text)
        while len(_json_version_cache) > JSON_VERSION_CACHE_MAX_PATHS:
            _json_version_cache.popitem(last=False)
    return payload


def _read_json(path: Path) -> dict[str, Any]:
    stat = path.stat()
    return _read_json_version(
        path.resolve().as_posix(), stat.st_mtime_ns, stat.st_size, stat.st_ino, stat.st_ctime_ns
    )


def _knowledge_graph_version(
    index_path_text: str,
    index_mtime_ns: int,
    index_size: int,
    index_inode: int,
    index_ctime_ns: int,
    philosophy_path_text: str,
    philosophy_mtime_ns: int,
    philosophy_size: int,
    philosophy_inode: int,
    philosophy_ctime_ns: int,
    bibliographic_path_text: str,
    bibliographic_mtime_ns: int,
    bibliographic_size: int,
    bibliographic_inode: int,
    bibliographic_ctime_ns: int,
    entity_registry_path_text: str,
    entity_registry_mtime_ns: int,
    entity_registry_size: int,
    entity_registry_inode: int,
    entity_registry_ctime_ns: int,
    relation_registry_path_text: str,
    relation_registry_mtime_ns: int,
    relation_registry_size: int,
    relation_registry_inode: int,
    relation_registry_ctime_ns: int,
) -> dict[str, Any]:
    corpus = _read_json_version(
        index_path_text, index_mtime_ns, index_size, index_inode, index_ctime_ns
    )
    philosophy = _read_json_version(
        philosophy_path_text,
        philosophy_mtime_ns,
        philosophy_size,
        philosophy_inode,
        philosophy_ctime_ns,
    )
    bibliographic = _read_json_version(
        bibliographic_path_text,
        bibliographic_mtime_ns,
        bibliographic_size,
        bibliographic_inode,
        bibliographic_ctime_ns,
    )
    entity_registry = _read_json_version(
        entity_registry_path_text,
        entity_registry_mtime_ns,
        entity_registry_size,
        entity_registry_inode,
        entity_registry_ctime_ns,
    )
    relation_registry = _read_json_version(
        relation_registry_path_text,
        relation_registry_mtime_ns,
        relation_registry_size,
        relation_registry_inode,
        relation_registry_ctime_ns,
    )
    return build_knowledge_graph(
        corpus,
        philosophy,
        bibliographic,
        entity_registry,
        relation_registry,
    )




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
    return data_root(explicit)


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
    bibliographic_graph_path: Path
    entity_type_registry_path: Path
    relation_type_registry_path: Path
    philosophy_post_planting_audit_path: Path
    evidence_projection_path: Path
    search_read_model_path: Path | None = None
    search_read_model_max_bytes: int = SEARCH_READ_MODEL_DEFAULT_MAX_BYTES
    search_read_model_max_postings: int = SEARCH_READ_MODEL_MAX_POSTINGS
    search_read_model_max_verify_chars: int = SEARCH_READ_MODEL_MAX_VERIFY_CHARS
    published_read_model_path: Path | None = None
    published_read_model_expected: dict[str, Any] | None = None
    published_exploration_checkpoint_path: Path | None = None
    source_read_service: SourceReadService | SelectedSourceReadService | None = None
    _prepared_reader: PublishedKnowledgeReadModel | None = field(default=None, init=False, repr=False, compare=False)
    _prepared_lens: PublishedLensService | None = field(default=None, init=False, repr=False, compare=False)
    _exploration: ExplorationService = field(init=False, repr=False, compare=False)
    _search_index: KnowledgeSearchIndex | None = field(default=None, init=False, repr=False, compare=False)
    _search_lock: Any = field(default_factory=Lock, init=False, repr=False, compare=False)
    _search_read_model: SQLiteKnowledgeSearchReadModel | None = field(default=None, init=False, repr=False, compare=False)
    _graph_index: KnowledgeGraphIndex | None = field(default=None, init=False, repr=False, compare=False)
    _graph_index_lock: Any = field(default_factory=Lock, init=False, repr=False, compare=False)
    _snapshot_lock: Any = field(default_factory=RLock, init=False, repr=False, compare=False)
    # The last graph returned to consumers is the CAS parent.  It is kept
    # separately from the on-disk source state because an owner may edit one
    # source carrier first and then publish its bounded replacement without
    # forcing an unrelated full rebuild before the addressed call.
    _published_graph: dict[str, Any] | None = field(default=None, init=False, repr=False, compare=False)
    _published_source_inputs: dict[str, bytes] | None = field(
        default=None, init=False, repr=False, compare=False
    )
    _published_catalog: dict[str, Any] | None = field(
        default=None, init=False, repr=False, compare=False
    )
    _published_catalog_graph: dict[str, Any] | None = field(
        default=None, init=False, repr=False, compare=False
    )
    _published_catalog_source_state: tuple[tuple[str, int, int, int, int], ...] | None = field(
        default=None, init=False, repr=False, compare=False
    )
    _addressed_graph: dict[str, Any] | None = field(default=None, init=False, repr=False, compare=False)
    _addressed_source_state: tuple[tuple[str, int, int, int, int], ...] | None = field(
        default=None, init=False, repr=False, compare=False
    )
    _published_source_state: tuple[tuple[str, int, int, int, int], ...] | None = field(
        default=None, init=False, repr=False, compare=False
    )

    _query_backend: Any = field(default=None, init=False, repr=False, compare=False)
    _query_signature: Any = field(default=None, init=False, repr=False, compare=False)
    _query_lock: Any = field(default_factory=Lock, init=False, repr=False, compare=False)

    def __post_init__(self):
        if (self.published_read_model_path is None) != (self.published_read_model_expected is None):
            raise ValueError("prepared reader requires both a path and an exact expected snapshot binding")
        if self.published_exploration_checkpoint_path is not None and self.published_read_model_path is None:
            raise ValueError("persistent exploration checkpoints require an explicitly selected prepared reader")
        if self.source_read_service is not None and not isinstance(self.source_read_service, (SourceReadService, SelectedSourceReadService)):
            raise TypeError("source_read_service must be an explicit SourceReadService")
        if self.published_read_model_path is not None:
            path = Path(self.published_read_model_path).expanduser()
            if not path.is_absolute():
                path = self.tos_root / path
            self._prepared_reader = PublishedKnowledgeReadModel(path, self.published_read_model_expected)
            self._prepared_lens = PublishedLensService(self._prepared_reader)
        if self.search_read_model_path is None:
            self.search_read_model_path = _default_search_read_model_path(self.tos_root)
        else:
            self.search_read_model_path = Path(self.search_read_model_path).expanduser()
            if not self.search_read_model_path.is_absolute():
                self.search_read_model_path = self.tos_root / self.search_read_model_path
        if self.search_read_model_max_bytes < 4096:
            raise ValueError("search_read_model_max_bytes must be at least one SQLite page")
        if self._prepared_reader is None:
            self._exploration = ExplorationService(self.knowledge_graph, query_store_provider=self._query_store)
        else:
            checkpoint = self.published_exploration_checkpoint_path
            if checkpoint is not None:
                checkpoint = Path(checkpoint).expanduser()
                if not checkpoint.is_absolute():
                    checkpoint = self.tos_root / checkpoint
            self._exploration = PublishedExplorationService(self._prepared_reader, checkpoint_path=checkpoint)

    def _query_store(self):
        """Select an explicit completed snapshot, never build during a request."""
        if self._prepared_reader is not None:
            return None
        configured = os.environ.get('TOS_QUERY_STORE_PATH')
        path = Path(configured).expanduser() if configured else self.tos_root / DEFAULT_RELATIVE_PATH
        if not path.is_absolute():
            path = self.tos_root / path
        inputs = {
            INDEX_RELATIVE_PATH.as_posix(): self.index_path,
            PHILOSOPHY_PROJECTION_RELATIVE_PATH.as_posix(): self.philosophy_graph_projection_path,
            BIBLIOGRAPHIC_GRAPH_RELATIVE_PATH.as_posix(): self.bibliographic_graph_path,
            ENTITY_TYPE_REGISTRY_RELATIVE_PATH.as_posix(): self.entity_type_registry_path,
            RELATION_TYPE_REGISTRY_RELATIVE_PATH.as_posix(): self.relation_type_registry_path,
        }
        # Legacy small fixtures retain their existing explicit in-memory mode.
        # A partitioned root requires the compiled store even if it is missing.
        partitioned = False
        for candidate in (self.index_path, self.bibliographic_graph_path):
            if candidate.is_file() and candidate.stat().st_size < 4 * 1024 * 1024:
                header = _read_json(candidate)
                if header.get('schema_version') == 'tos_partitioned_projection_v1' or header.get('schema') == 'tos_partitioned_projection_v1':
                    partitioned = True
        if not configured and not path.is_file() and not partitioned:
            return None
        if not path.is_file():
            raise QueryStoreRequired(f'query store build required: missing {path}; run explicit offline builder')
        try:
            signature = tuple((str(p.resolve()), st.st_ino, st.st_mtime_ns, st.st_ctime_ns, st.st_size)
                              for p in [path, *inputs.values()] for st in [p.stat()])
        except FileNotFoundError as exc:
            raise QueryStoreRequired('query store build required: snapshot input is missing') from exc
        with self._query_lock:
            if signature != self._query_signature:
                bindings = {}
                for name, candidate in inputs.items():
                    digest = hashlib.sha256()
                    with candidate.open('rb') as source:
                        for chunk in iter(lambda: source.read(1024 * 1024), b''):
                            digest.update(chunk)
                    bindings[name] = digest.hexdigest()
                self._query_backend = QueryStore(path, snapshot_bindings=bindings)
                self._query_signature = signature
            return self._query_backend

    def knowledge_explore(self, request: dict[str, Any]) -> dict[str, Any]:
        return self._exploration.explore(request)

    def knowledge_prepared_status(self) -> dict[str, Any] | None:
        """Check selected publication readiness, without scanning source rows."""
        return self._prepared_reader.status() if self._prepared_reader is not None else None

    def knowledge_exploration_capabilities(self) -> dict[str, Any]:
        return (self._exploration.capability() if self._prepared_reader is not None
                else exploration_capabilities())

    def knowledge_exploration_contracts(self) -> dict[str, Any]:
        return {
            "capabilities": self.knowledge_exploration_capabilities(),
            "request": _read_json(program_path("access/contracts/exploration-request.v1.schema.json")),
            "result": _read_json(program_path("access/contracts/exploration-result.v1.schema.json")),
            "request_v2": _read_json(program_path("access/contracts/exploration-request.v2.schema.json")),
            "result_v2": _read_json(program_path("access/contracts/exploration-result.v2.schema.json")),
        }

    @classmethod
    def discover(
        cls,
        tos_root: str | Path | None = None,
        index_path: str | Path | None = None,
        philosophy_graph_projection_path: str | Path | None = None,
        bibliographic_graph_path: str | Path | None = None,
        entity_type_registry_path: str | Path | None = None,
        relation_type_registry_path: str | Path | None = None,
        philosophy_post_planting_audit_path: str | Path | None = None,
        evidence_projection_path: str | Path | None = None,
        search_read_model_path: str | Path | None = None,
        search_read_model_max_bytes: int | None = None,
        search_read_model_max_postings: int = SEARCH_READ_MODEL_MAX_POSTINGS,
        search_read_model_max_verify_chars: int = SEARCH_READ_MODEL_MAX_VERIFY_CHARS,
        published_read_model_path: str | Path | None = None,
        published_read_model_expected: dict[str, Any] | None = None,
        published_exploration_checkpoint_path: str | Path | None = None,
        source_read_service: SourceReadService | SelectedSourceReadService | None = None,
    ) -> "ToSAccessCore":
        """Select legacy carrier reads, or explicitly pin the prepared reader.

        The prepared route serves catalog, full node/relation inspection,
        bounded exploration, and v9 lens/focus. Other knowledge operations refuse instead of silently
        rebuilding the graph. No environment variable activates this opt-in.
        """
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
        bibliographic_graph = Path(
            bibliographic_graph_path
            or os.environ.get("TOS_BIBLIOGRAPHIC_GRAPH_PATH")
            or root / BIBLIOGRAPHIC_GRAPH_RELATIVE_PATH
        ).expanduser()
        if not bibliographic_graph.is_absolute():
            bibliographic_graph = root / bibliographic_graph
        entity_registry = Path(
            entity_type_registry_path
            or os.environ.get("TOS_ENTITY_TYPE_REGISTRY_PATH")
            or root / ENTITY_TYPE_REGISTRY_RELATIVE_PATH
        ).expanduser()
        if not entity_registry.is_absolute():
            entity_registry = root / entity_registry
        relation_registry = Path(
            relation_type_registry_path
            or os.environ.get("TOS_RELATION_TYPE_REGISTRY_PATH")
            or root / RELATION_TYPE_REGISTRY_RELATIVE_PATH
        ).expanduser()
        if not relation_registry.is_absolute():
            relation_registry = root / relation_registry
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
        search_path = Path(
            search_read_model_path
            or os.environ.get("TOS_SEARCH_READ_MODEL_PATH")
            or _default_search_read_model_path(root)
        ).expanduser()
        if not search_path.is_absolute():
            search_path = root / search_path
        configured_budget = search_read_model_max_bytes
        if configured_budget is None:
            raw_budget = os.environ.get("TOS_SEARCH_READ_MODEL_MAX_BYTES")
            configured_budget = int(raw_budget) if raw_budget else SEARCH_READ_MODEL_DEFAULT_MAX_BYTES
        return cls(
            tos_root=root,
            index_path=index.resolve(),
            philosophy_graph_projection_path=philosophy_projection.resolve(),
            bibliographic_graph_path=bibliographic_graph.resolve(),
            entity_type_registry_path=entity_registry.resolve(),
            relation_type_registry_path=relation_registry.resolve(),
            philosophy_post_planting_audit_path=philosophy_audit.resolve(),
            evidence_projection_path=evidence_projection.resolve(),
            search_read_model_path=search_path.resolve(),
            search_read_model_max_bytes=configured_budget,
            search_read_model_max_postings=search_read_model_max_postings,
            search_read_model_max_verify_chars=search_read_model_max_verify_chars,
            published_read_model_path=Path(published_read_model_path) if published_read_model_path is not None else None,
            published_read_model_expected=published_read_model_expected,
            published_exploration_checkpoint_path=(Path(published_exploration_checkpoint_path)
                                                  if published_exploration_checkpoint_path is not None else None),
            source_read_service=source_read_service,
        )

    def index_exists(self) -> bool:
        return self.index_path.is_file()

    def index(self) -> dict[str, Any]:
        """Explicit full corpus export; selective query routes avoid this call."""
        return load_projection(self.index_path)

    def source_navigation(self, *, bibliographic_only: bool = False) -> dict[str, Any]:
        navigation = self.index().get("source_navigation")
        if not isinstance(navigation, dict):
            raise RuntimeError("ToS corpus index has no source_navigation surface")
        if navigation.get("schema_version") != "tos_source_navigation_v1":
            raise RuntimeError("ToS source_navigation schema_version must be tos_source_navigation_v1")
        if not bibliographic_only:
            return navigation
        # Legacy source descent/dossiers browse bibliographic identity. Dense
        # versioned text packet members use indexed knowledge routes instead.
        nodes = [n for n in navigation.get('nodes', []) if not (n.get('properties') or {}).get('packet_id')]
        ids = {n['node_id'] for n in nodes}
        edges = [e for e in navigation.get('edges', []) if e['from_id'] in ids and e['to_id'] in ids]
        return {**navigation, 'nodes': nodes, 'edges': edges,
                'counts': {**navigation.get('counts', {}), 'nodes': len(nodes), 'edges': len(edges)}}

    def source_descend(
        self,
        node_id: str,
        max_depth: int = 8,
        limit: int = 300,
    ) -> dict[str, Any]:
        """Walk downward through the authored source-navigation projection."""

        store = self._query_store()
        navigation = store.metadata['source_navigation_header'] if store else self.source_navigation(bibliographic_only=True)
        bounded_depth = _bounded_int(max_depth, 8, 1, 8)
        bounded_limit = _bounded_int(limit, 300, 1, 300)
        if store:
            nodes_by_id = store.source_nodes()
            outgoing = store.source_edges('outgoing')
        else:
            nodes_by_id = {str(node['node_id']): node for node in navigation.get('nodes', [])}
            outgoing = {}
            for edge in navigation.get('edges', []):
                outgoing.setdefault(str(edge['from_id']), []).append(edge)
            for edges in outgoing.values():
                edges.sort(key=lambda edge: str(edge.get('edge_id') or ''))
        if node_id not in nodes_by_id:
            raise KeyError(f'unknown ToS source-navigation node: {node_id}')

        queue: deque[tuple[str, int]] = deque([(node_id, 0)])
        depths = {node_id: 0}
        selected_edges: list[dict[str, Any]] = []
        truncated = False
        while queue:
            current, depth = queue.popleft()
            if depth >= bounded_depth:
                continue
            for edge in outgoing.get(current, []):
                target = str(edge.get("to_id") or "")
                if target not in nodes_by_id:
                    continue
                if target not in depths and len(depths) >= bounded_limit:
                    truncated = True
                    continue
                if edge not in selected_edges:
                    selected_edges.append(edge)
                if target not in depths:
                    depths[target] = depth + 1
                    queue.append((target, depth + 1))
        selected_nodes = [
            {**nodes_by_id[selected_id], "depth": depth}
            for selected_id, depth in sorted(depths.items(), key=lambda item: (item[1], item[0]))
        ]
        return {
            "schema": "tos_source_descent_v1",
            "root_id": node_id,
            "max_depth": bounded_depth,
            "limit": bounded_limit,
            "truncated": truncated,
            "counts": {"nodes": len(selected_nodes), "edges": len(selected_edges)},
            "nodes": selected_nodes,
            "edges": selected_edges,
            "authority_note": navigation.get("authority_boundary"),
        }

    def source_dossier(self, object_id: str, limit: int = 300) -> dict[str, Any]:
        """Return compact human and agent-facing context for one bibliographic object or Link."""

        store = self._query_store()
        navigation = store.metadata['source_navigation_header'] if store else self.source_navigation(bibliographic_only=True)
        bounded_limit = _bounded_int(limit, 300, 1, 300)
        nodes_by_id = store.source_nodes() if store else {
            str(node['node_id']): node for node in navigation.get('nodes', [])
        }
        selected = nodes_by_id.get(object_id)
        if selected is None:
            raise KeyError(f"unknown ToS dossier object: {object_id}")
        if selected.get("node_kind") not in {"work", "expression", "edition", "item", "file", "link"}:
            raise ValueError("dossiers are available for Work, Expression, Edition, Item, File, and Link objects")

        bibliographic_predicates = {"has_expression", "embodied_by", "exemplified_by"}
        link_predicates = {"described_by", "metadata_at", "downloadable_at", "rights_statement_at"}
        if store:
            incoming = store.source_edges('incoming')
            semantic_outgoing = store.source_edges('outgoing', semantic=True)
        else:
            all_edges = sorted(
                [edge for edge in navigation.get("edges", []) if isinstance(edge, dict)],
                key=lambda edge: str(edge.get("edge_id") or ""),
            )
            incoming: dict[str, list[dict[str, Any]]] = {}
            semantic_outgoing: dict[str, list[dict[str, Any]]] = {}
            bibliographic_predicates = {"has_expression", "embodied_by", "exemplified_by"}
            link_predicates = {"described_by", "metadata_at", "downloadable_at", "rights_statement_at"}
            for edge in all_edges:
                left = edge.get("from_id")
                right = edge.get("to_id")
                if not isinstance(left, str) or not isinstance(right, str):
                    continue
                incoming.setdefault(right, []).append(edge)
                if edge.get("edge_kind") == "authored_item_manifest" or (
                    edge.get("edge_kind") == "evidence_claim"
                    and edge.get("predicate_id") in bibliographic_predicates | link_predicates
                ):
                    semantic_outgoing.setdefault(left, []).append(edge)

        component_ids = {object_id}
        component_edges: dict[str, dict[str, Any]] = {}
        truncated = False

        def admit(node_id: str) -> bool:
            nonlocal truncated
            if node_id in component_ids:
                return True
            if node_id not in nodes_by_id:
                return False
            if len(component_ids) >= bounded_limit:
                truncated = True
                return False
            component_ids.add(node_id)
            return True

        # A Link dossier first climbs only its asserted bibliographic lineage
        # to the owning Work. A Work dossier already has its root and never
        # walks backward through a shared Item into neighboring Works.
        forward_roots = {object_id} if selected.get("node_kind") == "work" else set()
        if selected.get("node_kind") != "work":
            lineage_queue: deque[str] = deque([object_id])
            visited_lineage: set[str] = set()
            while lineage_queue:
                current = lineage_queue.popleft()
                if current in visited_lineage:
                    continue
                visited_lineage.add(current)
                current_kind = nodes_by_id[current].get("node_kind")
                if current_kind == "work":
                    forward_roots.add(current)
                    continue
                allowed_predicates = link_predicates if current_kind == "link" else bibliographic_predicates
                for edge in incoming.get(current, []):
                    structural_file_parent = current_kind == "file" and edge.get("edge_kind") == "authored_item_manifest"
                    if not structural_file_parent and (
                        edge.get("edge_kind") != "evidence_claim"
                        or edge.get("predicate_id") not in allowed_predicates
                    ):
                        continue
                    parent = str(edge.get("from_id") or "")
                    if not admit(parent):
                        continue
                    component_edges[str(edge.get("edge_id") or "")] = edge
                    lineage_queue.append(parent)
            if not forward_roots:
                forward_roots = {
                    node_id
                    for node_id in component_ids
                    if nodes_by_id[node_id].get("node_kind") != "link"
                }

        forward_queue: deque[str] = deque(sorted(forward_roots))
        visited_forward: set[str] = set()
        while forward_queue:
            current = forward_queue.popleft()
            if current in visited_forward:
                continue
            visited_forward.add(current)
            for edge in semantic_outgoing.get(current, []):
                target = str(edge.get("to_id") or "")
                if not admit(target):
                    continue
                component_edges[str(edge.get("edge_id") or "")] = edge
                forward_queue.append(target)

        # Add only the plantings that point to a Work in this component, then
        # walk their branch ancestors upward. This preserves complete tree
        # paths without opening a route sideways into unrelated dossiers.
        ancestor_queue: deque[str] = deque()
        work_ids = {
            node_id
            for node_id in component_ids
            if nodes_by_id[node_id].get("node_kind") == "work"
        }
        for work_id in sorted(work_ids):
            for edge in incoming.get(work_id, []):
                if edge.get("edge_kind") != "authored_source_planting":
                    continue
                parent = str(edge.get("from_id") or "")
                if parent not in nodes_by_id:
                    continue
                if parent not in component_ids and len(component_ids) >= bounded_limit:
                    truncated = True
                    continue
                component_ids.add(parent)
                component_edges[str(edge.get("edge_id") or "")] = edge
                ancestor_queue.append(parent)

        visited_ancestors: set[str] = set()
        while ancestor_queue:
            current = ancestor_queue.popleft()
            if current in visited_ancestors:
                continue
            visited_ancestors.add(current)
            current_kind = nodes_by_id[current].get("node_kind")
            for edge in incoming.get(current, []):
                is_branch_parent = edge.get("edge_kind") == "authored_branch_hierarchy"
                is_planting_parent = (
                    current_kind == "source_planting"
                    and edge.get("edge_kind") == "authored_source_planting"
                    and edge.get("predicate_id") == "has_source_planting"
                )
                if not (is_branch_parent or is_planting_parent):
                    continue
                parent = str(edge.get("from_id") or "")
                if parent not in nodes_by_id:
                    continue
                if parent not in component_ids and len(component_ids) >= bounded_limit:
                    truncated = True
                    continue
                component_ids.add(parent)
                component_edges[str(edge.get("edge_id") or "")] = edge
                ancestor_queue.append(parent)

        component_nodes = [nodes_by_id[node_id] for node_id in sorted(component_ids)]
        grouped_chain = {
            kind: [node for node in component_nodes if node.get("node_kind") == kind]
            for kind in ("branch", "era", "region", "tradition", "source_planting", "work", "expression", "edition", "item", "file", "link")
        }
        outgoing: dict[str, list[dict[str, Any]]] = {}
        for edge in component_edges.values():
            outgoing.setdefault(str(edge.get("from_id") or ""), []).append(edge)
        tree_paths: list[dict[str, Any]] = []
        for era in grouped_chain["era"]:
            era_id = str(era.get("node_id") or "")
            frontier: deque[tuple[str, list[str], list[str]]] = deque([(era_id, [era_id], [])])
            seen = {era_id}
            while frontier:
                current, node_path, edge_path = frontier.popleft()
                if current == object_id:
                    tree_paths.append({"node_ids": node_path, "edge_ids": edge_path})
                    break
                for edge in sorted(outgoing.get(current, []), key=lambda item: str(item.get("edge_id") or "")):
                    target = str(edge.get("to_id") or "")
                    if target and target not in seen:
                        seen.add(target)
                        frontier.append((target, [*node_path, target], [*edge_path, str(edge.get("edge_id") or "")]))
        rights = [
            record
            for record in (store.rights(component_ids) if store else navigation.get("rights", []))
            if isinstance(record, dict)
            and set(_string_list(record.get("scope_refs"))) & component_ids
        ]
        decision_scope_ids = {object_id}
        if selected.get("node_kind") == "link":
            decision_scope_ids = {
                str(edge.get("from_id"))
                for edge in component_edges.values()
                if edge.get("to_id") == object_id and edge.get("edge_kind") == "evidence_claim"
            }
        decision_rights = [
            record
            for record in rights
            if set(_string_list(record.get("scope_refs"))) & decision_scope_ids
        ]

        dossier_links = [selected] if selected.get("node_kind") == "link" else grouped_chain["link"]
        link_statuses = {
            str(node.get("properties", {}).get("access_status") or "unknown")
            for node in dossier_links
        }
        if "open_download" in link_statuses:
            technical_access = "downloadable"
        elif "open_view" in link_statuses:
            technical_access = "viewable"
        elif "metadata_only" in link_statuses:
            technical_access = "metadata_only"
        elif link_statuses & {"restricted", "login_required", "unavailable"}:
            technical_access = "restricted_or_unavailable"
        else:
            technical_access = "unknown"

        positive_statuses = {"licensed", "public_domain_reviewed"}
        positive_rights = [
            record
            for record in decision_rights
            if record.get("assessment_status") in positive_statuses
            and record.get("redistribution_posture") in {"authorized", "authorized_with_conditions"}
        ]
        reviewed_positive = [
            record
            for record in positive_rights
            if record.get("review_status") in {"accepted", "accepted_with_limits"}
        ]
        if reviewed_positive:
            rights_posture = "reviewed_reuse_route"
        elif positive_rights:
            rights_posture = "candidate_requires_human_review"
        elif decision_rights:
            rights_posture = "not_cleared"
        else:
            rights_posture = "unknown"
        gaps: list[str] = []
        if not decision_rights:
            gaps.append("no associated public rights record")
        if positive_rights and not reviewed_positive:
            gaps.append("positive rights route exists but has no accepted human review")
        if not grouped_chain["link"]:
            gaps.append("no first-class associated Link record")

        source_refs = sorted(
            {
                str(ref)
                for node in component_nodes
                for ref in [node.get("source_ref")]
                if isinstance(ref, str) and ref
            }
            | {
                str(ref)
                for edge in component_edges.values()
                for ref in edge.get("source_refs", [])
                if isinstance(ref, str) and ref
            }
            | {
                str(record.get("source_ref"))
                for record in rights
                if isinstance(record.get("source_ref"), str)
            }
        )
        return {
            "schema": "tos_source_dossier_v1",
            "object_id": object_id,
            "object": selected,
            "agent_summary": {
                "technical_access": technical_access,
                "rights_posture": rights_posture,
                "human_review_required": not bool(reviewed_positive),
                "can_conclude_legal_openness": bool(reviewed_positive),
                "availability_is_license": False,
                "rights_scope_refs": sorted(decision_scope_ids),
                "gaps": gaps,
            },
            "chain": grouped_chain,
            "tree_paths": tree_paths,
            "relations": [component_edges[key] for key in sorted(component_edges)],
            "rights": sorted(rights, key=lambda record: str(record.get("rights_id") or "")),
            "source_refs": source_refs,
            "truncated": truncated,
            "authority_note": navigation.get("authority_boundary"),
        }

    def source_read_capabilities(self) -> dict[str, Any]:
        """Report the explicit exact-source owner binding, if selected."""
        return (self.source_read_service.capabilities()
                if self.source_read_service is not None else unavailable_capabilities())

    def source_read_contract(self) -> dict[str, Any]:
        """Return the transport contract without selecting or reading a source."""
        contract = _read_json(program_path(SOURCE_READ_CONTRACT_RELATIVE_PATH))
        return {
            "schema": "tos_source_read_contract_bundle_v1",
            "contract": contract,
            "descriptor": contract_summary(),
            "source_ref": SOURCE_READ_CONTRACT_RELATIVE_PATH.as_posix(),
            "authority_boundary": {
                "is_source": False,
                "writes_to_source": False,
                "grants_current_use": False,
                "source_owner": "Tree-of-Sophia/source-witnesses",
                "note": "Exact metadata selection grants no text access. An explicitly selected native owner separately checks recorded public rights for native_public_unit; no arbitrary source access, new rights or current-use grant follows.",
            },
        }

    def source_handle_discover(self, request: dict[str, Any]) -> dict[str, Any]:
        """Issue one exact source handle through the selected owner readers."""
        if self.source_read_service is None:
            raise SourceReadError("source-owner-reader-not-configured")
        return self.source_read_service.discover(request)

    def source_read(self, request: dict[str, Any]) -> dict[str, Any]:
        """Read one exact owner-selected source record through the ABI."""
        if self.source_read_service is None:
            raise SourceReadError("source-owner-reader-not-configured")
        return self.source_read_service.read(request)

    def philosophy_projection_exists(self) -> bool:
        return self.philosophy_graph_projection_path.is_file()

    def philosophy_projection(self) -> dict[str, Any]:
        return _checked_knowledge_schema(_read_json(self.philosophy_graph_projection_path), "philosophy")

    def bibliographic_graph(self) -> dict[str, Any]:
        payload = load_projection(self.bibliographic_graph_path)
        if payload.get("schema_version") != "tos_source_witness_bibliographic_graph_v1":
            raise RuntimeError(
                "ToS bibliographic claim graph schema_version must be "
                "tos_source_witness_bibliographic_graph_v1"
            )
        return payload

    def entity_type_registry(self) -> dict[str, Any]:
        return _checked_knowledge_schema(_read_json(self.entity_type_registry_path), "entity_type_registry")

    def relation_type_registry(self) -> dict[str, Any]:
        return _checked_knowledge_schema(_read_json(self.relation_type_registry_path), "relation_type_registry")

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

    def knowledge_header(self) -> dict[str, Any]:
        """Read completed snapshot metadata without exporting graph records."""
        if store := self._query_store():
            return dict(store.header)
        return {key: value for key, value in self.knowledge_graph().items() if key not in ('nodes', 'relations')}

    def corpus_header(self) -> dict[str, Any]:
        """Read corpus metadata without loading source-navigation records."""
        if store := self._query_store():
            return store.corpus_payload(('graph_views',))
        return self.index()

    def knowledge_graph(self) -> dict[str, Any]:
        """Return a public read model with display fields, not a content-completeness verdict."""
        if self._prepared_reader is not None:
            raise PublishedReadModelError(
                "prepared reader does not materialize a full graph; this operation is not yet available "
                "on the prepared route (legacy compatibility must be selected explicitly)"
            )
        if store := self._query_store():
            # An explicit full export, never the ordinary constructor route.
            return {**store.header, 'nodes': list(store.rows('knowledge_nodes')),
                    'relations': list(store.rows('knowledge_relations'))}
        published_graph: dict[str, Any] | None = None
        with self._snapshot_lock:
            input_state = self._knowledge_input_state()
            if self._published_graph is not None and input_state == self._published_source_state:
                return self._published_graph
            # A source projection changed after an in-memory addressed update;
            # the owner must re-enter the complete builder for the new source
            # snapshot rather than layering edits across unknown inputs.
            self._addressed_graph = None
            self._addressed_source_state = None

            # A full build reads several independently written source
            # projections. Do not publish a graph under a state tuple that was
            # observed only after a source changed during the build.
            for _attempt in range(3):
                state_before = self._knowledge_input_state()
                graph = _knowledge_graph_version(*self._knowledge_graph_version_args(state_before))
                source_inputs = self._knowledge_source_inputs()
                state_after = self._knowledge_input_state()
                if state_before == state_after:
                    self._published_graph = graph
                    self._published_source_inputs = self._canonical_source_inputs(source_inputs)
                    self._published_source_state = state_after
                    self._published_catalog = None
                    self._published_catalog_graph = None
                    self._published_catalog_source_state = None
                    published_graph = graph
                    break
            if published_graph is None:
                raise RuntimeError("ToS knowledge source projections changed during graph build")
        # Do not retain an index for a superseded graph.  This runs after the
        # snapshot lock is released so index readers can take their own lock
        # without creating a snapshot/index lock-order cycle.
        self._invalidate_snapshot_indexes(published_graph)
        return published_graph

    def _invalidate_snapshot_indexes(self, graph: dict[str, Any]) -> None:
        """Drop only indexes that do not belong to the current published graph.

        ``graph`` may already be superseded by a later publication before this
        cleanup gets the index lock.  Re-read the authoritative current graph
        while holding each respective index lock so an older cleanup cannot
        evict a newer index.
        """
        with self._search_lock:
            with self._snapshot_lock:
                current = self._published_graph
                if current is not None and self._search_index is not None and self._search_index.graph is not current:
                    self._search_index = None
                if current is not None and self._search_read_model is not None and self._search_read_model.graph is not current:
                    # Do not close the old connection here: a concurrent
                    # reader may still hold it.  The immutable path is
                    # replaced atomically by the next snapshot owner.
                    self._search_read_model = None
        with self._graph_index_lock:
            with self._snapshot_lock:
                current = self._published_graph
                if current is not None and self._graph_index is not None and self._graph_index.graph is not current:
                    self._graph_index = None

    def _knowledge_input_state(self) -> tuple[tuple[str, int, int, int, int], ...]:
        """Return the source-file state that bounds an in-memory addressed snapshot."""
        paths = self._knowledge_input_paths()
        state = []
        for path in paths:
            stat = path.stat()
            state.append(
                (
                    path.resolve().as_posix(),
                    stat.st_mtime_ns,
                    stat.st_size,
                    stat.st_ino,
                    stat.st_ctime_ns,
                )
            )
        return tuple(state)

    def _knowledge_input_paths(self) -> tuple[Path, ...]:
        return (
            self.index_path,
            self.philosophy_graph_projection_path,
            self.bibliographic_graph_path,
            self.entity_type_registry_path,
            self.relation_type_registry_path,
        )

    def _knowledge_source_inputs(self, *, reader=None) -> dict[str, dict[str, Any]]:
        """Read the complete direct carrier set for exact owner transitions."""
        reader = _read_json if reader is None else reader
        return {
            "corpus": reader(self.index_path),
            "philosophy": reader(self.philosophy_graph_projection_path),
            "bibliographic": reader(self.bibliographic_graph_path),
            "entity_type_registry": reader(self.entity_type_registry_path),
            "relation_type_registry": reader(self.relation_type_registry_path),
        }

    @staticmethod
    def _canonical_source_inputs(
        inputs: dict[str, dict[str, Any]],
    ) -> dict[str, bytes]:
        """Keep immutable compact carrier snapshots for the next CAS delta."""
        return {
            name: ToSAccessCore._canonical_json_bytes(payload)
            for name, payload in inputs.items()
        }

    @staticmethod
    def _canonical_json_bytes(value: Any) -> bytes:
        """Encode JSON with the same type-sensitive canonical contract."""
        return json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")

    @staticmethod
    def _addressed_source_collection(
        inputs: dict[str, dict[str, Any]], source_graph: str
    ) -> tuple[dict[str, Any], str]:
        if source_graph == "philosophy":
            return inputs["philosophy"], "nodes"
        if source_graph == "canon":
            return inputs["corpus"], "nodes"
        if source_graph == "source-navigation":
            return inputs["corpus"]["source_navigation"], "nodes"
        if source_graph == "source-claims":
            return inputs["bibliographic"], "nodes"
        raise AddressedUpdateError(f"unsupported addressed source graph: {source_graph}")

    @staticmethod
    def _source_record_matches(record: Any, source_id: str) -> bool:
        return isinstance(record, dict) and any(
            record.get(key) == source_id for key in ("node_id", "id", "path")
        )

    def _validate_addressed_source_transition(
        self,
        previous_inputs: dict[str, bytes],
        current_inputs: dict[str, dict[str, Any]],
        source_graph: str,
        source_id: str,
        source_record: dict[str, Any],
    ) -> dict[str, Any]:
        """Require that the disk delta is exactly the submitted one carrier.

        A stat tuple detects races, but cannot prove that a second source edit
        was not bundled into a one-record addressed publication. Compare the
        complete parsed carrier set against a copy of the previous inputs with
        only the addressed record replaced. This is a bounded owner-side scan
        of direct source carriers, not semantic acceptance or a graph rebuild.
        """
        try:
            before = {
                name: json.loads(payload.decode("utf-8"))
                for name, payload in previous_inputs.items()
            }
        except (UnicodeDecodeError, json.JSONDecodeError, AttributeError) as exc:
            raise AddressedUpdateError(
                "addressed update parent source snapshot is unreadable; run a complete graph build"
            ) from exc
        after = current_inputs
        if set(before) != set(after):
            raise AddressedUpdateError(
                "addressed source transition changed the complete carrier set; "
                "submit one exact replacement or run complete source assembly"
            )
        before_doc, before_field = self._addressed_source_collection(before, source_graph)
        after_doc, after_field = self._addressed_source_collection(after, source_graph)
        before_records = before_doc.get(before_field)
        after_records = after_doc.get(after_field)
        if not isinstance(before_records, list) or not isinstance(after_records, list):
            raise AddressedUpdateError(
                f"addressed source {source_graph} does not expose a record collection"
            )
        before_matches = [
            position
            for position, record in enumerate(before_records)
            if self._source_record_matches(record, source_id)
        ]
        after_matches = [
            position
            for position, record in enumerate(after_records)
            if self._source_record_matches(record, source_id)
        ]
        if len(before_matches) != 1 or len(after_matches) != 1:
            raise AddressedUpdateError(
                f"addressed source transition for {source_graph}:{source_id} must retain one exact carrier"
            )
        after_record = after_records[after_matches[0]]
        try:
            expected_record_bytes = self._canonical_json_bytes(source_record)
            after_record_bytes = self._canonical_json_bytes(after_record)
        except (TypeError, ValueError, OverflowError) as exc:
            raise AddressedUpdateError(
                "addressed source record is not canonical JSON; run complete source assembly"
            ) from exc
        if after_record_bytes != expected_record_bytes:
            raise AddressedUpdateError(
                "addressed source record does not match the current on-disk owner carrier"
            )
        if before_matches[0] != after_matches[0]:
            raise AddressedUpdateError(
                "addressed source carrier moved position; run complete source assembly"
            )
        # ``before`` came from JSON decoding, so replacing only this one record
        # is enough to build the expected carrier set. Compare canonical bytes,
        # not Python equality: JSON distinguishes false/0 and 1/1.0 at this
        # owner boundary even though Python considers those values equal.
        before_doc[before_field][before_matches[0]] = copy.deepcopy(source_record)
        if any(
            self._canonical_json_bytes(before[name])
            != self._canonical_json_bytes(after[name])
            for name in before
        ):
            raise AddressedUpdateError(
                "addressed source transition contains undeclared carrier changes; "
                "submit one exact replacement or run complete source assembly"
            )
        return {
            "mode": "exact-single-record-delta",
            "source_graph": source_graph,
            "source_id": source_id,
            "changed_records": 1,
            "source_scan": "complete-direct-carrier-set",
        }

    @staticmethod
    def _knowledge_graph_version_args(
        state: tuple[tuple[str, int, int, int, int], ...],
    ) -> tuple[Any, ...]:
        args: list[Any] = []
        for path_text, mtime_ns, size, inode, ctime_ns in state:
            args.extend((path_text, mtime_ns, size, inode, ctime_ns))
        return tuple(args)

    def knowledge_graph_addressed(
        self,
        previous_graph: dict[str, Any],
        source_graph: str,
        source_id: str,
        source_record: dict[str, Any],
        *,
        source_revision: str,
        expected_parent_revision: str | None = None,
        return_report: bool = False,
    ) -> dict[str, Any]:
        """Apply one owner-supplied replacement to an existing graph snapshot.

        This is the core owner call site for the bounded projection fast path.
        The caller supplies the exact replacement carrier and complete target
        source revision; additions, removals, source assembly, and source
        writes remain on the full builder/owner routes.
        """
        with self._snapshot_lock:
            # Resolve the CAS parent from the last published snapshot before
            # looking at source files.  A source owner may have already written
            # the exact replacement carrier; rebuilding here would consume the
            # edit and make the bounded publication path appear stale.
            current = self._published_graph
            if current is None:
                current = self.knowledge_graph()
            if previous_graph is not current:
                raise AddressedUpdateError(
                    "addressed update parent is stale; previous snapshot is not the current published snapshot"
                )
            expected = expected_parent_revision
            if expected is None:
                expected = previous_graph.get("source_revision") if isinstance(previous_graph, dict) else None
            if expected != current.get("source_revision"):
                raise AddressedUpdateError(
                    "addressed update parent is stale; current snapshot revision is "
                    f"{current.get('source_revision')!r}, expected {expected!r}"
                )

            # The source state must bracket every registry read and bounded
            # recomputation. Never bind a successor to the post-build tuple if
            # a source file changed while it was running.
            state_before = self._knowledge_input_state()
            if self._published_source_inputs is None:
                raise AddressedUpdateError(
                    "addressed update parent has no captured source inputs; run a complete graph build"
                )
            source_inputs = self._knowledge_source_inputs()
            actual_source_revision = knowledge_source_revision(
                source_inputs["corpus"],
                source_inputs["philosophy"],
                source_inputs["bibliographic"],
                source_inputs["entity_type_registry"],
                source_inputs["relation_type_registry"],
            )
            if source_revision != actual_source_revision:
                raise AddressedUpdateError(
                    "addressed update target source_revision does not match the complete "
                    "current source carrier set; previous snapshot remains published"
                )
            source_transition = self._validate_addressed_source_transition(
                self._published_source_inputs,
                source_inputs,
                source_graph,
                source_id,
                source_record,
            )
            source_after_read = self._knowledge_input_state()
            if state_before != source_after_read:
                raise AddressedUpdateError(
                    "ToS knowledge source projections changed while reading the addressed source transition; "
                    "previous snapshot remains published"
                )
            entity_registry = self.entity_type_registry()
            relation_registry = self.relation_type_registry()
            updated = addressed_update_knowledge_graph(
                previous_graph,
                source_graph,
                source_id,
                source_record,
                entity_registry,
                relation_registry,
                source_revision=source_revision,
                return_report=return_report,
            )
            state_after = self._knowledge_input_state()
            if state_before != state_after:
                raise AddressedUpdateError(
                    "ToS knowledge source projections changed during addressed update; "
                    "previous snapshot remains published"
                )

            graph = updated["graph"] if return_report else updated
            if return_report:
                updated["report"]["source_transition"] = source_transition
                updated["report"]["input_traversal"]["source_assembly"] = "exact-transition-checked"
                updated["report"]["input_traversal"]["source_scan"] = "complete-direct-carrier-set"
            self._published_source_inputs = self._canonical_source_inputs(source_inputs)
            self._published_graph = graph
            self._published_catalog = None
            self._published_catalog_graph = None
            self._published_catalog_source_state = None
            self._addressed_graph = graph
            self._addressed_source_state = state_after
            self._published_source_state = state_after
        # Search/inspection indexes are identity-bound and lazily rebuild on
        # the next query against this newly published in-memory snapshot. Run
        # the invalidation after releasing the snapshot lock so readers can
        # take their respective index lock without a lock-order cycle.
        self._invalidate_snapshot_indexes(graph)
        return updated

    def knowledge_catalog(self) -> dict[str, Any]:
        """Describe the compositional grammar, vocabulary, and stored lens specs."""
        if self._prepared_reader is not None:
            return self._prepared_reader.catalog()
        if store := self._query_store():
            return store.metadata['catalog']
        return self.knowledge_snapshot()["catalog"]

    def knowledge_snapshot(self) -> dict[str, dict[str, Any]]:
        """Return one graph/catalog pair bound to the same published snapshot.

        Aggregate consumers must not fetch a catalog and then independently
        resolve a graph: an addressed publication can occur between those two
        reads.  Keep both products under the snapshot lock and recheck the
        complete source state after reading their carriers.
        """
        if store := self._query_store():
            # All rows and catalog use this same immutable store object.
            return {'graph': {**store.header, 'nodes': list(store.rows('knowledge_nodes')),
                              'relations': list(store.rows('knowledge_relations'))},
                    'catalog': store.metadata['catalog']}
        with self._snapshot_lock:
            for _attempt in range(3):
                graph = self.knowledge_graph()
                state_before = self._knowledge_input_state()
                if self._published_source_state != state_before:
                    continue
                if (
                    self._published_catalog is not None
                    and self._published_catalog_graph is graph
                    and self._published_catalog_source_state == state_before
                ):
                    state_after = self._knowledge_input_state()
                    if state_before == state_after:
                        return {"graph": graph, "catalog": self._published_catalog}
                    continue
                corpus = self.index()
                philosophy = self.philosophy_projection()
                entity_registry = self.entity_type_registry()
                relation_registry = self.relation_type_registry()
                state_after = self._knowledge_input_state()
                if state_before != state_after:
                    continue
                catalog = build_knowledge_catalog(
                    graph,
                    corpus,
                    philosophy,
                    entity_registry,
                    relation_registry,
                )
                self._published_catalog = catalog
                self._published_catalog_graph = graph
                self._published_catalog_source_state = state_after
                return {"graph": graph, "catalog": catalog}
            raise RuntimeError("ToS knowledge source projections changed during catalog build")

    def knowledge_snapshot_once(self, *, include_catalog_inputs: bool = False) -> dict[str, Any]:
        """Explicit one-shot full bootstrap, without mutable-core retention.

        Uses the same graph/catalog builders and source state as the legacy
        snapshot. It neither populates nor evicts another reader's caches,
        retains canonical source bytes for a future CAS delta, nor installs
        the result as this core's current mutable snapshot. Source drift fails
        this attempt; the caller may retry explicitly with a fresh output.

        An explicit offline maintenance caller may also request copy-isolated
        CatalogInputs from these same actual registry and lens carriers. The
        default packet remains unchanged; no inputs are reconstructed from a
        catalog or retained as a mutable-core/source-transition baseline.
        """
        if type(include_catalog_inputs) is not bool:
            raise ValueError("include_catalog_inputs must be a boolean")
        if self._prepared_reader is not None:
            raise PublishedReadModelError("prepared reader cannot bootstrap source carriers")
        from .normalization_cache import active_cache

        before = self._knowledge_input_state()
        inputs = self._knowledge_source_inputs(reader=_read_json_file)
        if self._knowledge_input_state() != before:
            raise RuntimeError("source changed during one-shot carrier read")
        # Match the schema checks performed by knowledge_snapshot's catalog
        # path, sharing their authority instead of adding a second policy.
        for name in _KNOWLEDGE_SCHEMA_RULES:
            _checked_knowledge_schema(inputs[name], name)
        token = active_cache.set(None)
        try:
            graph = build_knowledge_graph(inputs["corpus"], inputs["philosophy"],
                inputs["bibliographic"], inputs["entity_type_registry"], inputs["relation_type_registry"])
            if self._knowledge_input_state() != before:
                raise RuntimeError("source changed during one-shot graph build")
            catalog = build_knowledge_catalog(graph, inputs["corpus"], inputs["philosophy"],
                inputs["entity_type_registry"], inputs["relation_type_registry"])
            catalog_inputs = None
            if include_catalog_inputs:
                from .catalog_semantics import CatalogInputs
                # The sequence profile retains the actual graph encounter
                # order; prepared publication gives it sparse source tokens.
                catalog_inputs = CatalogInputs.from_graph(graph, inputs["corpus"], inputs["philosophy"],
                    inputs["entity_type_registry"], inputs["relation_type_registry"])
            if self._knowledge_input_state() != before:
                raise RuntimeError("source changed during one-shot catalog build")
        finally:
            active_cache.reset(token)
        result = {"graph": graph, "catalog": catalog, "source_state": before}
        if include_catalog_inputs:
            result["catalog_inputs"] = catalog_inputs
        return result

    def knowledge_contracts(self) -> dict[str, Any]:
        """Return the executable API map and JSON Schemas through one public read route."""
        contracts = {
            contract_id: _read_json(
                self.tos_root / relative_path
                if contract_id in {"entity_type_registry", "relation_type_registry"}
                else program_path(relative_path)
            )
            for contract_id, relative_path in KNOWLEDGE_CONTRACT_RELATIVE_PATHS.items()
        }
        return {
            "schema": "tos_knowledge_contract_bundle_v1",
            "contracts": contracts,
            "source_refs": [
                relative_path.as_posix()
                for relative_path in KNOWLEDGE_CONTRACT_RELATIVE_PATHS.values()
            ],
            "authority_boundary": {
                "is_source": False,
                "writes_to_tree": False,
                "source_owner": "Tree-of-Sophia/access/contracts",
                "note": "This packet transports versioned access contracts; it does not author ToS meaning.",
            },
        }

    def knowledge_search(
        self,
        query: str = "",
        *,
        sources: list[str] | None = None,
        kind_ids: list[str] | None = None,
        predicate_ids: list[str] | None = None,
        offset: int = 0,
        limit: int = 40,
    ) -> dict[str, Any]:
        """Search the normalized human/agent knowledge surface without choosing a legacy mode."""
        if store := self._query_store():
            return store.search(query, sources=sources, kind_ids=kind_ids, predicate_ids=predicate_ids, offset=offset, limit=limit)
        graph = self.knowledge_graph()
        index = self._search_index_for_snapshot(graph)
        return search_knowledge_graph(
            graph,
            query,
            sources=sources,
            kind_ids=kind_ids,
            predicate_ids=predicate_ids,
            offset=offset,
            limit=limit,
            search_index=index,
        )

    def knowledge_search_capabilities(self) -> dict[str, Any]:
        """Describe selected engines; do not materialize a compatibility graph."""
        from .search_read_model import SEARCH_NGRAM_SIZE
        legacy = self._prepared_reader is None
        store = self._query_store() if legacy else None
        indexed = legacy and (store is None or store.metadata.get('search_accelerator', {}).get('mode') == 'fts5-trigram')
        compressed = {"available": False, "schema": "tos_knowledge_search_compressed_v3",
                      "reason": "explicit-local-prepared-publication-required", "writes_to_tree": False}
        if self._prepared_reader is not None:
            from .published_search import PublishedSearchService
            compressed = PublishedSearchService(self._prepared_reader).capability()
        return {"schema": "tos_knowledge_search_capabilities_v1", "default_mode": "legacy",
                "explicit_mode_required": not legacy, "writes_to_tree": False,
                "modes": {
                    "legacy": {"available": legacy, "schema": "tos_knowledge_search_v1",
                               "verification": "engine-selection-only", "pagination": "offset"},
                    "indexed": {"available": indexed, "schema": "tos_knowledge_search_indexed_v2",
                                "verification": "engine-selection-only", "pagination": "cursor",
                                "min_normalized_query_code_points": SEARCH_NGRAM_SIZE},
                    "compressed": compressed}}

    def knowledge_search_compressed(
        self, query: str = "", *, sources: list[str] | None = None,
        kind_ids: list[str] | None = None, predicate_ids: list[str] | None = None,
        cursor: str | None = None, limit: int = 40,
    ) -> dict[str, Any]:
        """Read compressed v3 matches and exact bodies in one prepared snapshot."""
        from .compressed_search_store import SearchUnavailable
        from .published_search import PublishedSearchService
        if self._prepared_reader is None:
            raise SearchUnavailable("compressed search requires an explicitly selected local prepared publication")
        return PublishedSearchService(self._prepared_reader).search(
            query, sources=sources, kind_ids=kind_ids, predicate_ids=predicate_ids,
            cursor=cursor, limit=limit)

    def _search_read_model_for_snapshot(
        self, graph: dict[str, Any]
    ) -> SQLiteKnowledgeSearchReadModel:
        """Open or atomically build the persistent carrier for one snapshot."""
        with self._search_lock:
            current = self._search_read_model
            if current is not None and current.graph is graph:
                return current
            path = self.search_read_model_path
            if path is None:  # pragma: no cover - __post_init__ supplies it
                raise SearchReadModelError("search read-model path is not configured")
            try:
                model = SQLiteKnowledgeSearchReadModel.open(graph, path)
            except (OSError, SearchReadModelSnapshotError):
                model = SQLiteKnowledgeSearchReadModel.build(
                    graph,
                    path,
                    max_bytes=self.search_read_model_max_bytes,
                    max_postings=self.search_read_model_max_postings,
                )
            with self._snapshot_lock:
                if self._published_graph is None or self._published_graph is graph:
                    self._search_read_model = model
            return model

    def knowledge_search_indexed(
        self,
        query: str = "",
        *,
        sources: list[str] | None = None,
        kind_ids: list[str] | None = None,
        predicate_ids: list[str] | None = None,
        cursor: str | None = None,
        limit: int = 40,
    ) -> dict[str, Any]:
        """Search through the persistent bounded carrier (explicit v2 mode)."""
        # Validate the transport query before touching the graph/read-model
        # path.  In particular, a rejected overlong/non-string query must not
        # cold-build a snapshot merely to fail at the indexed boundary.
        normalized_query = normalize_search_query(query)
        bounded_limit = _bounded_int(limit, 40, 1, 100)

        if sources:
            if any(not isinstance(value, str) for value in sources):
                raise SearchReadModelError("knowledge search filters must contain strings")
            unknown_sources = sorted(set(sources) - set(KNOWLEDGE_SOURCES))
            if unknown_sources:
                raise SearchReadModelError(
                    f"unsupported knowledge sources: {', '.join(unknown_sources)}"
                )
            normalized_sources = sorted(set(sources))
        else:
            normalized_sources = sorted(KNOWLEDGE_SOURCES)
        store = self._query_store()
        graph = store.header if store is not None else self.knowledge_graph()
        request_filters = {
            "sources": normalized_sources,
            "kind_ids": sorted(set(kind_ids or ())),
            "predicate_ids": sorted(set(predicate_ids or ())),
        }
        node_cursor = relation_cursor = None
        node_exhausted = relation_exhausted = False
        if cursor is not None:
            payload = _indexed_search_cursor_decode(cursor)
            if (
                payload.get("source_revision") != graph.get("source_revision")
                or payload.get("query") != normalized_query
                or payload.get("filters") != request_filters
            ):
                raise SearchReadModelSnapshotError("indexed knowledge search cursor does not match the snapshot/query")
            expected_keys = {
                "filters",
                "nodes",
                "nodes_exhausted",
                "query",
                "relations",
                "relations_exhausted",
                "schema",
                "source_revision",
            }
            if set(payload) != expected_keys:
                raise SearchReadModelError("invalid indexed knowledge search cursor")
            if not isinstance(payload["nodes_exhausted"], bool) or not isinstance(
                payload["relations_exhausted"], bool
            ):
                raise SearchReadModelError("invalid indexed knowledge search cursor")
            node_cursor = payload.get("nodes")
            relation_cursor = payload.get("relations")
            node_exhausted = payload["nodes_exhausted"]
            relation_exhausted = payload["relations_exhausted"]
            if node_exhausted:
                if node_cursor is not None:
                    raise SearchReadModelError("invalid indexed knowledge search cursor")
            elif not isinstance(node_cursor, str) or not node_cursor:
                raise SearchReadModelError("invalid indexed knowledge search cursor")
            if relation_exhausted:
                if relation_cursor is not None:
                    raise SearchReadModelError("invalid indexed knowledge search cursor")
            elif not isinstance(relation_cursor, str) or not relation_cursor:
                raise SearchReadModelError("invalid indexed knowledge search cursor")

        model = store if store is not None else self._search_read_model_for_snapshot(graph)
        empty_page = lambda: SearchReadModelPage((), 0, 0, False, None, ordering_scope="global-rank")
        node_page = (
            empty_page()
            if node_exhausted
            else model.ranked_page(
                "nodes", query, sources=normalized_sources, kind_ids=kind_ids, cursor=node_cursor,
                page_size=bounded_limit, max_verify_chars=self.search_read_model_max_verify_chars,
            )
        )
        relation_page = (
            empty_page()
            if relation_exhausted
            else model.ranked_page(
                "relations", query, sources=normalized_sources, predicate_ids=predicate_ids, cursor=relation_cursor,
                page_size=bounded_limit, max_verify_chars=self.search_read_model_max_verify_chars,
            )
        )

        def items(kind: str, page: Any) -> list[dict[str, Any]]:
            result: list[dict[str, Any]] = []
            for row in page.rows:
                position = int(row["position"])
                result.append(model.source_item(kind, position))
            return result

        next_cursor = None
        if node_page.next_cursor is not None or relation_page.next_cursor is not None:
            next_node_exhausted = node_page.next_cursor is None
            next_relation_exhausted = relation_page.next_cursor is None
            next_cursor = _indexed_search_cursor_encode(
                {
                    "schema": SEARCH_READ_MODEL_CURSOR_SCHEMA,
                    "source_revision": graph.get("source_revision"),
                    "query": normalized_query,
                    "filters": request_filters,
                    "nodes": node_page.next_cursor,
                    "relations": relation_page.next_cursor,
                    "nodes_exhausted": next_node_exhausted,
                    "relations_exhausted": next_relation_exhausted,
                }
            )
        return {
            "schema": "tos_knowledge_search_indexed_v2",
            "source_revision": graph["source_revision"],
            "query": query,
            "filters": request_filters,
            "page": {
                "cursor": cursor,
                "next_cursor": next_cursor,
                "limit_per_kind": bounded_limit,
                "ordering_scope": "global-rank",
                "has_more": next_cursor is not None,
            },
            "counts": {
                "matching_nodes": (
                    len(node_page.rows)
                    if cursor is None and not node_page.has_more
                    else None
                ),
                "matching_relations": (
                    len(relation_page.rows)
                    if cursor is None and not relation_page.has_more
                    else None
                ),
                "returned_nodes": len(node_page.rows),
                "returned_relations": len(relation_page.rows),
                "scope": "exact-if-kind-exhausted-without-continuation",
            },
            "nodes": items("nodes", node_page),
            "relations": items("relations", relation_page),
            "authority_boundary": graph.get("authority_boundary", {}),
            "work": {"nodes": node_page.as_dict()["work"], "relations": relation_page.as_dict()["work"]},
        }

    def _search_index_for_snapshot(self, graph: dict[str, Any]) -> KnowledgeSearchIndex:
        """Return an index without recaching a graph superseded in-flight."""
        with self._search_lock:
            # Keep lock order search -> snapshot for cache checks. Release the
            # snapshot lock while the index is built so publication is not
            # blocked by an in-flight reader, then recheck before caching.
            with self._snapshot_lock:
                current = self._published_graph
                cacheable = current is None or current is graph
                if cacheable and self._search_index is not None and self._search_index.graph is graph:
                    return self._search_index
            index = KnowledgeSearchIndex(graph)
            with self._snapshot_lock:
                if self._published_graph is None or self._published_graph is graph:
                    self._search_index = index
            return index

    def knowledge_node(self, node_id: str, relation_limit: int = 200) -> dict[str, Any]:
        """Inspect one normalized node (or all namespaced matches for a native ID)."""
        if self._prepared_reader is not None:
            return self._prepared_reader.node(node_id, relation_limit)
        if store := self._query_store():
            return store.inspect_node(node_id, relation_limit)
        graph = self.knowledge_graph()
        return inspect_knowledge_node(graph, node_id, relation_limit, graph_index=self._current_graph_index(graph))

    def knowledge_relation(self, relation_id: str) -> dict[str, Any]:
        """Inspect one normalized relation and its display-complete endpoints."""
        if self._prepared_reader is not None:
            return self._prepared_reader.relation(relation_id)
        if store := self._query_store():
            return store.inspect_relation(relation_id)
        graph = self.knowledge_graph()
        return inspect_knowledge_relation(graph, relation_id, graph_index=self._current_graph_index(graph))

    def knowledge_temporal_compare(self, request: dict[str, Any]) -> dict[str, Any]:
        """Compare the date envelopes of two exact Claims, without adjudication."""
        if self._prepared_reader is not None:
            return self._prepared_reader.temporal_compare(request)
        if store := self._query_store():
            from .temporal_comparison import compare_temporal_operands
            return compare_temporal_operands(store.header['source_revision'], request,
                lambda identifier: list(store.rows('knowledge_nodes', 'id=?', (identifier,), limit=2)))
        graph = self.knowledge_graph()
        return compare_temporal_claims(graph, request, graph_index=self._current_graph_index(graph))

    def _current_graph_index(self, graph: dict[str, Any]) -> KnowledgeGraphIndex:
        with self._graph_index_lock:
            # Keep lock order graph-index -> snapshot for cache checks. Release
            # the snapshot lock while the index is built so publication is not
            # blocked by an in-flight reader, then recheck before caching.
            with self._snapshot_lock:
                current = self._published_graph
                cacheable = current is None or current is graph
                if cacheable and self._graph_index is not None and self._graph_index.graph is graph:
                    return self._graph_index
            index = KnowledgeGraphIndex(graph)
            with self._snapshot_lock:
                if self._published_graph is None or self._published_graph is graph:
                    self._graph_index = index
            return index

    def knowledge_focus(
        self,
        node_id: str,
        *,
        sources: list[str] | None = None,
        depth: int = 1,
        direction: str = "either",
        predicate_ids: list[str] | None = None,
        node_limit: int = 200,
        relation_limit: int = 400,
        profile: str = "overview",
    ) -> dict[str, Any]:
        """Construct a bounded radial lens around one exact or unambiguous node identity."""
        if self._prepared_lens is not None:
            return self._prepared_lens.focus(
                node_id, sources=sources, depth=depth, direction=direction,
                predicate_ids=predicate_ids, node_limit=node_limit,
                relation_limit=relation_limit, profile=profile,
            )
        if store := self._query_store():
            return store.focus(node_id, sources=sources, depth=depth, direction=direction, predicate_ids=predicate_ids, node_limit=node_limit, relation_limit=relation_limit, profile=profile)
        graph = self.knowledge_graph()
        return focus_knowledge_node(
            graph,
            node_id,
            sources=sources,
            depth=depth,
            direction=direction,
            predicate_ids=predicate_ids,
            node_limit=node_limit,
            relation_limit=relation_limit,
            profile=profile,
            graph_index=self._current_graph_index(graph),
        )

    def compile_knowledge_lens(self, spec: dict[str, Any]) -> dict[str, Any]:
        """Compile and execute a bounded read-only lens supplied by a human or agent."""
        if self._prepared_lens is not None:
            return self._prepared_lens.execute(spec)
        if store := self._query_store():
            return store.execute_lens(spec)
        graph = self.knowledge_graph()
        return execute_knowledge_lens(graph, spec, graph_index=self._current_graph_index(graph))

    def stored_knowledge_lens(self, lens_id: str) -> dict[str, Any]:
        """Compile one source-backed stored lens through the same generic engine."""
        if self._prepared_lens is not None:
            # Both reads enforce the same immutable expected binding (including
            # the publication epoch). A publication between them must refuse,
            # not execute an old catalog entry on a newly selected snapshot.
            catalog = self.knowledge_catalog()
            spec = next((item for item in catalog.get("lenses", [])
                         if isinstance(item, dict) and item.get("lens_id") == lens_id), None)
            if spec is None:
                raise KeyError(f"unknown ToS knowledge lens: {lens_id}")
            return self._prepared_lens.execute(spec)
        if store := self._query_store():
            spec = next((item for item in store.metadata['catalog'].get('lenses', [])
                         if isinstance(item, dict) and item.get('lens_id') == lens_id), None)
            if spec is None:
                raise KeyError(f"unknown ToS knowledge lens: {lens_id}")
            return store.execute_lens(spec)
        snapshot = self.knowledge_snapshot()
        spec = next(
            (
                item
                for item in snapshot["catalog"].get("lenses", [])
                if isinstance(item, dict) and item.get("lens_id") == lens_id
            ),
            None,
        )
        if spec is None:
            raise KeyError(f"unknown ToS knowledge lens: {lens_id}")
        # Execute against the exact graph used to derive the lens catalog.
        # Calling compile_knowledge_lens would resolve the graph a second time
        # and could mix a newly published snapshot with the selected spec.
        graph = snapshot["graph"]
        return execute_knowledge_lens(graph, spec, graph_index=self._current_graph_index(graph))

    def status(self) -> dict[str, Any]:
        exists = self.index_exists()
        store = self._query_store() if exists else None
        payload = store.corpus_payload(('graph_views',)) if store else (self.index() if exists else {})
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
        store = self._query_store()
        payload = store.corpus_payload(('branches', 'graph_views')) if store else self.index()
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
        if store := self._query_store():
            return self._search_payload(query, resource_kind, store.corpus_search(query, _bounded_int(limit, 20, 1, 100), resource_kind))
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
        store = self._query_store()
        if store:
            clauses, values = [], []
            for name, value in [('resource_kind', resource_kind), ('owner_branch', owner_branch)]:
                if value:
                    clauses.append("json_extract(payload,'$." + name + "')=?")
                    values.append(value)
            payload = {**store.metadata['corpus_header'], 'resources': list(store.raw('corpus/resources', where=' AND '.join(clauses) or '1', params=values, limit=_bounded_int(limit,100,1,1000)))}
        else:
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
        store = self._query_store()
        if store:
            payload = store.corpus_payload(('relation_packs',))
            payload['nodes'] = list(store.raw('corpus/nodes', where="json_extract(payload,'$.node_id')=?", params=[node_id]))
            payload['relation_edges'] = list(store.raw('corpus/relation_edges', where="json_extract(payload,'$.from_id')=? OR json_extract(payload,'$.to_id')=?", params=[node_id,node_id]))
        else:
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
        store = self._query_store()
        if store:
            payload = dict(store.metadata['corpus_header'])
            payload['relation_packs'] = list(store.raw('corpus/relation_packs', where="json_extract(payload,'$.pack_id')=?", params=[pack_id]))
            payload['relation_edges'] = list(store.raw('corpus/relation_edges', where="json_extract(payload,'$.pack_id')=?", params=[pack_id]))
        else:
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
        limit = _bounded_int(limit, 100, 1, 1000)
        store = self._query_store()
        if store:
            payload = store.corpus_payload(('graph_views', 'relation_packs'))
            payload['branches'] = list(store.raw('corpus/branches', limit=limit))
            if view_id == 'route-graph':
                pack_ids = [pack['pack_id'] for pack in payload['relation_packs'] if pack.get('owner_branch') == 'ToS/canon']
                clause, values = store.membership("json_extract(payload,'$.pack_id')", pack_ids)
            else:
                clause, values = "json_extract(payload,'$.owner_branch')=?", ['ToS/candidate-intake']
            payload['relation_edges'] = list(store.raw('corpus/relation_edges', where=clause, params=values, limit=limit)) if view_id != 'corpus-topology' else []
            endpoint_ids = {e[f] for e in payload['relation_edges'] for f in ('from_id','to_id')}
            clause, values = store.membership("json_extract(payload,'$.node_id')", endpoint_ids)
            payload['nodes'] = list(store.raw('corpus/nodes', where=clause, params=values))
        else:
            payload = self.index()
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
        payload = self.corpus_header()
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
            payload = self.corpus_header()
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
