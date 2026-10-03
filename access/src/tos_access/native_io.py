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
    pass


def _bounded_json(value, cap, deadline, cancelled=None, closing=None):
    """Bound ordinary host JSON before and during encoding, without ToS rules."""
    minimum = 0
    visits = 0
    def walk(member, depth):
        nonlocal minimum, visits
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
                minimum += (2 if point in (34, 92, 8, 9, 10, 12, 13) else
                    6 if point < 32 or 0xD800 <= point <= 0xDFFF else
                    1 if point < 0x80 else 2 if point < 0x800 else
                    3 if point < 0x10000 else 4)
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
    payload = bytearray()
    encoder = json.JSONEncoder(ensure_ascii=False, allow_nan=False, separators=(',', ':'))
    for part in encoder.iterencode(value):
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
                 cancelled, deadline, env):
        from . import native_dispatch
        self._env = env
        self._arguments = arguments
        self._prefix = prefix
        self._dispatch = Path(native_dispatch.__file__).resolve(strict=True)
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
        self._pending = bytearray()
        self._diagnostic = bytearray()
        self._readers = []
        self._terminal = None

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
            self._child = subprocess.Popen(
                [sys.executable, '-I', '-S', '-B', '-c', program, str(self._dispatch),
                 str(self._prefix), json.dumps(self._arguments),
                 json.dumps([int(sig) for sig in previous]), str(os.getpid())],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, start_new_session=True, env=self._env,
            )
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
        self.write_input(b'', close=True)

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
            if self._terminal is None:
                time.sleep(min(0.01, max(0, self._work_deadline - time.monotonic())))
        status = (self._terminal.si_status if self._terminal.si_code == os.CLD_EXITED
                  else -self._terminal.si_status)
        if status not in self._valid_returncodes:
            raise ValueError(self._diagnostic.decode('utf-8', 'replace').strip()
                             or 'native operation refused the input or resource envelope')
        return status

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


def _caller_deadline(absolute_deadline):
    start = time.monotonic()
    if absolute_deadline is not None and (type(absolute_deadline) not in (int, float)
            or not math.isfinite(absolute_deadline) or absolute_deadline <= start):
        raise ValueError('native caller needs a finite remaining absolute deadline')
    return min(start + 50, absolute_deadline) if absolute_deadline is not None else start + 50


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


@contextmanager
def owned_exchange(arguments, *, prefix=None, input_cap=16 * 1024 * 1024,
                   frame_cap=4 * 1024 * 1024, valid_returncodes=(0,),
                   cancelled=None, absolute_deadline=None, env=None):
    """One synchronous pipe owner, usable from a caller-owned async worker.

    Only the channel is exposed; child PID/Popen/reap stay private to this owner.
    The caller owns domain interpretation and joins its SDK workers before exit.
    """
    deadline = _caller_deadline(absolute_deadline)
    _contract(arguments, input_cap, frame_cap, valid_returncodes)
    environment = _environment(env)
    selected = prefix or os.environ.get('TOS_NATIVE_PREFIX')
    if not selected:
        raise ValueError('native operation requires installed Rust software: set TOS_NATIVE_PREFIX or --native-prefix')
    channel = _Exchange(list(arguments), selected, input_cap, frame_cap,
                        tuple(valid_returncodes), cancelled, deadline, environment)
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
        except BaseException as error:
            raise error from primary


def native_packets(arguments, value=None, *, prefix=None, input_cap=16 * 1024 * 1024,
                   frame_cap=4 * 1024 * 1024, valid_returncodes=(0,),
                   cancelled=None, absolute_deadline=None, env=None):
    """Thin output-only JSON facade over the same owned stdin/stdout channel."""
    deadline = _caller_deadline(absolute_deadline)
    _contract(arguments, input_cap, frame_cap, valid_returncodes)
    environment = _environment(env)
    selected = prefix or os.environ.get('TOS_NATIVE_PREFIX')
    if not selected:
        raise ValueError('native operation requires installed Rust software: set TOS_NATIVE_PREFIX or --native-prefix')
    payload = None if value is None else _bounded_json(value, input_cap, deadline - 5, cancelled)
    failures = []
    writer = None
    with owned_exchange(arguments, prefix=selected, input_cap=input_cap,
                        frame_cap=frame_cap, valid_returncodes=valid_returncodes,
                        cancelled=cancelled, absolute_deadline=deadline, env=environment) as channel:
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
