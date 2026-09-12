"""Explicit empty-store bulk loading; scratch is never a publication.

The caller owns the main transaction. A separate, exclusively created scratch
database retains bounded compressed term tails, not one row per membership.
Neither this module nor its failure path commits/rolls back the main database.
"""
from __future__ import annotations

from collections import OrderedDict
from array import array
from dataclasses import dataclass
import hashlib
import hmac
import os
from pathlib import Path
import sqlite3
import time

from . import compressed_search_store as search


@dataclass(frozen=True)
class BulkBootstrapLimits:
    """Explicit scratch page/write admission plus bounded Python work buffers."""

    max_bytes: int
    max_mutations: int
    max_cached_terms: int = 8192
    max_cached_bytes: int = 8 * 1024 * 1024
    batch_size: int = 1024
    max_cached_tails: int = 8192
    max_tail_bytes: int = 8 * 1024 * 1024

    def validate(self):
        for name, low, high in (("max_bytes", 65536, 2**40),
                                ("max_mutations", 1, search.MAX_ADDRESS),
                                ("max_cached_terms", 1, 8192),
                                ("max_cached_bytes", 1, 8 * 1024 * 1024),
                                ("batch_size", 1, 1024),
                                ("max_cached_tails", 1, 8192),
                                ("max_tail_bytes", 1, 8 * 1024 * 1024)):
            search._integer(getattr(self, name), name, low, high)


class _BulkWriter(search._Writer):
    def __init__(self, db, maximum, limits):
        super().__init__(db, maximum, mode="bootstrap")
        self.limits = limits
        self.scratch_mutations = 0
        self.cache = OrderedDict()
        self.cached_bytes = self.peak_cached_bytes = self.peak_cached_terms = 0
        self.dictionary_hits = self.dictionary_misses = 0
        self.write_calls = 0
        self.reverse_memberships = 0

    def _admit(self, count):
        if self.mutations + self.scratch_mutations + count > self.maximum:
            raise search.SearchBudgetExceeded("bulk main/scratch mutation budget exceeded; caller must abort publication")

    def write(self, sql, parameters=()):
        self._admit(1)
        self.write_calls += 1
        result = super().write(sql, parameters)
        self._admit(0)
        return result

    def term(self, kind, plane, n, key):
        token = (kind, plane, n, key)
        cached = self.cache.pop(token, None)
        if cached is not None:
            self.dictionary_hits += 1
            self.cache[token] = cached
            return cached[0]
        self.dictionary_misses += 1
        found = self.db.execute("SELECT term_id FROM search_terms WHERE kind=? AND plane=? AND n=? AND term_key=?", token).fetchone()
        term = (found[0] if found is not None else self.write(
            "INSERT INTO search_terms(kind,plane,n,term_key,posting_count) VALUES (?,?,?,?,0)", token).lastrowid)
        size = 192 + len(key)  # Accounted payload, not an RSS estimate.
        if size <= self.limits.max_cached_bytes:
            while self.cache and (len(self.cache) >= self.limits.max_cached_terms
                                  or self.cached_bytes + size > self.limits.max_cached_bytes):
                _, (_, previous) = self.cache.popitem(last=False)
                self.cached_bytes -= previous
            self.cache[token] = (term, size)
            self.cached_bytes += size
            self.peak_cached_bytes = max(self.peak_cached_bytes, self.cached_bytes)
            self.peak_cached_terms = max(self.peak_cached_terms, len(self.cache))
        return term

    def insert_document(self, document):
        key, terms = self.prepare(document)
        # The existing PK/UNIQUE constraints own duplicate address, exact
        # identity and within-kind sort-key rejection, without old-row scans.
        self.write("INSERT INTO search_documents VALUES (?,?,?,?,?)", (
            document.doc_id, document.kind, search._bytes(search._json(document.identifier)),
            key, search._bytes(search._json(document.filters))))
        self.save_values(document)
        ids = {self.term(document.kind, plane, n, term_key) for plane, n, term_key in sorted(terms)}
        if len(ids) != len(terms):
            raise search.SearchUnavailable("dictionary aliases distinct document terms")
        self.save_reverse(document.doc_id, document.kind, ids, insert=True)
        self.reverse_memberships += len(ids)


