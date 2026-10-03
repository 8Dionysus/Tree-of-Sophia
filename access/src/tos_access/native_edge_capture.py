"""Returning transport for the installed offline Edge capture operation.

The caller owns source/pair admission, its read transactions, catalog/binding
selection and output targets. This bridge only freezes those borrowed views
and runs the existing installed-prefix verifier in a child. Rust owns capture
rules and returns its receipt unchanged. No Python oracle is a fallback.
"""
from __future__ import annotations

import json
import hashlib
import math
import os
from dataclasses import dataclass
from pathlib import Path
import selectors
import shutil
import signal
import sqlite3
import stat
import struct
import subprocess
import sys
import tempfile
import time

from . import native_dispatch

# Two bindings are capped at 1 MiB each by selected_binding; source-input
# JSON is at most 1 MiB before outer-string escaping, and two projection roots
# are at most 256 KiB each. The Rust ABI includes escaped strings and framing.
# Must match the coherent edge_offline_capture request ABI, not older products.
_REQUEST_BYTES = 10 * 1024**2
_SNAPSHOT_ORDER = (
    'd1_database', 'before_prepared_database', 'after_prepared_database',
)
_SNAPSHOT_FIELDS = frozenset(_SNAPSHOT_ORDER)

# File transport only: these IDs identify the existing Rust owner schemas.
# Unowned ordinary tables remain inert evidence; their SQL is never executed.
_D1_TABLES = tuple(enumerate((
    'knowledge_nodes', 'knowledge_relations', 'knowledge_search_documents',
    'knowledge_search_grams', 'knowledge_search_gram_stats', 'knowledge_lens_order',
    'source_navigation_nodes', 'source_navigation_node_payload',
    'source_navigation_edges', 'source_navigation_edge_payload',
    'source_navigation_rights', 'source_navigation_rights_payload', 'edge_meta',
    'knowledge_exploration_clock',
), 1)) + ((0x10, 'knowledge_compact_lens'), (0x11, 'knowledge_compact_lens_state'),
          (0x12, 'knowledge_lens_memberships'), (0x13, 'knowledge_lens_membership_state'))
_PREPARED_TABLES = tuple(enumerate((
    'edge_meta', 'knowledge_exploration_clock', 'knowledge_lens_order',
    'prepared_documents', 'prepared_state', 'knowledge_nodes',
    'knowledge_relations', 'prepared_source_state',
), 0x20))
_FRAME_HEADER = struct.Struct('<8sBBHHIQ')


def _quoted(name):
    return '"' + name.replace('"', '""') + '"'


def _frame_cursor(connection):
    cursor = connection.cursor()
    # Do not modify the caller's connection policy. Expressions below avoid
    # declared-type converters; TEXT is fetched as raw BLOB bytes.
    cursor.row_factory = None
    return cursor


def _wire_string(raw, width=2, nullable=False):
    if raw is None:
        if not nullable:
            raise ValueError('native Edge schema string is absent')
        return ((1 << (8 * width)) - 1).to_bytes(width, 'little')
    if type(raw) is not bytes:
        raise ValueError('native Edge schema string is not raw bytes')
    raw.decode('utf-8', 'strict')
    maximum = (1 << (8 * width)) - 1 - int(nullable)
    if len(raw) > maximum:
        raise ValueError('native Edge schema string exceeds frame width')
    return len(raw).to_bytes(width, 'little') + raw


class _SchemaAllocation:
    """Cumulative conservative logical allocation, never an RSS estimate."""
    def __init__(self, frame_bytes, maximum=None):
        self.maximum = frame_bytes * 128 if maximum is None else maximum
        if type(self.maximum) is not int or self.maximum <= 0:
            raise ValueError('native Edge schema allocation requires a positive finite cap')
        self.encoding = 'utf-8'
        self.reserved = 0

    def reserve(self, size):
        if type(size) is not int or size < 0 or size > self.maximum - self.reserved:
            raise ValueError('native Edge cumulative schema allocation exceeds transport envelope')
        self.reserved += size


