"""Bounded, immutable SQLite read model for exact knowledge substring search.

The access core still owns the normalized graph and its source revision.  This
module owns only a derived query carrier: every document is admitted before the
carrier is published, and every posting is complete for the immutable snapshot.
It is deliberately independent of FTS.  FTS tokenization cannot reproduce the
transport contract's ``lower()`` + substring semantics (including punctuation,
JSON escaping, Unicode and short queries).
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import os
import sqlite3
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from threading import RLock
from typing import Any, Iterable


SEARCH_READ_MODEL_SCHEMA = "tos_knowledge_search_read_model_v3"
SEARCH_CURSOR_SCHEMA = "tos_knowledge_search_cursor_v1"
SEARCH_NGRAM_SIZE = 3
SEARCH_READ_MODEL_PAGE_SIZE = 256
SEARCH_READ_MODEL_MAX_CANDIDATES = 50_000
SEARCH_READ_MODEL_MAX_VERIFY_CHARS = 16_000_000
SEARCH_READ_MODEL_MAX_DOCUMENT_CHARS = 8_000_000
SEARCH_READ_MODEL_MAX_POSTINGS = 10_000_000
# No guessed full-corpus quota is safe.  The owner must pass a measured quota
# (including temporary/final-file headroom) from its runtime storage policy.
SEARCH_READ_MODEL_DEFAULT_BYTES: int | None = None
SEARCH_READ_MODEL_PAGE_BYTES = 4096
SEARCH_CURSOR_TTL_SECONDS = 15 * 60
SEARCH_NORMALIZATION_IMPLEMENTATION = "lower-json-sort-keys-ensure-ascii-false-surrogatepass-v1"
SEARCH_READ_MODEL_MAX_QUERY_CHARS = 256
SEARCH_READ_MODEL_MAX_FILTER_VALUES = 100
SEARCH_READ_MODEL_MAX_FILTER_VALUE_CHARS = 256


class SearchReadModelError(RuntimeError):
    """The derived carrier cannot be admitted or does not match its snapshot."""


class SearchReadModelBuildError(SearchReadModelError):
    """A complete read model could not be built within its explicit budget."""


class SearchReadModelSnapshotError(SearchReadModelError):
    """A carrier belongs to another immutable source snapshot."""


class SearchReadModelUnindexedError(SearchReadModelError):
    """The requested query shape has no admitted bounded carrier route."""


def normalize_search_query(value: Any) -> str:
    """Apply the same query normalization as the transport contract.

    ``lower`` is intentional.  ``casefold`` would make ``Straße`` match
    ``STRASSE`` and would change the existing ABI.
    """

    if not isinstance(value, str):
        raise SearchReadModelError("knowledge search query must be a string")
    if len(value) > SEARCH_READ_MODEL_MAX_QUERY_CHARS:
        raise SearchReadModelError("knowledge search query exceeds 256 characters")
    normalized = value.strip()
    needle = normalized.lower()
    if len(needle) > SEARCH_READ_MODEL_MAX_QUERY_CHARS:
        raise SearchReadModelError("knowledge search query exceeds 256 characters")
    return needle


def _canonical_filter_digest(
    *,
    sources: Iterable[str] | None,
    kind_ids: Iterable[str] | None,
    predicate_ids: Iterable[str] | None,
) -> str:
    payload = {
        "sources": sorted(set(sources or ())),
        "kind_ids": sorted(set(kind_ids or ())),
        "predicate_ids": sorted(set(predicate_ids or ())),
    }
    return hashlib.sha256(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    ).hexdigest()


def _cursor_encode(payload: dict[str, Any]) -> str:
    raw = json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return base64.urlsafe_b64encode(raw).decode("ascii").rstrip("=")


def _cursor_decode(value: str) -> dict[str, Any]:
    if not isinstance(value, str) or not value or len(value) > 2048:
        raise SearchReadModelError("invalid knowledge search cursor")
    try:
        padded = value + "=" * (-len(value) % 4)
        decoded = json.loads(base64.urlsafe_b64decode(padded.encode("ascii")))
    except (ValueError, UnicodeError, binascii.Error, json.JSONDecodeError) as error:
        raise SearchReadModelError("invalid knowledge search cursor") from error
    if not isinstance(decoded, dict) or decoded.get("schema") != SEARCH_CURSOR_SCHEMA:
        raise SearchReadModelError("invalid knowledge search cursor")
    return decoded


@dataclass(frozen=True)
class SearchReadModelPage:
    """One bounded candidate page, before caller-side display ranking."""

    rows: tuple[dict[str, Any], ...]
    candidate_rows: int
    verified_chars: int
    has_more: bool
    next_cursor: str | None
    ordering_scope: str = "candidate-position"
    sql_pages: int = 0

    def as_dict(self) -> dict[str, Any]:
        return {
            "rows": list(self.rows),
            "candidate_rows": self.candidate_rows,
            "verified_chars": self.verified_chars,
            "has_more": self.has_more,
            "next_cursor": self.next_cursor,
            "ordering_scope": self.ordering_scope,
            "work": {
                "candidate_rows": self.candidate_rows,
                "verified_chars": self.verified_chars,
                "sql_pages": self.sql_pages,
            },
        }


class SQLiteKnowledgeSearchReadModel:
    """A complete, source-revision-bound SQLite substring index.

    The builder writes a temporary file and atomically replaces the requested
    path only after every document and 3-gram posting has been committed.
    Query pages use keyset continuation and never perform a global count or
    global rank sort.  The caller can rank the bounded page and advertise that
    its ordering scope is page-local/candidate-position rather than pretending
    it is a global result order. Queries shorter than three characters use
    bounded position pages; three-character-and-longer queries use the
    complete 3-gram carrier.
    """

    def __init__(self, graph: dict[str, Any], path: Path, connection: sqlite3.Connection):
        self.graph = graph
        self.path = Path(path)
        self.connection = connection
        self.source_revision = str(graph.get("source_revision") or "")
        # Positions are dense over dict-valued graph entries, matching the
        # builder.  The caller owns this immutable source snapshot and its
        # source_revision; SQLite carries metadata, compact selected-document
        # digests, and candidate postings only.  We do not rehash the whole
        # graph on every query.
        self._graph_documents = {
            kind: tuple(item for item in (graph.get(kind) or ()) if isinstance(item, dict))
            for kind in ("nodes", "relations")
        }
        self._lock = RLock()

    @classmethod
    def build(
        cls,
        graph: dict[str, Any],
        path: str | Path,
        *,
        max_bytes: int | None = SEARCH_READ_MODEL_DEFAULT_BYTES,
        max_postings: int = SEARCH_READ_MODEL_MAX_POSTINGS,
        max_document_chars: int = SEARCH_READ_MODEL_MAX_DOCUMENT_CHARS,
    ) -> "SQLiteKnowledgeSearchReadModel":
        target = Path(path).expanduser()
        if not target.is_absolute():
            raise SearchReadModelBuildError("search read-model path must be absolute")
        if max_bytes is None or max_bytes < SEARCH_READ_MODEL_PAGE_BYTES or max_postings < 1 or max_document_chars < 1:
            raise SearchReadModelBuildError("search read-model budgets must be positive")
        target.parent.mkdir(parents=True, exist_ok=True)
        temp_name: str | None = None
        connection: sqlite3.Connection | None = None
        try:
            with tempfile.NamedTemporaryFile(
                dir=target.parent, prefix=f".{target.name}.", suffix=".tmp", delete=False
            ) as temporary:
                temp_name = temporary.name
            # DELETE journal mode avoids a persistent WAL side file.  The
            # caller must reserve headroom for this temporary DB plus the
            # eventual final file; max_page_count is enforced during writes.
            connection = sqlite3.connect(temp_name)
            connection.execute(f"PRAGMA max_page_count = {max_bytes // SEARCH_READ_MODEL_PAGE_BYTES}")
            connection.execute("PRAGMA journal_mode=DELETE")
            connection.execute("PRAGMA synchronous=FULL")
            connection.execute("PRAGMA foreign_keys=ON")
            connection.executescript(
                """
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE search_documents (
                    kind TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    id TEXT NOT NULL,
                    source_graph TEXT NOT NULL,
                    kind_id TEXT NOT NULL,
                    predicate_id TEXT NOT NULL,
                    id_lower TEXT NOT NULL,
                    native_id_lower TEXT NOT NULL,
                    identity_values TEXT NOT NULL,
                    visible_values TEXT NOT NULL,
                    document_chars INTEGER NOT NULL,
                    document_digest BLOB NOT NULL,
                    PRIMARY KEY (kind, position)
                ) WITHOUT ROWID;
                CREATE TABLE search_grams (
                    kind TEXT NOT NULL,
                    n INTEGER NOT NULL,
                    gram BLOB NOT NULL,
                    position INTEGER NOT NULL,
                    PRIMARY KEY (kind, n, gram, position)
                ) WITHOUT ROWID;
                CREATE TABLE search_gram_stats (
                    kind TEXT NOT NULL,
                    n INTEGER NOT NULL,
                    gram BLOB NOT NULL,
                    postings INTEGER NOT NULL,
                    PRIMARY KEY (kind, n, gram)
                ) WITHOUT ROWID;
                CREATE TABLE search_pending_grams (
                    n INTEGER NOT NULL,
                    gram BLOB NOT NULL,
                    PRIMARY KEY (n, gram)
                ) WITHOUT ROWID;
                """
            )
            revision = str(graph.get("source_revision") or "")
            if not revision:
                raise SearchReadModelBuildError("search read-model requires a non-empty source revision")
            snapshot_digest = cls._snapshot_digest(graph)
            connection.executemany(
                "INSERT INTO metadata(key,value) VALUES (?,?)",
                (
                    ("schema", SEARCH_READ_MODEL_SCHEMA),
                    ("source_revision", revision),
                    ("graph_schema", str(graph.get("schema") or "")),
                    ("snapshot_digest", snapshot_digest),
                    ("normalization", SEARCH_NORMALIZATION_IMPLEMENTATION),
                    ("ngram_size", str(SEARCH_NGRAM_SIZE)),
                    ("complete", "false"),
                ),
            )
            posting_count = 0
            for kind, items in (("nodes", graph.get("nodes")), ("relations", graph.get("relations"))):
                if not isinstance(items, list):
                    continue
                for position, item in enumerate(item for item in items if isinstance(item, dict)):
                    document = cls._searchable(item)
                    if len(document) > max_document_chars:
                        raise SearchReadModelBuildError(
                            f"{kind} document exceeds search read-model character budget"
                        )
                    id_lower, native_id_lower, identity_values, visible_values = cls._rank_fields(
                        item, relation=kind == "relations"
                    )
                    connection.execute(
                        "INSERT INTO search_documents(kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest) "
                        "VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
                        (
                            kind,
                            position,
                            str(item.get("id") or ""),
                            str(item.get("source_graph") or ""),
                            str(item.get("kind_id") or ""),
                            str(item.get("predicate_id") or ""),
                            id_lower,
                            native_id_lower,
                            identity_values,
                            visible_values,
                            len(document),
                            hashlib.sha256(document.encode("utf-8", "surrogatepass")).digest(),
                        ),
                    )
                    # One document contributes one posting per gram.  Keep
                    # the deduplication table in SQLite rather than a Python
                    # set: a large transformed document must consume the
                    # admitted disk budget, not an unbounded heap allocation.
                    connection.execute("DELETE FROM search_pending_grams")
                    # Keep the persistent carrier compact. Short queries do
                    # not have a sound 3-gram candidate; they use the same
                    # bounded position seed and opaque continuation as an
                    # empty query rather than demanding a second full index.
                    for n in (SEARCH_NGRAM_SIZE,):
                        connection.executemany(
                            "INSERT OR IGNORE INTO search_pending_grams(n,gram) VALUES (?,?)",
                            (
                                (n, document[offset : offset + n].encode("utf-8", "surrogatepass"))
                                for offset in range(len(document) - n + 1)
                            ),
                        )
                    document_postings = int(
                        connection.execute("SELECT COUNT(*) FROM search_pending_grams").fetchone()[0]
                    )
                    posting_count += document_postings
                    if posting_count > max_postings:
                        raise SearchReadModelBuildError("search read-model posting budget exceeded")
                    connection.executemany(
                        "INSERT INTO search_grams(kind,n,gram,position) VALUES (?,?,?,?)",
                        (
                            (kind, n, gram, position)
                            for n, gram in connection.execute("SELECT n,gram FROM search_pending_grams")
                        ),
                    )
                    connection.execute("DELETE FROM search_pending_grams")
                    cls._check_page_budget(connection, max_bytes)
            connection.executescript(
                """
                INSERT INTO search_gram_stats(kind,n,gram,postings)
                SELECT kind,n,gram,COUNT(*) FROM search_grams GROUP BY kind,n,gram;
                CREATE INDEX search_document_filter ON search_documents(kind,source_graph,kind_id,predicate_id,position);
                DROP TABLE search_pending_grams;
                """
            )
            cls._check_page_budget(connection, max_bytes)
            connection.execute("UPDATE metadata SET value='true' WHERE key='complete'")
            connection.commit()
            cls._check_page_budget(connection, max_bytes)
            connection.close()
            connection = None
            os.replace(temp_name, target)
            temp_name = None
            return cls.open(graph, target)
        except sqlite3.DatabaseError as error:
            raise SearchReadModelBuildError("search read-model disk budget exceeded or SQLite write failed") from error
        finally:
            if connection is not None:
                connection.close()
            if temp_name is not None:
                try:
                    os.unlink(temp_name)
                except FileNotFoundError:
                    pass

    @staticmethod
    def _searchable(item: dict[str, Any]) -> str:
        # Importing knowledge here would create a cycle.  This is intentionally
        # the same stable JSON/lower contract as knowledge._searchable.
        return json.dumps(item, ensure_ascii=False, sort_keys=True).lower()

    @classmethod
    def _snapshot_digest(cls, graph: dict[str, Any]) -> str:
        """Bind a carrier to exact searchable source content, not revision text alone."""
        digest = hashlib.sha256()
        for kind in ("nodes", "relations"):
            for position, item in enumerate(item for item in (graph.get(kind) or ()) if isinstance(item, dict)):
                document = cls._searchable(item).encode("utf-8", "surrogatepass")
                digest.update(kind.encode("ascii"))
                digest.update(b"\0")
                digest.update(str(position).encode("ascii"))
                digest.update(b"\0")
                digest.update(hashlib.sha256(document).digest())
        return digest.hexdigest()

    @staticmethod
    def _rank_fields(item: dict[str, Any], *, relation: bool) -> tuple[str, str, str, str]:
        """Return compact inputs to the existing four-level search rank.

        The complete serialized document stays source-owned and is never
        retained in this carrier. Arrays are JSON so punctuation and Unicode
        in display values remain lossless for SQLite ``json_each`` checks.
        """
        display = item.get("display") if isinstance(item.get("display"), dict) else {}
        primary_field = "label" if relation else "title"
        visible_fields = (
            ("label", "inverse_label", "statement", "explanation")
            if relation
            else ("title", "kind_label", "summary")
        )

        def values(fields: tuple[str, ...]) -> list[str]:
            result: list[str] = []
            for field in fields:
                value = display.get(field)
                if isinstance(value, dict):
                    result.extend(
                        str(candidate).lower()
                        for candidate in value.values()
                        if isinstance(candidate, str)
                    )
                elif isinstance(value, str):
                    result.append(value.lower())
            return result

        return (
            str(item.get("id") or "").lower(),
            str(item.get("native_id") or "").lower(),
            json.dumps(values((primary_field,)), ensure_ascii=False, separators=(",", ":")),
            json.dumps(values(visible_fields), ensure_ascii=False, separators=(",", ":")),
        )

    @staticmethod
    def _check_page_budget(connection: sqlite3.Connection, max_bytes: int) -> None:
        page_size = int(connection.execute("PRAGMA page_size").fetchone()[0])
        page_count = int(connection.execute("PRAGMA page_count").fetchone()[0])
        if page_count * page_size > max_bytes:
            raise SearchReadModelBuildError("search read-model disk budget exceeded")

    @classmethod
    def open(cls, graph: dict[str, Any], path: str | Path) -> "SQLiteKnowledgeSearchReadModel":
        target = Path(path).expanduser()
        if not target.is_absolute() or not target.is_file():
            raise SearchReadModelSnapshotError("search read-model is unavailable")
        before_state = cls._stat_path(target)
        connection = sqlite3.connect(target, check_same_thread=False)
        try:
            connection.execute("PRAGMA query_only=ON")
            rows = dict(connection.execute("SELECT key,value FROM metadata"))
        except sqlite3.DatabaseError as error:
            connection.close()
            raise SearchReadModelSnapshotError("search read-model is corrupt") from error
        try:
            after_state = cls._stat_path(target)
        except SearchReadModelSnapshotError:
            connection.close()
            raise
        if before_state != after_state:
            connection.close()
            raise SearchReadModelSnapshotError("search read-model changed while opening")
        if (
            rows.get("schema") != SEARCH_READ_MODEL_SCHEMA
            or rows.get("complete") != "true"
            or rows.get("normalization") != SEARCH_NORMALIZATION_IMPLEMENTATION
            or rows.get("ngram_size") != str(SEARCH_NGRAM_SIZE)
            or rows.get("graph_schema") != str(graph.get("schema") or "")
            or rows.get("snapshot_digest") != cls._snapshot_digest(graph)
        ):
            connection.close()
            raise SearchReadModelSnapshotError("search read-model is not complete")
        if rows.get("source_revision") != str(graph.get("source_revision") or ""):
            connection.close()
            raise SearchReadModelSnapshotError("search read-model source revision differs")
        model = cls(graph, target, connection)
        model._file_state = after_state
        return model

    @staticmethod
    def _stat_path(path: Path) -> tuple[int, int, int]:
        try:
            stat = path.stat()
        except OSError as error:
            raise SearchReadModelSnapshotError("search read-model disappeared") from error
        return (stat.st_ino, stat.st_mtime_ns, stat.st_size)

    def _path_state(self) -> tuple[int, int, int]:
        return self._stat_path(self.path)

    def _source_document(self, kind: str, position: int, expected: tuple[Any, ...]) -> str:
        """Load and validate one exact document from the source graph.

        The read model intentionally does not retain a second full document
        blob.  Its source graph is the owner of searchable content; compact
        metadata and a per-document digest make selected-document drift fail
        closed before returning a result.  Complete snapshot identity still
        belongs to the caller-owned source_revision; a zero-result gram lookup
        does not rehash the whole source graph.
        """

        documents = self._graph_documents.get(kind, ())
        if position < 0 or position >= len(documents):
            raise SearchReadModelSnapshotError("search document disappeared from source graph")
        item = documents[position]
        expected_id, expected_source, expected_kind_id, expected_predicate, expected_chars, expected_digest = expected
        if (
            str(item.get("id") or "") != str(expected_id)
            or str(item.get("source_graph") or "") != str(expected_source)
            or str(item.get("kind_id") or "") != str(expected_kind_id)
            or str(item.get("predicate_id") or "") != str(expected_predicate)
        ):
            raise SearchReadModelSnapshotError("search document metadata differs from source graph")
        search_text = self._searchable(item)
        if len(search_text) != int(expected_chars):
            raise SearchReadModelSnapshotError("search document length differs from source graph")
        digest = hashlib.sha256(search_text.encode("utf-8", "surrogatepass")).digest()
        if digest != bytes(expected_digest):
            raise SearchReadModelSnapshotError("search document digest differs from source graph")
        return search_text

    def source_item(self, kind: str, position: int) -> dict[str, Any]:
        """Return one source-owned item by its dense carrier position.

        Callers must have validated the selected row through ``_source_document``
        (or otherwise hold the same immutable snapshot).  Keeping this lookup
        positional avoids rebuilding a full per-page list for every result.
        """
        documents = self._graph_documents.get(kind, ())
        if position < 0 or position >= len(documents):
            raise SearchReadModelSnapshotError("search document disappeared from source graph")
        return documents[position]

    def _assert_rank_fields(
        self,
        kind: str,
        position: int,
        expected: tuple[str, str, str, str],
    ) -> None:
        documents = self._graph_documents.get(kind, ())
        if position < 0 or position >= len(documents):
            raise SearchReadModelSnapshotError("search document disappeared from source graph")
        actual = self._rank_fields(documents[position], relation=kind == "relations")
        if actual != expected:
            raise SearchReadModelSnapshotError("search document rank fields differ from source graph")

    def _assert_stable(self) -> None:
        if self._path_state() != self._file_state:
            raise SearchReadModelSnapshotError("search read-model changed during query")

    def close(self) -> None:
        with self._lock:
            self.connection.close()

    def __enter__(self) -> "SQLiteKnowledgeSearchReadModel":
        return self

    def __exit__(self, _type: Any, _value: Any, _traceback: Any) -> None:
        self.close()

    def _cursor_payload(
        self,
        *,
        kind: str,
        query: str,
        n: int | None,
        gram: str | None,
        position: int,
        filters_digest: str,
    ) -> dict[str, Any]:
        return {
            "schema": SEARCH_CURSOR_SCHEMA,
            "source_revision": self.source_revision,
            "kind": kind,
            "query": query,
            "n": n,
            "gram": gram,
            "position": position,
            "filters_digest": filters_digest,
            "issued_at": int(time.time()),
            "expires_at": int(time.time()) + SEARCH_CURSOR_TTL_SECONDS,
        }

    def candidate_page(
        self,
        kind: str,
        query: Any = "",
        *,
        sources: Iterable[str] | None = None,
        kind_ids: Iterable[str] | None = None,
        predicate_ids: Iterable[str] | None = None,
        cursor: str | None = None,
        page_size: int = SEARCH_READ_MODEL_PAGE_SIZE,
        max_candidates: int = SEARCH_READ_MODEL_MAX_CANDIDATES,
        max_verify_chars: int = SEARCH_READ_MODEL_MAX_VERIFY_CHARS,
    ) -> SearchReadModelPage:
        if kind not in {"nodes", "relations"}:
            raise SearchReadModelError("search read-model kind is invalid")
        if page_size < 1 or page_size > 1000 or max_candidates < page_size or max_verify_chars < 1:
            raise SearchReadModelError("search read-model query budgets are invalid")
        needle = normalize_search_query(query)
        with self._lock:
            self._assert_stable()
        filter_value_count = 0

        def bounded_filter(values: Iterable[str] | None) -> tuple[str, ...]:
            nonlocal filter_value_count
            if values is None:
                return ()
            raw_values: list[str] = []
            try:
                for value in values:
                    if filter_value_count >= SEARCH_READ_MODEL_MAX_FILTER_VALUES:
                        raise SearchReadModelError("knowledge search filters exceed bounded query input")
                    if not isinstance(value, str):
                        raise SearchReadModelError("knowledge search filters must contain strings")
                    if len(value) > SEARCH_READ_MODEL_MAX_FILTER_VALUE_CHARS:
                        raise SearchReadModelError("knowledge search filters exceed bounded query input")
                    raw_values.append(value)
                    filter_value_count += 1
            except TypeError as error:
                raise SearchReadModelError("knowledge search filters must be iterable") from error
            return tuple(sorted(set(raw_values)))

        source_values = bounded_filter(sources)
        kind_values = bounded_filter(kind_ids)
        predicate_values = bounded_filter(predicate_ids)
        filters_digest = _canonical_filter_digest(
            sources=source_values, kind_ids=kind_values, predicate_ids=predicate_values
        )
        after_position = -1
        if cursor is not None:
            payload = _cursor_decode(cursor)
            if (
                payload.get("source_revision") != self.source_revision
                or payload.get("kind") != kind
                or payload.get("query") != needle
                or payload.get("filters_digest") != filters_digest
                or not isinstance(payload.get("position"), int)
                or not isinstance(payload.get("expires_at"), int)
                or payload.get("expires_at") < int(time.time())
            ):
                raise SearchReadModelSnapshotError("knowledge search cursor does not match this snapshot/query")
            after_position = payload["position"]

        n: int | None = None
        gram: str | None = None
        stats_lookups = 0
        if len(needle) >= SEARCH_NGRAM_SIZE:
            n = SEARCH_NGRAM_SIZE
            grams = tuple(dict.fromkeys(needle[offset : offset + n] for offset in range(len(needle) - n + 1)))
            with self._lock:
                stats = []
                stats_lookups = len(grams)
                for candidate in grams:
                    candidate_bytes = candidate.encode("utf-8", "surrogatepass")
                    row = self.connection.execute(
                        "SELECT postings FROM search_gram_stats WHERE kind=? AND n=? AND gram=?",
                        (kind, n, candidate_bytes),
                    ).fetchone()
                    stats.append((int(row[0]) if row else 0, candidate))
            posting_count, selected_gram = min(stats, key=lambda pair: pair[0])
            if posting_count == 0:
                # Complete index and absent gram: exact zero, not an
                # unindexed/fallback condition.
                with self._lock:
                    self._assert_stable()
                return SearchReadModelPage((), 0, 0, False, None, sql_pages=stats_lookups)
            gram_bytes = selected_gram.encode("utf-8", "surrogatepass")
            # Seed the page from the selected posting list before applying
            # source/type filters.  A highly common gram plus a selective
            # source filter must not make SQLite walk an unbounded posting
            # list just to discover that most rows are filtered out.
            seed_sql = (
                "SELECT position FROM search_grams "
                "WHERE kind=? AND n=? AND gram=? AND position>? "
                "ORDER BY position LIMIT ?"
            )
            seed_bindings: list[Any] = [kind, n, gram_bytes, after_position]
        else:
            # Empty and short queries have no sound 3-gram candidate. A
            # bounded position seed preserves continuation semantics without a
            # hidden one-shot full-corpus scan.
            seed_sql = (
                "SELECT position FROM search_documents "
                "WHERE kind=? AND position>? ORDER BY position LIMIT ?"
            )
            seed_bindings = [kind, after_position]

        # One extra row tells us whether continuation exists.  It is still
        # bounded by max_candidates and never turns into a global COUNT.  The
        # seed itself is bounded before the filtered document join.
        fetch_limit = min(page_size + 1, max_candidates + 1)
        with self._lock:
            seed_bindings.append(fetch_limit)
            seed_rows = self.connection.execute(seed_sql, seed_bindings).fetchall()
            seed_positions = [int(row[0]) for row in seed_rows]
            if not seed_positions:
                self._assert_stable()
                return SearchReadModelPage((), 0, 0, False, None, sql_pages=stats_lookups + 1)
            admitted_positions = seed_positions[:page_size]
            position_placeholders = ",".join("?" for _ in admitted_positions)
            filter_where = []
            filter_bindings: list[Any] = []
            if source_values:
                filter_where.append("d.source_graph IN (" + ",".join("?" for _ in source_values) + ")")
                filter_bindings.extend(source_values)
            if kind_values:
                filter_where.append("d.kind_id IN (" + ",".join("?" for _ in kind_values) + ")")
                filter_bindings.extend(kind_values)
            if predicate_values:
                filter_where.append("d.predicate_id IN (" + ",".join("?" for _ in predicate_values) + ")")
                filter_bindings.extend(predicate_values)
            filter_sql = " AND ".join(filter_where) or "1=1"
            raw_rows = self.connection.execute(
                "SELECT d.position,d.id,d.source_graph,d.kind_id,d.predicate_id,d.document_chars,d.document_digest "
                f"FROM search_documents d WHERE d.kind=? AND d.position IN ({position_placeholders}) "
                f"AND {filter_sql} ORDER BY d.position",
                [kind, *admitted_positions, *filter_bindings],
            ).fetchall()
            self._assert_stable()
        candidate_rows = len(admitted_positions)
        rows: list[dict[str, Any]] = []
        verified_chars = 0
        last_position = after_position
        budget_break = False
        consumed_positions = 0
        for raw in raw_rows[:page_size]:
            position, identifier, source_graph, kind_id, predicate_id, document_chars, document_digest = raw
            if verified_chars + int(document_chars) > max_verify_chars:
                # The current candidate is not consumed.  Resume from the
                # preceding position, preserving exact continuation.
                budget_break = True
                break
            with self._lock:
                self._assert_stable()
                search_text = self._source_document(
                    kind,
                    int(position),
                    (identifier, source_graph, kind_id, predicate_id, document_chars, document_digest),
                )
                self._assert_stable()
            verified_chars += len(search_text)
            last_position = int(position)
            consumed_positions += 1
            if needle and needle not in search_text:
                continue
            rows.append(
                {
                    "position": int(position),
                    "id": str(identifier),
                    "source_graph": str(source_graph),
                    "kind_id": str(kind_id),
                    "predicate_id": str(predicate_id),
                }
            )
        seed_has_more = len(seed_positions) > page_size
        has_more = budget_break or seed_has_more
        next_cursor = None
        if budget_break and last_position == after_position:
            raise SearchReadModelError("search verification budget is below one candidate document")
        # If filters removed every row in this bounded seed, advance to the
        # seed's last position so a source-fenced continuation makes progress.
        if has_more and consumed_positions == len(raw_rows[:page_size]):
            last_position = max(last_position, seed_positions[min(page_size, len(seed_positions)) - 1])
        if has_more and last_position > after_position:
            next_cursor = _cursor_encode(
                self._cursor_payload(
                    kind=kind,
                    query=needle,
                    n=n,
                    gram=gram,
                    position=last_position,
                    filters_digest=filters_digest,
                )
            )
        with self._lock:
            self._assert_stable()
        return SearchReadModelPage(
            tuple(rows),
            candidate_rows,
            verified_chars,
            has_more,
            next_cursor,
            sql_pages=stats_lookups + 2 + consumed_positions,
        )

    @staticmethod
    def _rank_expression(alias: str = "d") -> str:
        identity_exact = (
            f"{alias}.id_lower = ? OR {alias}.native_id_lower = ? OR "
            f"EXISTS (SELECT 1 FROM json_each({alias}.identity_values) v WHERE v.value = ?)"
        )
        identity_prefix = (
            f"instr({alias}.id_lower, ?) = 1 OR instr({alias}.native_id_lower, ?) = 1 OR "
            f"EXISTS (SELECT 1 FROM json_each({alias}.identity_values) v WHERE instr(v.value, ?) = 1)"
        )
        visible = (
            f"EXISTS (SELECT 1 FROM json_each({alias}.visible_values) v WHERE instr(v.value, ?) > 0)"
        )
        return f"CASE WHEN {identity_exact} THEN 0 WHEN {identity_prefix} THEN 1 WHEN {visible} THEN 2 ELSE 3 END"

    @staticmethod
    def _rank_bindings(needle: str) -> list[str]:
        return [needle, needle, needle, needle, needle, needle, needle]

    def ranked_page(
        self,
        kind: str,
        query: Any = "",
        *,
        sources: Iterable[str] | None = None,
        kind_ids: Iterable[str] | None = None,
        predicate_ids: Iterable[str] | None = None,
        cursor: str | None = None,
        page_size: int = SEARCH_READ_MODEL_PAGE_SIZE,
        max_candidates: int = SEARCH_READ_MODEL_MAX_CANDIDATES,
        max_verify_chars: int = SEARCH_READ_MODEL_MAX_VERIFY_CHARS,
    ) -> SearchReadModelPage:
        """Return a globally ranked page without materializing graph matches.

        This is an explicit indexed route.  The carrier can prove complete
        candidate membership only for three-character-and-longer queries; a
        shorter query fails closed so callers cannot accidentally turn this
        mode into a full-corpus scan.  Rank is the same four-level transport
        rule as ``knowledge._knowledge_search_rank`` and ties by stable lower
        ID plus source position.
        """
        if kind not in {"nodes", "relations"}:
            raise SearchReadModelError("search read-model kind is invalid")
        if page_size < 1 or page_size > 1000 or max_candidates < page_size or max_verify_chars < 1:
            raise SearchReadModelError("search read-model query budgets are invalid")
        needle = normalize_search_query(query)
        if len(needle) < SEARCH_NGRAM_SIZE:
            raise SearchReadModelUnindexedError(
                "indexed knowledge search requires a query of at least three characters"
            )

        filter_value_count = 0

        def bounded_filter(values: Iterable[str] | None) -> tuple[str, ...]:
            nonlocal filter_value_count
            if values is None:
                return ()
            raw_values: list[str] = []
            try:
                for value in values:
                    if filter_value_count >= SEARCH_READ_MODEL_MAX_FILTER_VALUES:
                        raise SearchReadModelError("knowledge search filters exceed bounded query input")
                    if not isinstance(value, str):
                        raise SearchReadModelError("knowledge search filters must contain strings")
                    if len(value) > SEARCH_READ_MODEL_MAX_FILTER_VALUE_CHARS:
                        raise SearchReadModelError("knowledge search filters exceed bounded query input")
                    raw_values.append(value)
                    filter_value_count += 1
            except TypeError as error:
                raise SearchReadModelError("knowledge search filters must be iterable") from error
            return tuple(sorted(set(raw_values)))

        source_values = bounded_filter(sources)
        kind_values = bounded_filter(kind_ids)
        predicate_values = bounded_filter(predicate_ids)
        filters_digest = _canonical_filter_digest(
            sources=source_values, kind_ids=kind_values, predicate_ids=predicate_values
        )
        cursor_rank = 0
        cursor_id = ""
        cursor_position = -1
        if cursor is not None:
            payload = _cursor_decode(cursor)
            if (
                payload.get("source_revision") != self.source_revision
                or payload.get("kind") != kind
                or payload.get("query") != needle
                or payload.get("filters_digest") != filters_digest
                or payload.get("ordering") != "rank-id-position"
                or not isinstance(payload.get("rank"), int)
                or not isinstance(payload.get("id"), str)
                or not isinstance(payload.get("position"), int)
                or not isinstance(payload.get("expires_at"), int)
                or payload.get("expires_at") < int(time.time())
            ):
                raise SearchReadModelSnapshotError("knowledge search cursor does not match this snapshot/query")
            cursor_rank = payload["rank"]
            cursor_id = payload["id"]
            cursor_position = payload["position"]

        grams = tuple(dict.fromkeys(needle[offset : offset + SEARCH_NGRAM_SIZE] for offset in range(len(needle) - SEARCH_NGRAM_SIZE + 1)))
        with self._lock:
            self._assert_stable()
            stats: list[tuple[int, str]] = []
            for gram in grams:
                row = self.connection.execute(
                    "SELECT postings FROM search_gram_stats WHERE kind=? AND n=? AND gram=?",
                    (kind, SEARCH_NGRAM_SIZE, gram.encode("utf-8", "surrogatepass")),
                ).fetchone()
                stats.append((int(row[0]) if row else 0, gram))
            posting_count, selected_gram = min(stats, key=lambda pair: pair[0])
            if posting_count == 0:
                return SearchReadModelPage((), 0, 0, False, None, ordering_scope="global-rank", sql_pages=len(grams))
            if posting_count > max_candidates:
                raise SearchReadModelError(
                    "indexed knowledge search candidate budget exceeded; continue through the legacy route"
                )

            rank_expression = self._rank_expression()
            filter_where: list[str] = []
            filter_bindings: list[Any] = []
            if source_values:
                filter_where.append("d.source_graph IN (" + ",".join("?" for _ in source_values) + ")")
                filter_bindings.extend(source_values)
            if kind_values:
                filter_where.append("d.kind_id IN (" + ",".join("?" for _ in kind_values) + ")")
                filter_bindings.extend(kind_values)
            if predicate_values:
                filter_where.append("d.predicate_id IN (" + ",".join("?" for _ in predicate_values) + ")")
                filter_bindings.extend(predicate_values)
            filter_sql = " AND ".join(filter_where) or "1=1"
            gram_bytes = selected_gram.encode("utf-8", "surrogatepass")
            # The selected posting list is complete but a single trigram is
            # only a candidate.  Fetch bounded ranked batches and verify the
            # complete needle against the source graph, skipping false
            # positives instead of treating them as carrier corruption.
            after_key: tuple[int, str, int] | None = (
                (cursor_rank, cursor_id, cursor_position) if cursor is not None else None
            )
            rows: list[dict[str, Any]] = []
            candidate_examined = 0
            verified_chars = 0
            sql_pages = len(grams)
            while len(rows) < page_size + 1 and candidate_examined < max_candidates:
                fetch_limit = min(max(page_size + 1, 64), max_candidates - candidate_examined)
                continuation_sql = ""
                continuation_bindings: list[Any] = []
                if after_key is not None:
                    after_rank, after_id, after_position = after_key
                    continuation_sql = (
                        f" AND ({rank_expression} > ? OR ({rank_expression} = ? AND "
                        "(d.id_lower > ? OR (d.id_lower = ? AND d.position > ?))))"
                    )
                    continuation_bindings.extend(
                        self._rank_bindings(needle)
                        + [after_rank]
                        + self._rank_bindings(needle)
                        + [after_rank, after_id, after_id, after_position]
                    )
                sql = (
                    f"SELECT d.position,d.id,d.source_graph,d.kind_id,d.predicate_id,d.id_lower,"
                    f"d.native_id_lower,d.identity_values,d.visible_values,d.document_chars,d.document_digest,"
                    f"{rank_expression} AS search_rank "
                    "FROM search_grams g JOIN search_documents d "
                    "ON d.kind=g.kind AND d.position=g.position "
                    "WHERE g.kind=? AND g.n=? AND g.gram=? AND "
                    f"{filter_sql}{continuation_sql} "
                    f"ORDER BY search_rank,d.id_lower,d.position LIMIT ?"
                )
                # SQLite binds placeholders in statement order: the SELECT
                # rank expression comes first, followed by posting/filter
                # predicates and the optional keyset continuation.
                bindings: list[Any] = self._rank_bindings(needle)
                bindings.extend([kind, SEARCH_NGRAM_SIZE, gram_bytes])
                bindings.extend(filter_bindings)
                bindings.extend(continuation_bindings)
                bindings.append(fetch_limit)
                with self._lock:
                    raw_rows = self.connection.execute(sql, bindings).fetchall()
                    self._assert_stable()
                sql_pages += 1
                if not raw_rows:
                    break
                for raw in raw_rows:
                    (
                        position,
                        identifier,
                        source_graph,
                        kind_id,
                        predicate_id,
                        id_lower,
                        native_id_lower,
                        identity_values,
                        visible_values,
                        document_chars,
                        document_digest,
                        search_rank,
                    ) = raw
                    after_key = (int(search_rank), str(id_lower), int(position))
                    candidate_examined += 1
                    if verified_chars + int(document_chars) > max_verify_chars:
                        raise SearchReadModelError("search verification budget is below the ranked page")
                    expected_rank_fields = (
                        str(id_lower),
                        str(native_id_lower),
                        str(identity_values),
                        str(visible_values),
                    )
                    with self._lock:
                        self._assert_stable()
                        search_text = self._source_document(
                            kind,
                            int(position),
                            (identifier, source_graph, kind_id, predicate_id, document_chars, document_digest),
                        )
                        self._assert_rank_fields(kind, int(position), expected_rank_fields)
                        self._assert_stable()
                    verified_chars += len(search_text)
                    if needle not in search_text:
                        # A selected gram is necessary, not sufficient, for
                        # the full substring.  Continue the bounded scan.
                        continue
                    rows.append(
                        {
                            "position": int(position),
                            "id": str(identifier),
                            "source_graph": str(source_graph),
                            "kind_id": str(kind_id),
                            "predicate_id": str(predicate_id),
                            "search_rank": int(search_rank),
                        }
                    )
                    if len(rows) >= page_size + 1:
                        break
                if len(rows) >= page_size + 1:
                    break
                if len(raw_rows) < fetch_limit:
                    break

            has_more = len(rows) > page_size
            next_cursor = None
            if has_more and rows:
                last = rows[page_size - 1]
                next_cursor = _cursor_encode(
                    {
                        **self._cursor_payload(
                            kind=kind,
                            query=needle,
                            n=SEARCH_NGRAM_SIZE,
                            gram=selected_gram,
                            position=int(last["position"]),
                            filters_digest=filters_digest,
                        ),
                        "ordering": "rank-id-position",
                        "rank": int(last["search_rank"]),
                        "id": str(last["id"]).lower(),
                    }
                )
            return SearchReadModelPage(
                tuple(rows[:page_size]),
                candidate_examined,
                verified_chars,
                has_more,
                next_cursor,
                ordering_scope="global-rank",
                sql_pages=sql_pages,
            )


__all__ = [
    "SEARCH_CURSOR_SCHEMA",
    "SEARCH_READ_MODEL_DEFAULT_BYTES",
    "SEARCH_READ_MODEL_MAX_CANDIDATES",
    "SEARCH_READ_MODEL_MAX_DOCUMENT_CHARS",
    "SEARCH_READ_MODEL_MAX_POSTINGS",
    "SEARCH_READ_MODEL_MAX_VERIFY_CHARS",
    "SEARCH_READ_MODEL_PAGE_SIZE",
    "SEARCH_READ_MODEL_SCHEMA",
    "SEARCH_NORMALIZATION_IMPLEMENTATION",
    "SEARCH_CURSOR_TTL_SECONDS",
    "SearchReadModelBuildError",
    "SearchReadModelError",
    "SearchReadModelUnindexedError",
    "SearchReadModelPage",
    "SearchReadModelSnapshotError",
    "SQLiteKnowledgeSearchReadModel",
    "normalize_search_query",
]
