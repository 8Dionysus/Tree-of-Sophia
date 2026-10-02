"""Returning transport for the installed offline Edge capture operation.

The caller owns source/pair admission, its read transactions, catalog/binding
selection and output targets. This bridge only freezes those borrowed views
and runs the existing installed-prefix verifier in a child. Rust owns capture
rules and returns its receipt unchanged. No Python oracle is a fallback.
"""
from __future__ import annotations

import json
import math
import os
from dataclasses import dataclass
from pathlib import Path
import selectors
import shutil
import signal
import sqlite3
import stat
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
_SNAPSHOT_FIELDS = frozenset({
    'd1_database', 'before_prepared_database', 'after_prepared_database',
})


@dataclass(frozen=True)
class NativeCaptureContext:
    """Explicit installed software and finite transport selection.

    This selects software and transport resources, never source admission.
    The deadline is the caller's original absolute monotonic deadline.
    """
    prefix: Path
    scratch: Path
    deadline: float
    max_snapshot_bytes: int
    max_stream_bytes: int

    def run(self, request, snapshots):
        return capture(self.prefix, request, snapshots, scratch=self.scratch,
                       deadline=self.deadline,
                       max_snapshot_bytes=self.max_snapshot_bytes,
                       max_stream_bytes=self.max_stream_bytes)


class CaptureCustodyError(RuntimeError):
    """Owned child/group not proven released; retain its selected inputs."""


def _active(deadline):
    if time.monotonic() >= deadline:
        raise TimeoutError('native Edge capture deadline exceeded')


def _identity(fd):
    value = os.fstat(fd)
    return (value.st_dev, value.st_ino, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


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


def _snapshot(connection, path, remaining, deadline):
    if not isinstance(connection, sqlite3.Connection) or not connection.in_transaction:
        raise ValueError('native Edge capture requires caller-held snapshots')
    _active(deadline)
    # Reading through the borrowed connection preserves its selected SQLite
    # view, including WAL contents. Reopening its pathname would not do so.
    page_size = connection.execute('PRAGMA main.page_size').fetchone()[0]
    page_count = connection.execute('PRAGMA main.page_count').fetchone()[0]
    if (type(page_size) is not int or type(page_count) is not int
            or page_size <= 0 or page_count <= 0
            or page_size * page_count > remaining):
        raise ValueError('native Edge snapshot copy exceeds selected byte budget')
    # serialize reads this connection's pager, including its uncommitted view.
    # backup would retry indefinitely if this is an active write transaction.
    # Preflight its allocation BEFORE asking SQLite for the complete image.
    expected_bytes = page_size * page_count
    raw = connection.serialize(name='main')
    _active(deadline)
    if len(raw) != expected_bytes:
        raise ValueError('native Edge snapshot serialization size changed')
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        view = memoryview(raw)
        while view:
            _active(deadline)
            written = os.write(fd, view[:65_536])
            if written <= 0:
                raise OSError('native Edge snapshot write made no progress')
            view = view[written:]
        written_stamp = _identity(fd)
    finally:
        os.close(fd)
    del view, raw
    _active(deadline)
    if not connection.in_transaction:
        raise ValueError('caller released its native Edge snapshot')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        info = os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= remaining
                or _identity(fd) != written_stamp):
            raise ValueError('native Edge snapshot copy has invalid final size')
        return fd, _identity(fd)
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
    end = min(deadline, time.monotonic() + 5)
    for sig, allowance in ((signal.SIGTERM, 1), (signal.SIGKILL, 4)):
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
        'import importlib.util,sys;from pathlib import Path;'
        's=importlib.util.spec_from_file_location("native_selected_dispatch",sys.argv[1]);'
        'm=importlib.util.module_from_spec(s);s.loader.exec_module(m);'
        'm.run(Path(sys.argv[2]),["edge-offline-capture","--request",sys.argv[3]])'
    )
    selector = selectors.DefaultSelector()
    child = None
    captured = {'stdout': bytearray(), 'stderr': bytearray()}
    primary = None
    cleanup_error = None
    operation_end = deadline - 5
    try:
        _active(operation_end)
        child = subprocess.Popen(
            [sys.executable, '-B', '-c', program, str(dispatch), str(prefix), str(request_path)],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True,
        )
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
            deadline: float, max_snapshot_bytes: int, max_stream_bytes: int):
    """Run one native capture using unchanged owner request and held snapshots.

    ``snapshots`` maps native database fields to already selected, caller-held
    transactions (including uncommitted views). No BEGIN/COMMIT/rollback is
    performed on them. SQLite serialize support is required; no backup or
    pathname reopening is a fallback. Native semantic validation still owns
    the serialized input, and WAL/dirty-view compatibility requires its real
    controlled consumer checks.
    The caller supplies one absolute monotonic deadline and admitted copy/
    stream bounds; its whole storage envelope must also cover SQLite scratch
    and native output files. Copy bytes are not an allocator/RSS estimate.
    Scratch is an exclusively owned private namespace for this operation.
    """
    if (type(deadline) not in (int, float) or not math.isfinite(deadline)
            or type(max_snapshot_bytes) is not int or max_snapshot_bytes <= 0
            or type(max_stream_bytes) is not int or max_stream_bytes <= 0):
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
            # Emitter custody requires an auxiliary manifest, but the maintained
            # API publishes only its selected SQL targets and returned receipt.
            # This actual native output stays in the exclusive transport scope.
            selected['manifest_json'] = str(working / 'manifest.json')
            remaining = max_snapshot_bytes
            for index, (field, connection) in enumerate(snapshots.items()):
                path = working / f'snapshot-{index}.sqlite'
                fd, stamp = _snapshot(connection, path, remaining, deadline)
                held.append((fd, path, stamp, connection))
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
                            named.st_mtime_ns, named.st_ctime_ns) != stamp):
                    raise ValueError('native Edge capture snapshot custody changed')
            value = json.loads(result, object_pairs_hook=_unique)
            if type(value) is not dict or value.get('schema') != 'tos_edge_offline_capture_result_v1':
                raise ValueError('native Edge capture result profile differs')
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
