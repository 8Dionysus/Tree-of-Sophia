"""Offline exact catalog index in a caller-owned SQLite transaction.

Never opens a transaction, commits, rolls back, publishes prepared metadata or
builds a missing index at request time. On ANY failure the caller must roll back
its entire transaction. Stable (kind,id), row digests and owner sequence numbers
bind selected contributors; physical doc integers are internal only.
"""
from __future__ import annotations

import hashlib
import json
import sqlite3
import zlib
from collections import Counter, OrderedDict
from dataclasses import dataclass
from pathlib import Path

from . import knowledge as k
from .catalog_semantics import (CatalogChange, CatalogInputs, CatalogRow, PROJECTOR_VERSION,
    CANONICAL_ORDER, catalog_digest, encoded, finalized_header, order_key, order_value,
    render_catalog, route_counts, row_facts)

SCHEMA = 'tos-catalog-index-v1'


class CatalogIndexError(ValueError):
    pass


class CatalogIndexBudgetError(CatalogIndexError):
    pass


@dataclass(frozen=True)
class CatalogLimits:
    max_changes: int = 4096
    max_incident_relations: int = 16384
    max_row_bytes: int = 8 * 1024 * 1024
    max_delta_bytes: int = 64 * 1024 * 1024
    max_catalog_entries: int = 100000
    max_aggregate_bytes: int = 64 * 1024 * 1024
    max_catalog_bytes: int = 16 * 1024 * 1024
    max_index_bytes: int = 4 * 1024 * 1024 * 1024

    def __post_init__(self):
        if any(type(value) is not int or value <= 0 for value in vars(self).values()):
            raise ValueError('catalog limits must be positive integers')


def catalog_projector_digest():
    """Conservative executable binding; unrelated owner-code drift also refuses."""
    here = Path(__file__)
    digest = hashlib.sha256(PROJECTOR_VERSION.encode())
    for path in (here, here.with_name('catalog_semantics.py'), here.with_name('knowledge.py')):
        digest.update(path.read_bytes())
    return digest.hexdigest()


_DDL = (
    'CREATE TABLE catalog_state (singleton INTEGER PRIMARY KEY CHECK(singleton=1), schema TEXT NOT NULL, '
    'projector TEXT NOT NULL, binding TEXT NOT NULL, header_digest TEXT NOT NULL, catalog_digest TEXT NOT NULL)',
    'CREATE TABLE catalog_atoms (atom INTEGER PRIMARY KEY, value TEXT NOT NULL UNIQUE)',
    'CREATE TABLE catalog_totals (key INTEGER PRIMARY KEY, n INTEGER NOT NULL CHECK(n>0))',
    'CREATE TABLE catalog_contributors (doc INTEGER PRIMARY KEY, kind TEXT NOT NULL, id TEXT NOT NULL, '
    'source_order BLOB NOT NULL, row_digest TEXT NOT NULL, facts_digest TEXT NOT NULL, facts BLOB NOT NULL, '
    'summary TEXT NOT NULL, from_id TEXT, to_id TEXT, seal TEXT NOT NULL, UNIQUE(kind,id), UNIQUE(kind,source_order))',
    "CREATE INDEX catalog_from ON catalog_contributors(from_id) WHERE kind='relation'",
    "CREATE INDEX catalog_to ON catalog_contributors(to_id) WHERE kind='relation'",
    'CREATE TABLE catalog_occurrences (bucket INTEGER NOT NULL, value INTEGER NOT NULL, doc INTEGER NOT NULL, '
    'source_order BLOB NOT NULL, position INTEGER NOT NULL, PRIMARY KEY(bucket,value,doc)) WITHOUT ROWID',
    'CREATE INDEX catalog_occurrence_doc ON catalog_occurrences(doc)',
    'CREATE INDEX catalog_occurrence_first ON catalog_occurrences(bucket,value,source_order,position)',
    'CREATE TABLE catalog_heads (bucket INTEGER NOT NULL, value INTEGER NOT NULL, doc INTEGER NOT NULL, '
    'source_order BLOB NOT NULL, position INTEGER NOT NULL, PRIMARY KEY(bucket,value)) WITHOUT ROWID',
    'CREATE INDEX catalog_head_first ON catalog_heads(bucket,source_order,position)',
)


