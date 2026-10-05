"""One Linux installed-software stdin/stdout owner; no ToS semantic rules.

The leader stays unreaped through all guarded group signals. Only this owner
performs the final wait; readers/writers never poll or reap the process.
An unverifiable ownership error refuses cleanup; it never claims terminal custody.
No explicit PID/group action follows that loss. Standard Popen destruction may
subsequently perform its own nonblocking reap without any group signal.
"""
from __future__ import annotations

from contextlib import contextmanager
import errno
import json
import math
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import threading
import time


class NativeCancelled(InterruptedError):
    pass


class NativeCustodyError(RuntimeError):
    def custody_snapshot(self):
        """Bounded immutable facts; never grants numeric signal/reap authority."""
        owner = getattr(self, '_owner', None)
        if owner is None:
            return {'child_released': False, 'owner_bound': False}
        return owner.custody_snapshot()



def _bounded_json(value, cap, deadline, cancelled=None, closing=None, receiving_state=None):
    """Bound ordinary host JSON before and during encoding, without ToS rules."""
    minimum = 0
    visits = 0
    upper = 0
    if receiving_state is not None:
        import types
        g = receiving_state.geometry
        frame = 0
        codes = [_bounded_json.__code__]
        codes += [c for c in _bounded_json.__code__.co_consts if isinstance(c, types.CodeType)]
        codes += [json.JSONEncoder.iterencode.__code__, json.encoder._make_iterencode.__code__]
        for code in codes:
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            frame += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + g.unicode_bytes(24))
        receiving_state.reserve(65 * frame + 8 * g.dict_bytes(64))
    def walk(member, depth):
        nonlocal minimum, visits, upper
        if receiving_state is not None:
            receiving_state.visit()
            if type(member) is str:
                upper += 2
            elif type(member) is int:
                upper += (member.bit_length() * 30103 // 100000) + 4
            elif type(member) in (dict, list, tuple):
                upper += 2 + 4 * len(member)
            else:
                upper += 64
            if upper > cap:
                # This is an allocation upper bound, not a semantic byte
                # refusal: clamp workspace to the maintained actual wire cap.
                upper = cap
        visits += 1
        if depth > 64 or visits > 1_000_000:
            raise ValueError('native input JSON depth/visit budget exceeded')
        if (cancelled is not None and cancelled.is_set()) or (closing is not None and closing.is_set()):
            raise NativeCancelled('native operation cancelled')
        if time.monotonic() >= deadline:
            raise TimeoutError('native input encoding deadline exceeded')
        if type(member) not in (dict, list, tuple, str, int, float, bool, type(None)):
            raise ValueError('native input requires ordinary JSON host values')
        if type(member) is str:
            if len(member) > cap - minimum:
                raise ValueError('native input JSON byte budget exceeded')
            minimum += 2
            for index, character in enumerate(member):
                point = ord(character)
                character_bytes = (2 if point in (34, 92, 8, 9, 10, 12, 13) else
                    6 if point < 32 or 0xD800 <= point <= 0xDFFF else
                    1 if point < 0x80 else 2 if point < 0x800 else
                    3 if point < 0x10000 else 4)
                minimum += character_bytes
                if receiving_state is not None:
                    upper = min(cap, upper + character_bytes)
                if minimum > cap:
                    raise ValueError('native input JSON byte budget exceeded')
                if index % 4096 == 0:
                    if (cancelled is not None and cancelled.is_set()) or (closing is not None and closing.is_set()):
                        raise NativeCancelled('native operation cancelled')
                    if time.monotonic() >= deadline:
                        raise TimeoutError('native input encoding deadline exceeded')
        else:
            minimum += 1
            if minimum > cap:
                raise ValueError('native input JSON byte budget exceeded')
        if type(member) is int and member.bit_length() > 14290:
            raise ValueError('native input integer digit budget exceeded')
        if type(member) in (dict, list, tuple):
            if len(member) > 1_000_000 - visits:
                raise ValueError('native input JSON visit budget exceeded')
            if type(member) is dict:
                for key, child in member.items():
                    walk(key, depth + 1)
                    walk(child, depth + 1)
            else:
                for child in member:
                    walk(child, depth + 1)
    # A huge individual string must refuse before iterencode allocates its
    # quoted chunk. This conservative lower bound also bounds all such chunks.
    walk(value, 0)
    if receiving_state is not None:
        g = receiving_state.geometry
        # Price actual quoted input geometry up to the existing wire cap.
        # Encoder chunks, UTF8 copies, bytearray resize and caller write copy
        # coexist. Keep this debit through terminal/traceback lifetime.
        receiving_state.reserve(2 * g.unicode_bytes(upper)
            + 4 * (g.bytes_base + upper)
            + 2 * bytearray.__basicsize__ + 2 * upper)
    payload = bytearray()
    encoder = json.JSONEncoder(ensure_ascii=False, allow_nan=False, separators=(',', ':'))
    for part in encoder.iterencode(value):
        if receiving_state is not None:
            receiving_state.visit()
        if (cancelled is not None and cancelled.is_set()) or (closing is not None and closing.is_set()):
            raise NativeCancelled('native operation cancelled')
        if time.monotonic() >= deadline:
            raise TimeoutError('native input encoding deadline exceeded')
        encoded = part.encode('utf-8', 'backslashreplace')
        if len(encoded) > cap - len(payload):
            raise ValueError('native input JSON byte budget exceeded; use the root streaming route')
        payload.extend(encoded)
    return payload



_SIGNALS = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP, signal.SIGQUIT)


