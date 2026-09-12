"""Offline-owned compressed search-v3 store; not wired into public access yet.

Stable numeric addresses are independent of reference ordering. Each term has
an indexed directory of stable fences and at most 256 ordered delta-varints.
Readers never load a graph or calculate its digest. The producer supplies the
exact normalized carrier and snapshot binding; this module grants no authority.
"""
from __future__ import annotations

import hashlib
import hmac
import json
import os
from pathlib import Path
import sqlite3
import time
import unicodedata
from dataclasses import dataclass
from functools import wraps
from typing import Any, Iterable

SCHEMA = "tos_knowledge_search_compressed_v3"
ALGORITHM = "python-lower-json-default-order-v1"
STORAGE_VERSION = 2
BLOCK_SIZE = 256
CHUNK_SIZE = 32768
MAX_ADDRESS = 2**53 - 1
MAX_DOCUMENT_BYTES = 8 * 1024 * 1024
MAX_ROW_BYTES = 1_900_000
MAX_TERMS_PER_DOCUMENT = 200_000
MAX_VALUES_PER_DOCUMENT = 8192
MAX_HEADER_BYTES = 65536
MAX_QUERY_BYTES = 65536
MIN_METADATA_BYTES = 4 * 1024 * 1024
MIN_RESPONSE_BYTES = MAX_ROW_BYTES + 8192
RESPONSE_OVERHEAD_BYTES = 4096
MAX_BLOCK_BYTES = BLOCK_SIZE * 8

class SearchInvalidRequest(ValueError):
    """Invalid caller input; adapters may map to HTTP 400."""


class SearchStaleBinding(ValueError):
    """Selected binding or pinned incarnation changed; HTTP 409."""


class SearchCursorError(ValueError):
    """Invalid or publication-incompatible continuation; HTTP 400."""


class SearchCursorExpired(SearchCursorError):
    """Authentic continuation has reached its absolute expiry; HTTP 410."""


class SearchUnavailable(ValueError, sqlite3.DatabaseError):
    """Missing, unreadable or corrupt search publication; HTTP 503."""


class SearchBudgetExceeded(ValueError, sqlite3.DatabaseError):
    """Publication or hard framing budget exceeded; owner refusal (HTTP 413)."""


def _typed_errors(method):
    @wraps(method)
    def call(*args, **kwargs):
        try:
            return method(*args, **kwargs)
        except (SearchInvalidRequest, SearchStaleBinding, SearchCursorError,
                SearchUnavailable, SearchBudgetExceeded):
            raise
        except sqlite3.IntegrityError as exc:
            raise SearchInvalidRequest(str(exc)) from exc
        except sqlite3.DatabaseError as exc:
            error = SearchBudgetExceeded if getattr(exc, "sqlite_errorcode", None) == sqlite3.SQLITE_FULL else SearchUnavailable
            raise error(str(exc)) from exc
        except (ValueError, TypeError, OverflowError) as exc:
            raise SearchInvalidRequest(str(exc)) from exc
    return call


def _require_transaction(connection: sqlite3.Connection) -> None:
    if not isinstance(connection, sqlite3.Connection) or not connection.in_transaction:
        raise SearchInvalidRequest("an already-open caller SQLite transaction is required")


def _page_cap(connection: sqlite3.Connection, requested_pages: int) -> int:
    # This is the entire main database, including the owner's carrier tables.
    # Never enlarge an independently configured, stricter owner connection cap.
    current = connection.execute("PRAGMA max_page_count").fetchone()[0]
    pages = min(current, requested_pages)
    if connection.execute("PRAGMA page_count").fetchone()[0] > pages:
        raise SearchBudgetExceeded("existing whole database exceeds search publication page cap")
    actual = connection.execute(f"PRAGMA max_page_count={pages}").fetchone()[0]
    if actual != pages:
        raise SearchBudgetExceeded("could not enforce whole database page cap")
    return pages


def _json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True)


def _bytes(value: str) -> bytes:
    return value.encode("utf-8", errors="surrogatepass")


def _integer(value: Any, name: str, minimum: int, maximum: int) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        raise SearchInvalidRequest(f"{name} must be an integer in {minimum}..{maximum}")
    return value


def _cursor_integer(value: Any, name: str, minimum: int, maximum: int) -> int:
    try:
        return _integer(value, name, minimum, maximum)
    except SearchInvalidRequest as exc:
        raise SearchCursorError(str(exc)) from exc


def order_key(identifier: Any, source_order: int) -> bytes:
    """Python codepoint ordering, then producer's within-tie source position.

    Nonzero three-byte symbols followed by zero preserve prefix ordering,
    including NUL, astral codepoints and lone surrogates. Never use SQL lower.
    """
    _integer(source_order, "source_order", 0, MAX_ADDRESS)
    return b"".join((ord(c) + 1).to_bytes(3, "big") for c in str(identifier or "").lower()) + b"\0\0\0" + source_order.to_bytes(8, "big")


def encode_postings(addresses: Iterable[int]) -> bytes:
    result = bytearray()
    previous = 0
    for address in addresses:
        _integer(address, "doc_id", 1, MAX_ADDRESS)
        delta = address - previous
        value = 2 * delta if delta >= 0 else -2 * delta - 1
        while value >= 128:
            result.append((value & 127) | 128)
            value >>= 7
        result.append(value)
        previous = address
    return bytes(result)