def _schema_rows(cursor, query, preflight, params, remaining, deadline, allocation):
    # COUNT/length runs before any metadata tuple/bytes copies. All query text
    # is owner-authored. SQL VM cancellation remains the caller/outer holder's
    # responsibility; these checks are cooperative before/after each query.
    _active(deadline)
    cursor.execute(preflight, params)
    count, raw_bytes = cursor.fetchone()
    _active(deadline)
    if (type(count) is not int or type(raw_bytes) is not int or count < 0
            or raw_bytes < 0 or raw_bytes * 3 + count * 16 > remaining):
        raise ValueError('native Edge schema metadata exceeds frame envelope')
    allocation.reserve(count * 512 + raw_bytes * 24)
    cursor.execute(query, params)
    rows = []
    used = 0
    while True:
        _active(deadline)
        row = cursor.fetchone()
        if row is None:
            break
        row = tuple(v.decode(allocation.encoding, 'strict').encode('utf-8')
                    if type(v) is bytes else v for v in row)
        used += 64 + sum(len(v) if type(v) is bytes else 16 for v in row)
        if used > remaining:
            raise ValueError('native Edge schema state exceeds selected transport budget')
        rows.append(row)
        if len(rows) > count:
            raise ValueError('native Edge schema metadata count changed')
    if len(rows) != count:
        raise ValueError('native Edge schema metadata count changed')
    _active(deadline)
    return rows


