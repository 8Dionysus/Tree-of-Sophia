"""Owned native child session; control bytes never transfer stage/model authority.

The maintained Rust sdk-python-run bootstrap places this SDK in its original
setup scope before threads. Configuration records selection, not a grant.
The caller owns already-admitted encoded JSON and receiver buffers; this owner
keeps process/socket/cgroup custody through close ACK, EOF and terminal status.
"""
from __future__ import annotations

from contextlib import ExitStack, contextmanager
from dataclasses import dataclass
import json
import os
from pathlib import Path
import resource
import socket
import stat
import threading
import time

from . import native_dispatch, native_io
from .native_core_session_control import NativeSessionControl, NativeSessionLimits

_SETUP = 536870912
_CONSUMER = 2684354560
_AGGREGATE = 3221225472


class NativeSessionRefused(ValueError):
    """Authentic bounded native refusal; message/code remain borrowed values."""
    __slots__ = ('code',)

    @classmethod
    def from_envelope(cls, envelope, state, schema):
        if (type(envelope) is not dict or set(envelope) != {'schema_version', 'ok', 'error', 'code'}
                or envelope.get('schema_version') != schema or envelope.get('ok') is not False
                or type(envelope['error']) is not str or not 0 < len(envelope['error']) <= 4096
                or type(envelope['code']) is not str or not 0 < len(envelope['code']) <= 128):
            raise ValueError('native SDK refusal envelope differs')
        g = state.geometry
        state.reserve(cls.__basicsize__ + g.gc_header + g.tuple_base + 2 * g.pointer)
        state.active()
        refusal = cls(envelope['error'])
        refusal.code = envelope['code']
        return refusal


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('repeated native SDK configuration field')
        result[key] = value
    return result


def _path(value, receiving_state=None):
    if receiving_state is not None:
        _path_workspace(value, receiving_state)
    if isinstance(value, Path):
        p = value  # genuine already-selected owner, not another Path copy
        raw = os.fspath(value)
    elif type(value) is str:
        raw = value
        p = Path(value)
    else:
        raise ValueError('native SDK bounded absolute path required')
    if len(raw.encode()) > 4096:
        raise ValueError('native SDK bounded absolute path required')
    if not p.is_absolute() or any(v in ('..', '.') for v in p.parts):
        raise ValueError('native SDK canonical path required')
    return p


def _path_workspace(value, state):
    g = state.geometry
    if isinstance(value, Path):
        parts = getattr(value, '_tail_cached', None)
        if parts is None:
            parts = getattr(value, '_parts', None)
        if type(parts) is not list:
            raise ValueError('native SDK selected Path parsed-cache ABI unavailable')
        characters = len(parts) + 2
        for part in parts:
            state.visit()
            characters += len(part)
        if characters > 4098:
            raise ValueError('native SDK selected Path original bound differs')
        # The Path/cache list is borrowed, already counted by the factory.
        # Formatting can create one cached string; .parts creates a tuple on
        #3.12+, and UTF8 length validation creates its own byte buffer.
        forecast = (g.unicode_bytes(characters) if getattr(value, '_str', None) is None else 0)
        forecast += (g.tuple_base + len(parts) * g.pointer
                     + g.bytes_base + 4 * characters)
        state.reserve(forecast)
        return
    if type(value) is not str or len(value) > 4096:
        raise ValueError('native SDK bounded absolute path required')
    components = 1
    for character in value:
        state.visit()
        if character == '/':
            components += 1
    # Real pathlib split/filter old+new tail lists, component Unicode owners,
    # cached parts tuple and formatted string. Existing input is borrowed.
    forecast = (Path.__basicsize__ + g.gc_header + g.dict_bytes(8)
                + 3 * g.list_bytes(components) + g.tuple_base + components * g.pointer
                + components * g.unicode_base + 4 * (len(value) + components)
                + g.unicode_bytes(len(value)) + g.bytes_base + 4 * len(value))
    state.reserve(forecast)