def _ordered(db, sql, parameters=()):
    """Refuse an unplanned sorter before opening the potentially large scan."""
    plan = [row[3] for row in db.execute("EXPLAIN QUERY PLAN " + sql, parameters)]
    if any("TEMP B-TREE" in detail.upper() for detail in plan):
        raise search.SearchUnavailable("bulk ordering requires an existing index, not a temporary sort")
    return db.execute(sql, parameters)


@dataclass
class _Tail:
    total: int
    last_rank: int
    addresses: array

    @property
    def size(self):
        # Accounted retained numeric payload and fixed frame, not an RSS bound.
        return 256 + 8 * len(self.addresses)


def _tail_digest(term, total, rank, payload):
    return hashlib.sha256(b"tos-search-bulk-tail-v1\0" + term.to_bytes(8, "big")
                          + total.to_bytes(8, "big") + rank.to_bytes(8, "big") + payload).digest()


class _Scratch:
    def __init__(self, path, limits, writer):
        self.path = Path(path).absolute()
        self.limits, self.writer = limits, writer
        self.db = None
        self.created = None
        self.peak_bytes = self.write_calls = 0
        self.cache = OrderedDict()
        self.cached_bytes = self.peak_cached_bytes = self.peak_cached_tails = 0
        self.hits = self.misses = self.evictions = self.read_calls = 0

    def __enter__(self):
        # No mkdir, overwrite, path adoption, ATTACH or process-global temp route.
        if self.path.parent.resolve() != self.path.parent:
            raise search.SearchInvalidRequest("bulk scratch parent must not traverse symlinks")
        # SQLite must not encounter or recover somebody else's adjacent journal
        # before our private journal_mode is set on the new empty database.
        for suffix in ("-journal", "-wal", "-shm"):
            adjacent = self.path.with_name(self.path.name + suffix)
            if os.path.lexists(adjacent):
                raise FileExistsError(f"bulk scratch sidecar already exists: {adjacent}")
        fd = os.open(self.path, os.O_CREAT | os.O_EXCL | os.O_WRONLY | getattr(os, "O_NOFOLLOW", 0), 0o600)
        self.created = os.fstat(fd)
        os.close(fd)
        try:
            self.db = sqlite3.connect(self.path, isolation_level=None)
            self._identity()
            # Only this disposable private database has no rollback journal.
            # It is never recovered, admitted, committed as truth or reused.
            self.db.execute("PRAGMA journal_mode=OFF")
            self.db.execute("PRAGMA cache_size=-2048")
            self.db.execute("PRAGMA mmap_size=0")
            self.page_size = self.db.execute("PRAGMA page_size").fetchone()[0]
            search._page_cap(self.db, self.limits.max_bytes // self.page_size)
            self.db.execute("BEGIN")
            self.db.execute("CREATE TABLE tails (term_id INTEGER PRIMARY KEY, total INTEGER NOT NULL, last_rank INTEGER NOT NULL, payload BLOB NOT NULL, digest BLOB NOT NULL)")
            self.measure()
            return self
        except BaseException:
            self.__exit__(None, None, None)
            raise

    def _identity(self):
        current = self.path.lstat()
        if (current.st_dev, current.st_ino) != (self.created.st_dev, self.created.st_ino):
            raise search.SearchUnavailable("exclusive bulk scratch inode changed")

    def measure(self):
        size = self.db.execute("PRAGMA page_count").fetchone()[0] * self.page_size
        self.peak_bytes = max(self.peak_bytes, size)
        if size > self.limits.max_bytes:
            raise search.SearchBudgetExceeded("bulk scratch page budget exceeded")

    def _save(self, term, tail):
        self.writer._admit(1)
        if self.writer.scratch_mutations + 1 > self.limits.max_mutations:
            raise search.SearchBudgetExceeded("bulk scratch mutation budget exceeded")
        payload = search.encode_postings(tail.addresses)
        digest = _tail_digest(term, tail.total, tail.last_rank, payload)
        before = self.db.total_changes
        self.write_calls += 1
        self.db.execute("INSERT INTO tails VALUES (?,?,?,?,?) ON CONFLICT(term_id) DO UPDATE SET "
                        "total=excluded.total,last_rank=excluded.last_rank,payload=excluded.payload,digest=excluded.digest",
                        (term, tail.total, tail.last_rank, payload, digest))
        changed = self.db.total_changes - before
        self.writer.scratch_mutations += changed
        if changed != 1:
            raise search.SearchUnavailable("unexpected bulk scratch write count")
        # max_page_count enforces every allocation. Page count never shrinks in
        # this private transaction; bounded checkpoints avoid a PRAGMA per miss.
        if self.write_calls % self.limits.batch_size == 0:
            self.measure()

    def _load(self, term, *, required=False):
        self.read_calls += 1
        row = self.db.execute("SELECT total,last_rank,length(payload),typeof(payload),length(digest),typeof(digest) "
                              "FROM tails WHERE term_id=?", (term,)).fetchone()
        if row is None:
            if required:
                raise search.SearchUnavailable("missing staged term tail")
            return _Tail(0, 0, array("Q"))
        total, rank, size, payload_type, digest_size, digest_type = row
        if (type(total) is not int or not 1 <= total <= search.MAX_ADDRESS
                or type(rank) is not int or not total <= rank <= search.MAX_ADDRESS
                or payload_type != "blob" or not 0 <= size <= search.MAX_BLOCK_BYTES
                or digest_type != "blob" or digest_size != 32):
            raise search.SearchUnavailable("invalid or oversized staged term tail")
        self.read_calls += 1
        payload, digest = self.db.execute("SELECT payload,digest FROM tails WHERE term_id=?", (term,)).fetchone()
        if not hmac.compare_digest(digest, _tail_digest(term, total, rank, payload)):
            raise search.SearchUnavailable("staged term tail digest mismatch")
        addresses = search.decode_postings(payload)
        if len(addresses) != total % search.BLOCK_SIZE or search.encode_postings(addresses) != payload:
            raise search.SearchUnavailable("noncanonical staged tail or count mismatch")
        return _Tail(total, rank, array("Q", addresses))

    def append(self, term, rank, doc_id):
        tail = self.cache.pop(term, None)
        if tail is None:
            self.misses += 1
            tail = self._load(term)
        else:
            self.hits += 1
            self.cached_bytes -= tail.size
        if rank <= tail.last_rank:
            raise search.SearchUnavailable("bulk document traversal is not strictly monotonic")
        tail.last_rank = rank
        tail.total += 1
        tail.addresses.append(doc_id)
        if len(tail.addresses) == search.BLOCK_SIZE:
            self._block(term, tail)
            tail.addresses = array("Q")
        if tail.size > self.limits.max_tail_bytes:
            self._save(term, tail)
            return
        while self.cache and (len(self.cache) >= self.limits.max_cached_tails
                              or self.cached_bytes + tail.size > self.limits.max_tail_bytes):
            previous_term, previous = self.cache.popitem(last=False)
            self.cached_bytes -= previous.size
            self.evictions += 1
            self._save(previous_term, previous)
        self.cache[term] = tail
        self.cached_bytes += tail.size
        self.peak_cached_bytes = max(self.peak_cached_bytes, self.cached_bytes)
        self.peak_cached_tails = max(self.peak_cached_tails, len(self.cache))

    def _block(self, term, tail):
        fence = (b"" if tail.total <= search.BLOCK_SIZE else
                 self.writer.keys([tail.addresses[0]])[tail.addresses[0]])
        self.writer.block(term, fence, tail.addresses)

    def flush(self):
        while self.cache:
            term, tail = self.cache.popitem(last=False)
            self.cached_bytes -= tail.size
            self._save(term, tail)
        self.measure()

    def __exit__(self, *unused):
        if self.db is not None:
            self.db.close()
            self.db = None
        if self.created is not None:
            # Never delete a replacement inode, an existing file, or siblings.
            try:
                self._identity()
            except FileNotFoundError:
                return
            self.path.unlink()


def _stage(writer, scratch):
    count = memberships = 0
    documents = _ordered(writer.db, "SELECT doc_id,kind FROM search_documents ORDER BY kind,sort_key")
    for count, (doc_id, kind) in enumerate(documents, 1):
        search._integer(count, "temporary document rank", 1, search.MAX_ADDRESS)
        terms = search._reverse_terms(writer.db, doc_id, kind, validate_terms=False)
        for term in terms:
            scratch.append(term, count, doc_id)
        memberships += len(terms)
    if memberships != writer.reverse_memberships:
        raise search.SearchUnavailable("bulk staged membership count differs")
    scratch.flush()
    return count


def _pack(writer, scratch):
    terms = memberships = 0
    for (term,) in _ordered(scratch.db, "SELECT term_id FROM tails ORDER BY term_id"):
        tail = scratch._load(term, required=True)
        if tail.addresses:
            scratch._block(term, tail)
        if writer.write("UPDATE search_terms SET posting_count=? WHERE term_id=?", (tail.total, term)).rowcount != 1:
            raise search.SearchUnavailable("staged tail references missing dictionary term")
        terms += 1
        memberships += tail.total
    if (memberships != writer.reverse_memberships
            or terms != writer.db.execute("SELECT count(*) FROM search_terms").fetchone()[0]):
        raise search.SearchUnavailable("packed term/membership totals differ from fresh producer")


@search._typed_errors
def initialize_bulk_transaction(connection, *, binding, documents, scratch_path,
                                scratch_limits: BulkBootstrapLimits,
                                max_mutations=2_000_000, max_bytes=64 * 1024 * 1024):
    """Explicit alternative to initialize_transaction; never an automatic retry.

    max_mutations charges main AND scratch DML. max_bytes caps only the main
    database; scratch_limits independently cap scratch pages and writes. After
    failure the owner must abort the publication. Its transaction is not managed.
    The exclusive scratch file is removed on both success and failure.
    """
    search._require_transaction(connection)
    header = search.SearchStore._header(binding)
    search._integer(max_bytes, "max_bytes", 65536, 2**40)
    if not isinstance(scratch_limits, BulkBootstrapLimits):
        raise search.SearchInvalidRequest("explicit BulkBootstrapLimits required")
    scratch_limits.validate()
    writer = _BulkWriter(connection, max_mutations, scratch_limits)
    # Reject main-path aliases/existing scratch before modifying its page cap
    # or schema. Opening scratch also establishes its independent hard cap.
    started = time.perf_counter()
    with _Scratch(scratch_path, scratch_limits, writer) as scratch:
        page_size = connection.execute("PRAGMA page_size").fetchone()[0]
        max_pages = search._page_cap(connection, max_bytes // page_size)
        for statement in search.DDL.split(";"):
            if statement.strip():
                connection.execute(statement)
        high_water = count = 0
        for document in documents:
            writer.insert_document(document)
            count += 1
            high_water = max(high_water, document.doc_id)
        loaded = time.perf_counter()
        writer.cache.clear()
        writer.cached_bytes = 0
        if _stage(writer, scratch) != count:
            raise search.SearchUnavailable("bulk document staging count differs")
        staged = time.perf_counter()
        _pack(writer, scratch)
        packed = time.perf_counter()
        writer.write("INSERT INTO search_header VALUES (1,?,?,?,?)", (header, os.urandom(32), high_water, max_pages))
        result = writer.report()
        result.update({"main_mutations": writer.mutations, "scratch_mutations": writer.scratch_mutations,
                       "mutations": writer.mutations + writer.scratch_mutations,
                       "main_write_calls": writer.write_calls, "scratch_write_calls": scratch.write_calls,
                       "documents": count, "high_water": high_water,
                       "database_bytes": connection.execute("PRAGMA page_count").fetchone()[0] * page_size,
                       "scratch_peak_bytes": scratch.peak_bytes,
                       "dictionary_hits": writer.dictionary_hits, "dictionary_misses": writer.dictionary_misses,
                       "dictionary_peak_entries": writer.peak_cached_terms, "dictionary_peak_bytes": writer.peak_cached_bytes,
                       "reverse_memberships": writer.reverse_memberships,
                       "tail_hits": scratch.hits, "tail_misses": scratch.misses,
                       "tail_evictions": scratch.evictions, "scratch_read_calls": scratch.read_calls,
                       "tail_peak_entries": scratch.peak_cached_tails, "tail_peak_bytes": scratch.peak_cached_bytes,
                       "stage_seconds": {"load": loaded - started, "stage": staged - loaded,
                                         "pack": packed - staged, "total": time.perf_counter() - started}})
    return result