def _logical_plan(connection, role, remaining, deadline, *, max_schema_allocation_bytes=None):
    registry = _D1_TABLES if role == 1 else _PREPARED_TABLES
    allocation = _SchemaAllocation(remaining, max_schema_allocation_bytes)
    cursor = _frame_cursor(connection)
    try:
        _active(deadline)
        cursor.execute('SELECT CAST(encoding AS BLOB) FROM pragma_encoding')
        encoded_encoding = cursor.fetchone()[0]
        choices = {b'UTF-8': ('UTF-8', 'utf-8', 1),
                   'UTF-16le'.encode('utf-16le'): ('UTF-16le', 'utf-16le', 2),
                   'UTF-16be'.encode('utf-16be'): ('UTF-16be', 'utf-16be', 3)}
        if encoded_encoding not in choices:
            raise ValueError('native Edge database encoding is unsupported')
        database_encoding, allocation.encoding, encoding_tag = choices[encoded_encoding]
        _active(deadline)
        cursor.execute("SELECT count(*),coalesce(sum(length(CAST(type AS BLOB))+"
                       "length(CAST(name AS BLOB))+length(CAST(tbl_name AS BLOB))+"
                       "coalesce(length(CAST(sql AS BLOB)),0)),0) FROM main.sqlite_schema")
        count, size = cursor.fetchone()
        if (type(count) is not int or type(size) is not int or count < 0
                or size < 0 or size * 3 + count * 64 > remaining):
            raise ValueError('native Edge schema exceeds selected frame budget')
        objects = _schema_rows(cursor,
            "SELECT CAST(type AS BLOB),CAST(name AS BLOB),CAST(tbl_name AS BLOB),"
            "CAST(sql AS BLOB) FROM main.sqlite_schema ORDER BY CAST(type AS BLOB),"
            "CAST(name AS BLOB)",
            "SELECT count(*),coalesce(sum(length(CAST(type AS BLOB))+length(CAST(name AS BLOB))+"
            "length(CAST(tbl_name AS BLOB))+coalesce(length(CAST(sql AS BLOB)),0)),0) FROM main.sqlite_schema",
            (), remaining, deadline, allocation)
        allocation.reserve(count * 32)
        objects.sort(key=lambda row: (row[0], row[1]))
        if len(objects) != count:
            raise ValueError('native Edge schema inventory changed')
        table_list = _schema_rows(cursor,
            "SELECT CAST(name AS BLOB),CAST(type AS BLOB),CAST(ncol AS INTEGER),"
            "CAST(wr AS INTEGER),CAST(strict AS INTEGER) "
            "FROM pragma_table_list WHERE schema='main' AND name!='sqlite_schema'",
            "SELECT count(*),coalesce(sum(length(CAST(name AS BLOB))+length(CAST(type AS BLOB))),0) "
            "FROM pragma_table_list WHERE schema='main' AND name!='sqlite_schema'",
            (), remaining, deadline, allocation)
        tables = {}
        for raw_name, kind, ncol, wr, strict in table_list:
            if kind == b'view':
                continue  # inert schema evidence only; never query its body
            if kind != b'table' or wr not in (0, 1) or strict not in (0, 1):
                raise ValueError('native Edge logical transport requires ordinary tables')
            name = raw_name.decode('utf-8', 'strict')
            if name in tables or ncol < 0 or ncol > 65535 or ncol * 64 > remaining:
                raise ValueError('native Edge table descriptor exceeds frame budget')
            # Declaration/default text occurs within the already bounded
            # sqlite_schema SQL. Descriptor copies and containers are separate
            # transient state, accounted in the source cost inventory.
            columns = _schema_rows(cursor,
                "SELECT CAST(cid AS INTEGER),CAST(name AS BLOB),CAST(type AS BLOB),"
                "CAST([notnull] AS INTEGER),CAST(pk AS INTEGER),CAST(hidden AS INTEGER),"
                "CAST(dflt_value AS BLOB) FROM pragma_table_xinfo(?) ORDER BY cid",
                "SELECT count(*),coalesce(sum(length(CAST(name AS BLOB))+length(CAST(type AS BLOB))+"
                "coalesce(length(CAST(dflt_value AS BLOB)),0)),0) FROM pragma_table_xinfo(?)",
                (name,), remaining, deadline, allocation)
            if len(columns) != ncol or any(row[0] != i for i, row in enumerate(columns)):
                raise ValueError('native Edge table columns changed')
            names = [row[1].decode('utf-8', 'strict') for row in columns]
            rowid = None if wr else next((alias for alias in ('_rowid_', 'rowid', 'oid')
                                         if alias.lower() not in {n.lower() for n in names}), None)
            if not wr and rowid is None:
                raise ValueError('native Edge source rowid aliases are shadowed')
            flags = wr | (strict << 1) | (4 if rowid is not None else 0)
            indices = _schema_rows(cursor,
                "SELECT CAST(name AS BLOB),CAST([unique] AS INTEGER),CAST(origin AS BLOB),"
                "CAST(partial AS INTEGER) "
                "FROM pragma_index_list(?) ORDER BY seq",
                "SELECT count(*),coalesce(sum(length(CAST(name AS BLOB))+length(CAST(origin AS BLOB))),0) "
                "FROM pragma_index_list(?)", (name,), remaining, deadline, allocation)
            encoded_indices = []
            for index_name, unique, origin, partial in indices:
                index_columns = _schema_rows(cursor,
                    "SELECT CAST(cid AS INTEGER),CAST(name AS BLOB),CAST([desc] AS INTEGER),"
                    "CAST(coll AS BLOB),CAST([key] AS INTEGER) "
                    "FROM pragma_index_xinfo(?) ORDER BY seqno",
                    "SELECT count(*),coalesce(sum(coalesce(length(CAST(name AS BLOB)),0)+"
                    "length(CAST(coll AS BLOB))),0) FROM pragma_index_xinfo(?)",
                    (index_name.decode('utf-8', 'strict'),), remaining, deadline, allocation)
                index_size = 2 + len(index_name) + 5 + sum(
                    4 + 2 + (len(colname) if colname is not None else 0) + 1 + 2 + len(coll) + 1
                    for _, colname, _, coll, _ in index_columns)
                allocation.reserve(index_size * 8 + 128)
                encoded = bytearray(_wire_string(index_name) + struct.pack('<BBB H', unique,
                    {b'c': 0, b'u': 1, b'pk': 2}[origin], partial, len(index_columns))
                )
                for cid, colname, desc, coll, key in index_columns:
                    _active(deadline)
                    encoded.extend(struct.pack('<i', cid) + _wire_string(colname, nullable=True)
                                   + bytes((desc,)) + _wire_string(coll) + bytes((key,)))
                encoded_indices.append(bytes(encoded))
            descriptor_size = sum(2 + len(row[1]) + 2 + len(row[2]) + 4 + 4
                                  + (len(row[6]) if row[6] is not None else 0) for row in columns)
            descriptor_size += sum(map(len, encoded_indices))
            allocation.reserve(descriptor_size * 8 + len(columns) * 128 + 256)
            descriptor = bytearray()
            for _, colname, decltype, notnull, pk, hidden, default in columns:
                _active(deadline)
                descriptor.extend(_wire_string(colname) + _wire_string(decltype)
                                  + struct.pack('<BHB', notnull, pk, hidden)
                                  + _wire_string(default, 4, nullable=True))
            for encoded in encoded_indices:
                _active(deadline)
                descriptor.extend(encoded)
            tables[name] = (raw_name, flags, columns, rowid, bytes(descriptor), len(indices))
        known_names = {name for _, name in registry}
        ordered = list(registry) + [(0xffff, name) for name in sorted(
            tables.keys() - known_names, key=lambda value: value.encode('utf-8'))]
        if len(ordered) - len(registry) > 65535:
            raise ValueError('native Edge opaque table count exceeds frame width')
        planned = []
        body_bytes = 0
        for table_id, name in ordered:
            if name not in tables:
                allocation.reserve((len(name) * 4 + 20) * 8 + 128)
                header = struct.pack('<HBB', table_id, 0, 0) + _wire_string(name.encode())
                header += struct.pack('<HHQ', 0, 0, 0)
                planned.append((name, header, None, 0))
                body_bytes += len(header)
                continue
            raw_name, flags, columns, rowid, descriptor, index_count = tables[name]
            table = 'main.' + _quoted(name)
            expressions = []
            for _, raw_col, *_ in columns:
                col = _quoted(raw_col.decode('utf-8', 'strict'))
                expressions.append("CASE typeof(" + col + ") WHEN 'null' THEN 1 "
                    "WHEN 'integer' THEN 9 WHEN 'real' THEN 9 ELSE 9+length(CAST("
                    + col + " AS BLOB)) END")
            expression = '+'.join(expressions) if expressions else '0'
            cursor.execute('SELECT count(*) FROM ' + table)
            row_count = cursor.fetchone()[0]
            _active(deadline)
            minimum_row_bytes = len(columns) + (8 if rowid else 0)
            if type(row_count) is not int or row_count < 0 or row_count * minimum_row_bytes > remaining:
                raise ValueError('native Edge row count exceeds selected frame envelope')
            cursor.execute('SELECT coalesce(sum(' + expression + '),0) AS encoded_bytes FROM ' + table)
            cell_bytes = cursor.fetchone()[0]
            _active(deadline)
            if (type(row_count) is not int or type(cell_bytes) is not int
                    or row_count < 0 or cell_bytes < 0):
                raise ValueError('native Edge row size preflight is invalid')
            allocation.reserve((len(raw_name) + len(descriptor) + 20) * 8)
            header = struct.pack('<HBB', table_id, 1, flags) + _wire_string(raw_name)
            header += struct.pack('<HHQ', len(columns), index_count, row_count) + descriptor
            body_bytes += len(header) + cell_bytes + row_count * (8 if rowid else 0)
            if body_bytes + _FRAME_HEADER.size + 32 > remaining:
                raise ValueError('native Edge logical frame exceeds selected byte budget')
            planned.append((name, header, tables[name], row_count))
            _active(deadline)
        object_bytes = []
        for kind, name, table_name, sql in objects:
            _active(deadline)
            allocation.reserve((1 + 2 + len(name) + 2 + len(table_name) + 4
                                + (len(sql) if sql is not None else 0)) * 8 + 128)
            object_bytes.append(bytes(({b'table': 1, b'index': 2, b'trigger': 3, b'view': 4}[kind],))
                                + _wire_string(name) + _wire_string(table_name)
                                + _wire_string(sql, 4, nullable=True))
        total = _FRAME_HEADER.size + body_bytes + sum(map(len, object_bytes)) + 32
        if total > remaining:
            raise ValueError('native Edge logical frame exceeds selected byte budget')
        _active(deadline)
        return registry, planned, object_bytes, total, database_encoding
    finally:
        cursor.close()