@dataclass(frozen=True)
class NativeSDKSearchCacheConfiguration:
    path: Path
    source_root: Path
    max_build_bytes: int
    max_temp_bytes: int

    def __post_init__(self):
        if any(not isinstance(value, Path) or not value.is_absolute()
               or any(part in ('.', '..') for part in value.parts)
               for value in (self.path, self.source_root)):
            raise ValueError('native SDK search cache paths must be canonical absolute paths')
        if any(type(value) is not int or not 0 < value < 1 << 64
               for value in (self.max_build_bytes, self.max_temp_bytes)):
            raise ValueError('native SDK search cache byte selectors must be positive u64 values')


@dataclass(frozen=True)
class NativeSDKStageConfiguration:
    setup_cgroup: Path
    consumer_cgroup: Path
    scratch_parent: Path
    unshare_exe: Path
    original_whole_deadline_ns: int
    original_work_deadline_ns: int
    persistent_store: Path | None
    setup_as_bytes: int | None = None
    guardian_state_bytes: int | None = None
    search_cache: NativeSDKSearchCacheConfiguration | None = None

    @classmethod
    def from_bootstrap_environment(cls, *, receiving_state=None):
        raw = os.environ.get('TOS_SDK_STAGE_CONFIG')
        if type(raw) is not str or len(raw) > 65536:
            raise ValueError('maintained native SDK bootstrap configuration absent or oversized')
        if receiving_state is None:
            v = json.loads(raw, object_pairs_hook=_unique)
        else:
            g = receiving_state.geometry
            receiving_state.reserve(g.bytes_base + 4 * len(raw)
                                    + memoryview.__basicsize__ + g.gc_header
                                    + cls.__basicsize__ + g.gc_header + g.dict_bytes(10)
                                    + NativeSDKSearchCacheConfiguration.__basicsize__ + g.gc_header
                                    + g.dict_bytes(4) + 2 * g.list_bytes(2))
            encoded = raw.encode('utf-8')
            if len(encoded) > 65536:
                raise ValueError('native SDK bootstrap configuration byte cap exceeded')
            v = receiving_state.decode(memoryview(encoded))
        if receiving_state is None and len(raw.encode()) > 65536:
            raise ValueError('native SDK bootstrap configuration byte cap exceeded')
        fields = {'schema', 'setup_cgroup', 'consumer_cgroup', 'scratch_parent',
                  'unshare_exe', 'original_whole_deadline_ns', 'original_work_deadline_ns',
                  'maximum_shutdown_ms', 'quota_bytes', 'inode_limit', 'working_ram_bytes',
                  'aggregate_ram_bytes', 'swap_max_bytes'}
        if type(v) is not dict or not fields <= v.keys() or v.keys() - fields - {'persistent_store', 'setup_as_bytes', 'guardian_state_bytes', 'search_cache'}:
            raise ValueError('native SDK bootstrap field contract differs')
        if v['schema'] != 'tos_sdk_stage_config_v1':
            raise ValueError('native SDK bootstrap schema differs')
        for name, expected in [('maximum_shutdown_ms', 5000), ('quota_bytes', _SETUP),
                ('inode_limit', 65536), ('working_ram_bytes', _CONSUMER),
                ('aggregate_ram_bytes', _AGGREGATE), ('swap_max_bytes', 0)]:
            if type(v[name]) is not int or v[name] != expected:
                raise ValueError('native SDK original finite physical profile differs')
        clocks = []
        for name in ('original_whole_deadline_ns', 'original_work_deadline_ns'):
            value = v[name]
            if type(value) is not str or not value.isascii() or not value.isdigit() or len(value) > 20:
                raise ValueError('native SDK original clock string differs')
            number = int(value)
            if not 0 < number < 1 << 64:
                raise ValueError('native SDK original clock outside u64')
            clocks.append(number)
        if clocks[0] - clocks[1] != 5000000000:
            raise ValueError('native SDK original shutdown reserve differs')
        setup_as = v.get('setup_as_bytes')
        guardian = v.get('guardian_state_bytes')
        if (setup_as is None) != (guardian is None):
            raise ValueError('native SDK setup address-space selectors must be paired')
        if setup_as is not None:
            if receiving_state is None:
                raise ValueError('native SDK controlled setup requires original receiving state')
            g = receiving_state.geometry
            receiving_state.reserve(2 * g.list_bytes(4) + 4 * (g.tuple_base + 2 * g.pointer)
                + 8 * (g.int_base + 3 * g.int_digit))
            if (type(setup_as) is not int or type(guardian) is not int
                    or setup_as <= 0 or guardian <= 0 or setup_as + guardian != _SETUP):
                raise ValueError('native SDK original setup address-space split differs')
            # The dedicated entry imports resource under its enforced aggregate
            # setup workspace before creating the receiving ledger.
            import resource
            if resource.getrlimit(resource.RLIMIT_AS) != (setup_as, _CONSUMER):
                raise ValueError('native SDK actual setup address-space pair differs')
            for name, expected in (
                    ('TOS_SDK_SETUP_AS_BYTES', setup_as),
                    ('TOS_SDK_GUARDIAN_STATE_BYTES', guardian),
                    ('TOS_SDK_ORIGINAL_WORK_DEADLINE_NS', clocks[1]),
                    ('TOS_SDK_ORIGINAL_WHOLE_DEADLINE_NS', clocks[0])):
                raw_projection = os.environ.get(name)
                if (type(raw_projection) is not str or not 0 < len(raw_projection) <= 20
                        or not raw_projection.isascii() or not raw_projection.isdigit()
                        or int(raw_projection) != expected):
                    raise ValueError('native SDK bounded bootstrap projection differs')
        cache_value = v.get('search_cache')
        search_cache = None
        if cache_value is not None:
            expected_cache_fields = {'path', 'source_root', 'max_build_bytes', 'max_temp_bytes'}
            if (type(cache_value) is not dict or set(cache_value) != expected_cache_fields
                    or type(cache_value['max_build_bytes']) is not int
                    or type(cache_value['max_temp_bytes']) is not int):
                raise ValueError('native SDK search cache bootstrap fields differ')
            search_cache = NativeSDKSearchCacheConfiguration(
                _path(cache_value['path'], receiving_state),
                _path(cache_value['source_root'], receiving_state),
                cache_value['max_build_bytes'], cache_value['max_temp_bytes'])
        return cls(*(_path(v[name], receiving_state) for name in ('setup_cgroup', 'consumer_cgroup',
                    'scratch_parent', 'unshare_exe')), *clocks,
                   _path(v['persistent_store'], receiving_state) if 'persistent_store' in v else None,
                   setup_as, guardian, search_cache)

    def active(self):
        now = time.monotonic_ns()
        if now >= self.original_work_deadline_ns:
            raise TimeoutError('native SDK original work cutoff expired')
        if self.original_whole_deadline_ns - now > 50000000000:
            raise ValueError('native SDK original whole profile exceeds 50 seconds')


