"""Private, disk-backed build intermediates. Never constructed by a query.

Each iterable decodes one JSON row at a time. SQLite owns ordering, keyed
lookups, and grouping; Python does not retain a full collection or task DAG.
These are explicit internal collection types, not a JSON-schema relaxation.
"""
from __future__ import annotations

import hashlib
import json
import sqlite3
import weakref
from collections.abc import MutableMapping, Sequence


def compact(value):
    # Collection membership is Python-value membership, so object insertion
    # order must not make two otherwise equal rows appear different.
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(',', ':'), allow_nan=False)


def json_chunks(value):
    """Canonical JSON chunks, including explicit disk collections, bounded by a row.

    Matching json.dumps(sort_keys=True,separators=(',',':'),ensure_ascii=False).
    It is intentionally independent of JSONEncoder's list-subclass fast path.
    """
    if not isinstance(value, (DiskMap, DiskSequence)):
        # Ordinary records already fit their input-record boundary. Let the
        # C JSON encoder handle their nested scalars as one chunk; walking
        # every scalar in Python makes repeated content digests needlessly
        # expensive. A container with disk collections falls through without
        # converting those collections to lists or dictionaries.
        try:
            encoded = compact(value)
        except TypeError:
            pass
        else:
            yield encoded
            return
    if isinstance(value, (dict, DiskMap)):
        yield '{'
        keys = value.sorted_keys() if isinstance(value, DiskMap) else sorted(value)
        for index, key in enumerate(keys):
            if index:
                yield ','
            yield compact(key)
            yield ':'
            yield from json_chunks(value[key])
        yield '}'
    elif isinstance(value, (list, tuple, DiskSequence)):
        yield '['
        for index, item in enumerate(value):
            if index:
                yield ','
            yield from json_chunks(item)
        yield ']'
    else:
        yield compact(value)


def canonical_digest(value):
    digest = hashlib.sha256()
    for chunk in json_chunks(value):
        digest.update(chunk.encode('utf-8'))
    return digest.hexdigest()


def _release_collection(owner_ref, collection):
    """Release one collection after its top-level view becomes unreachable."""
    owner = owner_ref()
    if owner is not None:
        try:
            owner._release_collection(collection)
        except sqlite3.ProgrammingError as error:
            # Callers normally close the owner before its connection.  A
            # direct sqlite3.Connection.close() is still possible in a test
            # or embedding process; a late finalizer must not report an
            # unraisable error or issue another query against that connection.
            if 'closed' not in str(error).lower():
                raise
            owner._closed = True


class DiskCollections:
    """A build-owned SQLite connection and uniquely named scratch collections."""
    def __init__(self, connection):
        self.connection = connection
        self.serial = 0
        self._closed = False
        connection.executescript('''
            CREATE TABLE IF NOT EXISTS _build_collections (
                collection INTEGER PRIMARY KEY AUTOINCREMENT);
            CREATE TABLE IF NOT EXISTS _build_rows (
                collection INTEGER NOT NULL, position INTEGER NOT NULL,
                sort_key TEXT, payload TEXT NOT NULL,
                PRIMARY KEY(collection,position));
            CREATE INDEX IF NOT EXISTS _build_rows_order ON _build_rows(collection,sort_key,position);
            CREATE TABLE IF NOT EXISTS _build_map (
                collection INTEGER NOT NULL, key TEXT NOT NULL, payload TEXT NOT NULL,
                PRIMARY KEY(collection,key));
            CREATE TABLE IF NOT EXISTS _build_groups (
                collection INTEGER NOT NULL, key TEXT NOT NULL, position INTEGER NOT NULL,
                payload TEXT NOT NULL, PRIMARY KEY(collection,key,position));
            CREATE INDEX IF NOT EXISTS _build_groups_value ON _build_groups(collection,key,payload);
        ''')

    def _register(self, view, collection):
        # The callback keeps only a weak owner reference. A bound owner method
        # would keep this build alive through weakref.finalize's registry.
        return weakref.finalize(view, _release_collection, weakref.ref(self), collection)

    def _name(self):
        # Allocate from the connection so a second build owner reusing this
        # connection cannot silently address the first owner's rows.
        cursor = self.connection.execute('INSERT INTO _build_collections DEFAULT VALUES')
        self.serial = cursor.lastrowid
        return self.serial

    def sequence(self, values=()):
        result = DiskSequence(self, self._name())
        result.extend(values)
        return result

    def mapping(self, pairs=()):
        result = DiskMap(self, self._name())
        for key, value in pairs:
            result[key] = value
        return result

    def groups(self):
        return DiskGroups(self, self._name())

    def counts(self, values):
        result = self.mapping()
        for value in values:
            result[value] = result.get(value, 0) + 1
        return result

    def _release_collection(self, collection):
        if self._closed:
            return
        for name in ('_build_rows', '_build_map', '_build_groups'):
            self.connection.execute('DELETE FROM ' + name + ' WHERE collection=?', (collection,))
        self.connection.execute('DELETE FROM _build_collections WHERE collection=?', (collection,))

    def close(self):
        """Stop finalizers before the owning SQLite connection is closed."""
        self._closed = True

    def drop(self):
        if self._closed:
            return
        # Mark closed before dropping shared tables. A view can remain alive
        # after this terminal owner operation; its finalizer must not race the
        # DROP statements or issue SQL after the connection is closed.
        self._closed = True
        for name in ('_build_rows', '_build_map', '_build_groups', '_build_collections'):
            self.connection.execute('DROP TABLE ' + name)