class _Exchange:
    def __init__(self, arguments, prefix, input_cap, frame_cap, valid_returncodes,
                 cancelled, deadline, env, pass_fds=(), selected_image=None):
        from . import native_dispatch
        self._env = env
        self._selected_image = selected_image
        self._pass_fds = pass_fds
        self._sender_uid = os.getuid()
        self._sender_gid = os.getgid()
        self._arguments = arguments
        self._prefix = prefix
        self._dispatch = (Path(native_dispatch.__file__).resolve(strict=True)
                          if selected_image is None else None)
        self._input_cap = input_cap
        self._frame_cap = frame_cap
        self._valid_returncodes = valid_returncodes
        self._cancelled = cancelled
        self._deadline = deadline
        self._work_deadline = deadline - 5
        self._closing = threading.Event()
        self._send_lock = threading.Lock()
        self._child = None
        self._unknown = False
        self._reaped = False
        self._release_complete = False
        self._pending = bytearray()
        self._diagnostic = bytearray()
        self._readers = []
        self._terminal = None
        self._terminal_observed_ns = None

    def custody_snapshot(self):
        """Release facts only; no external signal/reap authority."""
        return {'child_released': self._release_complete, 'owner_bound': True,
                'child_pid': self._child.pid if self._child is not None else None,
                'ownership_known': not self._unknown, 'reaped': self._reaped,
                'deadline': self._deadline, 'selected_prefix': str(self._prefix),
                'operation': self._arguments[0] if self._arguments else None}

    def _active(self):
        if self._closing.is_set() or (self._cancelled is not None and self._cancelled.is_set()):
            raise NativeCancelled('owned native operation cancelled')
        if time.monotonic() >= self._work_deadline:
            raise TimeoutError('native operation deadline exceeded')

    def _open(self):
        self._active()
        previous = signal.pthread_sigmask(signal.SIG_BLOCK, _SIGNALS)
        try:
            # No preexec_fn in a threaded caller. The bootstrap restores the
            # child's exact prior mask, and binds parent death before dispatch.
            program = (
                'import importlib.util,sys,json,os,signal,ctypes;from pathlib import Path;'
                'p=int(sys.argv[5]);'
                'c=ctypes.CDLL(None,use_errno=True);'
                'r=c.prctl(1,int(signal.SIGKILL),0,0,0);'
                'r==0 or sys.exit(125);'
                'os.getppid()==p or sys.exit(125);'
                'signal.pthread_sigmask(signal.SIG_SETMASK,set(json.loads(sys.argv[4])));'
                "s=importlib.util.spec_from_file_location('native_selected_dispatch',sys.argv[1]);"
                'm=importlib.util.module_from_spec(s);s.loader.exec_module(m);'
                'm.run(Path(sys.argv[2]),json.loads(sys.argv[3]))'
            )
            if self._selected_image is None:
                self._child = subprocess.Popen(
                    [sys.executable, '-I', '-S', '-B', '-c', program, str(self._dispatch),
                     str(self._prefix), json.dumps(self._arguments),
                     json.dumps([int(sig) for sig in previous]), str(os.getpid())],
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, start_new_session=True, env=self._env,
                    pass_fds=self._pass_fds,
                )
            else:
                image_fd, label = self._selected_image
                if (type(image_fd) is not int or image_fd < 0 or type(label) is not str
                        or not self._arguments or self._arguments[0] not in ('private-stage-run', 'sdk-host-session')):
                    raise ValueError('native SDK direct image must select its owned issuer')
                # Caller holds verified_image through terminal custody. The
                # borrowed ELF is never reopened by its mutable installation
                # path, and this branch spawns no transient Python dispatcher.
                held = os.fstat(image_fd)
                mask = ','.join(str(int(sig)) for sig in sorted(previous))
                argv = [label, self._arguments[0], '--expected-parent-pid', str(os.getpid()),
                        '--restore-signal-mask', mask, *self._arguments[1:]]
                descriptors = _borrowed_fds((*self._pass_fds, image_fd))
                self._child = subprocess.Popen(argv, executable='/proc/self/fd/' + str(image_fd),
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    start_new_session=True, env=self._env, pass_fds=descriptors)
                after = os.fstat(image_fd)
                if (held.st_dev, held.st_ino, held.st_size, held.st_mtime_ns, held.st_ctime_ns) != (
                        after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                    raise ValueError('native SDK borrowed installed image changed during spawn')
            # Acquisition and stream registration finish while cancellation is
            # masked; the outer owner finally already covers this whole phase.
            for stream in (self._child.stdin, self._child.stdout, self._child.stderr):
                os.set_blocking(stream.fileno(), False)
            self._readers = [self._child.stdout, self._child.stderr]
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous)

    def _observe(self, cleanup=False):
        if self._unknown or self._reaped:
            raise NativeCustodyError('native child ownership unavailable; no explicit group signal or reap')
        while True:
            if time.monotonic() >= (self._deadline if cleanup else self._work_deadline):
                raise TimeoutError('original native custody deadline exceeded')
            try:
                observed = os.waitid(os.P_PID, self._child.pid,
                                     os.WEXITED | os.WNOHANG | os.WNOWAIT)
                if observed is not None and observed.si_pid not in (0, self._child.pid):
                    self._unknown = True
                    raise NativeCustodyError('native waitid returned a different child; ownership unverifiable')
                return None if observed is not None and observed.si_pid == 0 else observed
            except (InterruptedError, KeyboardInterrupt):
                if not cleanup:
                    raise
            except ChildProcessError as error:
                self._unknown = True
                raise NativeCustodyError('native child ownership lost; no explicit group signal or reap') from error
            except OSError as error:
                if error.errno == errno.EINTR:
                    if cleanup:
                        continue
                    raise
                self._unknown = True
                raise NativeCustodyError('native child ownership unverifiable; no explicit group signal or reap') from error
            except BaseException:
                self._unknown = True
                raise

    def write_input(self, raw, *, close=False):
        """Write bounded bytes under the same operation clock; no subprocess API."""
        if not isinstance(raw, (bytes, bytearray)) or len(raw) > self._input_cap + 1:
            raise ValueError('native request frame exceeds the byte bound')
        self._active()
        remaining = self._work_deadline - time.monotonic()
        if remaining <= 0 or not self._send_lock.acquire(timeout=remaining):
            raise TimeoutError('native writer deadline exceeded')
        try:
            view = memoryview(raw)
            while view:
                self._active()
                ready = select.select([], [self._child.stdin], [],
                    min(0.05, max(0, self._work_deadline - time.monotonic())))[1]
                if not ready:
                    continue
                try:
                    count = os.write(self._child.stdin.fileno(), view[:65536])
                except BlockingIOError:
                    continue
                if count <= 0:
                    raise OSError('native input write made no progress')
                view = view[count:]
            if close:
                self._child.stdin.close()
        finally:
            self._send_lock.release()

    def send(self, value):
        """Send one host JSON frame; SDK/domain interpretation stays with callers."""
        self._active()
        raw = _bounded_json(value, self._input_cap, self._work_deadline, self._cancelled, self._closing)
        raw.extend(b'\n')
        self.write_input(raw)

    def close_input(self):
        """Close only the owned input pipe, including after operation cancellation.

        EOF is cleanup, not another request write. The sole reader still owns
        terminal validation and reap; cancellation remains set for that reader.
        """
        remaining = self._deadline - time.monotonic()
        if remaining <= 0 or not self._send_lock.acquire(timeout=remaining):
            raise TimeoutError('native input close deadline exceeded')
        try:
            if time.monotonic() >= self._deadline:
                raise TimeoutError('native input close deadline exceeded')
            self._child.stdin.close()
        finally:
            self._send_lock.release()

    def receive_descriptors(self, peer, *, expected_count):
        """Receive an authenticated zero-or-one-FD state reply from this child.

        Return (None, marker) for expected_count=0 or (caller-owned FileIO,
        marker) for expected_count=1. The caller interprets marker bytes and
        the descriptor. SO_PASSCRED must be enabled before the child starts.
        """
        if type(expected_count) is not int or expected_count not in (0, 1):
            raise ValueError('native descriptor reply expects exactly zero or one descriptor')
        self._active()
        if self._readers or self._terminal is None:
            raise NativeCustodyError('native descriptor delivery requires complete EOF and terminal observation')
        self.finish()  # Recheck accepted terminal status without reaping.
        import array
        import io
        import socket
        import struct
        if (type(peer) is not socket.socket or peer.family != socket.AF_UNIX
                or peer.getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) != socket.SOCK_SEQPACKET
                or peer.getsockopt(socket.SOL_SOCKET, socket.SO_PASSCRED) != 1):
            raise ValueError('native descriptor delivery requires a private credentialed UNIX SEQPACKET socket')
        delivered = []
        file = None
        previous = signal.pthread_sigmask(signal.SIG_BLOCK, _SIGNALS)
        try:
            self._observe()
            data, controls, flags, address = peer.recvmsg(
                4096, socket.CMSG_SPACE(16 * array.array('i').itemsize)
                + socket.CMSG_SPACE(struct.calcsize('3i')),
                socket.MSG_DONTWAIT | socket.MSG_CMSG_CLOEXEC)
            # Register every delivered descriptor before rejecting any control
            # envelope, so malformed/multiple controls cannot leak an FD.
            for level, kind, raw in controls:
                if level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS:
                    values = array.array('i')
                    values.frombytes(raw[:len(raw) - len(raw) % values.itemsize])
                    delivered.extend(values)
            credentials = [raw for level, kind, raw in controls
                           if level == socket.SOL_SOCKET and kind == socket.SCM_CREDENTIALS]
            rights = [raw for level, kind, raw in controls
                      if level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS]
            expected_controls = 1 + expected_count
            envelope_matches = (
                expected_count == 0 and not rights and not delivered
                or expected_count == 1 and len(rights) == 1
                and len(rights[0]) == array.array('i').itemsize and len(delivered) == 1
            )
            if (flags & (socket.MSG_TRUNC | socket.MSG_CTRUNC)
                    or len(controls) != expected_controls or len(credentials) != 1
                    or len(credentials[0]) != struct.calcsize('3i') or not envelope_matches):
                raise NativeCustodyError('native descriptor envelope differs')
            pid, uid, gid = struct.unpack('3i', credentials[0])
            if pid != self._child.pid or uid != self._sender_uid or gid != self._sender_gid:
                raise NativeCustodyError('native descriptor sender differs from the held leader')
            self._observe()
            self._active()
            if expected_count == 0:
                return None, data
            file = io.FileIO(delivered[0], mode='rb', closefd=True)
            delivered.clear()
            return file, data
        except BaseException as primary:
            if file is not None:
                try:
                    file.close()
                except BaseException as error:
                    primary.add_note(f'native received descriptor close failed: {type(error).__name__}')
            for fd in delivered:
                try:
                    os.close(fd)
                except BaseException as error:
                    primary.add_note(f'native delivered descriptor close failed: {type(error).__name__}')
            raise
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous)

    def receive_descriptor(self, peer):
        """Backward-compatible one-FD adapter for existing selected providers."""
        return self.receive_descriptors(peer, expected_count=1)

    def poll_control_session(self):
        """Drain bounded diagnostics without parsing a second output protocol."""
        self._active()
        for reader in select.select(self._readers, [], [], 0)[0]:
            try:
                chunk = os.read(reader.fileno(), 65536)
            except BlockingIOError:
                continue
            if not chunk:
                self._readers.remove(reader)
                continue
            if reader is self._child.stdout:
                raise ValueError('native control session produced unexpected stdout')
            if len(chunk) > 65536 - len(self._diagnostic):
                raise ValueError('native diagnostic exceeds the byte bound')
            self._diagnostic.extend(chunk)
        self._observe()

    def finish_control_session(self):
        """After close ACK, authenticate real EOF and cleanup terminal by whole cutoff.

        Query work is finished; this observes the original reserved shutdown
        interval without admitting another query or renewing either clock.
        """
        while self._readers or self._terminal is None:
            if self._cancelled is not None and self._cancelled.is_set():
                raise NativeCancelled('native control session owner cancelled')
            if time.monotonic() >= self._deadline:
                raise TimeoutError('native control session original cleanup cutoff expired')
            ready = select.select(self._readers, [], [],
                min(0.05, max(0, self._deadline - time.monotonic())))[0]
            for reader in ready:
                try:
                    chunk = os.read(reader.fileno(), 65536)
                except BlockingIOError:
                    continue
                if not chunk:
                    self._readers.remove(reader)
                elif reader is self._child.stdout:
                    raise ValueError('native control session produced unexpected stdout')
                else:
                    if len(chunk) > 65536 - len(self._diagnostic):
                        raise ValueError('native diagnostic exceeds the byte bound')
                    self._diagnostic.extend(chunk)
            self._terminal = self._observe(cleanup=True)
            if self._terminal is not None and self._terminal_observed_ns is None:
                self._terminal_observed_ns = time.monotonic_ns()
            if not self._readers and self._terminal is None:
                time.sleep(min(0.01, max(0, self._deadline - time.monotonic())))
        status = (self._terminal.si_status if self._terminal.si_code == os.CLD_EXITED
                  else -self._terminal.si_status)
        if status not in self._valid_returncodes:
            raise ValueError(self._diagnostic.decode('utf-8', 'replace').strip()
                             or 'native control session terminal refused')
        self._observe(cleanup=True)
        return status

    def frames(self):
        """Yield bounded JSONL bytes, draining bounded stderr in the same reader."""
        while self._readers:
            self._active()
            ready = select.select(self._readers, [], [],
                min(0.05, max(0, self._work_deadline - time.monotonic())))[0]
            for reader in ready:
                try:
                    chunk = os.read(reader.fileno(), 65536)
                except BlockingIOError:
                    continue
                if not chunk:
                    self._readers.remove(reader)
                    if reader is self._child.stdout and self._pending:
                        raise ValueError('native stream ended inside a frame')
                    continue
                if reader is self._child.stderr:
                    if len(chunk) > 65536 - len(self._diagnostic):
                        raise ValueError('native diagnostic exceeds the byte bound')
                    self._diagnostic.extend(chunk)
                    continue
                self._pending.extend(chunk)
                while b'\n' in self._pending:
                    line, _, rest = self._pending.partition(b'\n')
                    if len(line) > self._frame_cap:
                        raise ValueError('native response frame exceeds the byte bound')
                    self._pending = bytearray(rest)
                    yield bytes(line)
                if len(self._pending) > self._frame_cap:
                    raise ValueError('native response frame exceeds the byte bound')
        self.finish()

    def finish(self):
        """Observe terminal status without releasing the leader's PID/PGID."""
        if self._readers:
            raise NativeCustodyError('native stream EOF not observed')
        while self._terminal is None:
            self._active()
            self._terminal = self._observe()
            if self._terminal is not None:
                self._terminal_observed_ns = time.monotonic_ns()
            if self._terminal is None:
                time.sleep(min(0.01, max(0, self._work_deadline - time.monotonic())))
        status = (self._terminal.si_status if self._terminal.si_code == os.CLD_EXITED
                  else -self._terminal.si_status)
        if status not in self._valid_returncodes:
            raise ValueError(self._diagnostic.decode('utf-8', 'replace').strip()
                             or 'native operation refused the input or resource envelope')
        return status

    def terminal_observed_ns(self):
        """First authenticated terminal observation, not the actual exit time."""
        if (self._readers or self._terminal is None or self._terminal_observed_ns is None
                or self._unknown or self._reaped):
            raise NativeCustodyError('accepted unreaped native terminal observation unavailable')
        self.finish()
        self._observe()
        return self._terminal_observed_ns

    def _release(self):
        self._closing.set()
        if self._child is None:
            return
        previous = signal.pthread_sigmask(signal.SIG_BLOCK, _SIGNALS)
        failure = None
        notes = []
        try:
            if not self._unknown and not self._reaped:
                for sig in (signal.SIGTERM, signal.SIGKILL):
                    if self._unknown or time.monotonic() >= self._deadline:
                        break
                    try:
                        # A fresh matching WNOWAIT custody check precedes EVERY
                        # group signal; a recoverable error does not skip KILL.
                        self._observe(cleanup=True)
                        try:
                            os.killpg(self._child.pid, sig)
                        except ProcessLookupError:
                            pass
                    except BaseException as error:
                        notes.append(f'{sig.name}: {type(error).__name__}')
                        failure = failure or error
                # The final possible anchored signal precedes the sole wait.
                # Unknown custody never permits a numeric reap.
                if not self._unknown:
                    try:
                        remaining = self._deadline - time.monotonic()
                        if remaining <= 0:
                            raise TimeoutError('no remaining native reap budget')
                        self._child.wait(timeout=remaining)
                        self._reaped = True
                    except BaseException as error:
                        notes.append(f'wait: {type(error).__name__}')
                        failure = failure or error
            else:
                notes.append('ownership unavailable')
                failure = NativeCustodyError('native child ownership lost; cleanup cannot target a recycled group')
            # A writer cannot use a recycled descriptor while close runs.
            remaining = self._deadline - time.monotonic()
            acquired = remaining > 0 and self._send_lock.acquire(timeout=remaining)
            if not acquired:
                notes.append('writer pipe custody unavailable')
                failure = failure or NativeCustodyError('native writer did not release pipe custody')
            try:
                for stream in (self._child.stdout, self._child.stderr,
                               self._child.stdin if acquired else None):
                    try:
                        if stream is not None:
                            stream.close()
                    except BaseException as error:
                        notes.append(f'pipe close: {type(error).__name__}')
                        failure = failure or error
            finally:
                if acquired:
                    self._send_lock.release()
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous)
        if failure is not None:
            raise NativeCustodyError('owned native child cleanup failed: ' + '; '.join(notes)) from failure