class CatalogIndex:
    def __init__(self, connection: sqlite3.Connection, limits: CatalogLimits | None = None):
        self.connection = connection
        self.limits = limits or CatalogLimits()
        self._atoms = OrderedDict()
        self._spent = 0
        self._failed = False

    def _transaction(self):
        if self._failed:
            raise CatalogIndexError('catalog index instance failed; roll back and create a new instance')
        if not self.connection.in_transaction:
            raise CatalogIndexError('catalog operation requires an existing caller-owned transaction')

    def _budget(self, amount):
        self._spent += amount
        if self._spent > self.limits.max_delta_bytes:
            raise CatalogIndexBudgetError('catalog selected contributor bytes exceeded')

    def _size(self):
        pages = self.connection.execute('PRAGMA page_count').fetchone()[0]
        page_size = self.connection.execute('PRAGMA page_size').fetchone()[0]
        if pages * page_size > self.limits.max_index_bytes:
            raise CatalogIndexBudgetError('catalog database page budget exceeded')

    def _storage_limit(self):
        self._size()
        page_size = self.connection.execute('PRAGMA page_size').fetchone()[0]
        allowed = self.limits.max_index_bytes // page_size
        existing = self.connection.execute('PRAGMA max_page_count').fetchone()[0]
        # Shared prepared/search owners may already impose a stricter ceiling.
        # Never increase it; SQLite enforces this ceiling before allocating a
        # page, unlike a post-write page_count observation alone.
        ceiling = min(existing, allowed)
        if ceiling < 1:
            raise CatalogIndexBudgetError('catalog database page budget is below one page')
        actual = self.connection.execute(f'PRAGMA max_page_count={ceiling}').fetchone()[0]
        if actual > ceiling:
            raise CatalogIndexBudgetError('catalog cannot enforce the requested database page ceiling')

    def _atom(self, value):
        if value in self._atoms:
            self._atoms.move_to_end(value)
            return self._atoms[value]
        self.connection.execute('INSERT INTO catalog_atoms(value) VALUES(?) ON CONFLICT(value) DO NOTHING', (value,))
        atom = self.connection.execute('SELECT atom FROM catalog_atoms WHERE value=?', (value,)).fetchone()[0]
        self._atoms[value] = atom
        if len(self._atoms) > 4096:
            self._atoms.popitem(last=False)
        return atom

    def _state(self, inputs):
        self._transaction()
        try:
            row = self.connection.execute('SELECT schema,projector,binding,header_digest,catalog_digest '
                'FROM catalog_state WHERE singleton=1 AND length(schema)<128 AND length(projector)=64 '
                'AND length(binding)=64 AND length(header_digest)=64 AND length(catalog_digest)=64').fetchone()
        except sqlite3.Error as error:
            raise CatalogIndexError('catalog index is absent or incompatible; explicit bootstrap required') from error
        if row is None or row[:3] != (SCHEMA, catalog_projector_digest(), inputs.binding):
            raise CatalogIndexError('catalog schema, projector or owner-input binding drift')
        expected = {statement.split()[2]: statement for statement in _DDL}
        placeholders = ','.join('?' for _ in expected)
        actual = dict(self.connection.execute(f'SELECT name,sql FROM sqlite_master WHERE name IN ({placeholders})',
                                             tuple(expected)))
        if actual != expected:
            raise CatalogIndexError('catalog physical schema or index definition drift')
        return row

    def _endpoint(self, identifier):
        row = self.connection.execute('SELECT source_order,row_digest,facts_digest,summary,seal '
            'FROM catalog_contributors WHERE kind=? AND id=? AND length(source_order)<=? '
            'AND length(row_digest)=64 AND length(facts_digest)=64 AND length(seal)=64 '
            'AND length(CAST(summary AS BLOB))<=?',
            ('node', identifier, self.limits.max_row_bytes, self.limits.max_row_bytes)).fetchone()
        if row is None:
            raise CatalogIndexError('catalog relation endpoint is absent: ' + str(identifier))
        if row[4] != self._seal('node', identifier, *row[:4]):
            raise CatalogIndexError('catalog endpoint summary binding mismatch')
        return json.loads(row[3])

    @staticmethod
    def _seal(kind, identifier, order, row_digest, facts_digest, summary):
        return catalog_digest([kind, identifier, order.hex(), row_digest, facts_digest, summary])

    def _facts(self, row, entries):
        if not isinstance(row.item, dict):
            raise CatalogIndexError('catalog replacement item must be an object')
        if self._order_profile == CANONICAL_ORDER:
            if row.source_order != (str(row.item.get('source_graph')), str(row.item.get('id'))):
                raise CatalogIndexError('canonical catalog order must match (source_graph,id) of its row')
        elif type(row.source_order) is not int:
            raise CatalogIndexError('owner-sequence catalog order requires integer positions')
        raw = encoded(row.item).encode()
        if len(raw) > self.limits.max_row_bytes:
            raise CatalogIndexBudgetError('catalog row bytes exceeded')
        counts, posts, summary = row_facts(row, entries)
        return hashlib.sha256(raw).hexdigest(), counts, posts, summary, len(raw)

    def _pack(self, counts, posts):
        raw = encoded({'counts': [[list(key), value] for key, value in sorted(counts.items()) if value],
                       'posts': posts}).encode()
        if len(raw) > self.limits.max_row_bytes * 4:
            raise CatalogIndexBudgetError('catalog contributor decoded bytes exceeded')
        self._budget(len(raw))
        blob = zlib.compress(raw, 6)
        if len(blob) > self.limits.max_row_bytes * 4:
            raise CatalogIndexBudgetError('catalog contributor compressed bytes exceeded')
        return hashlib.sha256(raw).hexdigest(), blob

    def _unpack(self, digest, blob):
        # Independent decoded cap prevents a damaged compressed contributor from
        # bypassing the selected-byte budget before digest verification.
        decoder = zlib.decompressobj()
        raw = decoder.decompress(blob, self.limits.max_row_bytes * 4 + 1)
        if len(raw) > self.limits.max_row_bytes * 4 or not decoder.eof or decoder.unused_data:
            raise CatalogIndexBudgetError('catalog contributor decoded bytes exceeded or invalid stream')
        if hashlib.sha256(raw).hexdigest() != digest:
            raise CatalogIndexError('catalog contributor digest mismatch')
        self._budget(len(raw))
        value = json.loads(raw)
        return Counter({tuple(key): count for key, count in value['counts']}), [
            (tuple(bucket), value, position) for bucket, value, position in value['posts']]

    def _head(self, bucket, value):
        row = self.connection.execute('SELECT doc,source_order,position FROM catalog_occurrences '
            'WHERE bucket=? AND value=? ORDER BY source_order,position LIMIT 1', (bucket, value)).fetchone()
        if row is None:
            self.connection.execute('DELETE FROM catalog_heads WHERE bucket=? AND value=?', (bucket, value))
        else:
            self.connection.execute('INSERT INTO catalog_heads VALUES(?,?,?,?,?) '
                'ON CONFLICT(bucket,value) DO UPDATE SET doc=excluded.doc,source_order=excluded.source_order,position=excluded.position',
                (bucket, value, *row))

    def _contributions(self, doc, order, before, after, before_posts, after_posts, *, old_order=None):
        difference = after.copy()
        difference.subtract(before)
        for key, delta in difference.items():
            if not delta:
                continue
            atom = self._atom(encoded(key))
            current = self.connection.execute('SELECT n FROM catalog_totals WHERE key=?', (atom,)).fetchone()
            total = (current[0] if current else 0) + delta
            if total < 0:
                raise CatalogIndexError('catalog negative aggregate indicates damaged contributor state')
            if total == 0:
                self.connection.execute('DELETE FROM catalog_totals WHERE key=?', (atom,))
            else:
                self.connection.execute('INSERT INTO catalog_totals VALUES(?,?) ON CONFLICT(key) DO UPDATE SET n=excluded.n',
                                        (atom, total))
        old = {(bucket, value): position for bucket, value, position in before_posts}
        new = {(bucket, value): position for bucket, value, position in after_posts}
        for key in old.keys() | new.keys():
            if old.get(key) == new.get(key) and key in old and key in new and old_order == order:
                continue
            bucket, value = self._atom(encoded(key[0])), self._atom(key[1])
            if key not in new:
                self.connection.execute('DELETE FROM catalog_occurrences WHERE bucket=? AND value=? AND doc=?',
                                        (bucket, value, doc))
            else:
                self.connection.execute('INSERT INTO catalog_occurrences VALUES(?,?,?,?,?) '
                    'ON CONFLICT(bucket,value,doc) DO UPDATE SET source_order=excluded.source_order,position=excluded.position',
                    (bucket, value, doc, order, new[key]))
            self._head(bucket, value)

    def _insert(self, row, entries, *, defer_routes=False):
        row_digest, counts, posts, summary, size = self._facts(row, entries)
        self._budget(size)
        if not defer_routes:
            counts.update(route_counts(row.kind, summary, self._endpoint))
        facts_digest, blob = self._pack(counts, posts)
        seal = self._seal(row.kind, row.id, order_key(row.source_order), row_digest, facts_digest, encoded(summary))
        cursor = self.connection.execute('INSERT INTO catalog_contributors '
            '(kind,id,source_order,row_digest,facts_digest,facts,summary,from_id,to_id,seal) VALUES(?,?,?,?,?,?,?,?,?,?)',
            (row.kind, row.id, order_key(row.source_order), row_digest, facts_digest, blob, encoded(summary),
             summary.get('from_id'), summary.get('to_id'), seal))
        self._contributions(cursor.lastrowid, order_key(row.source_order), Counter(), counts, [], posts)
        return cursor.lastrowid

    def _selected(self, kind, identifier):
        record = self.connection.execute('SELECT doc,source_order,row_digest,facts_digest,facts,summary,seal,from_id,to_id '
            'FROM catalog_contributors WHERE kind=? AND id=? AND length(source_order)<=? '
            'AND length(row_digest)=64 AND length(facts_digest)=64 AND length(seal)=64 '
            'AND length(facts)<=? AND length(CAST(summary AS BLOB))<=?',
            (kind, identifier, self.limits.max_row_bytes, self.limits.max_row_bytes * 4,
             self.limits.max_row_bytes)).fetchone()
        if record:
            self._budget(len(record[4]) + len(record[5].encode('utf-8')))
            if record[6] != self._seal(kind, identifier, record[1], record[2], record[3], record[5]):
                raise CatalogIndexError('catalog selected contributor binding mismatch')
            summary = json.loads(record[5])
            if (record[7], record[8]) != (summary.get('from_id'), summary.get('to_id')):
                raise CatalogIndexError('catalog selected contributor adjacency mismatch')
        elif self.connection.execute('SELECT 1 FROM catalog_contributors WHERE kind=? AND id=?',
                                     (kind, identifier)).fetchone():
            raise CatalogIndexError('catalog selected contributor framing or byte bound mismatch')
        return record[:6] if record else None

    def _remove(self, record):
        doc, order, _, digest, blob, _ = record
        counts, posts = self._unpack(digest, blob)
        self._contributions(doc, order, counts, Counter(), posts, [])
        self.connection.execute('DELETE FROM catalog_contributors WHERE doc=?', (doc,))

    def bootstrap(self, inputs: CatalogInputs, rows):
        """Stream nodes before relations; no payload list or graph reconstruction."""
        self._transaction()
        try:
            self._storage_limit()
            self._order_profile = inputs.source_order_profile
            if self.connection.execute("SELECT 1 FROM sqlite_master WHERE name GLOB 'catalog_*' LIMIT 1").fetchone():
                raise CatalogIndexError('catalog bootstrap requires an absent index')
            for statement in _DDL:
                self.connection.execute(statement)
            entries, _, _ = k._entity_registry_indexes(inputs.entity_type_registry)
            relation_phase = False
            for index, row in enumerate(rows):
                if row.kind == 'relation':
                    relation_phase = True
                elif relation_phase:
                    raise CatalogIndexError('bootstrap requires nodes before relations')
                self._spent = 0
                self._insert(row, entries)
                if index % 1024 == 0:
                    self._size()
            result = self._render(inputs)
            self.connection.execute('INSERT INTO catalog_state VALUES(1,?,?,?,?,?)',
                (SCHEMA, catalog_projector_digest(), inputs.binding,
                 catalog_digest(finalized_header(inputs, result)), catalog_digest(result)))
            self._size()
            return result
        except BaseException as error:
            self._failed = True
            if not isinstance(error, Exception):
                raise
            if isinstance(error, CatalogIndexError):
                raise
            if isinstance(error, sqlite3.Error) and getattr(error, 'sqlite_errorcode', None) == sqlite3.SQLITE_FULL:
                raise CatalogIndexBudgetError('catalog shared database page ceiling reached') from error
            raise CatalogIndexError('catalog bootstrap failed: ' + str(error)) from error

    def apply_delta(self, before: CatalogInputs, after: CatalogInputs, changes, *, expected_catalog_digest=None):
        """Subtract/add addressed facts, then recheck the final endpoint closure.

        Required old digests use catalog_digest(full_normalized_row), not a
        content_revision member. Update order may be omitted to retain position;
        inserts require an explicit owner order. Canonical tuple order uses
        exact (str(source_graph),str(id)) and never requires rank renumbering.
        """
        self._transaction()
        try:
            self._storage_limit()
            state = self._state(before)
            self._order_profile = after.source_order_profile
            if before.header_digest != state[3] or after.binding != before.binding:
                raise CatalogIndexError('catalog before header or after owner binding mismatch')
            if expected_catalog_digest is not None and expected_catalog_digest != state[4]:
                raise CatalogIndexError('catalog expected before digest mismatch')
            if catalog_digest(self._render(before)) != state[4]:
                raise CatalogIndexError('catalog before aggregate digest mismatch')
            seen, selected, affected = set(), [], set()
            self._spent = 0
            entries, _, _ = k._entity_registry_indexes(after.entity_type_registry)
            for change in _bounded(changes, self.limits.max_changes, 'catalog changes'):
                key = (change.kind, change.id)
                if (not isinstance(change.id, str) or not change.id or change.kind not in ('node', 'relation')
                        or change.operation not in ('insert', 'update', 'delete') or key in seen):
                    raise CatalogIndexError('duplicate or invalid catalog change')
                seen.add(key)
                record = self._selected(*key)
                if change.operation == 'insert':
                    if record is not None or change.expected_old_digest is not None or change.source_order is None:
                        raise CatalogIndexError('catalog insert requires absent row and explicit owner order')
                elif record is None or record[2] != change.expected_old_digest:
                    raise CatalogIndexError('catalog selected old row digest mismatch')
                if change.operation == 'delete':
                    if change.new_item is not None or change.source_order is not None:
                        raise CatalogIndexError('catalog deletion cannot carry a replacement')
                    facts = None
                else:
                    order = change.source_order if change.source_order is not None else order_value(record[1])
                    row = CatalogRow(change.kind, change.id, order, change.new_item)
                    facts = self._facts(row, entries)
                    self._budget(facts[4])
                    self._budget(len(encoded([list(facts[1].items()), facts[2], facts[3]]).encode()))
                # Keep only the reduced facts, never a second collection of
                # caller full-row payloads produced by a streaming change input.
                metadata = CatalogChange(change.operation, change.kind, change.id, change.expected_old_digest,
                                         source_order=change.source_order)
                selected.append((metadata, record, facts))
                if change.kind == 'node':
                    old_summary = json.loads(record[5]) if record else None
                    new_summary = facts[3] if facts else None
                    old_masks = (old_summary['legacy'], old_summary['typed']) if old_summary else None
                    new_masks = (new_summary['legacy'], new_summary['typed']) if new_summary else None
                    if old_masks != new_masks:
                        for column in ('from_id', 'to_id'):
                            cursor = self.connection.execute("SELECT id FROM catalog_contributors WHERE kind='relation' "
                                f'AND {column}=? LIMIT ?', (change.id, self.limits.max_incident_relations + 1))
                            for (identifier,) in cursor:
                                affected.add(identifier)
                                if len(affected) > self.limits.max_incident_relations:
                                    raise CatalogIndexBudgetError('catalog incident relation closure exceeded')
            # Release ONLY changed/deleted order addresses so a bounded packet
            # may swap owner positions or reuse a deleted position atomically.
            # Internal 00-prefixed addresses cannot collide with admitted I/T
            # keys and are never visible outside this caller-owned transaction.
            for change, record, facts in selected:
                if record and (facts is None or (change.source_order is not None
                                                and order_key(change.source_order) != record[1])):
                    temporary = b'\x00' + record[0].to_bytes(8, 'big')
                    seal = self._seal(change.kind, change.id, temporary, record[2], record[3], record[5])
                    self.connection.execute('UPDATE catalog_contributors SET source_order=?,seal=? WHERE doc=?',
                                            (temporary, seal, record[0]))
            # All node summaries move before any route recomputation. Intrinsic
            # relation facts are replaced without routes until the final pass.
            for change, record, facts in sorted(selected, key=lambda entry: entry[0].kind != 'node'):
                if change.kind == 'relation':
                    affected.add(change.id)
                if facts is None:
                    self._remove(record)
                    continue
                digest, counts, posts, summary, _ = facts
                if change.kind == 'node':
                    counts.update(route_counts('node', summary, self._endpoint))
                order = order_key(change.source_order) if change.source_order is not None else record[1]
                packed_digest, blob = self._pack(counts, posts)
                seal = self._seal(change.kind, change.id, order, digest, packed_digest, encoded(summary))
                if record:
                    old_counts, old_posts = self._unpack(record[3], record[4])
                    self.connection.execute('UPDATE catalog_contributors SET source_order=?,row_digest=?,facts_digest=?,facts=?, '
                        'summary=?,from_id=?,to_id=?,seal=? WHERE doc=?',
                        (order, digest, packed_digest, blob, encoded(summary), summary.get('from_id'), summary.get('to_id'), seal, record[0]))
                    self._contributions(record[0], order, old_counts, counts, old_posts, posts, old_order=record[1])
                else:
                    cursor = self.connection.execute('INSERT INTO catalog_contributors '
                        '(kind,id,source_order,row_digest,facts_digest,facts,summary,from_id,to_id,seal) VALUES(?,?,?,?,?,?,?,?,?,?)',
                        (change.kind, change.id, order, digest, packed_digest, blob, encoded(summary),
                         summary.get('from_id'), summary.get('to_id'), seal))
                    self._contributions(cursor.lastrowid, order, Counter(), counts, [], posts)
            if len(affected) > self.limits.max_incident_relations:
                raise CatalogIndexBudgetError('catalog changed relation closure exceeded')
            for identifier in affected:
                record = self._selected('relation', identifier)
                if record is None:
                    continue
                counts, posts = self._unpack(record[3], record[4])
                summary = json.loads(record[5])
                # Even predicates without route confirmation require both exact
                # final endpoints; dangling rows never disappear silently.
                self._endpoint(summary['from_id']); self._endpoint(summary['to_id'])
                new_counts = Counter({key: value for key, value in counts.items()
                                      if key[1] not in ('route-type', 'route-predicate')})
                new_counts.update(route_counts('relation', summary, self._endpoint))
                if new_counts != counts:
                    digest, blob = self._pack(new_counts, posts)
                    seal = self._seal('relation', identifier, record[1], record[2], digest, record[5])
                    self.connection.execute('UPDATE catalog_contributors SET facts_digest=?,facts=?,seal=? WHERE doc=?',
                                            (digest, blob, seal, record[0]))
                    self._contributions(record[0], record[1], counts, new_counts, [], [])
            result = self._render(after)
            self.connection.execute('UPDATE catalog_state SET header_digest=?,catalog_digest=? WHERE singleton=1',
                                    (catalog_digest(finalized_header(after, result)), catalog_digest(result)))
            self._size()
            return result
        except BaseException as error:
            self._failed = True
            if not isinstance(error, Exception):
                raise
            if isinstance(error, CatalogIndexError):
                raise
            if isinstance(error, sqlite3.Error) and getattr(error, 'sqlite_errorcode', None) == sqlite3.SQLITE_FULL:
                raise CatalogIndexBudgetError('catalog shared database page ceiling reached') from error
            raise CatalogIndexError('catalog delta failed: ' + str(error)) from error

    def _render(self, inputs):
        result = render_catalog(inputs, _SQLView(self), derive_counts=True)
        if len(encoded(result).encode()) > self.limits.max_catalog_bytes:
            raise CatalogIndexBudgetError('catalog output bytes exceeded')
        return result

    def render(self, inputs: CatalogInputs):
        self._transaction()
        try:
            self._size()
            state = self._state(inputs)
            if state[3] != inputs.header_digest:
                raise CatalogIndexError('catalog header digest mismatch')
            result = self._render(inputs)
            if catalog_digest(result) != state[4]:
                raise CatalogIndexError('catalog aggregate digest mismatch')
            return result
        except BaseException as error:
            self._failed = True
            if not isinstance(error, Exception):
                raise
            if isinstance(error, CatalogIndexError):
                raise
            raise CatalogIndexError('catalog render failed: ' + str(error)) from error


