"""Returning transport for the installed offline Edge capture operation.

The caller owns source/pair admission, its read transactions, catalog/binding
selection and output targets. Rust freezes those borrowed views through bounded exact-engine callbacks;
this bridge retains their lifetime
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
# At most two metadata files: 16 KiB aggregate, independent of stream bounds.
_EVIDENCE_METADATA_BYTES = 8 * 1024
_SNAPSHOT_ORDER = (
    'd1_database', 'before_prepared_database', 'after_prepared_database',
)
_SNAPSHOT_FIELDS = frozenset(_SNAPSHOT_ORDER)

@dataclass(frozen=True)
class NativeCaptureContext:
    """Explicit installed software and finite transport selection.

    This selects software and transport resources, never source admission.
    The deadline is the caller's original absolute monotonic deadline.
    Linux child parent-death guards preserve caller handlers; the whole owner
    supervisor owns descendants and scratch on abrupt caller termination.
    max_query_transport_bytes bounds aggregate helper RPC IO; when omitted,
    it is the original schema allocation cap (or snapshot cap times 128). RPC
    adapter allocation and Rust planning debit that same schema ledger.
    max_snapshot_bytes bounds aggregate encoded logical frames, including
    descriptors and hashes; it is not a SQLite page-image size estimate.
    """
    prefix: Path
    scratch: Path
    deadline: float
    max_snapshot_bytes: int
    max_stream_bytes: int
    max_schema_allocation_bytes: int | None = None
    max_query_transport_bytes: int | None = None

    def run(self, request, snapshots):
        return capture(self.prefix, request, snapshots, scratch=self.scratch,
                       deadline=self.deadline,
                       max_snapshot_bytes=self.max_snapshot_bytes,
                       max_stream_bytes=self.max_stream_bytes,
                       max_schema_allocation_bytes=self.max_schema_allocation_bytes,
                       max_query_transport_bytes=self.max_query_transport_bytes)


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


def _live_group(pgid, deadline):
    for entry in Path('/proc').iterdir():
        _active(deadline)
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
                _active(end)
                observed = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                if observed is not None and observed.si_pid not in (0, child.pid):
                    raise CaptureCustodyError('owned child observation identity differs')
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
            _active(end)
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            pass
        except BaseException:
            errors.append('group signal failed')
        phase = min(end, time.monotonic() + allowance)
        try:
            while time.monotonic() < phase and _live_group(child.pid, end):
                time.sleep(min(0.02, max(0, phase - time.monotonic())))
        except BaseException:
            errors.append('group census failed')
    try:
        if not ownership_lost and _live_group(child.pid, end):
            errors.append('owned group remains live')
    except BaseException:
        errors.append('final group census failed')
    try:
        # No unconditional wait: the original caller deadline owns reap too.
        if not ownership_lost:
            remaining = end - time.monotonic()
            if remaining <= 0:
                raise CaptureCustodyError('owned child original reap deadline expired')
            child.wait(timeout=remaining)
    except BaseException:
        errors.append('owned child remains unreaped')
    if errors:
        raise CaptureCustodyError('; '.join(errors))


def _write_private_evidence(fd, raw, deadline, counts):
    view = memoryview(raw)
    while view:
        _active(deadline)
        counts['write_attempted'] += len(view)
        written = os.write(fd, view)
        if written <= 0:
            raise CaptureCustodyError('private evidence write made no progress')
        counts['write_returned'] += written
        view = view[written:]


def _write_private_metadata(path, value, deadline):
    # Bound each chunk before encoding and the aggregate before retaining it.
    chunks = []
    used = 0
    for chunk in json.JSONEncoder(ensure_ascii=True, allow_nan=False,
                                  separators=(',', ':')).iterencode(value):
        _active(deadline)
        if len(chunk) > _EVIDENCE_METADATA_BYTES - used:
            raise CaptureCustodyError('private evidence metadata exceeds byte bound')
        raw = chunk.encode('ascii')
        chunks.append(raw)
        used += len(raw)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        counts = {'write_attempted': 0, 'write_returned': 0}
        for raw in chunks:
            _write_private_evidence(fd, raw, deadline, counts)
        _active(deadline)
        os.fsync(fd)
        _active(deadline)
    finally:
        primary = sys.exception()
        try:
            os.close(fd)
        except BaseException as error:
            if primary is not None:
                primary.add_note('private metadata descriptor close failed')
                raise primary from error
            raise


def _observe(prefix, request_path, deadline, stream_bytes, evidence_directory):
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
    evidence = {}
    hashes = {name: hashlib.sha256() for name in captured}
    eof = {name: False for name in captured}
    evidence_io = {name: {'write_attempted': 0, 'write_returned': 0}
                   for name in captured}
    primary = None
    cleanup_error = None
    operation_end = deadline - 5
    try:
        _active(operation_end)
        for name in captured:
            evidence[name] = os.open(evidence_directory / (name + '.bin'),
                                     os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                                     0o600)
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
                    eof[key.data] = True
                    selector.unregister(key.fileobj)
                    continue
                captured[key.data].extend(data)
                hashes[key.data].update(data)
                _write_private_evidence(evidence[key.data], data, operation_end,
                                        evidence_io[key.data])
                if len(captured[key.data]) > stream_bytes:
                    raise ValueError('native Edge capture response exceeds selected stream budget')
        while True:
            _active(operation_end)
            terminal = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            if terminal is not None and terminal.si_pid != 0:
                if terminal.si_pid != child.pid:
                    raise CaptureCustodyError('native Edge child ownership differs')
                if terminal.si_status:
                    status = terminal.si_status if terminal.si_code == os.CLD_EXITED else -terminal.si_status
                    raise RuntimeError(f'native Edge capture failed: status={status}; '
                                       f'stdout_bytes={len(captured["stdout"])}; '
                                       f'stderr_bytes={len(captured["stderr"])}')
                break
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
        stream_facts = {}
        for name, fd in evidence.items():
            try:
                facts = os.fstat(fd)
                named = (evidence_directory / (name + '.bin')).stat(follow_symlinks=False)
                if (facts.st_dev, facts.st_ino, facts.st_mode, facts.st_size,
                        facts.st_mtime_ns, facts.st_ctime_ns) != (
                        named.st_dev, named.st_ino, named.st_mode, named.st_size,
                        named.st_mtime_ns, named.st_ctime_ns):
                    raise CaptureCustodyError('private stream evidence custody changed')
                stream_facts[name] = dict(
                    file=name + '.bin', read_bytes=len(captured[name]),
                    read_sha256=hashes[name].hexdigest(), eof=eof[name],
                    stored_bytes=facts.st_size, stamp=[facts.st_dev, facts.st_ino,
                    facts.st_mode, facts.st_size, facts.st_mtime_ns, facts.st_ctime_ns],
                    **evidence_io[name])
            except BaseException as error:
                cleanup_error = cleanup_error or error
            finally:
                try:
                    os.close(fd)
                except BaseException as error:
                    cleanup_error = cleanup_error or error
        try:
            _write_private_metadata(evidence_directory / 'transport.json', {
                'schema': 'tos_edge_private_transport_observation_v1',
                'streams': stream_facts,
                'child_returncode': child.returncode if child is not None else None,
                'primary_class': type(primary).__name__[:64] if primary is not None else None,
                'cleanup_class': type(cleanup_error).__name__[:64] if cleanup_error is not None else None,
            }, deadline)
        except BaseException as error:
            cleanup_error = cleanup_error or error
    if cleanup_error is not None:
        if primary is not None:
            primary.add_note('private Edge cleanup or evidence write failed')
            raise primary from cleanup_error
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
            max_schema_allocation_bytes: int | None = None,
            max_query_transport_bytes: int | None = None):
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
    Its admitted SPACE must additionally cover two private stream files of
    at most ``max_stream_bytes + 1`` each and at most 16 KiB metadata. Streams
    are written under the original observation deadline. Failures after the
    private scope is created retain that scope and whatever request, frames,
    and stream output were materialized for owner diagnosis. Cleanup failure may leave partial metadata; no durability or
    complete evidence is claimed when its bounded write could not finish.
    Successful capture removes it. No native payload enters exception text.
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
    phase = 'working-directory'
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
            phase = 'typed-snapshots'
            snapshot_inventory = []
            from .native_edge_borrowed import snapshots_native
            schema_bytes = (max_schema_allocation_bytes if max_schema_allocation_bytes is not None
                            else max_snapshot_bytes * 128)
            rpc_bytes = schema_bytes if max_query_transport_bytes is None else max_query_transport_bytes
            held, snapshot_inventory, remaining_schema = snapshots_native(prefix, snapshots, working, deadline,
                max_snapshot_bytes, schema_bytes, rpc_bytes)
            selected['snapshot_schema_max_allocation_bytes'] = remaining_schema
            for (fd, path, stamp, connection), field in zip(held,
                    (field for field in _SNAPSHOT_ORDER if field in snapshots)):
                selected[field] = str(path)
            request_path = working / 'request.json'
            phase = 'request'
            request_complete = False
            fd = os.open(request_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, 'wb') as stream:
                encoder = json.JSONEncoder(ensure_ascii=False, allow_nan=False,
                                           separators=(',', ':'))
                request_bytes = 0
                request_hash = hashlib.sha256()
                for chunk in encoder.iterencode(selected):
                    _active(deadline)
                    raw = chunk.encode('utf-8')
                    if len(raw) > _REQUEST_BYTES - request_bytes:
                        raise ValueError('native Edge capture request exceeds native byte budget')
                    request_bytes += len(raw)
                    request_hash.update(raw)
                    stream.write(raw)
                del raw, chunk
            request_complete = True
            phase = 'native-observation'
            result = _observe(prefix, request_path, deadline, max_stream_bytes, working)
            phase = 'snapshot-currentness'
            for fd, path, stamp, connection in held:
                named = path.stat(follow_symlinks=False)
                if (_identity(fd) != stamp or not connection.in_transaction
                        or (named.st_dev, named.st_ino, named.st_size,
                            named.st_mtime_ns, named.st_ctime_ns, named.st_mode) != stamp):
                    raise ValueError('native Edge capture snapshot custody changed')
            phase = 'result-decoding'
            value = json.loads(result, object_pairs_hook=_unique)
            if type(value) is not dict or value.get('schema') != 'tos_edge_offline_capture_result_v2':
                raise ValueError('native Edge capture result profile differs')
            receipt = value.get('receipt')
            phase = 'receipt-binding'
            if (type(receipt) is not dict or receipt.get('snapshot_transport') != {
                    'schema': 'tos_edge_typed_snapshot_inventory_v1',
                    'snapshots': snapshot_inventory}):
                raise ValueError('native Edge imported snapshot evidence differs')
            _active(deadline)
        except BaseException as error:
            retain = True
            if isinstance(error, CaptureCustodyError) and hasattr(error, 'borrowed_custody'):
                held = error.borrowed_custody.get('descriptors', held)
                error.borrowed_custody['scratch_root_fd'] = root_fd
            error.add_note(f'private Edge evidence retained at {working}')
            try:
                _write_private_metadata(working / 'capture-failure.json', {
                    'schema': 'tos_edge_private_capture_failure_v1',
                    'primary_class': type(error).__name__[:64],
                    'phase': phase,
                    'request_file': 'request.json',
                    'request_bytes': request_bytes if 'request_bytes' in locals() else None,
                    'request_complete': request_complete if 'request_complete' in locals() else False,
                    'request_sha256': request_hash.hexdigest() if locals().get('request_complete') else None,
                    'snapshot_inventory': snapshot_inventory,
                }, deadline)
            except BaseException:
                error.add_note('private Edge failure metadata could not be completed')
            raise
    finally:
        cleanup_errors = []
        unproven = isinstance(sys.exception(), CaptureCustodyError) and hasattr(sys.exception(), 'borrowed_custody')
        for fd, *_ in (() if unproven else held):
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
            if not unproven:
                try:
                    os.close(root_fd)
                except OSError:
                    cleanup_errors.append('scratch directory descriptor close failed')
        if cleanup_errors and sys.exception() is not None:
            sys.exception().add_note('; '.join(cleanup_errors))
        elif cleanup_errors:
            raise CaptureCustodyError('; '.join(cleanup_errors))
    _active(deadline)
    return value


def __getattr__(name):
    # Retained receiving fixtures use these historical imports explicitly.
    # This compatibility lookup is never a production capture fallback.
    if name in {'_logical_write', '_logical_plan', '_logical_rows', '_D1_TABLES', '_snapshot'}:
        from . import native_edge_capture_oracle
        return getattr(native_edge_capture_oracle, name)
    raise AttributeError(name)