@dataclass(frozen=True)
class NativeCaptureContext:
    """Explicit installed software and finite transport selection.

    This selects software and transport resources, never source admission.
    The deadline is the caller's original absolute monotonic deadline.
    Linux child parent-death guards preserve caller handlers; the whole owner
    supervisor owns descendants and scratch on abrupt caller termination.
    max_snapshot_bytes bounds aggregate encoded logical frames, including
    descriptors and hashes; it is not a SQLite page-image size estimate.
    """
    prefix: Path
    scratch: Path
    deadline: float
    max_snapshot_bytes: int
    max_stream_bytes: int
    max_schema_allocation_bytes: int | None = None

    def run(self, request, snapshots):
        return capture(self.prefix, request, snapshots, scratch=self.scratch,
                       deadline=self.deadline,
                       max_snapshot_bytes=self.max_snapshot_bytes,
                       max_stream_bytes=self.max_stream_bytes,
                       max_schema_allocation_bytes=self.max_schema_allocation_bytes)


class CaptureCustodyError(RuntimeError):
    """Owned child/group not proven released; retain its selected inputs."""


def _active(deadline):
    if time.monotonic() >= deadline:
        raise TimeoutError('native Edge capture deadline exceeded')


def _identity(fd):
    value = os.fstat(fd)
    return (value.st_dev, value.st_ino, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns, value.st_mode)


def _directory_identity(fd):
    value = os.fstat(fd)
    return value.st_dev, value.st_ino


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate native Edge result member')
        result[key] = value
    return result