@dataclass(frozen=True)
class NativeSDKHostSessionRequest:
    """One caller operation clock; the native host owner issues every grant."""
    original_whole_deadline_ns: int
    original_work_deadline_ns: int

    @classmethod
    def from_operation_state(cls, state):
        g = state.geometry
        code = cls.from_operation_state.__func__.__code__
        slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
        scalar = max(g.unicode_bytes(20), int.__basicsize__ + 3 * g.int_digit)
        import types
        state.reserve(cls.__basicsize__ + g.gc_header + g.dict_bytes(2) + 2 * scalar
                      + types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar))
        work = int(state._deadline * 1e9)
        request = cls(work + 5000000000, work)
        request.active()
        return request

    def active(self):
        now = time.monotonic_ns()
        if (type(self.original_whole_deadline_ns) is not int
                or type(self.original_work_deadline_ns) is not int
                or self.original_whole_deadline_ns - self.original_work_deadline_ns != 5000000000
                or not now < self.original_work_deadline_ns < 1 << 64
                or self.original_whole_deadline_ns - now > 50000000000):
            raise ValueError('ordinary SDK original operation clock differs')


class NativeSDKHostCustody:
    """Native owns host leases/cgroups; Python holds the child through terminal."""
    def __init__(self, request, cancelled):
        self.config, self.cancelled, self.channel = request, cancelled, None
        self.verify_current()

    def verify_current(self):
        self.config.active()
        if self.cancelled.is_set():
            raise InterruptedError('ordinary SDK operation cancelled')

    def verify_after_shutdown(self):
        if time.monotonic_ns() >= self.config.original_whole_deadline_ns:
            raise TimeoutError('ordinary SDK original cleanup cutoff')
        if (self.channel is None or self.channel._unknown or self.channel._terminal is None
                or self.channel._terminal.si_code != os.CLD_EXITED or self.channel._terminal.si_status != 0):
            raise RuntimeError('native host grant/cgroup cleanup terminal unproved')

    def close(self):
        # No invented Python kernel-placement handles. owned_exchange retains
        # direct child/image custody on both primary failure and successful close.
        self.channel = None


