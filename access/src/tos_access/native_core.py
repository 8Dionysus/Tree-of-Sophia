"""Synchronous imported access over the established installed native MCP ABI.

Python owns child lifecycle and typed packet delivery. Rust owns query rules and
currentness. Software and selected data are explicit independent selections.
"""
from __future__ import annotations

import time
from pathlib import Path
from typing import Any
from concurrent.futures import ThreadPoolExecutor

from .native_mcp import NativeMCPServer
from .source_read_errors import SourceReadError


class NativeCore:
    """Synchronous packet facade over an authenticated native MCP child.

    Each call owns one helper thread, including when its caller already runs an
    async event loop. It joins that thread on success or failure; the existing
    native API owns process-group cleanup under the same original50s deadline.
    """

    def __init__(self, native_prefix: str | Path, arguments=(), *, inherit_data_selection: bool = True):
        self._server = NativeMCPServer(native_prefix, arguments, inherit_data_selection=inherit_data_selection)

    def _native_result(self, operation: str, arguments, *, absolute_deadline=None):
        start = time.monotonic()
        if absolute_deadline is not None:
            import math
            if type(absolute_deadline) not in (int, float) or not math.isfinite(absolute_deadline):
                raise ValueError("Native Core deadline must be finite")
            if absolute_deadline <= start:
                raise TimeoutError("Native Core deadline expired before setup")
        deadline = start + 50 if absolute_deadline is None else min(start + 50, absolute_deadline)
        def invoke():
            import anyio

            async def call():
                return await self._server._native_api(
                    operation, arguments, absolute_deadline=deadline)

            return anyio.run(call)

        # No detached executor or reusable background session can outlive this
        # method. All native child work uses the deadline established above.
        with ThreadPoolExecutor(max_workers=1, thread_name_prefix="tos-native-core") as owned:
            result = owned.submit(invoke).result()
        if time.monotonic() >= deadline:
            raise TimeoutError("Native Core deadline expired before returning the packet")
        return result

    def _packet(self, tool: str, request: dict, *, absolute_deadline=None, source_errors=True):
        result = self._native_result("call", (tool, request), absolute_deadline=absolute_deadline)
        if result.isError:
            message = " ".join(item.text for item in result.content if hasattr(item, "text"))
            if source_errors:
                if message == "exact source reader unavailable: no selected owner":
                    message = "source-owner-reader-not-configured"
                raise SourceReadError(message)
            from mcp.server.fastmcp.exceptions import ToolError
            raise ToolError(message)
        if type(result.structuredContent) is not dict:
            raise ValueError("Native source operation did not return a full object packet")
        return result.structuredContent

    def knowledge_catalog(self) -> dict:
        """Read the selected Original catalog; no synthesized capability fallback."""
        return self._packet("tos_knowledge_catalog", {}, source_errors=False)

    def knowledge_prepared_status(self) -> dict:
        """Read the explicitly selected prepared publication status."""
        return self._packet("tos_knowledge_prepared_status", {}, source_errors=False)

    def knowledge_exploration_capabilities(self) -> dict:
        """Project the selected capability from the same native contracts packet."""
        return self.knowledge_exploration_contracts()["capabilities"]

    def knowledge_exploration_contracts(self) -> dict:
        """Return software schemas with the selected native exploration capability."""
        return self._packet("tos_knowledge_exploration_contracts", {}, source_errors=False)

    def source_read_capabilities(self) -> dict:
        return self._packet("tos_source_read_capabilities", {})

    def source_read_contract(self) -> dict:
        return self._packet("tos_source_read_contract", {})

    def source_handle_discover(self, request: dict) -> dict:
        return self._packet("tos_source_handle_discover", request)

    def source_read(self, request: dict) -> dict:
        return self._packet("tos_source_read", request)

    def zarathustra_word_analysis_task(self, query: str, language: str = "ru",
                                      rank: int = 1,
                                      include_semantic_neighbors: bool = False, *,
                                      absolute_deadline=None) -> dict:
        deadline = time.monotonic() + 50 if absolute_deadline is None else absolute_deadline
        normalized_query = str(query).strip()
        if not normalized_query:
            raise ValueError("word-analysis query is required")
        if len(normalized_query) > 256:
            raise ValueError("word-analysis query exceeds 256 characters")
        normalized_language = str(language).strip().lower()
        if normalized_language not in {"de", "ru", "en"}:
            raise ValueError(f"unsupported word-analysis language: {normalized_language}")
        try:
            bounded_rank = int(rank)
        except (TypeError, ValueError):
            bounded_rank = 1
        return self._packet("tos_zarathustra_prepare_word_analysis", {
            "query": normalized_query, "language": normalized_language,
            "rank": max(1, min(100, bounded_rank)),
            "include_semantic_neighbors": bool(include_semantic_neighbors),
        }, absolute_deadline=deadline, source_errors=False)

    def zarathustra_reading_search(self, query: str, language: str = "ru",
                                   limit: int = 20,
                                   include_semantic_neighbors: bool = False,
                                   group_by: list[str] | None = None, *,
                                   absolute_deadline=None) -> dict:
        """Return the selected source-bound reading capability and full result."""
        deadline = time.monotonic() + 50 if absolute_deadline is None else absolute_deadline
        normalized_query = str(query).strip()
        if not normalized_query or len(normalized_query) > 256:
            raise ValueError("reading query must have 1..256 characters")
        normalized_language = str(language).strip().lower()
        if normalized_language not in {"de", "ru", "en"}:
            raise ValueError("reading language must be de, ru, or en")
        try:
            bounded_limit = int(limit)
        except (TypeError, ValueError):
            bounded_limit = 20
        groups = ["speaker", "formula"] if group_by is None else group_by
        if not isinstance(groups, list) or any(item not in {"speaker", "formula"} for item in groups):
            raise ValueError("reading group_by supports speaker and formula only")
        return self._packet("tos_zarathustra_reading_search", {
            "query": normalized_query, "language": normalized_language,
            "limit": max(0, min(100, bounded_limit)),
            "include_semantic_neighbors": bool(include_semantic_neighbors),
            "group_by": list(dict.fromkeys(groups)),
        }, absolute_deadline=deadline, source_errors=False)

    # These methods only frame the established native MCP contract. Rust owns
    # selection, normalization, bounds, query rules and disclosure fences.

    def knowledge_explore(self, request: dict[str, Any]) -> dict:
        return self._packet('tos_knowledge_explore', {'request': request}, source_errors=False)

    def knowledge_contracts(self) -> dict:
        return self._packet('tos_knowledge_contracts', {}, source_errors=False)

    def _knowledge_registry(self, key: str, schema_version: str) -> dict:
        packet = self.knowledge_contracts()
        if (packet.get("schema") != "tos_knowledge_contract_bundle_v1"
                or type(packet.get("contracts")) is not dict):
            raise ValueError("Native knowledge contract bundle has an invalid shape")
        registry = packet["contracts"].get(key)
        if (type(registry) is not dict
                or registry.get("schema_version") != schema_version):
            raise ValueError(f"Native selected {key} has an invalid shape")
        return registry

    def entity_type_registry(self) -> dict:
        """Return the exact selected entity registry carried by native contracts."""
        return self._knowledge_registry(
            "entity_type_registry", "tos_semantic_entity_type_registry_v1")

    def relation_type_registry(self) -> dict:
        """Return the exact selected relation registry carried by native contracts."""
        return self._knowledge_registry(
            "relation_type_registry", "tos_semantic_relation_type_registry_v1")

    def knowledge_search_capabilities(self) -> dict:
        return self._packet('tos_knowledge_search_capabilities', {}, source_errors=False)

    def knowledge_node(self, node_id: str, relation_limit: int=200) -> dict:
        return self._packet('tos_knowledge_node', {'node_id': node_id, 'relation_limit': relation_limit}, source_errors=False)

    def knowledge_relation(self, relation_id: str) -> dict:
        return self._packet('tos_knowledge_relation', {'relation_id': relation_id}, source_errors=False)

    def knowledge_temporal_compare(self, request: dict[str, Any]) -> dict:
        return self._packet('tos_knowledge_temporal_compare', {'request': request}, source_errors=False)

    def knowledge_focus(self, node_id: str, *, sources: list[str] | None=None, depth: int=1, direction: str='either', predicate_ids: list[str] | None=None, node_limit: int=200, relation_limit: int=400, profile: str='overview') -> dict:
        return self._packet('tos_knowledge_focus', {'node_id': node_id, 'sources': sources, 'depth': depth, 'direction': direction, 'predicate_ids': predicate_ids, 'node_limit': node_limit, 'relation_limit': relation_limit, 'profile': profile}, source_errors=False)

    def compile_knowledge_lens(self, spec: dict[str, Any]) -> dict:
        return self._packet('tos_knowledge_lens_compile', {'spec': spec}, source_errors=False)

    def stored_knowledge_lens(self, lens_id: str) -> dict:
        return self._packet('tos_knowledge_lens_open', {'lens_id': lens_id}, source_errors=False)

    def source_descend(self, node_id: str, max_depth: int=8, limit: int=300) -> dict:
        return self._packet('tos_source_descend', {'node_id': node_id, 'max_depth': max_depth, 'limit': limit}, source_errors=False)

    def source_dossier(self, object_id: str, limit: int=300) -> dict:
        return self._packet('tos_dossier_inspect', {'object_id': object_id, 'limit': limit}, source_errors=False)

    def status(self) -> dict:
        return self._packet('tos_corpus_status', {}, source_errors=False)

    def summary(self) -> dict:
        return self._packet('tos_corpus_summary', {}, source_errors=False)

    def search(self, query: str, limit: int=20, resource_kind: str | None=None) -> dict:
        return self._packet('tos_corpus_search', {'query': query, 'limit': limit, 'resource_kind': resource_kind}, source_errors=False)

    def resources(self, resource_kind: str | None=None, owner_branch: str | None=None, limit: int=100) -> dict:
        return self._packet('tos_corpus_resources', {'resource_kind': resource_kind, 'owner_branch': owner_branch, 'limit': limit}, source_errors=False)

    def node(self, node_id: str) -> dict:
        return self._packet('tos_corpus_node', {'node_id': node_id}, source_errors=False)

    def relation_pack(self, pack_id: str) -> dict:
        return self._packet('tos_corpus_relation_pack', {'pack_id': pack_id}, source_errors=False)

    def graph_view(self, view_id: str, limit: int=100) -> dict:
        return self._packet('tos_corpus_graph_view', {'view_id': view_id, 'limit': limit}, source_errors=False)

    def packet(self, query: str='', view_id: str | None=None, limit: int=20) -> dict:
        return self._packet('tos_corpus_packet', {'query': query, 'view_id': view_id, 'limit': limit}, source_errors=False)

    def philosophy_projection(self) -> dict:
        """Return one complete selected Original carrier under its native fence."""
        return self._packet('tos_philosophy_graph_scale_rows',
                            {'full_projection': True}, source_errors=False)

    def philosophy_status(self) -> dict:
        return self._packet('tos_philosophy_graph_status', {}, source_errors=False)

    def philosophy_views(self) -> dict:
        return self._packet('tos_philosophy_graph_views', {}, source_errors=False)

    def philosophy_layers(self) -> dict:
        return self._packet('tos_philosophy_graph_layers', {}, source_errors=False)

    def philosophy_contracts(self) -> dict:
        return self._packet('tos_philosophy_graph_contracts', {}, source_errors=False)

    def philosophy_view(self, view_id: str, limit: int=1000) -> dict:
        return self._packet('tos_philosophy_graph_view', {'view_id': view_id, 'limit': limit}, source_errors=False)

    def philosophy_clusters(self, view_id: str | None=None, cluster_kind: str | None=None, limit: int=80) -> dict:
        return self._packet('tos_philosophy_graph_clusters', {'view_id': view_id, 'cluster_kind': cluster_kind, 'limit': limit}, source_errors=False)

    def philosophy_scale_manifest(self, view_id: str | None=None, layers: list[str] | None=None) -> dict:
        return self._packet('tos_philosophy_graph_scale_manifest', {'view_id': view_id, 'layers': [] if layers is None else layers}, source_errors=False)

    def philosophy_scale_rows(self, table: str, view_id: str | None = None,
                              layers: list[str] | None = None) -> list[dict]:
        """Export one complete Rust-selected table under its held response budget."""
        packet = self._packet('tos_philosophy_graph_scale_rows', {
            'table': table, 'view_id': view_id,
            'layers': [] if layers is None else layers, 'export': True,
        }, source_errors=False)
        if (type(packet.get('rows')) is not list
                or packet.get('next_offset') is not None
                or packet.get('row_count') != packet.get('total_row_count')):
            raise ValueError('native scale export did not return one complete table')
        return packet['rows']

    def philosophy_scale_packet(self, table: str, view_id: str | None=None, layers: list[str] | None=None, offset: int=0, limit: int=1000) -> dict:
        return self._packet('tos_philosophy_graph_scale_rows', {'table': table, 'view_id': view_id, 'layers': [] if layers is None else layers, 'offset': offset, 'limit': limit}, source_errors=False)

    def philosophy_review_packet(self, view_id: str='chronology') -> dict:
        return self._packet('tos_philosophy_graph_review_packet', {'view_id': view_id}, source_errors=False)

    def philosophy_snapshot(self) -> dict:
        return self._packet('tos_philosophy_graph_snapshot', {}, source_errors=False)

    def philosophy_audit(self) -> dict:
        return self._packet('tos_philosophy_graph_audit', {}, source_errors=False)

    def _philosophy_audit_fields(self) -> tuple[bool, str, dict]:
        packet = self.philosophy_audit()
        if (packet.get("schema") != "tos_philosophy_mcp_audit_v1"
                or type(packet.get("audit_exists")) is not bool
                or not isinstance(packet.get("audit_path"), str)
                or type(packet.get("audit")) is not dict):
            raise ValueError("Native philosophy audit packet has an invalid shape")
        return packet["audit_exists"], packet["audit_path"], packet["audit"]

    def philosophy_audit_exists(self) -> bool:
        exists, _, _ = self._philosophy_audit_fields()
        return exists

    def philosophy_audit_payload(self) -> dict:
        exists, path, payload = self._philosophy_audit_fields()
        if not exists:
            raise FileNotFoundError(path)
        return payload

    def philosophy_unresolved(self, view_id: str | None=None) -> dict:
        return self._packet('tos_philosophy_graph_unresolved', {'view_id': view_id}, source_errors=False)

    def philosophy_node(self, node_id: str) -> dict:
        return self._packet('tos_philosophy_graph_node', {'node_id': node_id}, source_errors=False)

    def philosophy_edge(self, edge_id: str) -> dict:
        return self._packet('tos_philosophy_graph_edge', {'edge_id': edge_id}, source_errors=False)

    def philosophy_epistemic_packet(self, item_id: str, view_id: str | None=None, limit: int=80) -> dict:
        return self._packet('tos_philosophy_epistemic_packet', {'item_id': item_id, 'view_id': view_id, 'limit': limit}, source_errors=False)

    def evidence_lens_packet(self, mode: str, item_id: str, view_id: str | None=None, limit: int=80) -> dict:
        return self._packet('tos_evidence_lens', {'mode': mode, 'item_id': item_id, 'view_id': view_id, 'limit': limit}, source_errors=False)

    def philosophy_neighborhood(self, node_id: str, depth: int=1, layers: list[str] | None=None, predicates: list[str] | None=None, limit: int=80) -> dict:
        return self._packet('tos_philosophy_graph_neighborhood', {'node_id': node_id, 'depth': depth, 'layers': [] if layers is None else layers, 'predicates': [] if predicates is None else predicates, 'limit': limit}, source_errors=False)

    def philosophy_path_between(self, from_id: str, to_id: str, layers: list[str] | None=None, predicates: list[str] | None=None, max_depth: int=6, direction: str='outgoing', view_id: str | None=None, excluded_edge_ids: list[str] | None=None, alternative_limit: int=1) -> dict:
        return self._packet('tos_philosophy_graph_path', {'from_id': from_id, 'to_id': to_id, 'layers': [] if layers is None else layers, 'predicates': [] if predicates is None else predicates, 'max_depth': max_depth, 'direction': direction, 'view_id': view_id, 'excluded_edge_ids': [] if excluded_edge_ids is None else excluded_edge_ids, 'alternative_limit': alternative_limit}, source_errors=False)

    def philosophy_search(self, query: str, limit: int=20) -> dict:
        return self._packet('tos_philosophy_graph_search', {'query': query, 'limit': limit}, source_errors=False)

    def philosophy_packet(self, query: str='', view_id: str | None=None, limit: int=20) -> dict:
        return self._packet('tos_philosophy_graph_packet', {'query': query, 'view_id': view_id, 'limit': limit}, source_errors=False)

    def knowledge_search(self, query: str='', *, sources: list[str] | None=None, kind_ids: list[str] | None=None, predicate_ids: list[str] | None=None, offset: int=0, limit: int=40) -> dict:
        return self._packet("tos_knowledge_search", {'query': query, 'sources': sources, 'kind_ids': kind_ids, 'predicate_ids': predicate_ids, 'offset': offset, 'limit': limit, 'mode': 'legacy'}, source_errors=False)

    def knowledge_search_indexed(self, query: str='', *, sources: list[str] | None=None, kind_ids: list[str] | None=None, predicate_ids: list[str] | None=None, cursor: str | None=None, limit: int=40, search_read_model: dict[str, Any] | None=None) -> dict:
        request = {'query': query, 'sources': sources, 'kind_ids': kind_ids,
                   'predicate_ids': predicate_ids, 'cursor': cursor, 'limit': limit,
                   'mode': 'indexed'}
        if search_read_model is not None:
            request['search_read_model'] = search_read_model
        return self._packet("tos_knowledge_search", request, source_errors=False)

    def knowledge_search_compressed(self, query: str='', *, sources: list[str] | None=None, kind_ids: list[str] | None=None, predicate_ids: list[str] | None=None, cursor: str | None=None, limit: int=40) -> dict:
        return self._packet("tos_knowledge_search", {'query': query, 'sources': sources, 'kind_ids': kind_ids, 'predicate_ids': predicate_ids, 'cursor': cursor, 'limit': limit, 'mode': 'compressed'}, source_errors=False)

    def render_resource(self, uri: str) -> str:
        """Return the native resource's exact text carrier."""
        from mcp.types import TextResourceContents
        result = self._native_result("read_resource", uri)
        if len(result.contents) != 1 or not isinstance(result.contents[0], TextResourceContents):
            raise ValueError("Native Core resource requires one text carrier")
        return result.contents[0].text

    def read_resource(self, uri: str) -> dict:
        """Decode the already authorized native resource packet."""
        import json
        result = json.loads(self.render_resource(uri))
        if type(result) is not dict:
            raise ValueError("Native Core resource did not return an object packet")
        return result

    def source_gap_search(self, query: str, limit: int = 20) -> dict:
        return self._packet("tos_source_gap_search", {"query": query, "limit": limit}, source_errors=False)

    def philosophy_lens_packet(self, view_id: str, limit: int = 20) -> dict:
        return self._packet("tos_philosophy_graph_lens_packet", {"view_id": view_id, "limit": limit}, source_errors=False)

    def zarathustra_word_analysis_public_capability(self) -> dict:
        return self._packet("tos_zarathustra_word_analysis_public_capability", {}, source_errors=False)

    def zarathustra_reading_public_capability(self) -> dict:
        return self._packet("tos_zarathustra_reading_public_capability", {}, source_errors=False)