def _logical_rows(connection, name, table, remaining, deadline, *, database_encoding='UTF-8'):
    """Bounded raw storage-class rows from the same selected connection."""
    _, _, columns, rowid, _, _ = table
    quoted_columns = [_quoted(row[1].decode('utf-8', 'strict')) for row in columns]
    size_expression = '+'.join(
        "CASE typeof(" + col + ") WHEN 'null' THEN 1 WHEN 'integer' THEN 9 "
        "WHEN 'real' THEN 9 ELSE 9+length(CAST(" + col + " AS BLOB)) END"
        for col in quoted_columns) or '0'
    fields = ['CAST(' + _quoted(rowid) + ' AS INTEGER)'] if rowid else []
    for col in quoted_columns:
        fields.extend(("CAST(typeof(" + col + ") AS BLOB)",
            "CASE WHEN typeof(" + col + ") IN ('integer','real') THEN " + col + " ELSE NULL END",
            "CASE WHEN typeof(" + col + ") IN ('text','blob') THEN CAST(" + col + " AS BLOB) ELSE NULL END"))
    order = _quoted(rowid) if rowid else ','.join(
        quoted_columns[i] for i, row in sorted(enumerate(columns), key=lambda p: p[1][4]) if row[4])
    cursor = _frame_cursor(connection)
    try:
        # WHERE prevents SQLite/Python copying an oversized row after a
        # concurrent mutation. The exact planned row count then refuses it.
        cursor.execute('SELECT ' + ','.join(expression + ' AS cell_' + str(index)
                                           for index, expression in enumerate(fields))
                       + ' FROM main.' + _quoted(name)
                       + ' WHERE (' + size_expression + ') <= ?'
                       + (' ORDER BY ' + order if order else ''), (remaining,))
        while True:
            _active(deadline)
            row = cursor.fetchone()
            if row is None:
                break
            offset = 1 if rowid else 0
            if rowid and type(row[0]) is not int:
                raise ValueError('native Edge rowid is not INTEGER')
            cells = []
            for index in range(offset, len(row), 3):
                kind, number, raw = row[index:index + 3]
                kind = kind.decode(database_encoding, 'strict').encode('ascii')
                if not (kind == b'null' or kind == b'integer' and type(number) is int
                        or kind == b'real' and type(number) is float
                        or kind in (b'text', b'blob') and type(raw) is bytes):
                    raise ValueError('native Edge cell type/size changed during capture')
                cells.append((kind, number, raw))
            yield row[0] if rowid else None, cells
        _active(deadline)
    finally:
        cursor.close()


def _logical_write(connection, fd, role, remaining, deadline, *, max_schema_allocation_bytes=None):
    registry, tables, objects, total, database_encoding = _logical_plan(
        connection, role, remaining, deadline,
        max_schema_allocation_bytes=max_schema_allocation_bytes)
    digest = hashlib.sha256()
    opaque_digest = hashlib.sha256()
    object_digest = hashlib.sha256()
    opaque_tables = []
    current_opaque = None
    written_total = 0

    def emit(raw, hashed=True):
        nonlocal written_total
        if written_total + len(raw) > total:
            raise ValueError('native Edge logical view grew during capture')
        if hashed:
            digest.update(raw)
        if current_opaque is not None:
            current_opaque.update(raw)
            opaque_digest.update(raw)
        if fd is None:
            _active(deadline)
            written_total += len(raw)
            return
        view = memoryview(raw)
        while view:
            _active(deadline)
            written = os.write(fd, view[:65_536])
            if written <= 0:
                raise OSError('native Edge logical snapshot write made no progress')
            written_total += written
            view = view[written:]

    emit(_FRAME_HEADER.pack(b'TOSLSNP1', role,
                           {'UTF-8': 1, 'UTF-16le': 2, 'UTF-16be': 3}[database_encoding], len(registry),
                           len(tables) - len(registry), len(objects), total))
    cursor = _frame_cursor(connection)
    try:
        for name, header, table, expected_rows in tables:
            _active(deadline)
            current_opaque = hashlib.sha256() if header[:2] == b'\xff\xff' else None
            emit(header)
            if table is None:
                continue
            _, flags, columns, rowid, _, _ = table
            observed = 0
            for source_rowid, cells in _logical_rows(connection, name, table, remaining, deadline,
                                                    database_encoding=database_encoding):
                if observed >= expected_rows:
                    raise ValueError('native Edge logical row count changed')
                if rowid:
                    emit(struct.pack('<q', source_rowid))
                for kind, number, raw in cells:
                    if kind == b'null':
                        emit(b'\0')
                    elif kind == b'integer':
                        emit(b'\1' + struct.pack('<q', number))
                    elif kind == b'real':
                        emit(b'\2' + struct.pack('<d', number))
                    else:
                        emit(bytes((3 if kind == b'text' else 4,)) + struct.pack('<Q', len(raw)))
                        emit(raw)
                observed += 1
            if observed != expected_rows:
                raise ValueError('native Edge logical row count changed')
            if current_opaque is not None:
                opaque_tables.append({'name_sha256': hashlib.sha256(name.encode('utf-8')).hexdigest(),
                                      'row_count': observed,
                                      'logical_sha256': current_opaque.hexdigest()})
            current_opaque = None
        for raw in objects:
            object_digest.update(raw)
            emit(raw)
        if written_total != total - 32:
            raise ValueError('native Edge logical view size changed during capture')
        trailer = digest.digest()
        emit(trailer, hashed=False)
        digest.update(trailer)
        _active(deadline)
        return {'database_encoding': database_encoding, 'frame_bytes': total, 'frame_sha256': digest.hexdigest(),
                'schema_objects_sha256': object_digest.hexdigest(),
                'opaque_tables_sha256': opaque_digest.hexdigest(),
                'opaque_tables': opaque_tables}
    finally:
        cursor.close()


