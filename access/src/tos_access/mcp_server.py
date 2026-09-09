from __future__ import annotations

import json
import logging
import os
from pathlib import Path
from threading import Lock
from typing import Any

from .core import ToSAccessCore


LOGGER = logging.getLogger(__name__)
DEFAULT_HTTP_PORT = 5429


def _run_server(server: Any) -> None:
    transport = os.environ.get("TOS_MCP_TRANSPORT") or os.environ.get("AOA_MCP_TRANSPORT", "stdio").strip() or "stdio"
    if transport == "stdio":
        server.run(transport="stdio")
        return
    if transport != "streamable-http":
        raise SystemExit(f"unsupported AOA_MCP_TRANSPORT: {transport}")
    host = os.environ.get("TOS_MCP_HOST") or os.environ.get("AOA_MCP_HOST", "127.0.0.1").strip()
    if host not in {"127.0.0.1", "localhost", "::1"}:
        raise SystemExit("AOA_MCP_HOST must remain loopback-only")
    server.settings.host = host
    server.settings.port = int(os.environ.get("TOS_MCP_PORT") or os.environ.get("AOA_MCP_PORT", DEFAULT_HTTP_PORT))
    server.run(transport="streamable-http")


def build_server(
    tos_root: str | Path | None = None,
    index_path: str | Path | None = None,
    philosophy_graph_projection_path: str | Path | None = None,
    philosophy_post_planting_audit_path: str | Path | None = None,
) -> Any:
    try:
        from mcp.server.fastmcp import FastMCP  # type: ignore[import-not-found]
    except ImportError as exc:
        raise SystemExit("Missing dependency 'mcp'. Install with: python -m pip install -e .") from exc

    mcp = FastMCP("tree-of-sophia", json_response=True)
    state_lock = Lock()
    cached_state: ToSAccessCore | None = None

    def current_state() -> ToSAccessCore:
        nonlocal cached_state
        resolved = ToSAccessCore.discover(
            tos_root=tos_root,
            index_path=index_path,
            philosophy_graph_projection_path=philosophy_graph_projection_path,
            philosophy_post_planting_audit_path=philosophy_post_planting_audit_path,
        )
        # Discover path changes on every call, but keep the shared graph/index
        # for unchanged paths. Core readers still observe current file versions.
        # Equality excludes disposable indexes/checkpoints and compares paths.
        with state_lock:
            if cached_state is None or cached_state != resolved:
                cached_state = resolved
            return cached_state

    # Tools rediscover source paths on each call. Exploration keeps
    # its disposable checkpoints for the lifetime of this MCP server only.
    from .exploration import ExplorationService
    exploration = ExplorationService(lambda: current_state().knowledge_graph())

    @mcp.tool()
    def tos_knowledge_explore(request: dict[str, Any]) -> dict[str, Any]:
        """Start a read-only neighborhood or continue with cursor only; expires after 15 minutes.

        Fixed query/page sizes; no authored writes. Upsert context nodes by ID.
        Snapshot conflict or expired checkpoint requires restarting from focus.
        """
        return exploration.explore(request)

    @mcp.tool()
    def tos_knowledge_exploration_contracts() -> dict[str, Any]:
        """Read exploration capabilities and request/result schemas; local/native only."""
        return current_state().knowledge_exploration_contracts()

    @mcp.tool()
    def tos_corpus_status() -> dict[str, Any]:
        """Return ToS corpus index path, counts, graph views, and authority boundary."""
        return current_state().status()

    @mcp.tool()
    def tos_corpus_summary() -> dict[str, Any]:
        """Return a compact whole-corpus summary from the ToS-owned index."""
        return current_state().summary()

    @mcp.tool()
    def tos_corpus_search(query: str, limit: int = 20, resource_kind: str | None = None) -> dict[str, Any]:
        """Search nodes, resources, manifests, branches, and graph views in the ToS corpus index."""
        return current_state().search(query=query, limit=limit, resource_kind=resource_kind)

    @mcp.tool()
    def tos_knowledge_catalog() -> dict[str, Any]:
        """Return node kinds, predicates, fields, limits, and stored LensSpec definitions for the generic backend."""
        return current_state().knowledge_catalog()

    @mcp.tool()
    def tos_knowledge_contracts() -> dict[str, Any]:
        """Return the versioned API operation map and JSON Schemas used by human and agent constructors."""
        return current_state().knowledge_contracts()

    @mcp.tool()
    def tos_knowledge_search(
        query: str = "",
        sources: list[str] | None = None,
        kind_ids: list[str] | None = None,
        predicate_ids: list[str] | None = None,
        offset: int = 0,
        limit: int = 40,
    ) -> dict[str, Any]:
        """Search the unified display-complete graph without choosing a philosophy/corpus legacy mode."""
        return current_state().knowledge_search(
            query,
            sources=sources,
            kind_ids=kind_ids,
            predicate_ids=predicate_ids,
            offset=offset,
            limit=limit,
        )

    @mcp.tool()
    def tos_knowledge_node(node_id: str, relation_limit: int = 200) -> dict[str, Any]:
        """Inspect a normalized knowledge node and its human-readable related relations."""
        return current_state().knowledge_node(node_id, relation_limit)

    @mcp.tool()
    def tos_knowledge_relation(relation_id: str) -> dict[str, Any]:
        """Inspect a normalized relation together with its display-complete endpoints."""
        return current_state().knowledge_relation(relation_id)

    @mcp.tool()
    def tos_knowledge_temporal_compare(request: dict[str, Any]) -> dict[str, Any]:
        """Compare two exact Claim date envelopes, not event truth. Discover the request schema with tos_knowledge_contracts."""
        return current_state().knowledge_temporal_compare(request)

    @mcp.tool()
    def tos_knowledge_focus(
        node_id: str,
        sources: list[str] | None = None,
        depth: int = 1,
        direction: str = "either",
        predicate_ids: list[str] | None = None,
        node_limit: int = 200,
        relation_limit: int = 400,
        profile: str = "overview",
    ) -> dict[str, Any]:
        """Center a bounded radial lens on an exact or uniquely resolved node identity."""
        return current_state().knowledge_focus(
            node_id,
            sources=sources,
            depth=depth,
            direction=direction,
            predicate_ids=predicate_ids,
            node_limit=node_limit,
            relation_limit=relation_limit,
            profile=profile,
        )

    @mcp.tool()
    def tos_knowledge_lens_compile(spec: dict[str, Any]) -> dict[str, Any]:
        """Validate and execute an arbitrary bounded, read-only tos_lens_spec_v1 construction."""
        return current_state().compile_knowledge_lens(spec)

    @mcp.tool()
    def tos_knowledge_lens_open(lens_id: str) -> dict[str, Any]:
        """Execute a stored LensSpec through the same generic backend used for arbitrary constructions."""
        return current_state().stored_knowledge_lens(lens_id)

    @mcp.tool()
    def tos_source_descend(node_id: str, max_depth: int = 8, limit: int = 300) -> dict[str, Any]:
        """Walk from an era, region, tradition, planting, or source object down the source-navigation graph."""
        return current_state().source_descend(node_id=node_id, max_depth=max_depth, limit=limit)

    @mcp.tool()
    def tos_dossier_inspect(object_id: str, limit: int = 300) -> dict[str, Any]:
        """Return a compact dossier for one bibliographic carrier or Link without converting availability into a rights conclusion."""
        return current_state().source_dossier(object_id=object_id, limit=limit)

    @mcp.tool()
    def tos_corpus_resources(
        resource_kind: str | None = None,
        owner_branch: str | None = None,
        limit: int = 100,
    ) -> dict[str, Any]:
        """List indexed ToS resources with optional kind and owner-branch filters."""
        return current_state().resources(resource_kind=resource_kind, owner_branch=owner_branch, limit=limit)

    @mcp.tool()
    def tos_corpus_node(node_id: str) -> dict[str, Any]:
        """Return one indexed ToS node and relation edges connected to it."""
        return current_state().node(node_id=node_id)

    @mcp.tool()
    def tos_corpus_relation_pack(pack_id: str) -> dict[str, Any]:
        """Return one indexed ToS relation pack and its edges."""
        return current_state().relation_pack(pack_id=pack_id)

    @mcp.tool()
    def tos_corpus_graph_view(view_id: str, limit: int = 100) -> dict[str, Any]:
        """Return a named graph review view over the whole ToS corpus index."""
        return current_state().graph_view(view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_corpus_packet(query: str = "", view_id: str | None = None, limit: int = 20) -> dict[str, Any]:
        """Return a compact task packet with optional search and graph-view context."""
        return current_state().packet(query=query, view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_zarathustra_prepare_word_analysis(
        query: str,
        language: str = "ru",
        rank: int = 1,
        include_semantic_neighbors: bool = False,
    ) -> dict[str, Any]:
        """Return one exact-source local analysis task; never accept or persist its interpretation."""
        return current_state().zarathustra_word_analysis_task(
            query=query,
            language=language,
            rank=rank,
            include_semantic_neighbors=include_semantic_neighbors,
        )

    @mcp.tool()
    def tos_philosophy_graph_status() -> dict[str, Any]:
        """Return ToS philosophy graph projection path, counts, graph views, and authority boundary."""
        return current_state().philosophy_status()

    @mcp.tool()
    def tos_philosophy_graph_views() -> dict[str, Any]:
        """List ToS philosophy graph views materialized by the ToS-owned projection export."""
        return current_state().philosophy_views()

    @mcp.tool()
    def tos_philosophy_graph_layers() -> dict[str, Any]:
        """Return ToS-owned philosophy graph layers and layer counts for runtime filtering."""
        return current_state().philosophy_layers()

    @mcp.tool()
    def tos_philosophy_graph_contracts() -> dict[str, Any]:
        """Return the bounded MCP access contract for ToS philosophy graph packets."""
        return current_state().philosophy_contracts()

    @mcp.tool()
    def tos_philosophy_graph_scale_manifest(view_id: str | None = None, layers: list[str] | None = None) -> dict[str, Any]:
        """Return compact row counts and packet routes for ToS philosophy scale projection access."""
        return current_state().philosophy_scale_manifest(view_id=view_id, layers=layers)

    @mcp.tool()
    def tos_philosophy_graph_scale_rows(
        table: str,
        view_id: str | None = None,
        layers: list[str] | None = None,
        offset: int = 0,
        limit: int = 1000,
    ) -> dict[str, Any]:
        """Return a paginated normalized scale table, including cluster membership rows."""
        return current_state().philosophy_scale_packet(
            table=table,
            view_id=view_id,
            layers=layers,
            offset=offset,
            limit=limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_view(view_id: str, limit: int = 1000) -> dict[str, Any]:
        """Return one ToS philosophy graph view packet with projected nodes, edges, and source refs."""
        return current_state().philosophy_view(view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_clusters(
        view_id: str | None = None,
        cluster_kind: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return compact ToS philosophy graph clusters, optionally filtered by view and cluster kind."""
        return current_state().philosophy_clusters(view_id=view_id, cluster_kind=cluster_kind, limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_node(node_id: str) -> dict[str, Any]:
        """Return one projected ToS philosophy node and related projected edges."""
        return current_state().philosophy_node(node_id=node_id)

    @mcp.tool()
    def tos_philosophy_graph_edge(edge_id: str) -> dict[str, Any]:
        """Return one projected ToS philosophy edge and its endpoint nodes."""
        return current_state().philosophy_edge(edge_id=edge_id)

    @mcp.tool()
    def tos_philosophy_epistemic_packet(
        item_id: str,
        view_id: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return projected source routes, challenge signals, and authority posture for one selected item."""
        return current_state().philosophy_epistemic_packet(item_id=item_id, view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_evidence_lens(
        mode: str,
        item_id: str,
        view_id: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return a public-safe Evidence Lens packet joining a selection to explicit owner routes and gaps."""
        return current_state().evidence_lens_packet(
            mode=mode,
            item_id=item_id,
            view_id=view_id,
            limit=limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_neighborhood(
        node_id: str,
        depth: int = 1,
        layers: list[str] | None = None,
        predicates: list[str] | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return the projected neighborhood around one ToS philosophy node."""
        return current_state().philosophy_neighborhood(
            node_id=node_id,
            depth=depth,
            layers=layers,
            predicates=predicates,
            limit=limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_path(
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
        """Return deterministic bounded paths with direction, view, and edge-exclusion constraints."""
        return current_state().philosophy_path_between(
            from_id=from_id,
            to_id=to_id,
            layers=layers,
            predicates=predicates,
            max_depth=max_depth,
            direction=direction,
            view_id=view_id,
            excluded_edge_ids=excluded_edge_ids,
            alternative_limit=alternative_limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_review_packet(view_id: str = "chronology") -> dict[str, Any]:
        """Return one compact ToS-owned review packet for a philosophy graph lens."""
        return current_state().philosophy_review_packet(view_id=view_id)

    @mcp.tool()
    def tos_philosophy_graph_snapshot() -> dict[str, Any]:
        """Return ToS-owned philosophy graph snapshot fingerprints for diff-aware review."""
        return current_state().philosophy_snapshot()

    @mcp.tool()
    def tos_philosophy_graph_audit() -> dict[str, Any]:
        """Return the ToS-owned post-planting audit packet when present."""
        return current_state().philosophy_audit()

    @mcp.tool()
    def tos_philosophy_graph_unresolved(view_id: str | None = None) -> dict[str, Any]:
        """Return unresolved review surfaces for all philosophy graph lenses or one selected lens."""
        return current_state().philosophy_unresolved(view_id=view_id)

    @mcp.tool()
    def tos_philosophy_graph_packet(query: str = "", view_id: str | None = None, limit: int = 20) -> dict[str, Any]:
        """Return a compact philosophy graph packet for agents with optional search and view context."""
        return current_state().philosophy_packet(query=query, view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_chronology_packet(limit: int = 20) -> dict[str, Any]:
        """Return the chronology lens packet for formation, fixation, canonization, and dating review."""
        return current_state().philosophy_lens_packet(view_id="chronology", limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_source_evidence_packet(limit: int = 20) -> dict[str, Any]:
        """Return the source-evidence lens packet for source refs, confidence, and witness review."""
        return current_state().philosophy_lens_packet(view_id="source-evidence", limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_concept_lineage_packet(limit: int = 20) -> dict[str, Any]:
        """Return the concept-lineage lens packet for concept/problem pressure and lineage review."""
        return current_state().philosophy_lens_packet(view_id="concept-lineage", limit=limit)

    @mcp.resource("tos-corpus://status")
    def status_resource() -> str:
        return json.dumps(current_state().status(), ensure_ascii=False, indent=2)

    @mcp.resource("tos-corpus://summary")
    def summary_resource() -> str:
        return json.dumps(current_state().summary(), ensure_ascii=False, indent=2)

    @mcp.resource("tos-corpus://graph-views")
    def graph_views_resource() -> str:
        return current_state().render_resource("tos-corpus://graph-views")

    @mcp.resource("tos-corpus://graph-view/{view_id}")
    def graph_view_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-corpus://graph-view/{view_id}")

    @mcp.resource("tos-philosophy://status")
    def philosophy_status_resource() -> str:
        return current_state().render_resource("tos-philosophy://status")

    @mcp.resource("tos-philosophy://views")
    def philosophy_views_resource() -> str:
        return current_state().render_resource("tos-philosophy://views")

    @mcp.resource("tos-philosophy://layers")
    def philosophy_layers_resource() -> str:
        return current_state().render_resource("tos-philosophy://layers")

    @mcp.resource("tos-philosophy://contracts")
    def philosophy_contracts_resource() -> str:
        return current_state().render_resource("tos-philosophy://contracts")

    @mcp.resource("tos-philosophy://scale-manifest")
    def philosophy_scale_manifest_resource() -> str:
        return current_state().render_resource("tos-philosophy://scale-manifest")

    @mcp.resource("tos-philosophy://snapshot")
    def philosophy_snapshot_resource() -> str:
        return current_state().render_resource("tos-philosophy://snapshot")

    @mcp.resource("tos-philosophy://audit")
    def philosophy_audit_resource() -> str:
        return current_state().render_resource("tos-philosophy://audit")

    @mcp.resource("tos-philosophy://clusters")
    def philosophy_clusters_resource() -> str:
        return current_state().render_resource("tos-philosophy://clusters")

    @mcp.resource("tos-philosophy://unresolved")
    def philosophy_unresolved_resource() -> str:
        return current_state().render_resource("tos-philosophy://unresolved")

    @mcp.resource("tos-philosophy://view/{view_id}")
    def philosophy_view_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://view/{view_id}")

    @mcp.resource("tos-philosophy://review-packet/{view_id}")
    def philosophy_review_packet_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://review-packet/{view_id}")

    @mcp.resource("tos-philosophy://edge/{edge_id}")
    def philosophy_edge_resource(edge_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://edge/{edge_id}")

    @mcp.resource("tos-philosophy://lens/{view_id}")
    def philosophy_lens_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://lens/{view_id}")

    @mcp.prompt(name="tos-corpus-review")
    def tos_corpus_review(view_id: str = "corpus-topology", query: str = "") -> str:
        """Prompt route for reviewing ToS corpus graph context."""
        return (
            f"Use tos_corpus_status(), then tos_corpus_packet(query={query!r}, view_id={view_id!r}). "
            "Treat Tree-of-Sophia source_refs returned by the packet as authority; treat native MCP and standalone runtime as read-only access surfaces."
        )

    @mcp.prompt(name="tos-philosophy-graph-review")
    def tos_philosophy_graph_review(view_id: str = "chronology", query: str = "") -> str:
        """Prompt route for reviewing ToS philosophy graph projection context."""
        return (
            f"Use tos_philosophy_graph_status(), tos_philosophy_graph_layers(), "
            f"tos_philosophy_graph_review_packet(view_id={view_id!r}), then "
            f"tos_philosophy_graph_packet(query={query!r}, view_id={view_id!r}). "
            "Treat ToS source_ref values as meaning authority; treat native MCP, UI, and optional integrations as projection/access surfaces only."
        )

    @mcp.prompt(name="tos-zarathustra-word-analysis")
    def tos_zarathustra_word_analysis(query: str, language: str = "ru", rank: int = 1) -> str:
        """Prompt route for source-first morphology, semantics, etymology, and English rendering."""
        return (
            "Call tos_zarathustra_prepare_word_analysis"
            f"(query={query!r}, language={language!r}, rank={rank}). "
            "Analyze every required stage in the returned task. Use point citations for etymology, "
            "keep German as source authority, Russian as a historical comparator, and English as an "
            "unreviewed candidate. Do not infer contextual meaning from etymology alone."
        )

    LOGGER.info("ToS corpus MCP server ready")
    return mcp


def main() -> None:
    logging.basicConfig(level=logging.INFO)
    _run_server(build_server())
