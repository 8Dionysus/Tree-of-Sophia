#!/usr/bin/env python3
"""Build the reproducible Cloudflare edge read model from ToS-owned exports."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sqlite3
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any, Iterable


WORKER_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = Path(__file__).resolve().parents[4]
ACCESS_SRC = REPO_ROOT / "access" / "src"
if ACCESS_SRC.as_posix() not in sys.path:
    sys.path.insert(0, ACCESS_SRC.as_posix())

from tos_access.core import ToSAccessCore, KNOWLEDGE_CONTRACT_RELATIVE_PATHS  # noqa: E402
from tos_access.portable_paths import normalize_paths  # noqa: E402
from tos_access import core as access_core  # noqa: E402
from tos_access.normalization_cache import NormalizationCache, normalization_processor_digest  # noqa: E402
from tos_access.processing import DEFAULT_CACHE_BYTES, DEFAULT_CACHE_ENTRIES  # noqa: E402
from tos_access.search_read_model import SEARCH_NGRAM_SIZE, SQLiteKnowledgeSearchReadModel  # noqa: E402
from tos_access.published_read_metadata import (  # noqa: E402
    emitted_row_digest,
    published_reader_metadata,
    published_row_digest_key,
    published_lens_metadata,
    lens_order_row,
)
_builder_dir = str(Path(__file__).resolve().parent)
if _builder_dir not in sys.path:
    sys.path.insert(0, _builder_dir)
from incremental_runtime import (  # noqa: E402
    MAX_D1_SQL_STATEMENT_BYTES,
    MAX_D1_SQL_ROW_VALUE_BYTES,
    DeltaRecorder,
    DiskRowBaseline,
)
from build_stages import BuildStages, atomic_json, build_lock, fingerprint, tree_paths  # noqa: E402


CORPUS_COLLECTIONS = ("nodes", "resources", "manifests", "branches", "graph_views")
STATIC_PHILOSOPHY_LIMITS = (1, 1000)
STATIC_CORPUS_LIMITS = (1, 100, 700, 1000)
SQL_CHUNK_BYTES = 32_000
READ_MODEL_SCHEMA_VERSION = "tos_cloudflare_edge_read_model_v9"
READ_MODEL_CONTENT_VERSION = "tos_cloudflare_edge_content_v2"
SEARCH_READ_MODEL_SCHEMA_VERSION = "tos_knowledge_search_read_model_v3"
SEARCH_READ_MODEL_MAX_POSTINGS = 10_000_000
PRODUCER_LOGICAL_BINDINGS_VERSION = "tos_producer_logical_bindings_v1"
MAX_PRODUCER_LOGICAL_BINDINGS = 128
MAX_PRODUCER_LOGICAL_BINDING_BYTES = 1_048_576
MAX_PRODUCER_LOGICAL_BINDING_NAME_BYTES = 4_096
MAX_PRODUCER_LOGICAL_BINDING_VALUE_BYTES = 65_536
MAX_PRODUCER_CARRIER_PATHS = 128
MAX_PRODUCER_CARRIER_LABEL_BYTES = 4_096
MAX_PRODUCER_CARRIER_PATH_BYTES = 4_096


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


def _legacy_carrier_paths(core: ToSAccessCore) -> tuple[tuple[str, Path], ...]:
    """Return the historical carrier order with producer-relative labels.

    This compatibility path is intentionally limited to carriers underneath
    the implementation root.  An owner-supplied scratch carrier set must use
    explicit logical labels, so it never needs to derive a label with
    ``Path.relative_to(REPO_ROOT)``.
    """
    paths = (
        core.index_path,
        core.philosophy_graph_projection_path,
        core.bibliographic_graph_path,
        core.entity_type_registry_path,
        core.relation_type_registry_path,
        core.evidence_projection_path,
        core.philosophy_post_planting_audit_path,
        *sorted((core.tos_root / "ToS/source-witnesses/access-requests/public-ledger").glob("*.access-request.json")),
    )
    result: list[tuple[str, Path]] = []
    for path in paths:
        path = Path(path).expanduser()
        try:
            label = path.relative_to(REPO_ROOT).as_posix()
        except ValueError as error:
            raise ValueError(
                "legacy core carriers must be under the producer implementation root; "
                "use ProducerCarrierSet.admit for external scratch paths"
            ) from error
        result.append((label, path))
    return tuple(result)


def _logical_bindings(value: Mapping[str, str] | Iterable[tuple[str, str]] | None) -> tuple[tuple[str, str], ...]:
    """Canonicalize owner-supplied logical source bindings without I/O."""
    if value is None:
        return ()
    try:
        items = iter(value.items() if isinstance(value, Mapping) else value)
    except TypeError as error:
        raise ValueError("producer logical source bindings must be iterable") from error
    result: list[tuple[str, str]] = []
    names: set[str] = set()
    for item in items:
        if len(result) >= MAX_PRODUCER_LOGICAL_BINDINGS:
            raise ValueError("producer logical source bindings exceed their count budget")
        if not isinstance(item, (tuple, list)) or len(item) != 2:
            raise ValueError("producer logical source bindings must be name/value pairs")
        name, binding = item
        if (type(name) is not str or not name or "\x00" in name
                or len(name.encode("utf-8")) > MAX_PRODUCER_LOGICAL_BINDING_NAME_BYTES
                or type(binding) is not str or not binding or "\x00" in binding
                or len(binding.encode("utf-8")) > MAX_PRODUCER_LOGICAL_BINDING_VALUE_BYTES):
            raise ValueError("producer logical source bindings must contain non-empty strings")
        if name in names:
            raise ValueError("duplicate producer logical source binding: " + name)
        names.add(name)
        result.append((name, binding))
    result.sort()
    encoded = compact_json(result).encode("utf-8")
    if len(encoded) > MAX_PRODUCER_LOGICAL_BINDING_BYTES:
        raise ValueError("producer logical source bindings exceed their byte budget")
    return tuple(result)


def _producer_carrier_paths(value: Mapping[str, str | Path] | Iterable[tuple[str, str | Path]]) -> tuple[tuple[str, Path], ...]:
    """Validate explicit logical labels and physical paths for a carrier set."""
    try:
        items = iter(value.items() if isinstance(value, Mapping) else value)
    except TypeError as error:
        raise ValueError("producer carrier paths must be iterable") from error
    result: list[tuple[str, Path]] = []
    labels: set[str] = set()
    for item in items:
        if len(result) >= MAX_PRODUCER_CARRIER_PATHS:
            raise ValueError("producer carrier paths exceed their count budget")
        if not isinstance(item, (tuple, list)) or len(item) != 2:
            raise ValueError("producer carrier paths must be name/path pairs")
        label, raw_path = item
        if (type(label) is not str or not label or "\x00" in label
                or len(label.encode("utf-8")) > MAX_PRODUCER_CARRIER_LABEL_BYTES
                or label in labels):
            raise ValueError("producer carrier paths require unique logical labels")
        try:
            path_text = os.fspath(raw_path)
        except TypeError as error:
            raise ValueError("producer carrier paths must be filesystem paths") from error
        if (type(path_text) is not str or "\x00" in path_text
                or len(path_text.encode("utf-8")) > MAX_PRODUCER_CARRIER_PATH_BYTES):
            raise ValueError("producer carrier paths must be absolute and bounded")
        path = Path(path_text).expanduser()
        if (not path.is_absolute() or ".." in path.parts
                or len(path.as_posix().encode("utf-8")) > MAX_PRODUCER_CARRIER_PATH_BYTES):
            raise ValueError("producer carrier paths must be absolute and bounded")
        if path.is_symlink():
            raise ValueError("producer carrier paths must not be symlinks")
        labels.add(label)
        result.append((label, path))
    return tuple(result)


class ProducerCarrierSet:
    """One owner-captured producer input snapshot and its logical bindings.

    ``knowledge`` and ``knowledge_catalog`` are captured as one pair.  The
    class does not assemble or semantically admit a source vector: an owner
    binder must supply already verified carriers whose graph/catalog both
    carry the same source revision.  No method writes that revision onto a
    stale graph.  The handle intentionally avoids copying giant carriers;
    nested values remain caller-owned and must be kept read-only for the
    lifetime of the producer call.  Physical paths are deliberately separate
    from logical labels, allowing frozen carriers in an external scratch directory without
    changing producer-root normalization or leaking scratch paths into the
    revision.
    """

    __slots__ = (
        "corpus", "philosophy", "knowledge", "knowledge_catalog", "evidence",
        "philosophy_audit", "word_analysis_capability", "carrier_paths", "logical_bindings",
    )

    def __init__(
        self,
        *,
        corpus: dict[str, Any],
        philosophy: dict[str, Any],
        knowledge: dict[str, Any],
        knowledge_catalog: dict[str, Any],
        evidence: dict[str, Any],
        philosophy_audit: dict[str, Any],
        word_analysis_capability: dict[str, Any],
        carrier_paths: Mapping[str, str | Path] | Iterable[tuple[str, str | Path]],
        logical_bindings: Mapping[str, str] | Iterable[tuple[str, str]] = (),
    ) -> None:
        self.carrier_paths = _producer_carrier_paths(carrier_paths)
        self.logical_bindings = _logical_bindings(logical_bindings)
        self.corpus = corpus
        self.philosophy = philosophy
        self.knowledge = knowledge
        self.knowledge_catalog = knowledge_catalog
        self.evidence = evidence
        self.philosophy_audit = philosophy_audit
        self.word_analysis_capability = word_analysis_capability
        self.validate()

    @property
    def source_revision(self) -> str:
        return self.knowledge["source_revision"]

    def validate(self) -> "ProducerCarrierSet":
        # The handle deliberately keeps caller-owned nested values instead of
        # copying potentially giant carriers.  Re-validate the bounded tuple
        # surfaces as well, so accidental top-level reassignment or nested
        # mutation cannot bypass the producer's admission checks between
        # capture and SQL generation.
        normalized_paths = _producer_carrier_paths(self.carrier_paths)
        if normalized_paths != self.carrier_paths:
            raise ValueError("producer carrier handle paths were mutated")
        normalized_bindings = _logical_bindings(self.logical_bindings)
        if normalized_bindings != self.logical_bindings:
            raise ValueError("producer logical source bindings were mutated")
        for name in (
            "corpus",
            "philosophy",
            "knowledge",
            "knowledge_catalog",
            "evidence",
            "philosophy_audit",
            "word_analysis_capability",
        ):
            if not isinstance(getattr(self, name), dict):
                raise ValueError(f"producer {name} carrier must be an object")
        source_revision = self.knowledge.get("source_revision")
        if type(source_revision) is not str or not source_revision:
            raise ValueError("producer graph requires an exact source revision")
        if self.knowledge_catalog.get("schema") != "tos_knowledge_catalog_v1":
            raise ValueError("producer catalog carrier has an invalid schema")
        if self.knowledge_catalog.get("source_revision") != source_revision:
            raise ValueError("producer graph/catalog source revisions do not match")
        if self.logical_bindings:
            bindings = dict(self.logical_bindings)
            if bindings.get("source_revision") != source_revision:
                raise ValueError("producer logical source binding does not match graph source revision")
        return self

    @classmethod
    def admit(
        cls,
        *,
        corpus: dict[str, Any],
        philosophy: dict[str, Any],
        knowledge: dict[str, Any],
        knowledge_catalog: dict[str, Any],
        evidence: dict[str, Any],
        philosophy_audit: dict[str, Any],
        word_analysis_capability: dict[str, Any],
        carrier_paths: Mapping[str, str | Path] | Iterable[tuple[str, str | Path]],
        logical_bindings: Mapping[str, str] | Iterable[tuple[str, str]] | None = None,
    ) -> "ProducerCarrierSet":
        """Admit a caller-owned snapshot with an explicit source binding."""
        normalized_bindings = _logical_bindings(logical_bindings)
        if not normalized_bindings:
            raise ValueError("explicit producer source binding required")
        return cls(
            corpus=corpus,
            philosophy=philosophy,
            knowledge=knowledge,
            knowledge_catalog=knowledge_catalog,
            evidence=evidence,
            philosophy_audit=philosophy_audit,
            word_analysis_capability=word_analysis_capability,
            carrier_paths=carrier_paths,
            logical_bindings=normalized_bindings,
        )

    @classmethod
    def from_core(
        cls,
        core: ToSAccessCore,
        *,
        carrier_paths: Mapping[str, str | Path] | Iterable[tuple[str, str | Path]] | None = None,
        logical_bindings: Mapping[str, str] | Iterable[tuple[str, str]] | None = None,
    ) -> "ProducerCarrierSet":
        """Capture one graph/catalog pair for compatibility with legacy callers."""
        snapshot = core.knowledge_snapshot()
        values = dict(
            corpus=core.index(),
            philosophy=core.philosophy_projection(),
            knowledge=snapshot["graph"],
            knowledge_catalog=snapshot["catalog"],
            evidence=core.evidence_projection(),
            philosophy_audit=core.philosophy_audit_payload() if core.philosophy_audit_exists() else {},
            word_analysis_capability=core.zarathustra_word_analysis_public_capability(),
            carrier_paths=_legacy_carrier_paths(core) if carrier_paths is None else carrier_paths,
            logical_bindings=logical_bindings,
        )
        # Keep the historical no-argument path source-compatible.  A caller
        # replacing physical carriers must use the explicit admission route,
        # which requires a matching logical source binding and cannot silently
        # turn an external scratch tree into a legacy producer input.
        if carrier_paths is not None:
            return cls.admit(**values)
        return cls(**values)


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
    index_path = output / "index.html"
    index_html = index_path.read_text(encoding="utf-8")
    for asset in sorted((output / "assets").rglob("*")):
        if not asset.is_file():
            continue
        relative_asset = asset.relative_to(output / "assets").as_posix()
        public_path = f"/static/assets/{relative_asset}"
        if public_path in index_html:
            fingerprint = hashlib.sha256(asset.read_bytes()).hexdigest()[:16]
            index_html = index_html.replace(public_path, f"{public_path}?v={fingerprint}")
    index_path.write_text(index_html, encoding="utf-8")
    static_assets = output / "static" / "assets"
    static_assets.parent.mkdir(parents=True)
    shutil.move(output / "assets", static_assets)

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
    write_json(output, "__edge/knowledge/catalog.json", normalize_paths(core.knowledge_catalog(), REPO_ROOT))
    write_json(output, "__edge/knowledge/contracts.json", normalize_paths(core.knowledge_contracts(), REPO_ROOT))
    exploration_contracts = core.knowledge_exploration_contracts()
    write_json(output, "__edge/knowledge/exploration-contracts.json",
               {key: value for key, value in exploration_contracts.items() if key != "capabilities"})
    write_json(output, "__edge/source-gaps/all.json", normalize_paths(core.source_gap_search("", limit=100), REPO_ROOT))
    write_json(output, "__edge/source-navigation/all.json", normalize_paths(core.source_navigation(bibliographic_only=True), REPO_ROOT))
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
    oversized = [path.relative_to(output).as_posix() for path in output.rglob('*')
                 if path.is_file() and path.stat().st_size >= 25 * 1024 * 1024]
    if oversized:
        raise RuntimeError('static assets exceed the 25 MiB delivery limit: ' + ', '.join(oversized))
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


def chunk_text(value: str, size: int = SQL_CHUNK_BYTES) -> list[str]:
    """Split text without breaking UTF-8 and keep escaped INSERTs below D1's limit."""
    chunks: list[str] = []
    encoded = value.encode('utf-8')
    start = 0
    while start < len(encoded):
        end = min(start + size, len(encoded))
        while end < len(encoded) and end > start and encoded[end] & 0xC0 == 0x80:
            end -= 1
        if end == start:
            raise ValueError('chunk size cannot hold one UTF-8 character')
        chunks.append(encoded[start:end].decode('utf-8'))
        start = end
    if not chunks:
        chunks.append('')
    return chunks