def _snapshot(connection, path, remaining, deadline, role=1, *, max_schema_allocation_bytes=None):
    if not isinstance(connection, sqlite3.Connection) or not connection.in_transaction:
        raise ValueError('native Edge capture requires caller-held snapshots')
    _active(deadline)
    # SQL on this held connection reads its selected dirty/WAL view. SQLite's
    # deserialize/memdb serialize fast path can copy the backing buffer and
    # omit dirty pager pages; backup can retry forever on an active writer.
    # The owned Rust importer consumes typed values and inert schema evidence,
    # never source DDL or a claim of physical SQLite page identity.
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        inventory = _logical_write(connection, fd, role, remaining, deadline,
                                   max_schema_allocation_bytes=max_schema_allocation_bytes)
        os.fsync(fd)
        written_stamp = _identity(fd)
    finally:
        os.close(fd)
    _active(deadline)
    if not connection.in_transaction:
        raise ValueError('caller released its native Edge snapshot')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        info = os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= remaining
                or _identity(fd) != written_stamp):
            raise ValueError('native Edge snapshot copy has invalid final size')
        return fd, _identity(fd), inventory
    except BaseException:
        os.close(fd)
        raise


def _live_group(pgid):
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            raw = (entry / 'stat').read_text()
            fields = raw[raw.rindex(')') + 2:].split()
            if int(fields[2]) == pgid and fields[0] not in ('Z', 'X'):
                return True
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    return False


def _release_child(child, deadline):
    # Accepted Edge receiver law: keep the unreaped leader as PGID anchor
    # until TERM/KILL and group observations are complete. Every cleanup phase
    # attempts its work even when a preceding signal or census fails.
    errors = []
    ownership_lost = False
    end = min(deadline, time.monotonic() + 5)
    for sig, allowance in ((signal.SIGTERM, 1), (signal.SIGKILL, 4)):
        if ownership_lost:
            continue
        while True:
            try:
                os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                break
            except (InterruptedError, KeyboardInterrupt):
                # An interrupted observation does not release the child.
                # Retry only within the original cleanup clock, never signal
                # without a fresh successful ownership observation.
                if time.monotonic() >= end:
                    ownership_lost = True
                    errors.append('owned child observation deadline expired')
                    break
            except ChildProcessError:
                ownership_lost = True
                errors.append('owned child anchor lost')
                break
            except BaseException:
                ownership_lost = True
                errors.append('owned child anchor unverifiable')
                break
        if ownership_lost:
            continue
        try:
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            pass
        except BaseException:
            errors.append('group signal failed')
        phase = min(end, time.monotonic() + allowance)
        try:
            while _live_group(child.pid) and time.monotonic() < phase:
                time.sleep(min(0.02, max(0, phase - time.monotonic())))
        except BaseException:
            errors.append('group census failed')
    try:
        # No unconditional wait: the original caller deadline owns reap too.
        if not ownership_lost:
            child.wait(timeout=max(0.001, end - time.monotonic()))
    except BaseException:
        errors.append('owned child remains unreaped')
    try:
        if _live_group(child.pid):
            errors.append('owned group remains live')
    except BaseException:
        errors.append('final group census failed')
    if errors:
        raise CaptureCustodyError('; '.join(errors))


