"""Bounded exact-engine callbacks for the installed Rust Edge encoder.

No table registry, schema interpretation or frame emitter lives here. Only
Rust-authored SELECTs run on the original, exclusively borrowed connections.
One row per cursor is retained; large raw cells travel in 64 KiB chunks.
"""
import json
import os
import sqlite3
import stat
import struct
import sys
import threading
import time
from .native_io import owned_exchange, NativeCustodyError

_FIELDS = ('d1_database', 'before_prepared_database', 'after_prepared_database')
_WIRE = 16 * 1024**2
_CHUNK = 65536
_CURSORS = 8


def snapshots_native(prefix, snapshots, directory, deadline, frame_bytes,
                     schema_bytes, rpc_bytes):
    from .native_edge_capture import CaptureCustodyError, _active, _identity, _unique
    selected = [(field, snapshots[field]) for field in _FIELDS if field in snapshots]
    if (not selected or len(selected) > 3
            or any(not isinstance(db, sqlite3.Connection) or not db.in_transaction for _, db in selected)):
        raise ValueError('caller-held SQLite views required')
    if (type(rpc_bytes) is not int or not 0 < rpc_bytes <= 2**63 - 1
            or type(schema_bytes) is not int or not 0 < schema_bytes <= 2**63 - 1):
        raise ValueError('finite original borrowed query budgets required')
    work = deadline - 5
    _active(work)
    paths = [directory / f'snapshot-{i}.lsnap' for i in range(len(selected))]
    cursors = {}
    next_cursor = 0
    sequence = 0
    io_remaining = rpc_bytes
    allocation_remaining = schema_bytes
    pending_allocation = 0
    stop = threading.Event()
    stepping = [None]
    lock = threading.Lock()
    held = []
    result = None
    channel = None

    def reserve(size):
        nonlocal allocation_remaining, pending_allocation
        if size < 0 or size > allocation_remaining:
            raise ValueError('borrowed query cumulative adapter allocation budget exceeded')
        allocation_remaining -= size
        pending_allocation += size

    def io_charge(size):
        nonlocal io_remaining
        if size > io_remaining:
            raise ValueError('borrowed query cumulative RPC IO budget exceeded')
        io_remaining -= size

    def watch():
        if stop.wait(max(0, work - time.monotonic())):
            return
        # Repeat until join: an interrupt just before SQLite starts stepping
        # must not be mistaken for cancellation of the later step.
        while not stop.is_set():
            with lock:
                db = stepping[0]
                if db is not None:
                    db.interrupt()
            if stop.wait(0.005):
                break

    if schema_bytes < 1_048_576 + 65536 + 4096 + _CURSORS * 256:
        raise ValueError('borrowed original schema cap cannot admit helper control workspace')
    reserve(16384 + _CURSORS * 256)
    watcher = threading.Thread(target=watch, name='tos-edge-sql-deadline')
    watcher.start()
    try:
        with owned_exchange(['edge-offline-capture', '--borrowed-frames'],
                prefix=str(prefix), input_cap=_WIRE, frame_cap=_WIRE,
                absolute_deadline=deadline, operation_seconds=1210) as channel:
            control = {'schema': 'tos_edge_borrowed_query_v1',
                'snapshots': [{'field': field, 'path': str(path)}
                              for (field, _), path in zip(selected, paths)],
                'frame_bytes': frame_bytes, 'schema_bytes': schema_bytes,
                'rpc_bytes': rpc_bytes,
                'work_deadline_ns': int(work * 1_000_000_000)}
            raw = json.dumps(control, separators=(',', ':'), allow_nan=False).encode()
            reserve(2 * len(raw))
            io_charge(len(raw) + 1)
            channel.write_input(raw + b'\n')
            pending_allocation = 0  # included in fixed native admission workspace
            for raw in channel.frames():
                _active(work)
                io_charge(len(raw) + 1)
                # Request decoding is prepaid by the native sender's SQL/
                # control envelope; replies consume their separate grant.
                packet = json.loads(raw, object_pairs_hook=_unique)
                if set(packet) == {'result'}:
                    if result is not None or cursors:
                        raise ValueError('borrowed helper result/cursor custody differs')
                    result = packet['result']
                    channel.close_input()
                    continue
                if result is not None or set(packet) != {'query', 'seq', 'adapter_grant', 'reply_cap'}:
                    raise ValueError('borrowed helper response profile differs')
                grant, reply_cap = packet['adapter_grant'], packet['reply_cap']
                if (type(grant) is not int or not 0 < grant <= schema_bytes
                        or type(reply_cap) is not int or not 0 < reply_cap <= _WIRE):
                    raise ValueError('borrowed shared reply grant differs')
                allocation_remaining = grant
                pending_allocation = 0
                sequence += 1
                if type(packet['seq']) is not int or packet['seq'] != sequence:
                    raise ValueError('borrowed helper sequence differs')
                query = packet['query']
                if type(query) is not dict or type(query.get('snapshot')) is not int or not 0 <= query['snapshot'] < len(selected):
                    raise ValueError('borrowed helper view selection differs')
                snapshot = query['snapshot']
                db = selected[snapshot][1]
                if not db.in_transaction:
                    raise ValueError('caller released original SQLite transaction')
                op = query.get('op')
                if op == 'held' and set(query) == {'op', 'snapshot'}:
                    value = True
                elif op == 'prepare' and set(query) == {'op', 'snapshot', 'sql'}:
                    sql = query['sql']
                    if (len(cursors) >= _CURSORS or type(sql) is not str
                            or not sql.startswith('SELECT ') or '\0' in sql
                            or len(sql) > _WIRE):
                        raise ValueError('borrowed SELECT/cursor cap differs')
                    reserve(2 * len(sql.encode()) + 256)
                    cursor = db.cursor()
                    cursor.row_factory = None
                    try:
                        with lock:
                            _active(work)
                            stepping[0] = db
                        cursor.execute(sql)
                        columns = len(cursor.description or ())
                        if not 0 < columns <= 196606:
                            raise ValueError('borrowed SQL column cap differs')
                    except BaseException as error:
                        try:
                            cursor.close()
                        except BaseException as cleanup:
                            error.add_note(f'prepare cursor close failed: {cleanup!r}')
                            next_cursor += 1
                            cursors[next_cursor] = [snapshot, cursor, None, 0]
                        raise
                    finally:
                        with lock:
                            stepping[0] = None
                    next_cursor += 1
                    cursors[next_cursor] = [snapshot, cursor, None, columns]
                    value = {'cursor': next_cursor, 'columns': columns}
                else:
                    identity = query.get('cursor')
                    if type(identity) is not int or identity not in cursors or cursors[identity][0] != snapshot:
                        raise ValueError('borrowed cursor selection differs')
                    state = cursors[identity]
                    if op == 'close' and set(query) == {'op', 'snapshot', 'cursor'}:
                        state[1].close()
                        del cursors[identity]
                        value = None
                    elif op == 'next' and set(query) == {'op', 'snapshot', 'cursor', 'raw_cap'}:
                        # Fixed tuple/pointer occupancy before fetch. SQL owner
                        # preflights raw lengths and filters rows by frame cap.
                        raw_cap = query['raw_cap']
                        if type(raw_cap) is not int or not 0 <= raw_cap <= schema_bytes:
                            raise ValueError('borrowed prepaid raw row cap differs')
                        reserve(128 + state[3] * 128)
                        with lock:
                            _active(work)
                            stepping[0] = db
                        try:
                            state[2] = state[1].fetchone()
                        finally:
                            with lock:
                                stepping[0] = None
                        row = state[2]
                        if row is None:
                            value = None
                        else:
                            if len(row) != state[3]:
                                raise ValueError('borrowed row width changed')
                            raw_bytes = sum(len(v) if type(v) is bytes else 8 for v in row)
                            # Owner length cursor/preflight reserved raw copies
                            # before fetchone; this check verifies alignment.
                            if raw_bytes > raw_cap:
                                raise ValueError('borrowed row exceeds prepaid raw cap')
                            reserve(state[3] * 256)
                            value = []
                            for cell in row:
                                if cell is None:
                                    value.append(['null'])
                                elif type(cell) is int and -(2**63) <= cell < 2**63:
                                    value.append(['integer', cell])
                                elif type(cell) is float:
                                    value.append(['real', struct.pack('<d', cell).hex()])
                                elif type(cell) is bytes:
                                    value.append(['blob', len(cell)])
                                else:
                                    raise ValueError('borrowed SELECT did not return raw SQLite evidence')
                    elif op == 'blob' and set(query) == {'op', 'snapshot', 'cursor', 'column', 'offset', 'count'}:
                        column, offset, count = (query[k] for k in ('column', 'offset', 'count'))
                        row = state[2]
                        if (any(type(v) is not int for v in (column, offset, count))
                                or row is None or not 0 <= column < len(row)
                                or type(row[column]) is not bytes or offset < 0
                                or not 0 < count <= _CHUNK or count > len(row[column]) - offset):
                            raise ValueError('borrowed raw cell chunk bounds differ')
                        reserve(count * 3 + 128)
                        value = row[column][offset:offset + count].hex()
                    else:
                        raise ValueError('borrowed query operation differs')
                _active(work)
                if not db.in_transaction:
                    raise ValueError('caller released original SQLite transaction')
                def wire_bound(member):
                    if type(member) is str:
                        # Hex/control are ASCII, and exact escaping length is
                        # computed before JSON serialization for other strings.
                        return 2 + sum(2 if c in '\"\\\b\f\n\r\t' else
                            6 if ord(c) < 32 else 1 if ord(c) < 128 else
                            2 if ord(c) < 2048 else 3 if ord(c) < 65536 else 4
                            for c in member)
                    if type(member) in (int, float, bool, type(None)):
                        return 32
                    if type(member) is list:
                        return 2 + sum(1 + wire_bound(v) for v in member)
                    if type(member) is dict:
                        return 2 + sum(2 + wire_bound(k) + wire_bound(v) for k, v in member.items())
                    raise ValueError('borrowed ordinary transport value required')
                response = {'seq': sequence, 'held': True, 'value': value, 'adapter_charge': 0}
                bound = wire_bound(response) + 64
                if bound > reply_cap:
                    raise ValueError('borrowed response preallocation bound exceeds frame cap')
                reserve(2 * bound)
                response['adapter_charge'] = pending_allocation
                # One bounded descriptor/chunk, never a table or full BLOB hex.
                raw = json.dumps(response, separators=(',', ':'), allow_nan=False).encode()
                if len(raw) > reply_cap:
                    raise ValueError('borrowed RPC frame cap exceeded')
                io_charge(len(raw) + 1)
                channel.write_input(raw + b'\n')
                pending_allocation = 0
        if (type(result) is not dict or result.get('schema') != 'tos_edge_borrowed_frames_v1'
                or type(result.get('snapshots')) is not list or len(result['snapshots']) != len(paths)):
            raise ValueError('borrowed encoder result differs')
        remaining = frame_bytes
        for (field, db), path, inventory in zip(selected, paths, result['snapshots']):
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
            held.append((fd, path, _identity(fd), db))
            info = os.fstat(fd)
            if (not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= remaining
                    or inventory.get('input_field') != field
                    or inventory.get('frame_bytes') != info.st_size or not db.in_transaction):
                raise ValueError('borrowed frame/view custody differs')
            remaining -= info.st_size
        remaining_schema = result.get('schema_remaining')
        if type(remaining_schema) is not int or not 0 < remaining_schema <= schema_bytes:
            raise ValueError('borrowed final schema budget differs')
        return held, result['snapshots'], remaining_schema
    except NativeCustodyError as error:
        # The exception transfers explicit references to the outer holder.
        # Do not continue to final capture or drop selected connections/frames.
        failure = CaptureCustodyError('borrowed encoder child cleanup unproven')
        failure.borrowed_custody = {'views': selected, 'frames': paths, 'directory': directory,
                                    'descriptors': held, 'child_released': False,
                                    'native_owner': error, 'helper_identity': error.custody_snapshot()}
        raise failure from error
    finally:
        primary = sys.exception()
        cleanup_errors = []
        # Attempt every cursor even if one close raises, and always stop/join
        # the worker before the borrowed view owner is allowed to continue.
        try:
            for _, cursor, _, _ in list(cursors.values()):
                try:
                    cursor.close()
                except BaseException as error:
                    cleanup_errors.append(('cursor', error))
        finally:
            stop.set()
            try:
                watcher.join(timeout=max(0, deadline - time.monotonic()))
            except BaseException as error:
                cleanup_errors.append(('watchdog join', error))
        joined = not watcher.is_alive()
        if not joined:
            cleanup_errors.append(('watchdog', RuntimeError('interrupt worker did not join')))
        if cleanup_errors:
            failure = primary if isinstance(primary, CaptureCustodyError) else CaptureCustodyError(
                'borrowed SQLite adapter cleanup unproven')
            identity = channel.custody_snapshot() if channel is not None else {
                'child_released': True, 'owner_bound': False, 'child_started': False}
            custody = getattr(failure, 'borrowed_custody', {})
            custody.update({'views': selected, 'frames': paths, 'directory': directory,
                'descriptors': held, 'child_released': identity['child_released'],
                'helper_identity': identity, 'watchdog_joined': joined,
                'watchdog_owner': {'thread': watcher, 'stop': stop,
                    'stepping': stepping, 'lock': lock}, 'cursor_owners': cursors})
            failure.borrowed_custody = custody
            for stage, error in cleanup_errors:
                failure.add_note(f'{stage} cleanup failed: {error!r}')
            if failure is not primary:
                raise failure from primary
        elif primary is not None and not isinstance(primary, CaptureCustodyError):
            failed_descriptors = []
            for descriptor in held:
                try:
                    os.close(descriptor[0])
                except BaseException as error:
                    primary.add_note(f'frame descriptor close failed: {error!r}')
                    failed_descriptors.append(descriptor)
            if failed_descriptors:
                failure = CaptureCustodyError('borrowed frame descriptor cleanup unproven')
                failure.borrowed_custody = {'views': selected, 'frames': paths,
                    'directory': directory, 'descriptors': failed_descriptors,
                    'child_released': channel.custody_snapshot()['child_released'],
                    'helper_identity': channel.custody_snapshot(), 'watchdog_joined': True}
                raise failure from primary