class SqlStatementWriter:
    """List-shaped streaming sink so large read models do not live twice in RAM."""

    def __init__(self, target: Path, delta: DeltaRecorder | None = None) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        self.target = target
        self.pending = target.with_name(target.name + ".next")
        self.stream = self.pending.open("w", encoding="utf-8")
        self.count = 0
        self.delta = delta
        self.finished = False
        self.published = False

    def append(self, statement: str) -> None:
        if self.delta is not None:
            self.delta.observe(statement)
        statement_size = len(statement.encode("utf-8"))
        if statement_size > MAX_D1_SQL_STATEMENT_BYTES:
            raise RuntimeError(
                f"D1 SQL statement exceeds {MAX_D1_SQL_STATEMENT_BYTES} bytes: {statement_size}"
            )
        self.stream.write(statement)
        self.stream.write("\n")
        self.count += 1

    def extend(self, statements: Iterable[str]) -> None:
        for statement in statements:
            self.append(statement)

    def finish(self, *, publish: bool = True) -> None:
        self.stream.flush()
        self.stream.close()
        self.finished = True
        if publish:
            self.publish()

    def publish(self) -> None:
        if not self.finished:
            raise ValueError("SQL statements must be finished before publication")
        if self.published:
            return
        self.pending.replace(self.target)
        self.published = True