class NativeSDKPlacement:
    """Read-only physical receiver binding, with held kernel directory identities."""
    def __init__(self, config):
        if not isinstance(config, NativeSDKStageConfiguration):
            raise TypeError('native SDK requires typed bootstrap configuration')
        self.config = config
        self._held = {}
        self._verification_deadline_ns = config.original_work_deadline_ns
        try:
            config.active()
            setup, consumer = config.setup_cgroup, config.consumer_cgroup
            if (setup.name != 'setup' or consumer.name != 'consumer'
                    or setup.parent != consumer.parent
                    or not setup.parent.name.startswith('tos-sdk-session-')
                    or not setup.parent.name.endswith('.scope')
                    or not setup.is_relative_to('/sys/fs/cgroup')):
                raise ValueError('native SDK original sibling scope placement differs')
            for path in (setup, consumer, setup.parent):
                self._held[path] = native_dispatch._directory(path, config.original_work_deadline_ns / 1e9)
            self.verify_current()
            if self._read(consumer, 'cgroup.procs', 4096).strip():
                raise ValueError('native SDK selected consumer already occupied')
        except BaseException:
            self.close()
            raise

    def _active(self):
        if time.monotonic_ns() >= self._verification_deadline_ns:
            raise TimeoutError('native SDK original receiver verification cutoff expired')

    def _read(self, root, name, cap):
        self._active()
        fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=self._held[root])
        try:
            if not stat.S_ISREG(os.fstat(fd).st_mode):
                raise ValueError('native SDK kernel member not regular')
            raw = bytearray()
            while True:
                self._active()
                chunk = os.read(fd, min(65536, cap + 1 - len(raw)))
                if not chunk:
                    return raw.decode('ascii', 'strict')
                raw.extend(chunk)
                if len(raw) > cap:
                    raise ValueError('native SDK kernel member exceeds bound')
        finally:
            os.close(fd)

    def verify_current(self):
        c = self.config
        self._active()
        if os.name != 'posix' or not Path('/proc/self').exists():
            raise ValueError('native SDK maintained Linux runtime unavailable')
        for path, fd in self._held.items():
            fresh = native_dispatch._directory(path, self._verification_deadline_ns / 1e9)
            try:
                a, b = os.fstat(fd), os.fstat(fresh)
                if (a.st_dev, a.st_ino) != (b.st_dev, b.st_ino):
                    raise ValueError('native SDK held scope name changed')
            finally:
                os.close(fresh)
        for root, maximum in [(c.setup_cgroup, _SETUP), (c.consumer_cgroup, _CONSUMER),
                              (c.setup_cgroup.parent, _AGGREGATE)]:
            if (self._read(root, 'memory.max', 64).strip() != str(maximum)
                    or self._read(root, 'memory.swap.max', 64).strip() != '0'
                    or self._read(root, 'cgroup.type', 64).strip() != 'domain'):
                raise ValueError('native SDK actual physical receiver bounds changed')
        proc_root = Path('/proc') / str(os.getpid())
        proc_fd = native_dispatch._directory(proc_root, self._verification_deadline_ns / 1e9)
        self._held[proc_root] = proc_fd
        try:
            membership = self._read(proc_root, 'cgroup', 4096).strip()
        finally:
            self._held.pop(proc_root)
            os.close(proc_fd)
        if membership != '0::/' + str(c.setup_cgroup.relative_to('/sys/fs/cgroup')):
            raise ValueError('SDK executing outside original setup receiver')
        actual_as = resource.getrlimit(resource.RLIMIT_AS)
        if c.setup_as_bytes is None:
            valid_as = actual_as == (_CONSUMER, _CONSUMER)
        else:
            valid_as = (0 < actual_as[0] <= c.setup_as_bytes
                        and actual_as[1] == _CONSUMER)
        if (not valid_as
                or resource.getrlimit(resource.RLIMIT_FSIZE) != (_SETUP, _SETUP)):
            raise ValueError('native SDK inherited original finite rlimit differs')
        self._active()

    def verify_after_shutdown(self):
        # Read-only final receiving fence uses only the original cleanup reserve.
        self._verification_deadline_ns = self.config.original_whole_deadline_ns
        try:
            self.verify_current()
        finally:
            self._verification_deadline_ns = self.config.original_work_deadline_ns

    def close(self):
        for fd in self._held.values():
            os.close(fd)
        self._held.clear()