def _observe(prefix, request_path, deadline, stream_bytes):
    # Exactly the same verifier route as NativeMCPServer. execve replaces the
    # child, so both verifier and ELF remain in the owned process group.
    dispatch = Path(native_dispatch.__file__).resolve(strict=True)
    program = (
        'import os,sys,signal,ctypes;'
        'expected=int(sys.argv[4]);'
        'libc=ctypes.CDLL(None,use_errno=True);'
        'libc.prctl.argtypes=[ctypes.c_int,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_ulong];'
        'libc.prctl.restype=ctypes.c_int;'
        'rc=libc.prctl(1,signal.SIGKILL,0,0,0);'
        'rc==0 or sys.exit("native Edge parent-death guard failed");'
        'os.getppid()==expected or os.kill(os.getpid(),signal.SIGKILL);'
        'signal.pthread_sigmask(signal.SIG_SETMASK,[]);'
        'import importlib.util;from pathlib import Path;'
        's=importlib.util.spec_from_file_location("native_selected_dispatch",sys.argv[1]);'
        'm=importlib.util.module_from_spec(s);s.loader.exec_module(m);'
        'm.run(Path(sys.argv[2]),["edge-offline-capture","--expected-parent-pid",sys.argv[4],"--request",sys.argv[3]])'
    )
    selector = selectors.DefaultSelector()
    child = None
    captured = {'stdout': bytearray(), 'stderr': bytearray()}
    primary = None
    cleanup_error = None
    operation_end = deadline - 5
    try:
        _active(operation_end)
        # Keep the child handle owned before a pending termination can raise.
        # A fresh Linux child arms parent-death SIGKILL and verifies the exact
        # caller PID before unmasking or dispatch; the native v2 entry rearms it.
        # This handles default termination in any caller thread without changing
        # caller handlers. The whole owner supervisor still owns descendants
        # and retained scratch after abrupt parent death; no finally is implied.
        # No preexec_fn is used on a possibly threaded caller.
        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT, signal.SIGTERM})
        try:
            child = subprocess.Popen(
                [sys.executable, '-I', '-S', '-B', '-c', program, str(dispatch), str(prefix), str(request_path), str(os.getpid())],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                start_new_session=True,
            )
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        for name in captured:
            stream = getattr(child, name)
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        while selector.get_map():
            _active(operation_end)
            for key, _ in selector.select(min(0.1, max(0, operation_end - time.monotonic()))):
                data = os.read(key.fileobj.fileno(), min(65_536, stream_bytes + 1 - len(captured[key.data])))
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                captured[key.data].extend(data)
                if len(captured[key.data]) > stream_bytes:
                    raise ValueError('native Edge capture response exceeds selected stream budget')
        while os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
            _active(operation_end)
            time.sleep(min(0.02, max(0, operation_end - time.monotonic())))
    except BaseException as error:
        primary = error
    finally:
        if child is not None:
            try:
                _release_child(child, deadline)
            except BaseException as error:
                cleanup_error = error
        for handle in ([child.stdout, child.stderr] if child is not None else []) + [selector]:
            try:
                if handle is not None:
                    handle.close()
            except BaseException:
                cleanup_error = cleanup_error or CaptureCustodyError('owned stream close failed')
    if cleanup_error is not None:
        raise cleanup_error from primary
    if primary is not None:
        raise primary
    _active(deadline)
    if child.returncode:
        # Never include native stdout/stderr or source payloads in errors.
        raise RuntimeError(f'native Edge capture failed: status={child.returncode}; '
                           f'stdout_bytes={len(captured["stdout"])}; '
                           f'stderr_bytes={len(captured["stderr"])}')
    return bytes(captured['stdout'])