def publish_prepared_files(files: tuple[tuple[Path, Path], ...]) -> None:
    """Publish a prepared output set with rollback on a partial rename.

    There is no multi-file filesystem transaction or crash-safe group commit.
    Keep exact sibling backups while renaming the three carriers so a Python,
    I/O, or KeyboardInterrupt failure cannot leave a newly generated SQL file
    paired with an older row index.  SIGKILL, power loss, or filesystem
    failure between renames remains outside this rollback guarantee.
    """
    backups: list[tuple[Path, Path]] = []
    published: list[Path] = []
    try:
        for pending, target in files:
            backup = target.with_name(target.name + ".rollback")
            if backup.exists():
                raise RuntimeError("stale prepared-output rollback file: " + str(backup))
            if target.exists():
                os.replace(target, backup)
                backups.append((target, backup))
            os.replace(pending, target)
            published.append(target)
    except BaseException:
        for target in reversed(published):
            target.unlink(missing_ok=True)
        for target, backup in reversed(backups):
            os.replace(backup, target)
        raise
    for _target, backup in backups:
        backup.unlink(missing_ok=True)


def append_chunkable_insert(
    writer: SqlStatementWriter,
    table: str,
    columns: tuple[str, ...],
    values: tuple[str, ...],
    *,
    selector_sql: str,
    chunked_text: dict[str, str],
) -> None:
    """Insert one row while preserving oversized text through bounded updates."""
    # SQL chunking bounds statements, not the eventual SQLite row. Use an
    # intentionally conservative upper bound including escaped text/header.
    if sum(len(value.encode('utf-8')) for value in values) + 1024 > MAX_D1_SQL_ROW_VALUE_BYTES:
        raise RuntimeError(f'{table} row exceeds the D1 row budget; split the record, never truncate it')
    statement = sql_insert(table, columns, values)
    if len(statement.encode("utf-8")) <= MAX_D1_SQL_STATEMENT_BYTES:
        writer.append(statement)
        return

    base_values = list(values)
    for column in chunked_text:
        try:
            position = columns.index(column)
        except ValueError as exc:
            raise RuntimeError(f"chunked column {column!r} is absent from {table}") from exc
        base_values[position] = sql_text("")
    writer.append(sql_insert(table, columns, tuple(base_values)))
    for column, value in chunked_text.items():
        for chunk in chunk_text(value):
            writer.append(
                f"UPDATE {table} SET {column} = {column} || {sql_text(chunk)} "
                f"WHERE {selector_sql};"
            )


def append_batched_inserts(
    writer: SqlStatementWriter,
    table: str,
    columns: tuple[str, ...],
    rows: Iterable[tuple[str, ...]],
) -> None:
    """Write compact bounded multi-value INSERTs for posting carriers."""
    prefix = f"INSERT INTO {table} ({','.join(columns)}) VALUES "
    batch: list[str] = []
    size = len(prefix.encode("utf-8")) + 1
    for values in rows:
        value_sql = "(" + ",".join(values) + ")"
        extra = len(value_sql.encode("utf-8")) + (1 if batch else 0)
        if batch and size + extra + 1 > MAX_D1_SQL_STATEMENT_BYTES:
            writer.append(prefix + ",".join(batch) + ";")
            batch = []
            size = len(prefix.encode("utf-8")) + 1
        if len(value_sql.encode("utf-8")) + len(prefix.encode("utf-8")) + 2 > MAX_D1_SQL_STATEMENT_BYTES:
            raise RuntimeError(f"{table} posting row exceeds D1 SQL statement budget")
        batch.append(value_sql)
        size += extra
    if batch:
        writer.append(prefix + ",".join(batch) + ";")