class NativeSDKSession:
    """Serialized bytes-only client; returned frame is borrowed until next call."""
    def __init__(self, control, channel, placement, limits, receiving_state=None):
        self._control, self._channel, self._placement, self._limits = control, channel, placement, limits
        self._receiving_state = receiving_state
        self._sequence = 1
        self._closed = False

    def _receive(self, allowed, sequence, phase):
        try:
            return self._control.receive(allowed, sequence)
        except EOFError as primary:
            # Preserve the original EOF. Drain only within the SAME original
            # whole cutoff, while the channel still holds unreaped custody.
            if self._receiving_state is not None:
                g = self._receiving_state.geometry
                try:
                    self._receiving_state.reserve(4 * g.unicode_bytes(65536 + 1024)
                                                  + 4 * (g.tuple_base + 4 * g.pointer))
                except BaseException as secondary:
                    primary.args = (str(primary) + '; diagnostic preadmission refused: '
                                    + type(secondary).__name__ + ':' + str(secondary),)
                    raise primary from secondary
            context = 'terminal accepted'
            try:
                self._channel.finish_control_session()
            except BaseException as secondary:
                context = type(secondary).__name__ + ':' + str(secondary)
            terminal = self._channel._terminal
            status = ('unobserved' if terminal is None or self._channel._unknown
                      else 'si_code=' + str(terminal.si_code) + ',si_status=' + str(terminal.si_status))
            primary.args = (str(primary) + '; phase=' + phase + '; sequence=' + str(sequence)
                            + '; native_terminal=' + status + '; context=' + context,)
            raise

    def call_bytes(self, payload):
        if self._closed or self._sequence > self._limits.max_calls:
            raise ValueError('native SDK session call lifecycle exhausted')
        self._placement.verify_current()
        self._control.send(1, self._sequence, payload)
        kind, result = self._receive(frozenset((3, 6)), self._sequence, 'reply')
        self._sequence += 1
        self._placement.verify_current()
        if kind == 6:
            state = self._receiving_state
            if state is None:
                raise ValueError('native SDK owner refused selected call')
            # Kind6 is a complete refusal before any result disclosure. Decode
            # once under the ORIGINAL ledger; no new clock/counter or retry.
            envelope = state.decode(result)
            raise NativeSessionRefused.from_envelope(envelope, state,
                'tos_native_core_snapshot_result_v1')
        return result

    def close(self):
        if self._closed:
            return
        self._placement.verify_current()
        self._control.send(4, self._sequence, b'')
        kind, payload = self._receive(frozenset((5, 6)), self._sequence, 'close_ack')
        if kind == 6:
            if self._receiving_state is None:
                raise ValueError('native SDK close refused')
            envelope = self._receiving_state.decode(payload)
            raise NativeSessionRefused.from_envelope(envelope, self._receiving_state,
                'tos_native_core_snapshot_result_v1')
        self._channel.finish_control_session()
        self._placement.verify_after_shutdown()
        self._closed = True


def _release_environment(release_root):
    if release_root is None:
        return None
    environment = dict(os.environ)
    environment['TOS_RELEASE_ROOT'] = os.fspath(release_root)
    return environment


