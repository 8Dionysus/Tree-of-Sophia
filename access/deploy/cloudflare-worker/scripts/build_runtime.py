#!/usr/bin/env python3
"""Build the reproducible Cloudflare edge read model from ToS-owned exports."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path
from typing import Any, Iterable


WORKER_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = Path(__file__).resolve().parents[4]
ACCESS_SRC = REPO_ROOT / "access" / "src"
if ACCESS_SRC.as_posix() not in sys.path:
    sys.path.insert(0, ACCESS_SRC.as_posix())

from tos_access.core import ToSAccessCore  # noqa: E402


CORPUS_COLLECTIONS = ("nodes", "resources", "manifests", "branches", "graph_views")
STATIC_PHILOSOPHY_LIMITS = (1, 1000)
STATIC_CORPUS_LIMITS = (1, 100, 700, 1000)
SQL_CHUNK_CHARS = 40_000


def compact_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def sql_text(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def sql_nullable(value: str | None) -> str:
    return "NULL" if value is None else sql_text(value)


def object_list(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item) for item in value if isinstance(item, str) and item]


def mask_for(values: Iterable[str], positions: dict[str, int]) -> int:
    mask = 0
    for value in values:
        position = positions.get(value)
        if position is not None:
            mask |= 1 << position
    return mask


def item_identity(item: dict[str, Any], fallback: str) -> str:
    for key in (
        "node_id",
        "edge_id",
        "cluster_id",
        "view_id",
        "resource_id",
        "manifest_id",
        "pack_id",
        "id",
        "path",
        "layer_id",
    ):
        value = item.get(key)
        if isinstance(value, str) and value:
            return value
    return fallback


def normalize_paths(value: Any, root: Path) -> Any:
    prefix = root.resolve().as_posix() + "/"
    if isinstance(value, str):
        if value == root.resolve().as_posix():
            return "Tree-of-Sophia"
        if value.startswith(prefix):
            return value.removeprefix(prefix)
        return value
    if isinstance(value, list):
        return [normalize_paths(item, root) for item in value]
    if isinstance(value, dict):
        return {key: normalize_paths(item, root) for key, item in value.items()}
    return value


def write_json(root: Path, relative: str, value: Any) -> None:
    target = root / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(compact_json(value) + "\n", encoding="utf-8")


def write_text(root: Path, relative: str, value: str) -> None:
    target = root / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(value, encoding="utf-8")


def build_static_assets(core: ToSAccessCore, output: Path) -> dict[str, Any]:
    web_dist = REPO_ROOT / "access" / "web" / "dist"
    if not (web_dist / "index.html").is_file():
        raise RuntimeError("missing access/web/dist; run the web build first")
    if output.exists():
        shutil.rmtree(output)
    shutil.copytree(web_dist, output)

    health = {
        "service": "tree-of-sophia-access",
        "ok": True,
        "write_enabled": False,
        "errors": [],
        "runtime": "cloudflare-worker",
    }
    write_json(output, "__edge/health.json", health)

    corpus_status = normalize_paths(core.status(), REPO_ROOT)
    corpus_summary = normalize_paths(core.summary(), REPO_ROOT)
    write_json(output, "__edge/corpus/status.json", corpus_status)
    write_json(output, "__edge/corpus/summary.json", corpus_summary)
    for view_id in corpus_status.get("graph_views", []):
        for limit in STATIC_CORPUS_LIMITS:
            packet = normalize_paths(core.graph_view(str(view_id), limit=limit), REPO_ROOT)
            write_json(output, f"__edge/corpus/graph-views/{view_id}/{limit}.json", packet)

    philosophy_status = normalize_paths(core.philosophy_status(), REPO_ROOT)
    philosophy_views = normalize_paths(core.philosophy_views(), REPO_ROOT)
    write_json(output, "__edge/philosophy/status.json", philosophy_status)
    write_json(output, "__edge/philosophy/views.json", philosophy_views)
    write_json(output, "__edge/philosophy/layers.json", normalize_paths(core.philosophy_layers(), REPO_ROOT))
    write_json(output, "__edge/philosophy/contracts.json", normalize_paths(core.philosophy_contracts(), REPO_ROOT))
    write_json(output, "__edge/philosophy/snapshot.json", normalize_paths(core.philosophy_snapshot(), REPO_ROOT))
    write_json(output, "__edge/philosophy/audit.json", normalize_paths(core.philosophy_audit(), REPO_ROOT))
    write_json(output, "__edge/philosophy/unresolved/all.json", normalize_paths(core.philosophy_unresolved(), REPO_ROOT))
    for view in philosophy_views.get("views", []):
        view_id = str(view.get("view_id") or "")
        if not view_id:
            continue
        write_json(
            output,
            f"__edge/philosophy/review-packet/{view_id}.json",
            normalize_paths(core.philosophy_review_packet(view_id), REPO_ROOT),
        )
        write_json(
            output,
            f"__edge/philosophy/unresolved/{view_id}.json",
            normalize_paths(core.philosophy_unresolved(view_id), REPO_ROOT),
        )
        for limit in STATIC_PHILOSOPHY_LIMITS:
            packet = normalize_paths(core.philosophy_view(view_id, limit=limit), REPO_ROOT)
            write_json(output, f"__edge/philosophy/views/{view_id}/{limit}.json", packet)

    headers = """/*
  Content-Security-Policy: default-src 'self'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'; worker-src 'self'
  Permissions-Policy: tools=(self), accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
  Cross-Origin-Resource-Policy: same-origin
  Origin-Agent-Cluster: ?1
  Referrer-Policy: no-referrer
  X-Content-Type-Options: nosniff
  X-Frame-Options: DENY

/static/*
  Cache-Control: public, max-age=31536000, immutable

/__edge/*
  Cache-Control: public, max-age=31536000, immutable
"""
    write_text(output, "_headers", headers)
    return {"corpus": corpus_status, "philosophy": philosophy_status}


def compact_philosophy_aux(
    collection: str,
    item: dict[str, Any],
) -> dict[str, Any]:
    result = dict(item)
    if collection == "views":
        node_ids = string_list(result.pop("node_ids", []))
        edge_ids = string_list(result.pop("edge_ids", []))
        result.pop("nodes", None)
        result.pop("edges", None)
        result["node_count"] = len(node_ids)
        result["edge_count"] = len(edge_ids)
    elif collection == "clusters":
        result["member_node_count"] = len(string_list(result.pop("member_node_ids", [])))
        result["member_edge_count"] = len(string_list(result.pop("member_edge_ids", [])))
    elif collection == "review_packets":
        diagnostics = result.pop("unresolved_diagnostics", [])
        result["unresolved_diagnostic_count"] = len(diagnostics) if isinstance(diagnostics, list) else 0
    return result


def edge_with_corpus_source_ref(
    edge: dict[str, Any],
    pack_paths: dict[str, str],
) -> dict[str, Any]:
    result = dict(edge)
    if not result.get("source_ref"):
        path = pack_paths.get(str(result.get("pack_id") or ""))
        if path:
            result["source_ref"] = path
    return result


def sql_insert(table: str, columns: tuple[str, ...], values: tuple[str, ...]) -> str:
    return f"INSERT INTO {table} ({','.join(columns)}) VALUES ({','.join(values)});"


def chunk_text(value: str, size: int = SQL_CHUNK_CHARS) -> list[str]:
    return [value[offset : offset + size] for offset in range(0, len(value), size)] or [""]


def build_read_model_sql(core: ToSAccessCore, target: Path, revision: str) -> dict[str, int]:
    philosophy = core.philosophy_projection()
    corpus = core.index()
    evidence = core.evidence_projection()
    audit = core.philosophy_audit_payload() if core.philosophy_audit_exists() else {}
    word_analysis_capability = core.zarathustra_word_analysis_task("__edge_capability_probe__")
    if word_analysis_capability.get("available") is True:
        raise RuntimeError(
            "the local Zarathustra word-analysis provider is available but has no Cloudflare edge adapter"
        )

    views = object_list(philosophy.get("views"))
    view_positions = {
        str(view.get("view_id")): index
        for index, view in enumerate(views)
        if isinstance(view.get("view_id"), str)
    }
    layers = object_list(philosophy.get("graph_layers"))
    layer_positions = {
        str(layer.get("layer_id")): index
        for index, layer in enumerate(layers)
        if isinstance(layer.get("layer_id"), str)
    }

    statements = [
        "PRAGMA foreign_keys=OFF;",
        "DROP TABLE IF EXISTS edge_meta_next;",
        "DROP TABLE IF EXISTS philosophy_nodes_next;",
        "DROP TABLE IF EXISTS philosophy_edges_next;",
        "DROP TABLE IF EXISTS philosophy_aux_next;",
        "DROP TABLE IF EXISTS philosophy_clusters_next;",
        "DROP TABLE IF EXISTS philosophy_cluster_nodes_next;",
        "DROP TABLE IF EXISTS philosophy_cluster_edges_next;",
        "DROP TABLE IF EXISTS philosophy_review_packets_next;",
        "DROP TABLE IF EXISTS corpus_items_next;",
        "DROP TABLE IF EXISTS corpus_edges_next;",
        "DROP TABLE IF EXISTS corpus_packs_next;",
        "CREATE TABLE edge_meta_next (key TEXT PRIMARY KEY, json TEXT NOT NULL);",
        "CREATE TABLE philosophy_nodes_next (id TEXT PRIMARY KEY, ord INTEGER NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, search_text TEXT NOT NULL);",
        "CREATE TABLE philosophy_edges_next (id TEXT PRIMARY KEY, ord INTEGER NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, predicate_id TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, search_text TEXT NOT NULL);",
        "CREATE TABLE philosophy_aux_next (collection TEXT NOT NULL, ord INTEGER NOT NULL, id TEXT NOT NULL, json TEXT NOT NULL, search_text TEXT NOT NULL, PRIMARY KEY (collection, ord));",
        "CREATE TABLE philosophy_clusters_next (id TEXT NOT NULL, ord INTEGER NOT NULL, sort_key TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY (id, part));",
        "CREATE TABLE philosophy_cluster_nodes_next (cluster_id TEXT NOT NULL, cluster_ord INTEGER NOT NULL, sort_key TEXT NOT NULL, member_ord INTEGER NOT NULL, item_id TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, PRIMARY KEY (cluster_id, member_ord));",
        "CREATE TABLE philosophy_cluster_edges_next (cluster_id TEXT NOT NULL, cluster_ord INTEGER NOT NULL, sort_key TEXT NOT NULL, member_ord INTEGER NOT NULL, item_id TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, PRIMARY KEY (cluster_id, member_ord));",
        "CREATE TABLE philosophy_review_packets_next (view_id TEXT PRIMARY KEY, json TEXT NOT NULL);",
        "CREATE TABLE corpus_items_next (collection TEXT NOT NULL, ord INTEGER NOT NULL, id TEXT NOT NULL, resource_kind TEXT, owner_branch TEXT, json TEXT NOT NULL, search_text TEXT NOT NULL, PRIMARY KEY (collection, ord));",
        "CREATE TABLE corpus_edges_next (id TEXT NOT NULL, ord INTEGER PRIMARY KEY, from_id TEXT NOT NULL, to_id TEXT NOT NULL, pack_id TEXT, owner_branch TEXT, json TEXT NOT NULL);",
        "CREATE TABLE corpus_packs_next (id TEXT PRIMARY KEY, ord INTEGER NOT NULL, json TEXT NOT NULL);",
    ]

    philosophy_top = {
        key: value
        for key, value in philosophy.items()
        if key not in {"nodes", "edges", "clusters", "views", "review_packets", "graph_layers"}
    }
    corpus_top = {
        key: value
        for key, value in corpus.items()
        if key
        not in {
            "nodes",
            "resources",
            "manifests",
            "branches",
            "graph_views",
            "relation_edges",
            "relation_packs",
        }
    }
    metadata = {
        "data_revision": {"sha256": revision},
        "view_positions": view_positions,
        "layer_positions": layer_positions,
        "philosophy_top": normalize_paths(philosophy_top, REPO_ROOT),
        "corpus_top": normalize_paths(corpus_top, REPO_ROOT),
        "evidence_projection": normalize_paths(evidence, REPO_ROOT),
        "philosophy_audit": normalize_paths(audit, REPO_ROOT),
        "word_analysis_capability": normalize_paths(word_analysis_capability, REPO_ROOT),
    }
    for key, value in metadata.items():
        statements.append(
            sql_insert("edge_meta_next", ("key", "json"), (sql_text(key), sql_text(compact_json(value))))
        )

    philosophy_nodes = object_list(philosophy.get("nodes"))
    for order, item in enumerate(philosophy_nodes):
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        statements.append(
            sql_insert(
                "philosophy_nodes_next",
                ("id", "ord", "view_mask", "layer_mask", "json", "search_text"),
                (
                    sql_text(str(item.get("node_id") or "")),
                    str(order),
                    str(mask_for(string_list(item.get("view_ids")), view_positions)),
                    str(mask_for(string_list(item.get("graph_layers")), layer_positions)),
                    sql_text(item_json),
                    sql_text(item_json.lower()),
                ),
            )
        )

    philosophy_edges = object_list(philosophy.get("edges"))
    for order, item in enumerate(philosophy_edges):
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        statements.append(
            sql_insert(
                "philosophy_edges_next",
                (
                    "id",
                    "ord",
                    "from_id",
                    "to_id",
                    "predicate_id",
                    "view_mask",
                    "layer_mask",
                    "json",
                    "search_text",
                ),
                (
                    sql_text(str(item.get("edge_id") or "")),
                    str(order),
                    sql_text(str(item.get("from_id") or "")),
                    sql_text(str(item.get("to_id") or "")),
                    sql_text(str(item.get("predicate_id") or "")),
                    str(mask_for(string_list(item.get("view_ids")), view_positions)),
                    str(mask_for(string_list(item.get("graph_layers")), layer_positions)),
                    sql_text(item_json),
                    sql_text(item_json.lower()),
                ),
            )
        )

    for collection in ("views", "clusters", "review_packets", "graph_layers"):
        for order, item in enumerate(object_list(philosophy.get(collection))):
            compact_item = compact_philosophy_aux(collection, item)
            item_json = compact_json(normalize_paths(compact_item, REPO_ROOT))
            statements.append(
                sql_insert(
                    "philosophy_aux_next",
                    ("collection", "ord", "id", "json", "search_text"),
                    (
                        sql_text(collection),
                        str(order),
                        sql_text(item_identity(item, f"{collection}:{order}")),
                        sql_text(item_json),
                        sql_text(item_json.lower()),
                    ),
                )
            )

    philosophy_clusters = object_list(philosophy.get("clusters"))
    cluster_node_memberships = 0
    cluster_edge_memberships = 0
    for cluster_order, item in enumerate(philosophy_clusters):
        cluster_id = str(item.get("cluster_id") or "")
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        sort_key = f"{item.get('cluster_kind') or ''}\u241f{item.get('label') or ''}"
        cluster_view_mask = mask_for(string_list(item.get("view_ids")), view_positions)
        cluster_layer_mask = mask_for(string_list(item.get("graph_layers")), layer_positions)
        for part, chunk in enumerate(chunk_text(item_json)):
            statements.append(
                sql_insert(
                    "philosophy_clusters_next",
                    ("id", "ord", "sort_key", "view_mask", "layer_mask", "part", "json_chunk"),
                    (
                        sql_text(cluster_id),
                        str(cluster_order),
                        sql_text(sort_key),
                        str(cluster_view_mask),
                        str(cluster_layer_mask),
                        str(part),
                        sql_text(chunk),
                    ),
                )
            )
        membership_common = {
            "cluster_id": item.get("cluster_id"),
            "source_ref": item.get("source_ref"),
            "source_refs": item.get("source_refs", []),
        }
        for member_order, node_id in enumerate(string_list(item.get("member_node_ids"))):
            membership = normalize_paths({**membership_common, "node_id": node_id}, REPO_ROOT)
            statements.append(
                sql_insert(
                    "philosophy_cluster_nodes_next",
                    ("cluster_id", "cluster_ord", "sort_key", "member_ord", "item_id", "view_mask", "layer_mask", "json"),
                    (
                        sql_text(cluster_id),
                        str(cluster_order),
                        sql_text(sort_key),
                        str(member_order),
                        sql_text(node_id),
                        str(cluster_view_mask),
                        str(cluster_layer_mask),
                        sql_text(compact_json(membership)),
                    ),
                )
            )
            cluster_node_memberships += 1
        for member_order, edge_id in enumerate(string_list(item.get("member_edge_ids"))):
            membership = normalize_paths({**membership_common, "edge_id": edge_id}, REPO_ROOT)
            statements.append(
                sql_insert(
                    "philosophy_cluster_edges_next",
                    ("cluster_id", "cluster_ord", "sort_key", "member_ord", "item_id", "view_mask", "layer_mask", "json"),
                    (
                        sql_text(cluster_id),
                        str(cluster_order),
                        sql_text(sort_key),
                        str(member_order),
                        sql_text(edge_id),
                        str(cluster_view_mask),
                        str(cluster_layer_mask),
                        sql_text(compact_json(membership)),
                    ),
                )
            )
            cluster_edge_memberships += 1

    for item in object_list(philosophy.get("review_packets")):
        view_id = str(item.get("view_id") or "")
        statements.append(
            sql_insert(
                "philosophy_review_packets_next",
                ("view_id", "json"),
                (sql_text(view_id), sql_text(compact_json(normalize_paths(item, REPO_ROOT)))),
            )
        )

    corpus_items_count = 0
    for collection in CORPUS_COLLECTIONS:
        for order, item in enumerate(object_list(corpus.get(collection))):
            item_json = compact_json(normalize_paths(item, REPO_ROOT))
            statements.append(
                sql_insert(
                    "corpus_items_next",
                    ("collection", "ord", "id", "resource_kind", "owner_branch", "json", "search_text"),
                    (
                        sql_text(collection),
                        str(order),
                        sql_text(item_identity(item, f"{collection}:{order}")),
                        sql_nullable(item.get("resource_kind") if isinstance(item.get("resource_kind"), str) else None),
                        sql_nullable(item.get("owner_branch") if isinstance(item.get("owner_branch"), str) else None),
                        sql_text(item_json),
                        sql_text(item_json.lower()),
                    ),
                )
            )
            corpus_items_count += 1

    pack_paths = {
        str(pack.get("pack_id")): str(pack.get("path"))
        for pack in object_list(corpus.get("relation_packs"))
        if isinstance(pack.get("pack_id"), str) and isinstance(pack.get("path"), str)
    }
    corpus_edges = [
        edge_with_corpus_source_ref(item, pack_paths)
        for item in object_list(corpus.get("relation_edges"))
    ]
    for order, item in enumerate(corpus_edges):
        statements.append(
            sql_insert(
                "corpus_edges_next",
                ("id", "ord", "from_id", "to_id", "pack_id", "owner_branch", "json"),
                (
                    sql_text(str(item.get("edge_id") or f"corpus-edge:{order}")),
                    str(order),
                    sql_text(str(item.get("from_id") or "")),
                    sql_text(str(item.get("to_id") or "")),
                    sql_nullable(item.get("pack_id") if isinstance(item.get("pack_id"), str) else None),
                    sql_nullable(item.get("owner_branch") if isinstance(item.get("owner_branch"), str) else None),
                    sql_text(compact_json(normalize_paths(item, REPO_ROOT))),
                ),
            )
        )

    corpus_packs = object_list(corpus.get("relation_packs"))
    for order, item in enumerate(corpus_packs):
        statements.append(
            sql_insert(
                "corpus_packs_next",
                ("id", "ord", "json"),
                (
                    sql_text(str(item.get("pack_id") or "")),
                    str(order),
                    sql_text(compact_json(normalize_paths(item, REPO_ROOT))),
                ),
            )
        )

    for table in (
        "edge_meta",
        "philosophy_nodes",
        "philosophy_edges",
        "philosophy_aux",
        "philosophy_clusters",
        "philosophy_cluster_nodes",
        "philosophy_cluster_edges",
        "philosophy_review_packets",
        "corpus_items",
        "corpus_edges",
        "corpus_packs",
    ):
        statements.append(f"DROP TABLE IF EXISTS {table};")
        statements.append(f"ALTER TABLE {table}_next RENAME TO {table};")

    statements.extend(
        (
            "CREATE INDEX philosophy_edges_from_idx ON philosophy_edges(from_id);",
            "CREATE INDEX philosophy_edges_to_idx ON philosophy_edges(to_id);",
            "CREATE INDEX philosophy_cluster_nodes_item_idx ON philosophy_cluster_nodes(item_id);",
            "CREATE INDEX philosophy_cluster_edges_item_idx ON philosophy_cluster_edges(item_id);",
            "CREATE INDEX corpus_items_id_idx ON corpus_items(id);",
            "CREATE INDEX corpus_edges_from_idx ON corpus_edges(from_id);",
            "CREATE INDEX corpus_edges_to_idx ON corpus_edges(to_id);",
            "CREATE INDEX corpus_edges_id_idx ON corpus_edges(id);",
            "PRAGMA optimize;",
        )
    )
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(statements) + "\n", encoding="utf-8")
    return {
        "philosophy_nodes": len(philosophy_nodes),
        "philosophy_edges": len(philosophy_edges),
        "philosophy_clusters": len(philosophy_clusters),
        "philosophy_cluster_node_memberships": cluster_node_memberships,
        "philosophy_cluster_edge_memberships": cluster_edge_memberships,
        "corpus_items": corpus_items_count,
        "corpus_edges": len(corpus_edges),
        "corpus_packs": len(corpus_packs),
        "sql_statements": len(statements),
    }


def data_revision(core: ToSAccessCore) -> str:
    digest = hashlib.sha256()
    for path in (
        core.index_path,
        core.philosophy_graph_projection_path,
        core.evidence_projection_path,
        core.philosophy_post_planting_audit_path,
    ):
        digest.update(path.relative_to(REPO_ROOT).as_posix().encode("utf-8"))
        digest.update(b"\0")
        if path.is_file():
            digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=WORKER_ROOT / "dist")
    parser.add_argument("--runtime", type=Path, default=WORKER_ROOT / "runtime")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    core = ToSAccessCore.discover(REPO_ROOT)
    revision = data_revision(core)
    static_summary = build_static_assets(core, args.output.resolve())
    counts = build_read_model_sql(core, (args.runtime / "read-model.sql").resolve(), revision)
    manifest = {
        "schema": "tos_cloudflare_edge_build_v1",
        "data_revision": revision,
        "source_owner": "Tree-of-Sophia",
        "source_paths": [
            core.index_path.relative_to(REPO_ROOT).as_posix(),
            core.philosophy_graph_projection_path.relative_to(REPO_ROOT).as_posix(),
            core.evidence_projection_path.relative_to(REPO_ROOT).as_posix(),
            core.philosophy_post_planting_audit_path.relative_to(REPO_ROOT).as_posix(),
        ],
        "counts": counts,
        "default_corpus_view": next(iter(static_summary["corpus"].get("graph_views", [])), ""),
        "default_philosophy_view": next(iter(static_summary["philosophy"].get("views", [])), "chronology"),
        "authority_limit": "Cloudflare carries a generated read model; ToS authored and reviewed surfaces remain authoritative.",
    }
    write_json(args.output.resolve(), "__edge/build-manifest.json", manifest)
    args.runtime.mkdir(parents=True, exist_ok=True)
    (args.runtime / "manifest.json").write_text(compact_json(manifest) + "\n", encoding="utf-8")
    print(compact_json(manifest))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