class PostingStatsStore:
    """Disk-backed unique n-gram counts for the indexed search carrier."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.unlink(missing_ok=True)
        self.connection = sqlite3.connect(self.path)
        self.connection.execute("PRAGMA journal_mode=OFF")
        self.connection.execute("PRAGMA synchronous=OFF")
        self.connection.execute("PRAGMA temp_store=FILE")
        self.connection.execute("PRAGMA cache_size=-32768")
        self.connection.execute(
            "CREATE TABLE gram_stats ("
            "kind TEXT NOT NULL, gram TEXT NOT NULL, postings INTEGER NOT NULL, "
            "PRIMARY KEY (kind, gram)) WITHOUT ROWID"
        )
        self.total = 0
        self.closed = False

    def add(self, kind: str, gram: str) -> None:
        self.connection.execute(
            "INSERT INTO gram_stats(kind,gram,postings) VALUES (?,?,1) "
            "ON CONFLICT(kind,gram) DO UPDATE SET postings=postings+1",
            (kind, gram),
        )
        self.total += 1
        if self.total % 4096 == 0:
            self.connection.commit()

    def rows(self):
        self.connection.commit()
        return self.connection.execute(
            "SELECT kind,gram,postings FROM gram_stats ORDER BY kind,gram"
        )

    def close(self) -> None:
        if self.closed:
            return
        self.connection.commit()
        self.connection.close()
        self.closed = True

    def __enter__(self) -> "PostingStatsStore":
        return self

    def __exit__(self, error_type, error, traceback) -> None:
        try:
            self.close()
        finally:
            # This database is a build-time accumulator, never a published
            # carrier.  Remove it on both success and failure so an interrupted
            # producer cannot be mistaken for reusable output.
            self.path.unlink(missing_ok=True)


def build_read_model_sql(
    core: ToSAccessCore,
    target: Path,
    revision: str,
    carrier_set: ProducerCarrierSet | None = None,
) -> dict[str, Any]:
    # Capture/admit the complete pair before creating target-side SQL or row
    # index files.  A supplied set is the only route for external frozen
    # carriers; legacy callers retain the existing core discovery behavior.
    carriers = ProducerCarrierSet.from_core(core) if carrier_set is None else carrier_set
    if not isinstance(carriers, ProducerCarrierSet):
        raise TypeError("carrier_set must be a ProducerCarrierSet")
    carriers.validate()
    philosophy = carriers.philosophy
    corpus = carriers.corpus
    knowledge = carriers.knowledge
    knowledge_catalog = carriers.knowledge_catalog
    evidence = carriers.evidence
    audit = carriers.philosophy_audit
    word_analysis_capability = carriers.word_analysis_capability
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

    target.parent.mkdir(parents=True, exist_ok=True)
    index_path = target.with_name('read-model.rows.json')
    deployed_path = target.with_name('read-model.deployed.rows.json')
    baseline_path = deployed_path if deployed_path.is_file() else index_path
    previous_index = (DiskRowBaseline(
        baseline_path,
        READ_MODEL_SCHEMA_VERSION,
        target.with_name('read-model.baseline.sqlite'),
    ) if baseline_path.is_file() else None)
    index_pending = index_path.with_name(index_path.name + '.next')
    delta = DeltaRecorder(
        target.with_name('read-model.delta.sql'),
        revision,
        READ_MODEL_SCHEMA_VERSION,
        previous_index,
        index_store_path=target.with_name('read-model.rows.index.sqlite'),
    )
    statements = SqlStatementWriter(target, delta)
    statements.extend((
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
        "DROP TABLE IF EXISTS knowledge_nodes_next;",
        "DROP TABLE IF EXISTS knowledge_relations_next;",
        "DROP TABLE IF EXISTS knowledge_search_documents_next;",
        "DROP TABLE IF EXISTS knowledge_search_grams_next;",
        "DROP TABLE IF EXISTS knowledge_search_gram_stats_next;",
        "DROP TABLE IF EXISTS knowledge_lens_order_next;",
        "CREATE TABLE edge_meta_next (key TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY (key, part));",
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
        "CREATE TABLE knowledge_nodes_next (id TEXT PRIMARY KEY, entity_id TEXT NOT NULL, native_id TEXT NOT NULL, source_graph TEXT NOT NULL, kind_id TEXT NOT NULL, type_id TEXT NOT NULL, title_text TEXT NOT NULL, summary_text TEXT NOT NULL, search_text TEXT NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE knowledge_relations_next (id TEXT PRIMARY KEY, native_id TEXT NOT NULL, source_graph TEXT NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, predicate_id TEXT NOT NULL, relation_type_id TEXT NOT NULL, label_text TEXT NOT NULL, explanation_text TEXT NOT NULL, search_text TEXT NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE knowledge_search_documents_next (kind TEXT NOT NULL, position INTEGER NOT NULL, id TEXT NOT NULL, source_graph TEXT NOT NULL, kind_id TEXT NOT NULL, predicate_id TEXT NOT NULL, id_lower TEXT NOT NULL, native_id_lower TEXT NOT NULL, identity_values TEXT NOT NULL, visible_values TEXT NOT NULL, document_chars INTEGER NOT NULL, document_digest TEXT NOT NULL, PRIMARY KEY (kind, position));",
        "CREATE TABLE knowledge_search_grams_next (kind TEXT NOT NULL, n INTEGER NOT NULL, gram TEXT NOT NULL, position INTEGER NOT NULL, PRIMARY KEY (kind, n, gram, position));",
        "CREATE TABLE knowledge_search_gram_stats_next (kind TEXT NOT NULL, n INTEGER NOT NULL, gram TEXT NOT NULL, postings INTEGER NOT NULL, PRIMARY KEY (kind, n, gram));",
        "CREATE TABLE knowledge_lens_order_next (kind TEXT NOT NULL, id TEXT NOT NULL, sort_key TEXT NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, PRIMARY KEY (kind, id));",
    ))

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
            "source_navigation",
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
        "knowledge_top": normalize_paths(
            {key: value for key, value in knowledge.items() if key not in {"nodes", "relations"}},
            REPO_ROOT,
        ),
        "knowledge_exploration_top": {
            "source_revision": knowledge["source_revision"],
            "authority_boundary": knowledge.get("authority_boundary", {}),
        },
        "knowledge_search_top": {
            "schema": SEARCH_READ_MODEL_SCHEMA_VERSION,
            "source_revision": knowledge["source_revision"],
            "ngram_size": SEARCH_NGRAM_SIZE,
            "matching_counts": "unknown-until-indexed-page-exhaustion",
        },
    }
    # The reader envelope needs only the graph binding fields. Keep the
    # producer's full graph out of this metadata copy; row JSON is emitted by
    # the table loops below and catalog remains its own bounded projection.
    reader_graph = {
        key: knowledge.get(key)
        for key in ("schema", "source_revision", "normalization_binding", "authority_boundary")
    }
    reader_metadata = published_reader_metadata(
        normalize_paths(reader_graph, REPO_ROOT),
        normalize_paths(knowledge_catalog, REPO_ROOT),
        READ_MODEL_SCHEMA_VERSION,
        revision,
        lens_metadata=published_lens_metadata(knowledge),
    )
    metadata.update(reader_metadata)
    for key, value in metadata.items():
        for part, chunk in enumerate(chunk_text(compact_json(value))):
            statements.append(
                sql_insert(
                    "edge_meta_next",
                    ("key", "part", "json_chunk"),
                    (sql_text(key), str(part), sql_text(chunk)),
                )
            )

    philosophy_nodes = object_list(philosophy.get("nodes"))
    for order, item in enumerate(philosophy_nodes):
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        item_id = str(item.get("node_id") or "")
        columns = ("id", "ord", "view_mask", "layer_mask", "json", "search_text")
        values = (
            sql_text(item_id),
            str(order),
            str(mask_for(string_list(item.get("view_ids")), view_positions)),
            str(mask_for(string_list(item.get("graph_layers")), layer_positions)),
            sql_text(item_json),
            sql_text(item_json.lower()),
        )
        append_chunkable_insert(
            statements,
            "philosophy_nodes_next",
            columns,
            values,
            selector_sql=f"id = {sql_text(item_id)}",
            chunked_text={"json": item_json, "search_text": item_json.lower()},
        )

    philosophy_edges = object_list(philosophy.get("edges"))
    for order, item in enumerate(philosophy_edges):
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        item_id = str(item.get("edge_id") or "")
        columns = (
            "id",
            "ord",
            "from_id",
            "to_id",
            "predicate_id",
            "view_mask",
            "layer_mask",
            "json",
            "search_text",
        )
        values = (
            sql_text(item_id),
            str(order),
            sql_text(str(item.get("from_id") or "")),
            sql_text(str(item.get("to_id") or "")),
            sql_text(str(item.get("predicate_id") or "")),
            str(mask_for(string_list(item.get("view_ids")), view_positions)),
            str(mask_for(string_list(item.get("graph_layers")), layer_positions)),
            sql_text(item_json),
            sql_text(item_json.lower()),
        )
        append_chunkable_insert(
            statements,
            "philosophy_edges_next",
            columns,
            values,
            selector_sql=f"id = {sql_text(item_id)}",
            chunked_text={"json": item_json, "search_text": item_json.lower()},
        )

    for collection in ("views", "clusters", "review_packets", "graph_layers"):
        for order, item in enumerate(object_list(philosophy.get(collection))):
            compact_item = compact_philosophy_aux(collection, item)
            item_json = compact_json(normalize_paths(compact_item, REPO_ROOT))
            columns = ("collection", "ord", "id", "json", "search_text")
            values = (
                sql_text(collection),
                str(order),
                sql_text(item_identity(item, f"{collection}:{order}")),
                sql_text(item_json),
                sql_text(item_json.lower()),
            )
            append_chunkable_insert(
                statements,
                "philosophy_aux_next",
                columns,
                values,
                selector_sql=f"collection = {sql_text(collection)} AND ord = {order}",
                chunked_text={"json": item_json, "search_text": item_json.lower()},
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
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        append_chunkable_insert(
            statements,
            "philosophy_review_packets_next",
            ("view_id", "json"),
            (sql_text(view_id), sql_text(item_json)),
            selector_sql=f"view_id = {sql_text(view_id)}",
            chunked_text={"json": item_json},
        )

    corpus_items_count = 0
    for collection in CORPUS_COLLECTIONS:
        for order, item in enumerate(object_list(corpus.get(collection))):
            item_json = compact_json(normalize_paths(item, REPO_ROOT))
            columns = (
                "collection",
                "ord",
                "id",
                "resource_kind",
                "owner_branch",
                "json",
                "search_text",
            )
            values = (
                sql_text(collection),
                str(order),
                sql_text(item_identity(item, f"{collection}:{order}")),
                sql_nullable(item.get("resource_kind") if isinstance(item.get("resource_kind"), str) else None),
                sql_nullable(item.get("owner_branch") if isinstance(item.get("owner_branch"), str) else None),
                sql_text(item_json),
                sql_text(item_json.lower()),
            )
            append_chunkable_insert(
                statements,
                "corpus_items_next",
                columns,
                values,
                selector_sql=f"collection = {sql_text(collection)} AND ord = {order}",
                chunked_text={"json": item_json, "search_text": item_json.lower()},
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
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        columns = ("id", "ord", "from_id", "to_id", "pack_id", "owner_branch", "json")
        values = (
            sql_text(str(item.get("edge_id") or f"corpus-edge:{order}")),
            str(order),
            sql_text(str(item.get("from_id") or "")),
            sql_text(str(item.get("to_id") or "")),
            sql_nullable(item.get("pack_id") if isinstance(item.get("pack_id"), str) else None),
            sql_nullable(item.get("owner_branch") if isinstance(item.get("owner_branch"), str) else None),
            sql_text(item_json),
        )
        append_chunkable_insert(
            statements,
            "corpus_edges_next",
            columns,
            values,
            selector_sql=f"ord = {order}",
            chunked_text={"json": item_json},
        )

    corpus_packs = object_list(corpus.get("relation_packs"))
    for order, item in enumerate(corpus_packs):
        item_id = str(item.get("pack_id") or "")
        item_json = compact_json(normalize_paths(item, REPO_ROOT))
        append_chunkable_insert(
            statements,
            "corpus_packs_next",
            ("id", "ord", "json"),
            (sql_text(item_id), str(order), sql_text(item_json)),
            selector_sql=f"id = {sql_text(item_id)}",
            chunked_text={"json": item_json},
        )

    knowledge_nodes = object_list(knowledge.get("nodes"))
    for item in knowledge_nodes:
        normalized = normalize_paths(item, REPO_ROOT)
        item_json = compact_json(normalized)
        display = normalized.get("display") if isinstance(normalized.get("display"), dict) else {}
        title = display.get("title") if isinstance(display.get("title"), dict) else {}
        summary = display.get("summary") if isinstance(display.get("summary"), dict) else {}
        item_id = str(normalized.get("id") or "")
        summary_text = str(summary.get("default") or "")
        search_text = SQLiteKnowledgeSearchReadModel._searchable(normalized)
        columns = ("id", "entity_id", "native_id", "source_graph", "kind_id", "type_id", "title_text", "summary_text", "search_text", "json")
        values = (
            sql_text(item_id),
            sql_text(str(normalized.get("entity_id") or "")),
            sql_text(str(normalized.get("native_id") or "")),
            sql_text(str(normalized.get("source_graph") or "")),
            sql_text(str(normalized.get("kind_id") or "")),
            sql_text(str(normalized.get("type_id") or "")),
            sql_text(str(title.get("default") or "").lower()),
            sql_text(summary_text),
            sql_text(search_text),
            sql_text(item_json),
        )
        append_chunkable_insert(
            statements,
            "knowledge_nodes_next",
            columns,
            values,
            selector_sql=f"id = {sql_text(item_id)}",
            chunked_text={
                "summary_text": summary_text,
                "search_text": search_text,
                "json": item_json,
            },
        )
        digest_key = published_row_digest_key("node", item_id)
        for part, chunk in enumerate(chunk_text(compact_json(emitted_row_digest(item_json)))):
            statements.append(
                sql_insert(
                    "edge_meta_next",
                    ("key", "part", "json_chunk"),
                    (sql_text(digest_key), str(part), sql_text(chunk)),
                )
            )

    knowledge_relations = object_list(knowledge.get("relations"))
    for item in knowledge_relations:
        normalized = normalize_paths(item, REPO_ROOT)
        item_json = compact_json(normalized)
        display = normalized.get("display") if isinstance(normalized.get("display"), dict) else {}
        label = display.get("label") if isinstance(display.get("label"), dict) else {}
        explanation = display.get("explanation") if isinstance(display.get("explanation"), dict) else {}
        item_id = str(normalized.get("id") or "")
        explanation_text = str(explanation.get("default") or "")
        search_text = SQLiteKnowledgeSearchReadModel._searchable(normalized)
        columns = ("id", "native_id", "source_graph", "from_id", "to_id", "predicate_id", "relation_type_id", "label_text", "explanation_text", "search_text", "json")
        values = (
            sql_text(item_id),
            sql_text(str(normalized.get("native_id") or "")),
            sql_text(str(normalized.get("source_graph") or "")),
            sql_text(str(normalized.get("from_id") or "")),
            sql_text(str(normalized.get("to_id") or "")),
            sql_text(str(normalized.get("predicate_id") or "")),
            sql_text(str(normalized.get("relation_type_id") or "")),
            sql_text(str(label.get("default") or "").lower()),
            sql_text(explanation_text),
            sql_text(search_text),
            sql_text(item_json),
        )
        append_chunkable_insert(
            statements,
            "knowledge_relations_next",
            columns,
            values,
            selector_sql=f"id = {sql_text(item_id)}",
            chunked_text={
                "explanation_text": explanation_text,
                "search_text": search_text,
                "json": item_json,
            },
        )
        digest_key = published_row_digest_key("relation", item_id)
        for part, chunk in enumerate(chunk_text(compact_json(emitted_row_digest(item_json)))):
            statements.append(
                sql_insert(
                    "edge_meta_next",
                    ("key", "part", "json_chunk"),
                    (sql_text(digest_key), str(part), sql_text(chunk)),
                )
            )

    for kind, items in (("node", knowledge_nodes), ("relation", knowledge_relations)):
        for item in items:
            statements.append(sql_insert("knowledge_lens_order_next",
                ("kind", "id", "sort_key", "from_id", "to_id"),
                tuple(sql_text(value) for value in lens_order_row(kind, item))))

    # The indexed search plane stores only compact rank carriers and complete
    # 3-gram postings. The source JSON/search_text rows above remain the
    # legacy v1 compatibility plane and are also used for exact substring
    # verification after a posting candidate is selected.
    search_posting_count = 0
    with PostingStatsStore(target.with_name('read-model.search-gram-stats.sqlite')) as search_gram_stats:
        for kind, source_items in (("nodes", knowledge_nodes), ("relations", knowledge_relations)):
            for position, item in enumerate(source_items):
                normalized = normalize_paths(item, REPO_ROOT)
                document = SQLiteKnowledgeSearchReadModel._searchable(normalized)
                id_lower, native_id_lower, identity_values, visible_values = SQLiteKnowledgeSearchReadModel._rank_fields(
                    normalized, relation=kind == "relations"
                )
                item_id = str(normalized.get("id") or "")
                document_bytes = document.encode("utf-8", "surrogatepass")
                append_chunkable_insert(
                    statements,
                    "knowledge_search_documents_next",
                    (
                        "kind", "position", "id", "source_graph", "kind_id", "predicate_id",
                        "id_lower", "native_id_lower", "identity_values", "visible_values",
                        "document_chars", "document_digest",
                    ),
                    (
                        sql_text(kind), str(position), sql_text(item_id),
                        sql_text(str(normalized.get("source_graph") or "")),
                        sql_text(str(normalized.get("kind_id") or "")),
                        sql_text(str(normalized.get("predicate_id") or "")),
                        sql_text(id_lower), sql_text(native_id_lower), sql_text(identity_values),
                        sql_text(visible_values), str(len(document)),
                        sql_text(hashlib.sha256(document_bytes).hexdigest()),
                    ),
                    selector_sql=f"kind = {sql_text(kind)} AND position = {position}",
                    chunked_text={},
                )
                grams = tuple(dict.fromkeys(
                    document[offset : offset + SEARCH_NGRAM_SIZE]
                    for offset in range(len(document) - SEARCH_NGRAM_SIZE + 1)
                ))
                search_posting_count += len(grams)
                if search_posting_count > SEARCH_READ_MODEL_MAX_POSTINGS:
                    raise RuntimeError("knowledge search posting budget exceeded")
                for gram in grams:
                    search_gram_stats.add(kind, gram)
                append_batched_inserts(
                    statements,
                    "knowledge_search_grams_next",
                    ("kind", "n", "gram", "position"),
                    ((sql_text(kind), str(SEARCH_NGRAM_SIZE), sql_text(gram), str(position)) for gram in grams),
                )

        append_batched_inserts(
            statements,
            "knowledge_search_gram_stats_next",
            ("kind", "n", "gram", "postings"),
            (
                (sql_text(kind), str(SEARCH_NGRAM_SIZE), sql_text(gram), str(postings))
                for kind, gram, postings in search_gram_stats.rows()
            ),
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
        "knowledge_nodes",
        "knowledge_relations",
        "knowledge_search_documents",
        "knowledge_search_grams",
        "knowledge_search_gram_stats",
        "knowledge_lens_order",
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
            "CREATE INDEX knowledge_nodes_native_idx ON knowledge_nodes(native_id);",
            "CREATE INDEX knowledge_nodes_entity_idx ON knowledge_nodes(entity_id);",
            "CREATE INDEX knowledge_nodes_source_kind_idx ON knowledge_nodes(source_graph, kind_id);",
            "CREATE INDEX knowledge_nodes_source_type_idx ON knowledge_nodes(source_graph, type_id);",
            "CREATE INDEX knowledge_relations_native_idx ON knowledge_relations(native_id);",
            "CREATE INDEX knowledge_relations_source_predicate_idx ON knowledge_relations(source_graph, predicate_id);",
            "CREATE INDEX knowledge_relations_source_type_idx ON knowledge_relations(source_graph, relation_type_id);",
            "CREATE INDEX knowledge_relations_from_idx ON knowledge_relations(from_id);",
            "CREATE INDEX knowledge_relations_to_idx ON knowledge_relations(to_id);",
            "CREATE INDEX knowledge_search_grams_lookup_idx ON knowledge_search_grams(kind,n,gram,position);",
            "CREATE INDEX knowledge_search_documents_source_kind_idx ON knowledge_search_documents(kind,source_graph,kind_id,position);",
            "CREATE INDEX knowledge_search_documents_source_predicate_idx ON knowledge_search_documents(kind,source_graph,predicate_id,position);",
            "CREATE INDEX knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id);",
            "CREATE INDEX knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id);",
            "CREATE INDEX knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id);",
            "CREATE INDEX knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id);",
            "PRAGMA optimize;",
        )
    )
    statements.append((WORKER_ROOT / "migrations/0001-exploration.sql").read_text(encoding="utf-8"))
    # A maintenance bootstrap replaces the metadata table and its triggers.
    # Invalidate retained execution state even if it republishes identical data.
    statements.append("UPDATE knowledge_exploration_clock SET epoch=epoch+1 WHERE singleton=1;")
    # Prepare every generated carrier before changing a final path.  The
    # publish helper then renames the SQL, delta, and row-index files together
    # with rollback if one local rename fails.
    statements.finish(publish=False)
    delta.finish(index_output=index_pending, publish=False)
    publish_prepared_files((
        (statements.pending, statements.target),
        (delta.pending_path, delta.target),
        (index_pending, index_path),
    ))
    return {
        "philosophy_nodes": len(philosophy_nodes),
        "philosophy_edges": len(philosophy_edges),
        "philosophy_clusters": len(philosophy_clusters),
        "philosophy_cluster_node_memberships": cluster_node_memberships,
        "philosophy_cluster_edge_memberships": cluster_edge_memberships,
        "corpus_items": corpus_items_count,
        "corpus_edges": len(corpus_edges),
        "corpus_packs": len(corpus_packs),
        "knowledge_nodes": len(knowledge_nodes),
        "knowledge_relations": len(knowledge_relations),
        "knowledge_search_postings": search_posting_count,
        "knowledge_search_schema": SEARCH_READ_MODEL_SCHEMA_VERSION,
        "sql_statements": statements.count,
        "delta": delta.summary(),
    }


def data_revision(core: ToSAccessCore, carrier_set: ProducerCarrierSet | None = None) -> str:
    digest = hashlib.sha256()
    digest.update(READ_MODEL_SCHEMA_VERSION.encode("utf-8"))
    digest.update(READ_MODEL_CONTENT_VERSION.encode("utf-8"))
    digest.update(SEARCH_READ_MODEL_SCHEMA_VERSION.encode("utf-8"))
    digest.update(b"\0")
    if carrier_set is None:
        knowledge_snapshot = core.knowledge_snapshot()
        knowledge = knowledge_snapshot["graph"]
        knowledge_catalog = knowledge_snapshot["catalog"]
        capability = core.zarathustra_word_analysis_public_capability()
        carrier_paths = _legacy_carrier_paths(core)
        logical_bindings = ()
    else:
        if not isinstance(carrier_set, ProducerCarrierSet):
            raise TypeError("carrier_set must be a ProducerCarrierSet")
        carrier_set.validate()
        knowledge = carrier_set.knowledge
        knowledge_catalog = carrier_set.knowledge_catalog
        capability = carrier_set.word_analysis_capability
        carrier_paths = carrier_set.carrier_paths
        logical_bindings = carrier_set.logical_bindings
    if (not isinstance(knowledge, dict) or not isinstance(knowledge_catalog, dict)
            or knowledge_catalog.get("schema") != "tos_knowledge_catalog_v1"
            or knowledge_catalog.get("source_revision") != knowledge.get("source_revision")):
        raise ValueError("producer graph/catalog source revisions do not match")
    digest.update(str(knowledge.get("source_revision") or "").encode("utf-8"))
    digest.update(b"\0")
    # Query bindings are serving metadata, not row content. A code-only
    # introduction/removal of this plane must not skip its D1 metadata update.
    digest.update(compact_json(knowledge.get('query_properties', [])).encode('utf-8'))
    digest.update(b"\0")
    digest.update(compact_json(published_lens_metadata(knowledge)).encode('utf-8'))
    digest.update(b"\0")
    for collection in ("nodes", "relations"):
        digest.update(collection.encode("utf-8"))
        digest.update(b"\0")
        for item in object_list(knowledge.get(collection)):
            digest.update(str(item.get("id") or "").encode("utf-8"))
            digest.update(b"\0")
            digest.update(str(item.get("content_revision") or "").encode("utf-8"))
            digest.update(b"\0")
    # The cold-reader envelope also binds normalization and authority posture.
    # Keep this compact graph header in the revision so metadata-only changes
    # cannot leave an older reader header deployed when row bytes are stable.
    reader_graph = {
        key: knowledge.get(key)
        for key in ("schema", "source_revision", "normalization_binding", "authority_boundary")
    }
    digest.update(compact_json(normalize_paths(reader_graph, REPO_ROOT)).encode("utf-8"))
    digest.update(b"\0")
    # The published catalog is a persisted read surface. Bind its normalized
    # emitted bytes so catalog-only changes cannot be skipped by deployment.
    digest.update(compact_json(normalize_paths(knowledge_catalog, REPO_ROOT)).encode("utf-8"))
    digest.update(b"\0")
    digest.update(compact_json(normalize_paths(capability, REPO_ROOT)).encode("utf-8"))
    digest.update(b"\0")
    if logical_bindings:
        digest.update(PRODUCER_LOGICAL_BINDINGS_VERSION.encode("utf-8"))
        digest.update(b"\0")
        digest.update(compact_json(list(logical_bindings)).encode("utf-8"))
        digest.update(b"\0")
    for label, path in carrier_paths:
        # Explicit ProducerCarrierSet paths may be outside REPO_ROOT.  Their
        # stable logical label, never a physical scratch path, is the revision
        # identity and the bytes remain the source of the content binding.
        digest.update(label.encode("utf-8"))
        digest.update(b"\0")
        if path.is_file():
            digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def parse_args(argv=None) -> argparse.Namespace:
    def positive(value):
        number = int(value)
        if number < 1:
            raise argparse.ArgumentTypeError('cache retention limits must be positive')
        return number
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=WORKER_ROOT / "dist")
    parser.add_argument("--runtime", type=Path, default=WORKER_ROOT / "runtime")
    parser.add_argument('--cache-max-mib', type=positive, default=DEFAULT_CACHE_BYTES // (1024 * 1024))
    parser.add_argument('--cache-max-entries', type=positive, default=DEFAULT_CACHE_ENTRIES)
    parser.add_argument('--cache-keep-runs', type=positive, default=3)
    return parser.parse_args(argv)


def build_inputs(core: ToSAccessCore) -> dict:
    """Conservative producer/input boundary, independent of Worker and UI code.

    Hash public projection inputs only, never corpus payloads or local providers.
    Directory membership catches additions and removals as well as byte changes.
    """
    paths = {str(path.resolve()): path for path in (
        core.index_path, core.philosophy_graph_projection_path, core.bibliographic_graph_path,
        core.entity_type_registry_path, core.relation_type_registry_path,
        core.evidence_projection_path, core.philosophy_post_planting_audit_path,
        *(core.tos_root / path for path in KNOWLEDGE_CONTRACT_RELATIVE_PATHS.values()),
        *sorted((core.tos_root / 'ToS/source-witnesses/access-requests/public-ledger').glob('*.access-request.json')),
    )}
    for directory, pattern in (
        (REPO_ROOT / 'access/src/tos_access', '*.py'),
        (REPO_ROOT / 'access/contracts', '*.json'),
        (WORKER_ROOT / 'scripts', '*.py'),
        (WORKER_ROOT / 'migrations', '*.sql'),
    ):
        paths.update({str(path.resolve()): path for path in directory.rglob(pattern)})
    return {'files': fingerprint(paths), 'python': list(sys.version_info[:3]),
            'schema': READ_MODEL_SCHEMA_VERSION, 'content': READ_MODEL_CONTENT_VERSION,
            'root': str(core.tos_root.resolve())}


def build(core: ToSAccessCore, output: Path, runtime: Path, *, cache_options=None) -> dict:
    """Called under the runtime lock; completion manifests are written last."""
    stages = BuildStages(runtime / 'build-stages.json')
    processing = None
    normalization = None

    def prepare_graph():
        nonlocal processing, normalization
        if processing is None:
            # The process-local input cache is keyed by the complete source
            # state, so embedded builders reread bytes after a content-based
            # invalidation even when an editor preserved file size and mtime;
            # no manual cache clear is needed.
            processor_digest = normalization_processor_digest(ACCESS_SRC / 'tos_access/knowledge.py')
            with NormalizationCache(runtime / 'normalization.sqlite', processor_digest, **(cache_options or {})) as cache:
                core.knowledge_graph()
            processing = cache.processing_report
            normalization = {'reused_steps': cache.hits, 'computed_steps': cache.misses}

    def sql_inputs():
        return {**build_inputs(core), 'runtime': str(runtime),
                'deployed_baseline': fingerprint({'rows': runtime / 'read-model.deployed.rows.json'})}

    def static_inputs():
        return {**build_inputs(core), 'output': str(output),
                'web': fingerprint(tree_paths(REPO_ROOT / 'access/web/dist', 'web'))}

    def sql_stage():
        prepare_graph()
        revision = data_revision(core)
        counts = build_read_model_sql(core, runtime / 'read-model.sql', revision)
        return {'data_revision': revision, 'counts': counts,
                'processing': processing, 'normalization_cache': normalization}

    def static_stage():
        prepare_graph()
        return build_static_assets(core, output)

    # These are exact generated completion markers, not source or cache history.
    # A partial/failed build cannot be deployed using a previous success manifest.
    for marker in (runtime / 'manifest.json', output / '__edge/build-manifest.json'):
        marker.unlink(missing_ok=True)
    sql = stages.run('read-model', sql_inputs,
                     lambda: {name: runtime / name for name in
                              ('read-model.sql', 'read-model.delta.sql', 'read-model.rows.json')}, sql_stage)
    static_summary = stages.run('static-responses', static_inputs,
                                lambda: {**tree_paths(output, 'static', exclude=('__edge/build-manifest.json',)),
                                         'static/index.html': output / 'index.html'}, static_stage)
    stages.verify()
    revision, counts = sql['data_revision'], sql['counts']
    manifest = {
        "schema": "tos_cloudflare_edge_build_v1",
        "read_model_schema": READ_MODEL_SCHEMA_VERSION,
        "data_revision": revision,
        "processing": processing if processing is not None else {
            'status': 'not-run', 'reason': 'completed-build-stage-reused',
            'origin_run_id': (sql.get('processing') or {}).get('run_id'),
            'executed': 0, 'reused': 0, 'is_semantic_acceptance': False,
        },
        "build_stages": stages.report,
        "source_owner": "Tree-of-Sophia",
        "source_paths": [
            core.index_path.relative_to(REPO_ROOT).as_posix(),
            core.philosophy_graph_projection_path.relative_to(REPO_ROOT).as_posix(),
            core.bibliographic_graph_path.relative_to(REPO_ROOT).as_posix(),
            core.entity_type_registry_path.relative_to(REPO_ROOT).as_posix(),
            core.relation_type_registry_path.relative_to(REPO_ROOT).as_posix(),
            core.evidence_projection_path.relative_to(REPO_ROOT).as_posix(),
            core.philosophy_post_planting_audit_path.relative_to(REPO_ROOT).as_posix(),
            *[
                path.relative_to(REPO_ROOT).as_posix()
                for path in sorted((core.tos_root / "ToS/source-witnesses/access-requests/public-ledger").glob("*.access-request.json"))
            ],
        ],
        "producer_paths": [
            "access/deploy/cloudflare-worker/scripts/build_runtime.py",
            "access/deploy/cloudflare-worker/scripts/build_stages.py",
            "access/deploy/cloudflare-worker/scripts/incremental_runtime.py",
            "access/src/tos_access/knowledge.py",
            "access/src/tos_access/human_form_codec.py",
            "access/src/tos_access/normalization_cache.py",
            "access/src/tos_access/published_read_metadata.py",
            "access/src/tos_access/processing.py",
            "access/src/tos_access/search_read_model.py",
        ],
        "contract_refs": [
            "access/contracts/knowledge-api.v1.json",
            "access/contracts/knowledge-graph.v1.schema.json",
            "access/contracts/lens-spec.v1.schema.json",
            "access/contracts/lens-result.v1.schema.json",
            "access/contracts/temporal-comparison-request.v1.schema.json",
            "access/contracts/temporal-comparison-result.v1.schema.json",
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            "ToS/contracts/semantic-relation-type-registry.schema.json",
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        ],
        "revision_policy": {
            "imports_when": "source data, normalized item content, capability data, published catalog, or read-model schema changes",
            "does_not_import_when": "only documentation or Worker-only code changes",
        },
        "counts": counts,
        "normalization_cache": normalization if normalization is not None else {'reused_steps': 0, 'computed_steps': 0},
        "default_corpus_view": next(iter(static_summary["corpus"].get("graph_views", [])), ""),
        "default_philosophy_view": next(iter(static_summary["philosophy"].get("views", [])), "chronology"),
        "authority_limit": "Cloudflare carries a generated read model; ToS authored and reviewed surfaces remain authoritative.",
    }
    atomic_json(output / '__edge/build-manifest.json', manifest)
    atomic_json(runtime / 'manifest.json', manifest)
    return manifest


def validate_output_paths(output: Path, runtime: Path) -> None:
    # The producer replaces output recursively. Never admit roots, source trees,
    # overlapping outputs/cache, or an ancestor of this repository as its target.
    forbidden = {Path('/'), Path.home(), *Path.home().parents}
    if (output in forbidden or runtime in forbidden
            or output == runtime or output in runtime.parents or runtime in output.parents
            or output == REPO_ROOT or output in REPO_ROOT.parents
            or (output.is_relative_to(REPO_ROOT) and output != WORKER_ROOT / 'dist')
            or (runtime.is_relative_to(REPO_ROOT) and runtime != WORKER_ROOT / 'runtime')):
        raise RuntimeError('unsafe or overlapping edge output/runtime paths')


def main() -> int:
    args = parse_args()
    output, runtime = args.output.resolve(), args.runtime.resolve()
    validate_output_paths(output, runtime)
    core = ToSAccessCore.discover(REPO_ROOT)
    with build_lock(runtime):
        manifest = build(core, output, runtime, cache_options={
            'max_cache_bytes': args.cache_max_mib * 1024 * 1024,
            'max_cache_entries': args.cache_max_entries,
            'keep_runs': args.cache_keep_runs,
        })
    print(compact_json(manifest))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