def capture(prefix: Path, request: dict, snapshots: dict, *, scratch: Path,
            deadline: float, max_snapshot_bytes: int, max_stream_bytes: int,
            max_schema_allocation_bytes: int | None = None):
    """Run one native capture using unchanged owner request and held snapshots.

    ``snapshots`` maps native database fields to already selected, caller-held
    transactions (including uncommitted views). No BEGIN/COMMIT/rollback is
    performed on them. Typed SQL reads preserve the selected view and raw TEXT
    bytes in the validated database encoding. Schema metadata is UTF-8 on the
    wire; source DDL is inert evidence. No borrowed-writer backup, serialization
    or pathname reopening is a fallback. Native semantic validation retains
    its owner rules; WAL/dirty-view compatibility requires real consumer checks.
    The caller supplies one absolute monotonic deadline and admitted copy/
    stream bounds; its whole storage envelope must also cover SQLite scratch
    and native output files. Copy bytes are not an allocator/RSS estimate.
    Scratch is an exclusively owned private namespace for this operation.
    """
    if (type(deadline) not in (int, float) or not math.isfinite(deadline)
            or type(max_snapshot_bytes) is not int or max_snapshot_bytes <= 0
            or type(max_stream_bytes) is not int or max_stream_bytes <= 0
            or (max_schema_allocation_bytes is not None and
                (type(max_schema_allocation_bytes) is not int or max_schema_allocation_bytes <= 0))):
        raise ValueError('native Edge capture requires finite selected bounds')
    if (not isinstance(prefix, Path) or not prefix.is_absolute()
            or not isinstance(scratch, Path) or not scratch.is_absolute()
            or scratch.resolve(strict=True) != scratch
            or type(request) is not dict or type(snapshots) is not dict
            or not snapshots or not set(snapshots) <= _SNAPSHOT_FIELDS
            or not set(snapshots) <= request.keys()
            or {field for field in _SNAPSHOT_FIELDS if request.get(field) is not None}
               != set(snapshots)):
        raise ValueError('native Edge capture requires explicit selected paths/snapshots')
    root_fd = os.open(scratch, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    root_identity = _directory_identity(root_fd)
    held = []
    working = None
    retain = False
    try:
        _active(deadline)
        working = Path(tempfile.mkdtemp(prefix='native-edge-', dir=scratch))
        try:
            selected = dict(request)
            selected['schema'] = 'tos_edge_offline_capture_request_v2'
            # Emitter custody requires an auxiliary manifest, but the maintained
            # API publishes only its selected SQL targets and returned receipt.
            # This actual native output stays in the exclusive transport scope.
            selected['manifest_json'] = str(working / 'manifest.json')
            selected['snapshot_frame_max_bytes'] = max_snapshot_bytes
            selected['snapshot_schema_max_allocation_bytes'] = (
                max_schema_allocation_bytes if max_schema_allocation_bytes is not None
                else max_snapshot_bytes * 128)
            remaining = max_snapshot_bytes
            snapshot_inventory = []
            for index, field in enumerate(field for field in _SNAPSHOT_ORDER if field in snapshots):
                connection = snapshots[field]
                path = working / f'snapshot-{index}.lsnap'
                fd, stamp, inventory = _snapshot(connection, path, remaining, deadline,
                                                 role=1 if field == 'd1_database' else 2,
                                                 max_schema_allocation_bytes=max_schema_allocation_bytes)
                held.append((fd, path, stamp, connection))
                snapshot_inventory.append(dict(input_field=field, **inventory))
                remaining -= stamp[2]
                selected[field] = str(path)
            request_path = working / 'request.json'
            fd = os.open(request_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, 'wb') as stream:
                encoder = json.JSONEncoder(ensure_ascii=False, allow_nan=False,
                                           separators=(',', ':'))
                request_bytes = 0
                for chunk in encoder.iterencode(selected):
                    _active(deadline)
                    raw = chunk.encode('utf-8')
                    if len(raw) > _REQUEST_BYTES - request_bytes:
                        raise ValueError('native Edge capture request exceeds native byte budget')
                    request_bytes += len(raw)
                    stream.write(raw)
                del raw, chunk
            result = _observe(prefix, request_path, deadline, max_stream_bytes)
            for fd, path, stamp, connection in held:
                named = path.stat(follow_symlinks=False)
                if (_identity(fd) != stamp or not connection.in_transaction
                        or (named.st_dev, named.st_ino, named.st_size,
                            named.st_mtime_ns, named.st_ctime_ns, named.st_mode) != stamp):
                    raise ValueError('native Edge capture snapshot custody changed')
            value = json.loads(result, object_pairs_hook=_unique)
            if type(value) is not dict or value.get('schema') != 'tos_edge_offline_capture_result_v2':
                raise ValueError('native Edge capture result profile differs')
            receipt = value.get('receipt')
            if (type(receipt) is not dict or receipt.get('snapshot_transport') != {
                    'schema': 'tos_edge_typed_snapshot_inventory_v1',
                    'snapshots': snapshot_inventory}):
                raise ValueError('native Edge imported snapshot evidence differs')
            _active(deadline)
        except CaptureCustodyError as error:
            retain = True
            raise CaptureCustodyError(f'{error}; retain selected inputs at {working}') from error
    finally:
        cleanup_errors = []
        for fd, *_ in held:
            try:
                os.close(fd)
            except OSError:
                cleanup_errors.append('snapshot descriptor close failed')
        if working is not None and not retain:
            try:
                shutil.rmtree(working)
            except OSError:
                cleanup_errors.append('snapshot directory cleanup failed')
        try:
            named = scratch.stat(follow_symlinks=False)
            if ((named.st_dev, named.st_ino) != root_identity
                    or not stat.S_ISDIR(named.st_mode)):
                cleanup_errors.append('scratch directory custody changed')
        except OSError:
            cleanup_errors.append('scratch directory custody unavailable')
        finally:
            os.close(root_fd)
        if cleanup_errors:
            raise CaptureCustodyError('; '.join(cleanup_errors))
    _active(deadline)
    return value