def _caller_deadline(absolute_deadline, operation_seconds=50):
    start = time.monotonic()
    try:
        finite_span = (type(operation_seconds) in (int, float)
                       and math.isfinite(operation_seconds))
    except OverflowError:
        finite_span = False
    if not finite_span or operation_seconds <= 5:
        raise ValueError('native operation needs a finite selected span greater than its 5s cleanup reserve')
    selected_deadline = start + operation_seconds
    if not math.isfinite(selected_deadline):
        raise ValueError('native selected deadline addition is not finite')
    try:
        finite_absolute = (type(absolute_deadline) in (int, float)
                           and math.isfinite(absolute_deadline))
    except OverflowError:
        finite_absolute = False
    if absolute_deadline is not None and (not finite_absolute or absolute_deadline <= start):
        raise ValueError('native caller needs a finite remaining absolute deadline')
    return min(selected_deadline, absolute_deadline) if absolute_deadline is not None else selected_deadline


def _contract(arguments, input_cap, frame_cap, valid_returncodes):
    if (type(input_cap) is not int or not 1 <= input_cap <= 16 * 1024 * 1024
            or type(frame_cap) is not int or not 1 <= frame_cap <= 64 * 1024 * 1024
            or not isinstance(arguments, (list, tuple)) or len(arguments) > 1024
            or any(type(arg) is not str or '\0' in arg for arg in arguments)
            or sum(len(arg) for arg in arguments) > 1048576
            or not isinstance(valid_returncodes, (list, tuple))
            or not 1 <= len(valid_returncodes) <= 256
            or any(type(code) is not int or not -255 <= code <= 255 for code in valid_returncodes)):
        raise ValueError('native exchange exceeds the selected host frame contract')