class _DiskCollection:
    def __init__(self, owner, collection):
        self.owner, self.collection = owner, collection
        self.connection = owner.connection
        self._finalizer = owner._register(self, collection)


class DiskSequence(_DiskCollection, Sequence):
    """Repeatable ordered JSON rows, with no implicit full-list conversion."""
    _knowledge_rows = True

    def __init__(self, owner, collection):
        super().__init__(owner, collection)
        self.count = 0
        self.sorted = False

    def append(self, value):
        self.connection.execute('INSERT INTO _build_rows VALUES (?,?,NULL,?)',
                                (self.collection, self.count, compact(value)))
        self.count += 1

    def extend(self, values):
        for value in values:
            self.append(value)

    def __iter__(self):
        # A row appended after sort has no key yet; list semantics place it
        # after the sorted prefix until the caller explicitly sorts again.
        order = 'sort_key IS NULL,sort_key,position' if self.sorted else 'position'
        for (raw,) in self.connection.execute(
                'SELECT payload FROM _build_rows WHERE collection=? ORDER BY ' + order,
                (self.collection,)):
            yield json.loads(raw)

    def __len__(self):
        return self.count

    def __getitem__(self, index):
        if isinstance(index, slice):
            start, stop, step = index.indices(self.count)
            return [self[i] for i in range(start, stop, step)]
        if index < 0:
            index += self.count
        if index < 0 or index >= self.count:
            raise IndexError(index)
        if not self.sorted:
            query, args = ('SELECT payload FROM _build_rows WHERE collection=? AND position=?',
                           (self.collection, index))
        else:
            query, args = ('SELECT payload FROM _build_rows WHERE collection=? ORDER BY sort_key IS NULL,sort_key,position LIMIT 1 OFFSET ?',
                           (self.collection, index))
        return json.loads(self.connection.execute(query, args).fetchone()[0])

    def __contains__(self, value):
        return self.connection.execute(
            'SELECT 1 FROM _build_rows WHERE collection=? AND payload=? LIMIT 1',
            (self.collection, compact(value))).fetchone() is not None

    def sort(self, *, key=None, keyfield=None):
        # Build callers use strings or tuples of strings, not locale collation.
        # JSON arrays preserve that tuple order; no integer ordering is assumed.
        if keyfield is not None:
            key = lambda value: value[keyfield]
        if key is None:
            key = lambda value: value
        # Stage computed keys in SQLite before updating the source table. This
        # avoids mutating a table while its read cursor is still producing
        # rows, without retaining the whole sequence in Python memory.
        temp_name = f'_build_sort_{self.collection}'
        self.connection.execute(f'CREATE TEMP TABLE "{temp_name}" '
                                '(position INTEGER PRIMARY KEY, sort_key TEXT NOT NULL)')
        try:
            for position, raw in self.connection.execute(
                    'SELECT position,payload FROM _build_rows WHERE collection=? ORDER BY position',
                    (self.collection,)):
                value = key(json.loads(raw))
                if not (isinstance(value, str) or isinstance(value, (tuple, list))
                        and all(isinstance(v, str) for v in value)):
                    raise TypeError('disk sequence ordering requires string fields')
                # UTF-8 BINARY follows Unicode codepoint order. Framing each string
                # with a NUL delimiter keeps prefix ordering correct (JSON does not).
                parts = [value] if isinstance(value, str) else value
                if any('\0' in part for part in parts):
                    raise ValueError('NUL in disk ordering key')
                sort_key = '\0'.join(parts) + '\0'
                self.connection.execute(f'INSERT INTO "{temp_name}" VALUES (?,?)',
                                        (position, sort_key))
            self.connection.execute(
                f'UPDATE _build_rows SET sort_key=(SELECT sort_key FROM "{temp_name}" '
                'WHERE position=_build_rows.position) WHERE collection=?',
                (self.collection,))
        finally:
            self.connection.execute(f'DROP TABLE "{temp_name}"')
        self.sorted = True