@contextmanager
def owned_native_sdk_session(*, prefix, root, startup_bytes, limits, cancelled,
                             receiver_buffer, frame_buffer, config, receiving_state=None,
                             session_operation='tos_native_session', snapshot_root=None,
                             search_cache_path=None, release_root=None):
    """Launch issuer directly from SDK using original clock and child-only ticket.

    Encoded startup and buffers are caller-owned original receiving state; no
    Python model/parser/allocation allowance is manufactured by this transport.
    """
    if type(session_operation) is not str or session_operation not in ('tos_native_session', 'tos_native_probe_session', 'tos_native_lazy_session', 'tos_native_ordinary_session'):
        raise ValueError('native SDK session profile operation unavailable')
    if not isinstance(limits, NativeSessionLimits) or not isinstance(cancelled, threading.Event):
        raise TypeError('native SDK original typed transport limits/cancellation required')
    if (getattr(config, 'search_cache', None) is not None
            and not isinstance(config.search_cache, NativeSDKSearchCacheConfiguration)):
        raise TypeError('native SDK search cache requires its typed bootstrap owner')
    if type(startup_bytes) is not bytes or len(startup_bytes) > 65536:
        raise ValueError('native SDK original bounded encoded startup required')
    selected_root = _path(root, receiving_state)
    if snapshot_root is not None:
        snapshot_root = _path(snapshot_root, receiving_state)
        if selected_root != snapshot_root / 'data':
            raise ValueError('native snapshot root/data selector differs')
    if release_root is not None:
        release_root = _path(release_root, receiving_state)
        if (session_operation != 'tos_native_ordinary_session' or snapshot_root is None):
            raise ValueError('ReferenceRelease requires an ordinary guarded snapshot session')
    if search_cache_path is not None:
        search_cache_path = _path(search_cache_path, receiving_state)
        if session_operation != 'tos_native_ordinary_session':
            raise ValueError('native cache host selector requires ordinary operation')
    if receiving_state is not None:
        from .native_core_session_launch_state import reserve_sdk_launch
        reserve_sdk_launch(receiving_state, config, prefix, selected_root,
                           session_operation=session_operation,
                           protected_root=snapshot_root if snapshot_root is not None else
                               selected_root if session_operation == 'tos_native_ordinary_session' else None,
                           search_cache_path=search_cache_path)
    with ExitStack() as stack:
        outside = isinstance(config, NativeSDKHostSessionRequest)
        if outside and session_operation != 'tos_native_ordinary_session':
            raise ValueError('outside host request requires ordinary native owner')
        placement = NativeSDKHostCustody(config, cancelled) if outside else NativeSDKPlacement(config)
        stack.callback(placement.close)
        parent, child = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
        stack.callback(parent.close)
        stack.callback(child.close)
        fd = child.fileno()
        if outside:
            arguments = ['sdk-host-session', '--root', str(selected_root),
                '--consumer-control-fd', str(fd), '--work-deadline-ns',
                str(config.original_whole_deadline_ns)]
        else:
            arguments = ['private-stage-run', '--unshare-exe', str(config.unshare_exe),
                '--consumer-cgroup', str(config.consumer_cgroup), '--scratch-parent', str(config.scratch_parent),
                '--quota-bytes', str(_SETUP), '--inodes', '65536', '--working-ram-bytes', str(_CONSUMER),
                '--work-deadline-ns', str(config.original_whole_deadline_ns), '--maximum-shutdown-ms', '5000',
                '--consumer-control-fd', str(fd)]
            if config.setup_as_bytes is not None:
                arguments += ['--sdk-setup-as-bytes', str(config.setup_as_bytes),
                              '--sdk-guardian-state-bytes', str(config.guardian_state_bytes)]
            if config.persistent_store is not None:
                arguments += ['--persistent-store', str(config.persistent_store)]
            if config.search_cache is not None:
                cache = config.search_cache
                arguments += ['--search-cache-path', str(cache.path),
                    '--search-cache-source-root', str(cache.source_root),
                    '--search-cache-max-build-bytes', str(cache.max_build_bytes),
                    '--search-cache-max-temp-bytes', str(cache.max_temp_bytes)]
            # The installed image path is selected/held by native_dispatch, never root.
            # /proc/self/exe names that same issuer image inside its selected child.
            arguments += ['--', '/proc/self/exe', 'native-process-exec',
                '--address-space-bytes', str(_CONSUMER), '--file-size-bytes', str(_SETUP), '--',
                'core-snapshot', '--root', str(selected_root), '--operation', session_operation,
                '--session-control-fd', str(fd), '--work-deadline-ns', str(config.original_work_deadline_ns)]
        if outside and snapshot_root is not None:
            arguments += ['--snapshot-root', str(snapshot_root)]
        elif not outside and session_operation == 'tos_native_ordinary_session':
            boundary = arguments.index('--')
            arguments[boundary:boundary] = ['--protected-ro', str(snapshot_root if snapshot_root is not None else selected_root)]
        if outside and search_cache_path is not None:
            arguments += ['--search-cache-path', str(search_cache_path)]
        selected_image = None
        if receiving_state is not None:
            selected_image = stack.enter_context(native_dispatch.verified_image(
                prefix, absolute_deadline=config.original_work_deadline_ns / 1e9,
                absolute_cleanup_deadline=config.original_whole_deadline_ns / 1e9,
                receiving_state=receiving_state))
        channel = stack.enter_context(native_io.owned_exchange(arguments, prefix=prefix,
            input_cap=65536, frame_cap=65536, cancelled=cancelled,
            absolute_deadline=config.original_whole_deadline_ns / 1e9,
            operation_seconds=50, env=_release_environment(release_root),
            pass_fds=(fd,), selected_image=selected_image))
        if outside:
            placement.channel = channel
        child.close()
        control = NativeSessionControl(parent, deadline=config.original_work_deadline_ns / 1e9,
            cancelled=cancelled, receiver_buffer=receiver_buffer, frame_buffer=frame_buffer,
            limits=limits, progress=channel.poll_control_session)
        channel.write_input(startup_bytes, close=True)
        session = NativeSDKSession(control, channel, placement, limits, receiving_state)
        _, startup = session._receive(frozenset((2,)), 0, 'startup')
        yield session, startup
        session.close()