def _environment(env):
    if env is None:
        return None
    if type(env) is not dict or len(env) > 4096:
        raise ValueError('native environment must be a bounded ordinary string mapping')
    total = 0
    result = {}
    for key, value in env.items():
        if (type(key) is not str or type(value) is not str or not key
                or '=' in key or '\0' in key or '\0' in value):
            raise ValueError('native environment requires valid string keys and values')
        total += len(key) + len(value) + 2
        if total > 1048576:
            raise ValueError('native environment byte budget exceeded')
        total += len(os.fsencode(key)) - len(key) + len(os.fsencode(value)) - len(value)
        if total > 1048576:
            raise ValueError('native environment byte budget exceeded')
        result[key] = value
    return result


def _borrowed_fds(value):
    # Descriptor roles and their authority belong to the selecting caller.
    # This transport borrows exact open FDs; it never closes parent copies.
    if (type(value) is not tuple or len(value) > 16
            or any(type(fd) is not int or fd < 3 for fd in value)
            or len(set(value)) != len(value)):
        raise ValueError('native pass_fds requires at most 16 distinct descriptor integers >=3')
    for fd in value:
        os.fstat(fd)
    return value


@contextmanager
def owned_exchange(arguments, *, prefix=None, input_cap=16 * 1024 * 1024,
                   frame_cap=4 * 1024 * 1024, valid_returncodes=(0,),
                   cancelled=None, absolute_deadline=None, env=None, pass_fds=(),
                   operation_seconds=50, selected_image=None):
    """One synchronous pipe owner, usable from a caller-owned async worker.

    Only the channel is exposed; child PID/Popen/reap stay private to this owner.
    The caller owns domain interpretation and joins its SDK workers before exit.
    Long operations require the selecting caller's explicit admitted finite span;
    it is a transport envelope, not a domain budget or host execution grant.
    """
    deadline = _caller_deadline(absolute_deadline, operation_seconds)
    _contract(arguments, input_cap, frame_cap, valid_returncodes)
    environment = _environment(env)
    descriptors = _borrowed_fds(pass_fds)
    selected = prefix or os.environ.get('TOS_NATIVE_PREFIX')
    if not selected:
        raise ValueError('native operation requires installed Rust software: set TOS_NATIVE_PREFIX or --native-prefix')
    channel = _Exchange(list(arguments), selected, input_cap, frame_cap,
                        tuple(valid_returncodes), cancelled, deadline, environment, descriptors, selected_image)
    primary = None
    try:
        channel._open()
        yield channel
    except BaseException as error:
        primary = error
        raise
    finally:
        try:
            channel._release()
            channel._release_complete = True
        except BaseException as error:
            if isinstance(error, NativeCustodyError):
                # Explicit retained native owner, not incidental traceback.
                # External holders receive facts only and must not reap/signal.
                error._owner = channel
            raise error from primary