class DiskMap(_DiskCollection, MutableMapping):
    def __init__(self, owner, collection):
        super().__init__(owner, collection)

    def __getitem__(self, key):
        row = self.connection.execute('SELECT payload FROM _build_map WHERE collection=? AND key=?',
                                      (self.collection, compact(key))).fetchone()
        if row is None:
            raise KeyError(key)
        return json.loads(row[0])

    def __setitem__(self, key, value):
        self.connection.execute('INSERT INTO _build_map VALUES (?,?,?) ON CONFLICT(collection,key) DO UPDATE SET payload=excluded.payload',
                                (self.collection, compact(key), compact(value)))

    def __delitem__(self, key):
        cursor = self.connection.execute('DELETE FROM _build_map WHERE collection=? AND key=?',
                                         (self.collection, compact(key)))
        if not cursor.rowcount:
            raise KeyError(key)

    def __iter__(self):
        for (key,) in self.connection.execute('SELECT key FROM _build_map WHERE collection=? ORDER BY rowid', (self.collection,)):
            yield json.loads(key)

    def sorted_keys(self):
        # Only string-keyed maps form JSON objects. json_extract restores exact
        # Unicode lexical ordering instead of comparing escaped JSON spellings.
        for (key,) in self.connection.execute('SELECT key FROM _build_map WHERE collection=? ORDER BY json_extract(key,\'$\') COLLATE BINARY', (self.collection,)):
            decoded = json.loads(key)
            if not isinstance(decoded, str):
                raise TypeError('JSON object keys must be strings')
            yield decoded

    def __len__(self):
        return self.connection.execute('SELECT count(*) FROM _build_map WHERE collection=?', (self.collection,)).fetchone()[0]

    def __contains__(self, key):
        return self.connection.execute('SELECT 1 FROM _build_map WHERE collection=? AND key=?',
                                       (self.collection, compact(key))).fetchone() is not None

    def items(self):
        for key, raw in self.connection.execute('SELECT key,payload FROM _build_map WHERE collection=? ORDER BY rowid', (self.collection,)):
            yield json.loads(key), json.loads(raw)

    def values(self):
        for (raw,) in self.connection.execute('SELECT payload FROM _build_map WHERE collection=? ORDER BY rowid', (self.collection,)):
            yield json.loads(raw)


class DiskGroups(_DiskCollection):
    def __init__(self, owner, collection):
        super().__init__(owner, collection)

    def __getitem__(self, key):
        return GroupRows(self, key)

    def get(self, key, default=None):
        rows = self[key]
        return rows if len(rows) else default

    def setdefault(self, key, default):
        return self[key]

    def items(self):
        for (key,) in self.connection.execute('SELECT DISTINCT key FROM _build_groups WHERE collection=? ORDER BY key', (self.collection,)):
            decoded = json.loads(key)
            yield decoded, self[decoded]


class GroupRows(Sequence):
    _knowledge_rows = True

    def __init__(self, groups, key):
        self.groups, self.key = groups, compact(key)
        self.connection = groups.connection

    def append(self, value):
        self.connection.execute(
            'INSERT INTO _build_groups(collection,key,position,payload) VALUES '
            '(?,?,coalesce((SELECT max(position)+1 FROM _build_groups '
            'WHERE collection=? AND key=?),0),?)',
            (self.groups.collection, self.key, self.groups.collection, self.key, compact(value)))

    def update(self, values):
        for value in values:
            if value not in self:
                self.append(value)

    def __iter__(self):
        for (raw,) in self.connection.execute('SELECT payload FROM _build_groups WHERE collection=? AND key=? ORDER BY position',
                                             (self.groups.collection, self.key)):
            yield json.loads(raw)

    def __len__(self):
        return self.connection.execute('SELECT count(*) FROM _build_groups WHERE collection=? AND key=?',
                                       (self.groups.collection, self.key)).fetchone()[0]

    def __contains__(self, value):
        return self.connection.execute('SELECT 1 FROM _build_groups WHERE collection=? AND key=? AND payload=? LIMIT 1',
                                       (self.groups.collection, self.key, compact(value))).fetchone() is not None

    def __getitem__(self, index):
        if isinstance(index, slice):
            return list(self)[index]
        if index < 0:
            index += len(self)
        row = self.connection.execute('SELECT payload FROM _build_groups WHERE collection=? AND key=? ORDER BY position LIMIT 1 OFFSET ?',
                                      (self.groups.collection, self.key, index)).fetchone()
        if row is None:
            raise IndexError(index)
        return json.loads(row[0])