def _bounded(values, maximum, label):
    for index, value in enumerate(values):
        if index >= maximum:
            raise CatalogIndexBudgetError(label + ' exceeded')
        yield value


class _SQLView:
    def __init__(self, index):
        self.index = index
        self._decoded = 0
        rows = index.connection.execute('SELECT CASE WHEN length(CAST(a.value AS BLOB))<=? '
            'THEN a.value ELSE NULL END,t.n FROM catalog_totals t '
            'JOIN catalog_atoms a ON a.atom=t.key LIMIT ?',
            (index.limits.max_aggregate_bytes, index.limits.max_catalog_entries + 1))
        self.counts = Counter()
        for key, count in _bounded(rows, index.limits.max_catalog_entries, 'catalog aggregate entries'):
            self._charge(key)
            self.counts[tuple(json.loads(key))] = count

    def _charge(self, value):
        if value is None:
            raise CatalogIndexBudgetError('catalog aggregate atom bytes exceeded')
        self._decoded += len(value.encode('utf-8'))
        if self._decoded > self.index.limits.max_aggregate_bytes:
            raise CatalogIndexBudgetError('catalog aggregate decoded bytes exceeded')

    def first_values(self, bucket, limit=None):
        maximum = self.index.limits.max_catalog_entries if limit is None else limit
        rows = self.index.connection.execute('SELECT CASE WHEN length(CAST(v.value AS BLOB))<=? '
            'THEN v.value ELSE NULL END FROM catalog_heads h '
            'JOIN catalog_atoms b ON b.atom=h.bucket JOIN catalog_atoms v ON v.atom=h.value '
            'WHERE b.value=? ORDER BY h.source_order,h.position LIMIT ?',
            (self.index.limits.max_aggregate_bytes, encoded(bucket), maximum + (1 if limit is None else 0)))
        result = []
        for (value,) in _bounded(rows, maximum, 'catalog ordered values'):
            self._charge(value)
            result.append(json.loads(value))
        return result