def native_packets(arguments, value=None, *, prefix=None, input_cap=16 * 1024 * 1024,
                   frame_cap=4 * 1024 * 1024, valid_returncodes=(0,),
                   cancelled=None, absolute_deadline=None, env=None, pass_fds=(),
                   operation_seconds=50):
    """Thin output-only JSON facade over the same owned stdin/stdout channel."""
    deadline = _caller_deadline(absolute_deadline, operation_seconds)
    _contract(arguments, input_cap, frame_cap, valid_returncodes)
    environment = _environment(env)
    descriptors = _borrowed_fds(pass_fds)
    selected = prefix or os.environ.get('TOS_NATIVE_PREFIX')
    if not selected:
        raise ValueError('native operation requires installed Rust software: set TOS_NATIVE_PREFIX or --native-prefix')
    payload = None if value is None else _bounded_json(value, input_cap, deadline - 5, cancelled)
    failures = []
    writer = None
    with owned_exchange(arguments, prefix=selected, input_cap=input_cap,
                        frame_cap=frame_cap, valid_returncodes=valid_returncodes,
                        cancelled=cancelled, absolute_deadline=deadline, env=environment,
                        pass_fds=descriptors, operation_seconds=operation_seconds) as channel:
        def write_input():
            try:
                channel.write_input(payload or b'', close=True)
            except BaseException as error:
                failures.append(error)
        try:
            writer = threading.Thread(target=write_input, name='tos-native-input')
            writer.start()
            for frame in channel.frames():
                yield json.loads(frame)
            if failures:
                raise failures[0]
        finally:
            # Cancellation wakes a blocked input write before owner cleanup.
            channel._closing.set()
            if writer is not None:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError('native input worker join budget expired')
                writer.join(timeout=remaining)
                if writer.is_alive():
                    raise NativeCustodyError('native input worker did not join')