def decode_postings(payload: bytes) -> list[int]:
    result: list[int] = []
    value = shift = previous = 0
    for byte in payload:
        value |= (byte & 127) << shift
        if byte & 128:
            shift += 7
            if shift > 56:
                raise SearchUnavailable("invalid posting varint")
            continue
        delta = -(value // 2) - 1 if value & 1 else value // 2
        previous += delta
        if not 1 <= previous <= MAX_ADDRESS:
            raise SearchUnavailable("invalid decoded doc_id")
        result.append(previous)
        if len(result) > BLOCK_SIZE:
            raise SearchUnavailable("oversized posting block")
        value = shift = 0
    if shift:
        raise SearchUnavailable("truncated posting varint")
    if len(result) != len(set(result)):
        raise SearchUnavailable("duplicate posting address")
    return result


@dataclass(frozen=True)
class PreparedSearchDocument:
    doc_id: int
    kind: str
    identifier: Any
    source_order: int
    searchable: str
    identities: tuple[str, ...]
    visible: tuple[str, ...]
    filters: dict[str, Any]

    @classmethod
    def from_item(cls, doc_id: int, kind: str, item: dict[str, Any], source_order: int) -> "PreparedSearchDocument":
        """Offline producer helper. Caller supplies actual source-order tokens."""
        if kind not in ("node", "relation") or not isinstance(item, dict):
            raise ValueError("expected node/relation and exact normalized object")
        display = item.get("display") if isinstance(item.get("display"), dict) else {}
        fields = ("label", "inverse_label", "statement", "explanation") if kind == "relation" else ("title", "kind_label", "summary")

        def values(field: str) -> tuple[str, ...]:
            value = display.get(field)
            if isinstance(value, dict):
                return tuple(v.lower() for v in value.values() if isinstance(v, str))
            return (value.lower(),) if isinstance(value, str) else ()

        return cls(doc_id, kind, item.get("id"), source_order, _json(item).lower(),
                   (str(item.get("id") or "").lower(), str(item.get("native_id") or "").lower()) + values(fields[0]),
                   tuple(v for field in fields for v in values(field)),
                   {key: item.get(key) for key in ("source_graph", "kind_id", "predicate_id", "type_id", "relation_type_id")})


@dataclass(frozen=True)
class SearchChange:
    operation: str
    doc_id: int
    document: PreparedSearchDocument | None = None


DDL = """
CREATE TABLE search_header (singleton INTEGER PRIMARY KEY CHECK(singleton=1), header TEXT NOT NULL, cursor_key BLOB NOT NULL, high_water INTEGER NOT NULL, max_pages INTEGER NOT NULL);
CREATE TABLE search_documents (doc_id INTEGER PRIMARY KEY, kind TEXT NOT NULL, identifier BLOB NOT NULL, sort_key BLOB NOT NULL, filters BLOB NOT NULL, UNIQUE(kind,sort_key), UNIQUE(kind,identifier));
CREATE TABLE search_values (doc_id INTEGER NOT NULL, category TEXT NOT NULL, field INTEGER NOT NULL, byte_length INTEGER NOT NULL, PRIMARY KEY(doc_id,category,field)) WITHOUT ROWID;
CREATE TABLE search_text_chunks (doc_id INTEGER NOT NULL, category TEXT NOT NULL, field INTEGER NOT NULL, chunk INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(doc_id,category,field,chunk)) WITHOUT ROWID;
CREATE TABLE search_terms (term_id INTEGER PRIMARY KEY, kind TEXT NOT NULL, plane INTEGER NOT NULL, n INTEGER NOT NULL, term_key BLOB NOT NULL, posting_count INTEGER NOT NULL, UNIQUE(kind,plane,n,term_key));
CREATE TABLE search_blocks (term_id INTEGER NOT NULL, lower_fence BLOB NOT NULL, posting_count INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(term_id,lower_fence)) WITHOUT ROWID;
CREATE INDEX search_blocks_nonempty ON search_blocks(term_id,lower_fence) WHERE posting_count>0;
CREATE TABLE search_document_terms (doc_id INTEGER NOT NULL, term_id INTEGER NOT NULL, PRIMARY KEY(doc_id,term_id)) WITHOUT ROWID;
"""


class _Writer:
    def __init__(self, connection: sqlite3.Connection, max_mutations: int):
        self.db = connection
        self.maximum = _integer(max_mutations, "max_mutations", 1, 20_000_000)
        self.mutations = 0
        self.blocks_written = 0
        self.payload_bytes_written = 0

    def write(self, sql: str, parameters: tuple = ()) -> sqlite3.Cursor:
        before = self.db.total_changes
        cursor = self.db.execute(sql, parameters)
        self.mutations += max(1, self.db.total_changes - before)
        if self.mutations > self.maximum:
            raise SearchBudgetExceeded("delta mutation budget exceeded; caller must roll back transaction")
        return cursor

    def block(self, term: int, fence: bytes, addresses: list[int]) -> None:
        payload = encode_postings(addresses)
        self.write("INSERT OR REPLACE INTO search_blocks VALUES (?,?,?,?)", (term, fence, len(addresses), payload))
        self.blocks_written += 1
        self.payload_bytes_written += len(payload)

    def keys(self, addresses: list[int]) -> dict[int, bytes]:
        result = {}
        for start in range(0, len(addresses), 100):
            part = addresses[start:start + 100]
            for row in self.db.execute("SELECT doc_id,sort_key FROM search_documents WHERE doc_id IN (" + ",".join("?" for _ in part) + ")", part):
                result[row[0]] = row[1]
        if len(result) != len(addresses):
            raise SearchUnavailable("posting references missing document")
        return result

    def membership(self, term: int, doc_id: int, key: bytes, *, insert: bool) -> None:
        block = self.db.execute("SELECT lower_fence,payload FROM search_blocks WHERE term_id=? AND lower_fence<=? ORDER BY lower_fence DESC LIMIT 1", (term, key)).fetchone()
        if block is None:
            if not insert:
                raise SearchUnavailable("missing deletion block")
            fence, addresses = b"", []
        else:
            fence, payload = block
            addresses = decode_postings(payload)
        if insert:
            if doc_id in addresses:
                raise ValueError("duplicate document membership")
            addresses.append(doc_id)
            keys = self.keys(addresses)
            addresses.sort(key=keys.__getitem__)
            if len(addresses) > BLOCK_SIZE:
                middle = len(addresses) // 2
                self.block(term, fence, addresses[:middle])
                self.block(term, keys[addresses[middle]], addresses[middle:])
            else:
                self.block(term, fence, addresses)
            self.write("INSERT INTO search_document_terms VALUES (?,?)", (doc_id, term))
            self.write("UPDATE search_terms SET posting_count=posting_count+1 WHERE term_id=?", (term,))
        else:
            if doc_id not in addresses:
                raise SearchUnavailable("missing deletion membership")
            addresses.remove(doc_id)
            # Keep empty fences: they are stable directory ranges, not tombstones
            # walked by queries. The next nonempty block uses an indexed seek.
            self.block(term, fence, addresses)
            self.write("DELETE FROM search_document_terms WHERE doc_id=? AND term_id=?", (doc_id, term))
            self.write("UPDATE search_terms SET posting_count=posting_count-1 WHERE term_id=?", (term,))

    def terms(self, document: PreparedSearchDocument) -> set[tuple[int, int, bytes]]:
        result = {(3, 0, b"")}
        for value in document.identities:
            # Queries are capped before lower(), which can expand length.
            # Exact keys longer than 768 codepoints cannot equal a valid query.
            if len(value) <= 768:
                result.add((0, 0, _bytes(value)))
            for n in (1, 2, 3):
                if len(value) >= n:
                    result.add((1, n, _bytes(value[:n])))
        for plane, values in ((2, document.visible), (3, (document.searchable,))):
            for value in values:
                for n in (1, 2, 3):
                    for i in range(len(value) - n + 1):
                        result.add((plane, n, _bytes(value[i:i + n])))
                        if len(result) > MAX_TERMS_PER_DOCUMENT:
                            raise SearchBudgetExceeded("prepared document distinct-term budget exceeded")
        return result

    def prepare(self, document: PreparedSearchDocument) -> tuple[bytes, set[tuple[int, int, bytes]]]:
        if not isinstance(document, PreparedSearchDocument) or document.kind not in ("node", "relation"):
            raise ValueError("expected prepared node/relation")
        _integer(document.doc_id, "doc_id", 1, MAX_ADDRESS)
        key = order_key(document.identifier, document.source_order)
        if len(key) + len(_bytes(_json(document.filters))) + len(_bytes(_json(document.identifier))) > MAX_ROW_BYTES:
            raise SearchBudgetExceeded("document metadata exceeds supported row size")
        if len(_bytes(document.searchable)) > MAX_DOCUMENT_BYTES:
            raise SearchBudgetExceeded("search document exceeds supported size")
        if len(document.identities) + len(document.visible) > MAX_VALUES_PER_DOCUMENT:
            raise SearchBudgetExceeded("prepared document value-count budget exceeded")
        total_bytes = 0
        for value in (document.searchable,) + document.identities + document.visible:
            if not isinstance(value, str) or len(_bytes(value)) > MAX_DOCUMENT_BYTES:
                raise ValueError("invalid prepared search value")
            total_bytes += len(_bytes(value))
        if total_bytes > 4 * MAX_DOCUMENT_BYTES:
            raise SearchBudgetExceeded("prepared document aggregate text budget exceeded")
        return key, self.terms(document)

    def save_values(self, document: PreparedSearchDocument) -> None:
        for category, values in (("identity", document.identities), ("visible", document.visible), ("full", (document.searchable,))):
            for field, value in enumerate(values):
                encoded = _bytes(value)
                self.write("INSERT INTO search_values VALUES (?,?,?,?)", (document.doc_id, category, field, len(encoded)))
                for start in range(0, len(encoded), CHUNK_SIZE):
                    self.write("INSERT INTO search_text_chunks VALUES (?,?,?,?,?)", (document.doc_id, category, field, start // CHUNK_SIZE, encoded[start:start + CHUNK_SIZE]))

    def replace(self, document: PreparedSearchDocument, *, insert: bool) -> None:
        key, new_terms = self.prepare(document)
        same_identity = self.db.execute("SELECT doc_id FROM search_documents WHERE kind=? AND identifier=?", (document.kind, _bytes(_json(document.identifier)))).fetchone()
        if same_identity is not None and same_identity[0] != document.doc_id:
            raise ValueError("duplicate exact source identity")
        old = self.db.execute("SELECT kind,sort_key FROM search_documents WHERE doc_id=?", (document.doc_id,)).fetchone()
        if insert == (old is not None):
            raise ValueError("insert/update existence mismatch")
        old_ids = {row[0] for row in self.db.execute("SELECT term_id FROM search_document_terms WHERE doc_id=?", (document.doc_id,))}
        new_ids = set()
        for plane, n, term_key in sorted(new_terms):
            row = self.db.execute("SELECT term_id FROM search_terms WHERE kind=? AND plane=? AND n=? AND term_key=?", (document.kind, plane, n, term_key)).fetchone()
            if row is None:
                term = self.write("INSERT INTO search_terms(kind,plane,n,term_key,posting_count) VALUES (?,?,?,?,0)", (document.kind, plane, n, term_key)).lastrowid
            else:
                term = row[0]
            new_ids.add(term)
        moved = old is not None and tuple(old) != (document.kind, key)
        for term in sorted(old_ids if moved else old_ids - new_ids):
            self.membership(term, document.doc_id, old[1], insert=False)
        if old is not None:
            self.write("DELETE FROM search_values WHERE doc_id=?", (document.doc_id,))
            self.write("DELETE FROM search_text_chunks WHERE doc_id=?", (document.doc_id,))
            self.write("UPDATE search_documents SET kind=?,identifier=?,sort_key=?,filters=? WHERE doc_id=?", (document.kind, _bytes(_json(document.identifier)), key, _bytes(_json(document.filters)), document.doc_id))
        else:
            self.write("INSERT INTO search_documents VALUES (?,?,?,?,?)", (document.doc_id, document.kind, _bytes(_json(document.identifier)), key, _bytes(_json(document.filters))))
        self.save_values(document)
        for term in sorted(new_ids if moved else new_ids - old_ids):
            self.membership(term, document.doc_id, key, insert=True)

    def delete(self, doc_id: int) -> None:
        old = self.db.execute("SELECT sort_key FROM search_documents WHERE doc_id=?", (doc_id,)).fetchone()
        if old is None:
            raise ValueError("delete targets missing document")
        terms = [row[0] for row in self.db.execute("SELECT term_id FROM search_document_terms WHERE doc_id=?", (doc_id,))]
        for term in terms:
            self.membership(term, doc_id, old[0], insert=False)
        for table in ("search_values", "search_text_chunks", "search_documents"):
            self.write(f"DELETE FROM {table} WHERE doc_id=?", (doc_id,))

    def report(self) -> dict[str, int]:
        return {"mutations": self.mutations, "blocks_written": self.blocks_written, "payload_bytes_written": self.payload_bytes_written}


class SearchStore:
    """Snapshot-selected reader and offline atomic publisher.

    New addresses must exceed the stored high-water mark. Updates keep their
    address. Equal-lower-ID order tokens come from the producer's true source
    order, not from doc_id; group retokenization is an explicit bounded delta.
    """

    @staticmethod
    def _header(binding: dict[str, Any]) -> str:
        if not isinstance(binding, dict) or not binding:
            raise ValueError("an explicit nonempty owner snapshot binding is required")
        header = _json({"schema": SCHEMA, "storage_version": STORAGE_VERSION, "algorithm": ALGORITHM, "unicode_version": unicodedata.unidata_version, "snapshot": binding})
        if len(_bytes(header)) > MAX_HEADER_BYTES:
            raise ValueError("complete framed search header exceeds 65536 bytes")
        return header

    @_typed_errors
    def __init__(self, path: str | Path, *, binding: dict[str, Any]):
        self.path = Path(path)
        self.header = self._header(binding)
        db = self._connect()
        try:
            self.generation = self._check(db)
        finally:
            db.close()

    def _connect(self) -> sqlite3.Connection:
        db = sqlite3.connect(self.path.resolve().as_uri() + "?mode=ro", uri=True)
        db.execute("PRAGMA query_only=ON")
        return db

    @staticmethod
    def _check_header(db: sqlite3.Connection, header: str, generation: bytes | None = None, work: dict | None = None) -> bytes:
        lengths = db.execute("SELECT length(CAST(header AS BLOB)),length(cursor_key) FROM search_header WHERE singleton=1").fetchone()
        if lengths is None or type(lengths[0]) is not int or lengths[0] > MAX_HEADER_BYTES or lengths[1] != 32:
            raise SearchUnavailable("invalid or oversized stored search header")
        row = db.execute("SELECT header,cursor_key FROM search_header WHERE singleton=1").fetchone()
        if work is not None:
            work["metadata_probes"] += 1
            work["metadata_rows"] += 1
            work["metadata_bytes"] += sum(lengths)
        if row is None or not isinstance(row[1], bytes):
            raise SearchUnavailable("invalid stored search incarnation")
        if row[0] != header or (generation is not None and row[1] != generation):
            raise SearchStaleBinding("search store snapshot/algorithm binding mismatch; restart query")
        return row[1]

    def _check(self, db: sqlite3.Connection, work: dict | None = None) -> bytes:
        return self._check_header(db, self.header, getattr(self, "generation", None), work)

    @classmethod
    @_typed_errors
    def initialize_transaction(cls, connection: sqlite3.Connection, *, binding: dict[str, Any], documents: Iterable[PreparedSearchDocument], max_mutations: int = 2_000_000, max_bytes: int = 64 * 1024 * 1024) -> dict[str, int]:
        """Create search tables inside the owner's open transaction.

        max_bytes caps the WHOLE main database. No transaction lifecycle is
        managed here; after any failure the owner must abort its publication.
        """
        _require_transaction(connection)
        header = cls._header(binding)
        _integer(max_bytes, "max_bytes", 65536, 2**40)
        writer = _Writer(connection, max_mutations)
        page_size = connection.execute("PRAGMA page_size").fetchone()[0]
        max_pages = _page_cap(connection, max_bytes // page_size)
        # executescript commits a pending transaction, even for plain DDL.
        for statement in DDL.split(";"):
            if statement.strip():
                connection.execute(statement)
        high_water = 0
        for document in documents:
            writer.replace(document, insert=True)
            high_water = max(high_water, document.doc_id)
        writer.write("INSERT INTO search_header VALUES (1,?,?,?,?)", (header, os.urandom(32), high_water, max_pages))
        result = writer.report()
        result["database_bytes"] = connection.execute("PRAGMA page_count").fetchone()[0] * page_size
        return result

    @classmethod
    @_typed_errors
    def publish_initial(cls, path: str | Path, *, binding: dict[str, Any], documents: Iterable[PreparedSearchDocument], max_mutations: int = 2_000_000, max_bytes: int = 64 * 1024 * 1024) -> dict[str, int]:
        path = Path(path)
        cls._header(binding)
        _integer(max_bytes, "max_bytes", 65536, 2**40)
        descriptor = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        os.close(descriptor)
        db = sqlite3.connect(path)
        try:
            db.execute("BEGIN IMMEDIATE")
            result = cls.initialize_transaction(db, binding=binding, documents=documents, max_mutations=max_mutations, max_bytes=max_bytes)
            db.commit()
            return result
        except BaseException:
            db.rollback()
            db.close()
            path.unlink()  # Only the new file exclusively created above.
            raise
        finally:
            db.close()

    @classmethod
    @_typed_errors
    def apply_delta_transaction(cls, connection: sqlite3.Connection, *, expected_binding: dict[str, Any], new_binding: dict[str, Any], changes: Iterable[SearchChange], max_mutations: int = 100_000) -> dict[str, int]:
        """Apply a delta without beginning, ending or closing owner work."""
        _require_transaction(connection)
        expected, new = cls._header(expected_binding), cls._header(new_binding)
        if expected == new:
            raise SearchInvalidRequest("delta requires a new snapshot binding")
        cls._check_header(connection, expected)
        row = connection.execute("SELECT high_water,max_pages FROM search_header WHERE singleton=1").fetchone()
        if row is None or type(row[0]) is not int or not 0 <= row[0] <= MAX_ADDRESS or type(row[1]) is not int or row[1] < 1:
            raise SearchUnavailable("invalid stored search allocation/page cap")
        high_water = row[0]
        _page_cap(connection, row[1])
        writer = _Writer(connection, max_mutations)
        seen = set()
        for change in changes:
            if not isinstance(change, SearchChange):
                raise SearchInvalidRequest("expected typed SearchChange")
            address = _integer(change.doc_id, "doc_id", 1, MAX_ADDRESS)
            if address in seen:
                raise SearchInvalidRequest("duplicate delta target")
            seen.add(address)
            if change.operation == "delete" and change.document is None:
                writer.delete(address)
            elif change.operation in ("insert", "update") and isinstance(change.document, PreparedSearchDocument) and change.document.doc_id == address:
                if change.operation == "insert":
                    if address <= high_water:
                        raise SearchInvalidRequest("insert would reuse/nonmonotonically allocate an address")
                    high_water = address
                writer.replace(change.document, insert=change.operation == "insert")
            else:
                raise SearchInvalidRequest("invalid typed search change")
        writer.write("UPDATE search_header SET header=?,high_water=?,cursor_key=? WHERE singleton=1", (new, high_water, os.urandom(32)))
        return writer.report()

    @classmethod
    @_typed_errors
    def apply_delta(cls, path: str | Path, *, expected_binding: dict[str, Any], new_binding: dict[str, Any], changes: Iterable[SearchChange], max_mutations: int = 100_000) -> dict[str, int]:
        db = sqlite3.connect(Path(path).resolve().as_uri() + "?mode=rw", uri=True)
        try:
            db.execute("BEGIN IMMEDIATE")
            result = cls.apply_delta_transaction(db, expected_binding=expected_binding, new_binding=new_binding, changes=changes, max_mutations=max_mutations)
            db.commit()
            return result
        except BaseException:
            db.rollback()
            raise
        finally:
            db.close()

    @staticmethod
    def _term(db: sqlite3.Connection, kind: str, phase: int, needle: str) -> int | None:
        if not needle:
            n, keys = 0, [b""]
        elif phase == 0:
            n, keys = 0, [_bytes(needle)]
        elif phase == 1:
            n = min(3, len(needle))
            keys = [_bytes(needle[:n])]
        else:
            n = min(3, len(needle))
            keys = sorted({_bytes(needle[i:i + n]) for i in range(len(needle) - n + 1)})
        best = None
        for key in keys:
            row = db.execute("SELECT term_id,posting_count FROM search_terms WHERE kind=? AND plane=? AND n=? AND term_key=?", (kind, phase, n, key)).fetchone()
            if row is None or not row[1]:
                return None
            candidate = (row[1], key, row[0])
            if best is None or candidate < best:
                best = candidate
        return best[2]

    @staticmethod
    def _candidates(db: sqlite3.Connection, term: int, after_id: int, kind: str, work: dict, max_bytes: int, position: dict):
        """Yield addresses only; None means the metadata budget needs a page.

        The cursor predecessor is an exact member of this immutable term.
        Skip its block prefix by address, without loading earlier metadata.
        Fence bytes stay in SQL; the caller supplies each consumed row's key.
        """
        if after_id:
            probe = db.execute("SELECT length(CAST(sort_key AS BLOB)) FROM search_documents WHERE doc_id=? AND kind=?", (after_id, kind)).fetchone()
            work["metadata_probes"] += 1
            if probe is None or type(probe[0]) is not int or probe[0] > MAX_ROW_BYTES:
                raise SearchUnavailable("invalid cursor predecessor metadata")
            row = db.execute("SELECT sort_key FROM search_documents WHERE doc_id=?", (after_id,)).fetchone()
            if not isinstance(row[0], bytes):
                raise SearchUnavailable("cursor predecessor sort key is not a byte key")
            position["last_key"] = row[0]
            work["metadata_bytes"] += probe[0]
            work["metadata_rows"] += 1
            suffix, args, order = "lower_fence<=?", (term, row[0]), "DESC"
        else:
            suffix, args, order = "1", (term,), "ASC"
        first = True
        while True:
            base = f"FROM search_blocks INDEXED BY search_blocks_nonempty WHERE term_id=? AND {suffix} AND posting_count>0 ORDER BY lower_fence {order} LIMIT 1"
            probe = db.execute("SELECT length(payload) " + base, args).fetchone()
            work["directory_probes"] += 1
            if probe is None:
                if first:
                    raise SearchUnavailable("selected term or cursor predecessor has no posting block")
                return
            if type(probe[0]) is not int or not 1 <= probe[0] <= MAX_BLOCK_BYTES:
                raise SearchUnavailable("invalid posting block size")
            if work["metadata_bytes"] + probe[0] > max_bytes:
                yield None
                return
            payload = db.execute("SELECT payload " + base, args).fetchone()[0]
            work["metadata_bytes"] += len(payload)
            work["blocks_decoded"] += 1
            addresses = decode_postings(payload)
            work["posting_entries_read"] += len(addresses)
            if first and after_id:
                if after_id not in addresses:
                    raise SearchUnavailable("cursor predecessor is not a member of its posting block")
                addresses = addresses[addresses.index(after_id) + 1:]
            first = False
            for address in addresses:
                yield address
            suffix, args, order = "lower_fence>?", (term, position["last_key"]), "ASC"

    @staticmethod
    def _read(db: sqlite3.Connection, doc_id: int, category: str, field: int, start: int, length: int) -> bytes:
        result = bytearray()
        while length:
            size = min(length, CHUNK_SIZE - start % CHUNK_SIZE)
            row = db.execute("SELECT substr(payload,?,?) FROM search_text_chunks WHERE doc_id=? AND category=? AND field=? AND chunk=?", (start % CHUNK_SIZE + 1, size, doc_id, category, field, start // CHUNK_SIZE)).fetchone()
            if row is None or len(row[0]) != size:
                raise SearchUnavailable("missing/truncated search text chunk")
            result.extend(row[0])
            start += size
            length -= size
        return bytes(result)

    @classmethod
    def _verify(cls, db: sqlite3.Connection, address: int, needle: bytes, state: dict, work: dict, max_work: int, max_bytes: int) -> bool | None:
        """Return full-text match, or None with resumable rank/text progress."""
        if state["stage"] == "matched":
            return True
        while work["operations"] < max_work:
            category, field, offset = state["stage"], state["field"], state["offset"]
            row = db.execute("SELECT field,byte_length FROM search_values WHERE doc_id=? AND category=? AND field>=? ORDER BY field LIMIT 1", (address, category, field)).fetchone()
            work["operations"] += 1
            if row is None:
                if category == "identity":
                    state.update(stage="visible" if state["rank"] > 1 else "full", field=0, offset=0)
                elif category == "visible":
                    state.update(stage="full", field=0, offset=0)
                else:
                    return False
                continue
            field, length = row
            remaining = max_bytes - work["verification_bytes"]
            if category == "identity":
                size = min(length, len(needle))
                if size > remaining:
                    return None
                prefix = cls._read(db, address, category, field, 0, size)
                work["verification_bytes"] += size
                if prefix == needle:
                    state["rank"] = min(state["rank"], 0 if length == len(needle) else 1)
                state.update(field=field + 1, offset=0)
                continue
            if not length:
                state.update(field=field + 1, offset=0)
                continue
            overlap = min(offset, len(needle) - 1)
            if remaining <= overlap:
                return None
            size = min(CHUNK_SIZE, length - offset, remaining - overlap)
            payload = cls._read(db, address, category, field, offset - overlap, size + overlap)
            work["verification_bytes"] += len(payload)
            if needle in payload:
                if category == "full":
                    return True
                state.update(rank=2, stage="full", field=0, offset=0)
                continue
            offset += size
            state.update(field=field + 1 if offset == length else field, offset=0 if offset == length else offset)
        return None

    @_typed_errors
    def query_page(self, *, kind: str, query: str = "", filters: dict[str, list[Any]] | None = None, page_size: int = 50, cursor: dict | None = None, candidate_budget: int = 256, verification_bytes: int = 65536, max_metadata_bytes: int = MIN_METADATA_BYTES, max_response_bytes: int = 4 * 1024 * 1024) -> dict:
        db = self._connect()
        try:
            db.execute("BEGIN")
            return self._query(db, header=self.header, generation=self.generation, kind=kind, query=query, filters=filters, page_size=page_size, cursor=cursor, candidate_budget=candidate_budget, verification_bytes=verification_bytes, max_metadata_bytes=max_metadata_bytes, max_response_bytes=max_response_bytes)
        finally:
            db.close()

    @classmethod
    @_typed_errors
    def query_transaction(cls, connection: sqlite3.Connection, *, binding: dict[str, Any], kind: str, query: str = "", filters: dict[str, list[Any]] | None = None, page_size: int = 50, cursor: dict | None = None, candidate_budget: int = 256, verification_bytes: int = 65536, max_metadata_bytes: int = MIN_METADATA_BYTES, max_response_bytes: int = 4 * 1024 * 1024) -> dict:
        """Read the checked search header and matches in the caller snapshot."""
        _require_transaction(connection)
        return cls._query(connection, header=cls._header(binding), kind=kind, query=query, filters=filters, page_size=page_size, cursor=cursor, candidate_budget=candidate_budget, verification_bytes=verification_bytes, max_metadata_bytes=max_metadata_bytes, max_response_bytes=max_response_bytes)

    @classmethod
    def _query(cls, db: sqlite3.Connection, *, header: str, generation: bytes | None = None, kind: str, query: str = "", filters: dict[str, list[Any]] | None = None, page_size: int = 50, cursor: dict | None = None, candidate_budget: int = 256, verification_bytes: int = 65536, max_metadata_bytes: int = MIN_METADATA_BYTES, max_response_bytes: int = 4 * 1024 * 1024) -> dict:
        if kind not in ("node", "relation"):
            raise ValueError("kind must be node or relation")
        normalized = str(query).strip()
        if len(normalized) > 256:
            raise ValueError("knowledge search query exceeds 256 characters")
        needle = normalized.lower()
        _integer(page_size, "page_size", 1, 100)
        _integer(candidate_budget, "candidate_budget", 2, 4096)
        _integer(verification_bytes, "verification_bytes", 8192, 8 * 1024 * 1024)
        _integer(max_metadata_bytes, "max_metadata_bytes", MIN_METADATA_BYTES, 64 * 1024 * 1024)
        _integer(max_response_bytes, "max_response_bytes", MIN_RESPONSE_BYTES, 16 * 1024 * 1024)
        filters = {} if filters is None else filters
        allowed = {"source_graph", "kind_id", "type_id"} if kind == "node" else {"source_graph", "predicate_id", "relation_type_id"}
        if not isinstance(filters, dict) or set(filters) - allowed or any(not isinstance(v, list) or len(v) > 100 for v in filters.values()):
            raise ValueError("invalid per-kind filters")
        query_frame = _bytes(_json([kind, needle, filters]))
        if len(query_frame) > MAX_QUERY_BYTES:
            raise ValueError("complete framed search query/filter exceeds 65536 bytes")
        query_hash = hashlib.sha256(_bytes(header) + b"\0" + query_frame).hexdigest()
        work = {"candidates": 0, "operations": 0, "verification_bytes": 0, "blocks_decoded": 0, "posting_entries_read": 0, "metadata_bytes": 0, "metadata_rows": 0, "metadata_probes": 0, "directory_probes": 0, "response_bytes": 0}
        cursor_key = cls._check_header(db, header, generation, work)
        if cursor is None:
            state = {"query": query_hash, "phase": 0 if needle else 3, "after": 0, "partial": None, "expires": int(time.time()) + 900}
        else:
            if not isinstance(cursor, dict) or set(cursor) != {"state", "mac"} or not isinstance(cursor["state"], dict) or not isinstance(cursor["mac"], str):
                raise SearchCursorError("invalid search cursor")
            try:
                encoded = _bytes(_json(cursor["state"]))
            except (ValueError, TypeError, OverflowError) as exc:
                raise SearchCursorError("invalid search cursor serialization") from exc
            if len(encoded) > 4096 or len(cursor["mac"]) != 64 or any(c not in "0123456789abcdef" for c in cursor["mac"]) or not hmac.compare_digest(hmac.new(cursor_key, encoded, hashlib.sha256).hexdigest(), cursor["mac"]):
                raise SearchCursorError("invalid search cursor integrity")
            state = json.loads(encoded.decode("utf-8", errors="surrogatepass"))
            if set(state) != {"query", "phase", "after", "partial", "expires"}:
                raise SearchCursorError("invalid search cursor state")
            _cursor_integer(state["phase"], "cursor phase", 0 if needle else 3, 3)
            _cursor_integer(state["after"], "cursor after", 0, MAX_ADDRESS)
            _cursor_integer(state["expires"], "cursor expiry", 1, MAX_ADDRESS)
            if state["partial"] is not None:
                partial = state["partial"]
                if not isinstance(partial, dict) or set(partial) != {"doc_id", "stage", "field", "offset", "rank"} or partial["stage"] not in ("identity", "visible", "full", "matched"):
                    raise SearchCursorError("invalid partial search cursor")
                _cursor_integer(partial["doc_id"], "cursor doc_id", 1, MAX_ADDRESS)
                _cursor_integer(partial["field"], "cursor field", 0, MAX_DOCUMENT_BYTES)
                _cursor_integer(partial["offset"], "cursor offset", 0, MAX_DOCUMENT_BYTES)
                _cursor_integer(partial["rank"], "cursor rank", 0, 3)
            if state["query"] != query_hash:
                raise SearchCursorError("search cursor query/snapshot mismatch")
            if state["expires"] <= time.time():
                raise SearchCursorExpired("search cursor expired")
        matches = []
        match_bytes = 0
        while state["phase"] < 4 and len(matches) < page_size and work["operations"] < candidate_budget:
            phase = state["phase"]
            term = cls._term(db, kind, phase, needle)
            if term is None:
                state.update(phase=phase + 1, after=0, partial=None)
                continue
            exhausted = True
            position = {}
            for address in cls._candidates(db, term, state["after"], kind, work, max_metadata_bytes, position):
                exhausted = False
                if address is None or work["operations"] >= candidate_budget:
                    break
                lengths = db.execute("SELECT length(CAST(identifier AS BLOB)),length(CAST(sort_key AS BLOB)),length(CAST(filters AS BLOB)) FROM search_documents WHERE doc_id=? AND kind=?", (address, kind)).fetchone()
                work["metadata_probes"] += 1
                work["operations"] += 1
                work["candidates"] += 1
                if lengths is None or any(type(value) is not int or value < 0 for value in lengths) or sum(lengths) > MAX_ROW_BYTES:
                    raise SearchUnavailable("posting references missing or oversized document metadata")
                if work["metadata_bytes"] + sum(lengths) > max_metadata_bytes:
                    break
                identifier, key, raw_filters = db.execute("SELECT identifier,sort_key,filters FROM search_documents WHERE doc_id=?", (address,)).fetchone()
                if any(not isinstance(value, bytes) for value in (identifier, key, raw_filters)):
                    raise SearchUnavailable("document metadata does not use byte framing")
                work["metadata_bytes"] += sum(lengths)
                work["metadata_rows"] += 1
                position["last_key"] = key
                # Literal false/zero/null remain JSON values, never coerced.
                try:
                    doc_filters = json.loads(raw_filters)
                    if not isinstance(doc_filters, dict):
                        raise ValueError("filters must be an object")
                except (ValueError, UnicodeError) as exc:
                    raise SearchUnavailable("invalid stored document filters") from exc
                if any(values and not any(type(doc_filters.get(field)) is type(v) and doc_filters.get(field) == v for v in values) for field, values in filters.items()):
                    state.update(after=address, partial=None)
                else:
                    partial = state["partial"] or {"doc_id": address, "stage": "identity", "field": 0, "offset": 0, "rank": 3}
                    if partial["doc_id"] != address:
                        raise SearchUnavailable("cursor document no longer matches posting")
                    found = True if not needle else cls._verify(db, address, _bytes(needle), partial, work, candidate_budget, verification_bytes)
                    if found is None:
                        state["partial"] = partial
                        break
                    if found and partial["rank"] == phase:
                        try:
                            exact_id = json.loads(identifier)
                        except (ValueError, UnicodeError) as exc:
                            raise SearchUnavailable("invalid stored document identity") from exc
                        match = {"doc_id": address, "id": exact_id, "rank": phase}
                        size = len(_bytes(_json(match))) + 2
                        if match_bytes + size + RESPONSE_OVERHEAD_BYTES > max_response_bytes:
                            partial.update(stage="matched", field=0, offset=0)
                            state["partial"] = partial
                            break
                        matches.append(match)
                        match_bytes += size
                    state.update(after=address, partial=None)
                if len(matches) >= page_size or work["operations"] >= candidate_budget:
                    break
                exhausted = True
            if exhausted:
                state.update(phase=phase + 1, after=0, partial=None)
            else:
                break
        has_more = state["phase"] < 4
        next_cursor = None
        if has_more:
            encoded = _bytes(_json(state))
            next_cursor = {"state": state, "mac": hmac.new(cursor_key, encoded, hashlib.sha256).hexdigest()}
        result = {"schema": SCHEMA, "matches": matches, "returned_count": len(matches), "total_matching": len(matches) if cursor is None and not has_more else None, "has_more": has_more, "next_cursor": next_cursor, "work": work}
        for _ in range(4):
            size = len(_bytes(_json(result)))
            if size == work["response_bytes"]:
                break
            work["response_bytes"] = size
        if work["response_bytes"] > max_response_bytes:
            raise SearchBudgetExceeded("search response framing exceeded its reserved overhead")
        return result

    @_typed_errors
    def storage_stats(self) -> dict[str, Any]:
        """Physical SQLite pages, including dictionaries, fences and reverse index."""
        db = self._connect()
        try:
            db.execute("BEGIN")
            self._check(db)
            result = {"database_bytes": db.execute("PRAGMA page_count").fetchone()[0] * db.execute("PRAGMA page_size").fetchone()[0], "file_bytes": self.path.stat().st_size}
            result["tables"] = {name: {"rows": db.execute(f"SELECT count(*) FROM {name}").fetchone()[0]} for name in ("search_documents", "search_terms", "search_blocks", "search_document_terms", "search_values", "search_text_chunks")}
            try:
                result["sqlite_objects_bytes"] = dict(db.execute("SELECT name,sum(pgsize) FROM dbstat GROUP BY name"))
            except sqlite3.OperationalError:
                result["sqlite_objects_bytes"] = None
            return result
        finally:
            db.close()