@contextmanager
def owned_native_snapshot_exchange(prefix, selection, state, operation_id,
                                   prior_fd=None, reply_fd=None, config=None,
                                   *, absolute_work_deadline=None, snapshot_root=None,
                                   release_root=None):
    """Keep authentic placement and original call custody through stdio terminal."""
    from .native_core_session_receiver import ReceiverState
    from .native_core_snapshot import NativeCoreSnapshotSelection, _INPUT_CAP, _FRAME_CAP
    from .native_core_session_launch_state import reserve_sdk_launch
    if (not isinstance(state, ReceiverState)
            or not isinstance(selection, NativeCoreSnapshotSelection)
            or operation_id not in ('tos_knowledge_graph', 'tos_knowledge_snapshot',
                                    'tos_knowledge_graph_addressed', 'tos_knowledge_snapshot_once')):
        raise TypeError('native retained operation requires original typed owners')
    for fd in (reply_fd, prior_fd):
        if fd is not None and (type(fd) is not int or fd < 3):
            raise ValueError('native retained inherited FD role unavailable')
    if prior_fd is not None and (reply_fd is None or prior_fd == reply_fd):
        raise ValueError('native retained reply/prior roles differ')
    if config is None:
        config = (NativeSDKStageConfiguration.from_bootstrap_environment(receiving_state=state)
                  if 'TOS_SDK_STAGE_CONFIG' in os.environ else
                  NativeSDKHostSessionRequest.from_operation_state(state))
    if (not isinstance(config, (NativeSDKStageConfiguration, NativeSDKHostSessionRequest))
            or state._deadline != config.original_work_deadline_ns / 1e9
            or state._limit != _SETUP):
        raise ValueError('native retained original operation owner differs')
    config.active()
    import math
    operation_deadline = config.original_work_deadline_ns / 1e9
    if absolute_work_deadline is not None:
        if (type(absolute_work_deadline) not in (int, float) or not math.isfinite(absolute_work_deadline)
                or absolute_work_deadline <= time.monotonic() or absolute_work_deadline > operation_deadline):
            raise ValueError('native retained public cutoff outside original work')
        operation_deadline = absolute_work_deadline
    cleanup_deadline = min(config.original_whole_deadline_ns / 1e9, operation_deadline + 5)
    prefix, root = _path(prefix, state), _path(selection.tos_root, state)
    reserve_sdk_launch(state, config, prefix, root, session_operation=operation_id,
                       input_cap=_INPUT_CAP, frame_cap=_FRAME_CAP,
                       protected_root=snapshot_root if snapshot_root is not None else root)
    if snapshot_root is not None:
        snapshot_root = _path(snapshot_root, state)
        if root != snapshot_root / 'data':
            raise ValueError('native retained snapshot root/data selector differs')
    if release_root is not None:
        release_root = _path(release_root, state)
        if snapshot_root is None:
            raise ValueError('ReferenceRelease requires its selected snapshot root')
    outside = isinstance(config, NativeSDKHostSessionRequest)
    with ExitStack() as stack:
        placement = NativeSDKHostCustody(config, state._cancelled) if outside else NativeSDKPlacement(config)
        stack.callback(placement.close)
        stage_control_fd = None
        if not outside and config.setup_as_bytes is not None:
            stage_control_fd = reply_fd
            if stage_control_fd is None:
                control_parent, control_child = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
                stack.callback(control_parent.close)
                stack.callback(control_child.close)
                stage_control_fd = control_child.fileno()
        roles = []
        for name, fd in (('--state-reply-fd', reply_fd), ('--snapshot-state-fd', prior_fd)):
            if fd is not None:
                roles += [name, str(fd)]
        if outside:
            arguments = ['sdk-host-session', '--root', str(root), '--snapshot-operation', operation_id,
                         '--work-deadline-ns', str(config.original_whole_deadline_ns)] + roles
        else:
            arguments = ['private-stage-run', '--unshare-exe', str(config.unshare_exe),
                '--consumer-cgroup', str(config.consumer_cgroup), '--scratch-parent', str(config.scratch_parent),
                '--quota-bytes', str(_SETUP), '--inodes', '65536', '--working-ram-bytes', str(_CONSUMER),
                '--work-deadline-ns', str(config.original_whole_deadline_ns), '--maximum-shutdown-ms', '5000'] + roles
            if config.setup_as_bytes is not None:
                arguments += ['--consumer-control-fd', str(stage_control_fd)]
                arguments += ['--sdk-setup-as-bytes', str(config.setup_as_bytes),
                              '--sdk-guardian-state-bytes', str(config.guardian_state_bytes)]
            if config.persistent_store is not None:
                arguments += ['--persistent-store', str(config.persistent_store)]
            arguments += ['--', '/proc/self/exe', 'native-process-exec',
                '--address-space-bytes', str(_CONSUMER), '--file-size-bytes', str(_SETUP), '--',
                'core-snapshot', '--root', str(root), '--operation', operation_id,
                '--native-ordinary-startup', '--work-deadline-ns', str(config.original_work_deadline_ns)] + roles
        if outside and snapshot_root is not None:
            arguments += ['--snapshot-root', str(snapshot_root)]
        elif not outside:
            boundary = arguments.index('--')
            arguments[boundary:boundary] = ['--protected-ro', str(snapshot_root if snapshot_root is not None else root)]
        image = stack.enter_context(native_dispatch.verified_image(prefix,
            absolute_deadline=operation_deadline,
            absolute_cleanup_deadline=cleanup_deadline,
            receiving_state=state))
        channel = stack.enter_context(native_io.owned_exchange(arguments, prefix=prefix,
            input_cap=_INPUT_CAP, frame_cap=_FRAME_CAP, cancelled=state._cancelled,
            absolute_deadline=cleanup_deadline,
            operation_seconds=50, env=_release_environment(release_root),
            pass_fds=tuple(dict.fromkeys(fd for fd in (reply_fd, prior_fd, stage_control_fd) if fd is not None)),
            selected_image=image))
        if outside:
            placement.channel = channel
        placement.verify_current()
        yield channel
        channel.finish()
        placement.verify_after_shutdown()
        if time.monotonic() >= operation_deadline:
            raise TimeoutError('native retained public work cutoff')
