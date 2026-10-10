"""State-admitted Linux path mechanics for the existing locations owner.

Import this module before constructing ReceiverState. It selects paths, never
ToS meaning, executable code, a release grant or a new workspace. The original
factory still authenticates native placement and all source/model fences.

The finite owned SDK profile accepts filesystem paths of at most 4096 encoded
bytes. Named-user/NSS home lookup and managed-release admission remain separate
owner routes. Neither is silently replaced with SourceRoot discovery.
"""
import ctypes
import errno
import os
from pathlib import Path
import stat
import struct
import sys
import threading
import types

from .native_core_session import _path_workspace
from .native_core_session_census import retained_owner_state


_CAP = 4096
_PathType = type(Path())
_AT_FDCWD = -100
_NOFOLLOW = 0x100
_TYPE = 1
_SIZE = 0x200
_INO = 0x100
if (sys.platform != 'linux' or struct.calcsize('P') != 8
        or os.uname().machine != 'x86_64'):
    raise ValueError('bounded native source discovery requires its Linux x86_64 owner')
_LIBC = ctypes.CDLL(None, use_errno=True)
_STATX = _LIBC.statx
_STATX.argtypes = (ctypes.c_int, ctypes.c_char_p, ctypes.c_int,
                  ctypes.c_uint, ctypes.c_void_p)
_STATX.restype = ctypes.c_int
_READLINK = _LIBC.readlinkat
_READLINK.argtypes = (ctypes.c_int, ctypes.c_char_p, ctypes.c_void_p,
                     ctypes.c_size_t)
_READLINK.restype = ctypes.c_ssize_t
_StatBuffer = ctypes.c_ubyte * 256  # Linux UAPI struct statx fixed256 bytes
_LinkBuffer = ctypes.c_ubyte * _CAP
_MODE = struct.Struct('=H')
_WORD = struct.Struct('=I')
_SIZE_WORD = struct.Struct('=Q')
_DEV_WORD = struct.Struct('=II')
_CARG_BYTES = type(ctypes.c_uint.from_param(0)).__basicsize__


class _ResultUnion(ctypes.Union):
    _fields_ = (('largest', ctypes.c_longdouble * 2), ('pointer', ctypes.c_void_p))


class _Argument(ctypes.Structure):
    _fields_ = (('ffi_type', ctypes.c_void_p), ('keep', ctypes.c_void_p),
                ('value', _ResultUnion))


_COOKIE = object()


class _Paths:
    __slots__ = ('state', 'g', 'reserved', 'stat_buffer', 'link_buffer')

    def __new__(cls, state):
        g = state.geometry
        framework = 0
        scalar = max(g.unicode_bytes(1), g.int_base + 3 * g.int_digit,
                     g.tuple_base + 5 * g.pointer)
        for fn in (cls.__new__, cls.__init__, cls.hold, cls.text, cls.path, cls.join,
                   cls.split, cls.encoded, cls.observe, cls.target,
                   cls.cwd, cls.identity, cls.env, cls.expand, cls.absolute, cls.resolve,
                   _path_workspace, PreparedSourceDiscovery.discover):
            code = fn.__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            framework += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
        # CData targets own their fixed native arrays. FFI parameter objects
        # and argument pointer slots are distinct fixed five-argument owners.
        # No ctypes string return or automatic readlink buffer growth occurs.
        cdata = (_StatBuffer.__basicsize__ + _LinkBuffer.__basicsize__
                 + 256 + _CAP + 2 * g.gc_header)
        # CPython callproc.c: converted CArg per argument, struct argument[],
        # atypes[]/avalues[], inargs/callargs tuples; libffi x86_64 scalar call
        # CIF and register_args storage. Both selected signatures have <=5
        # integer/pointer args (six GP registers), so no stack argument area.
        ffi = (5 * (_CARG_BYTES + ctypes.sizeof(_Argument) + 2 * g.pointer)
               + g.tuple_base + 5 * g.pointer
               + struct.calcsize('@IIPP II') + struct.calcsize('@6Q16QQ')
               + struct.calcsize('@PPnnII5P') + g.pointer)
        amount = cls.__basicsize__ + g.gc_header + framework + cdata + ffi
        state.reserve(amount)
        owner = object.__new__(cls)
        owner.reserved = amount
        return owner

    def __init__(self, state):
        self.state, self.g = state, state.geometry
        self.stat_buffer, self.link_buffer = _StatBuffer(), _LinkBuffer()

    def hold(self, amount):
        self.state.reserve(amount)
        self.reserved += amount

    def text(self, value):
        if type(value) is str:
            if len(value) > _CAP:
                raise ValueError('native selector original path cap exceeded')
            return value
        if type(value) is not _PathType:
            raise TypeError('native selector requires a str or Path owner')
        # Clone before formatting so a borrowed Path's previously unpriced
        # parsed/string cache is never mutated by this discovery operation.
        parts = getattr(value, '_raw_paths', None)
        if parts is None:
            parts = getattr(value, '_parts', None)
        if type(parts) is not list:
            raise ValueError('native selector Path allocator ABI unavailable')
        length = len(parts) + 2
        for part in parts:
            self.state.visit()
            if type(part) is not str:
                raise TypeError('native selector parsed Path owner differs')
            length += len(part)
        if length > _CAP + 2:
            raise ValueError('native selector original path cap exceeded')
        g = self.g
        self.hold(Path.__basicsize__ + g.gc_header + g.dict_bytes(8)
                  + 3 * g.list_bytes(length + 1)
                  + (length + 1) * g.unicode_base + 4 * (2 * length + 1)
                  + g.unicode_bytes(length))
        return os.fspath(Path(value))

    def path(self, raw):
        before = self.state._retained
        _path_workspace(raw, self.state)
        self.reserved += self.state._retained - before
        return Path(raw)

    def join(self, prefix, suffix):
        length = len(prefix) + len(suffix) + 1
        if length > _CAP:
            raise ValueError('native selector original path cap exceeded')
        self.hold(2 * self.g.unicode_bytes(length))
        return prefix + ('' if prefix.endswith('/') else '/') + suffix

    def split(self, raw):
        count = 1
        for char in raw:
            self.state.visit()
            if char == '/':
                count += 1
        self.hold(3 * self.g.list_bytes(count)
                  + count * self.g.unicode_base + 4 * (len(raw) + count))
        return raw.split('/')[::-1]

    def encoded(self, raw):
        for char in raw:
            self.state.visit()
            if char == '\0':
                raise ValueError('embedded null byte')
        self.hold(self.g.bytes_base + 4 * len(raw))
        value = os.fsencode(raw)
        if len(value) > _CAP:
            raise ValueError('native selector original encoded path cap exceeded')
        return value

    def observe(self, raw, *, follow=False):
        self.state.visit()
        encoded = self.encoded(raw)
        result = _STATX(_AT_FDCWD, encoded, 0 if follow else _NOFOLLOW,
                        _TYPE | _SIZE | _INO, self.stat_buffer)
        self.state.active()
        if result:
            return ctypes.get_errno(), 0, 0
        self.hold(3 * (self.g.tuple_base + self.g.pointer
                      + self.g.int_base + 3 * self.g.int_digit))
        mask = _WORD.unpack_from(self.stat_buffer, 0)[0]
        if mask & (_TYPE | _SIZE | _INO) != (_TYPE | _SIZE | _INO):
            raise OSError('native selector statx fields unavailable')
        return 0, _MODE.unpack_from(self.stat_buffer, 28)[0], _SIZE_WORD.unpack_from(self.stat_buffer, 40)[0]

    def target(self, raw):
        self.state.visit()
        encoded = self.encoded(raw)
        count = _READLINK(_AT_FDCWD, encoded, self.link_buffer, _CAP)
        self.state.active()
        if count < 0:
            return ctypes.get_errno(), None
        if count >= _CAP:
            raise ValueError('native selector symlink target cap exceeded')
        g = self.g
        self.hold(g.bytes_base + count + 2 * g.unicode_bytes(count)
                  + 2 * memoryview.__basicsize__ + 2 * g.gc_header)
        view = memoryview(self.link_buffer).cast('B')
        raw_bytes = bytes(view[:count])
        return 0, os.fsdecode(raw_bytes)

    def cwd(self):
        error, value = self.target('/proc/self/cwd')
        if error:
            raise OSError(error, 'native selector working directory unavailable')
        # The kernel readlink target is bounded; avoid libc getcwd's automatic
        # allocation growth for an arbitrarily deep or unlinked cwd.
        if self.identity('/proc/self/cwd') != self.identity(value):
            raise FileNotFoundError('native selector working directory unlinked')
        return value

    def env(self, name):
        # Linux _Environ owns encoded immutable bytes. Inspect the actual
        # borrowed value length before the standard filesystem decoder creates
        # a new Unicode owner; .get() would decode an unbounded value first.
        data = getattr(os.environ, '_data', None)
        if type(data) is not dict:
            raise ValueError('native selector environment owner ABI unavailable')
        self.hold(self.g.bytes_base + 4 * len(name))
        raw = data.get(os.fsencode(name))
        if raw is None:
            return None
        if type(raw) is not bytes or len(raw) > _CAP:
            raise ValueError('native selector environment path cap exceeded')
        self.hold(2 * self.g.unicode_bytes(len(raw)))
        self.state.visit()
        return os.fsdecode(raw)

    def identity(self, raw):
        error, _, _ = self.observe(raw, follow=True)
        if error:
            raise OSError(error, 'native selector working directory unavailable')
        self.hold(2 * self.g.tuple_base + 6 * self.g.pointer
                  + 3 * (self.g.int_base + 3 * self.g.int_digit))
        return (_SIZE_WORD.unpack_from(self.stat_buffer, 32)[0],
                *_DEV_WORD.unpack_from(self.stat_buffer, 136))

    def expand(self, value):
        raw = self.text(value)
        if not raw.startswith('~'):
            return raw
        if len(raw) > 1 and raw[1] != '/':
            raise ValueError('named-user home discovery requires its admitted NSS owner')
        home = self.env('HOME')
        if home is None:
            raise ValueError('home discovery without HOME requires its admitted NSS owner')
        if len(home) > _CAP:
            raise ValueError('native selector HOME cap exceeded')
        self.hold(2 * self.g.unicode_bytes(len(home) + len(raw)))
        return (home.rstrip('/') + raw[1:]) or '/'

    def absolute(self, value, base=None):
        raw = self.expand(value)
        p = self.path(raw)
        self.hold(self.g.unicode_bytes(len(raw) + 2))
        raw = os.fspath(p)  # pathlib's ordinary dot/separator normalization
        if not p.is_absolute():
            raw = self.join(self.cwd() if base is None else base, raw)
            p = self.path(raw)
            self.hold(self.g.unicode_bytes(len(raw) + 2))
            raw = os.fspath(p)
        return raw

    def resolve(self, raw):
        rest = self.split(raw)
        count = len(rest)
        path = '/' if raw.startswith('/') else self.cwd()
        self.hold(self.g.dict_bytes(0))
        seen = {}
        while count:
            self.state.visit()
            name = rest.pop()
            if name is None:
                seen[rest.pop()] = path
                continue
            count -= 1
            if not name or name == '.':
                continue
            if name == '..':
                self.hold(self.g.unicode_bytes(len(path)))
                path = path[:path.rindex('/')] or '/'
                continue
            newpath = self.join(path, name)
            error, mode, size = self.observe(newpath)
            if error or not stat.S_ISLNK(mode):
                path = newpath  # same nonstrict realpath ignored-stat law
                continue
            if newpath in seen:
                cached = seen[newpath]
                if cached is not None:
                    path = cached
                    continue
                if sys.version_info[:2] in ((3, 11), (3, 12)):
                    raise RuntimeError('Symlink loop in bounded native selector')
                path = newpath  # CPython3.13+ nonstrict loop law
                continue
            if size >= _CAP:
                raise ValueError('native selector symlink target cap exceeded')
            error, target = self.target(newpath)
            if error:
                path = newpath
                continue
            if target.startswith('/'):
                path = '/'
            self.hold(2 * self.g.dict_bytes(len(seen) + 1)
                      + 2 * self.g.list_bytes(len(rest) + 2))
            seen[newpath] = None
            rest.append(newpath)
            rest.append(None)
            target_parts = self.split(target)
            self.hold(2 * self.g.list_bytes(len(rest) + len(target_parts)))
            rest.extend(target_parts)
            count += len(target_parts)
        if sys.version_info[:2] in ((3, 11), (3, 12)):
            error, _, _ = self.observe(path, follow=True)
            if error == errno.ELOOP:
                raise RuntimeError('Symlink loop in bounded native selector')
        return path


def bounded_source_selection(selection_type, tos_root, query_store_path, selectors,
                             state, *, maximum_owner_objects, source_root_only=False):
    """Return the existing typed selection under one original setup ledger.

    Explicit -> environment -> root-relative is locations' existing law.
    All imports/preexisting raw arguments belong to the original caller setup
    baseline. Current managed release selection requires its genuine owner;
    this function cannot infer or fabricate release admission from a path.
    """
    from .locations import (PACKAGE_ROOT, _SOURCE_SELECTORS,
                            QUERY_STORE_RELATIVE_PATH)
    from .native_core_session_receiver import ReceiverState
    from .native_core_snapshot import _absolute_path
    if not isinstance(state, ReceiverState):
        raise TypeError('native discovery requires original ReceiverState')
    if type(source_root_only) is not bool:
        raise TypeError('native discovery scope selector must be boolean')
    g = state.geometry
    code = bounded_source_selection.__code__
    slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
    amount = (types.FrameType.__basicsize__ + g.gc_header
              + slots * (g.pointer + max(g.unicode_bytes(1), g.int_base + 3 * g.int_digit)))
    amount += types.MethodType.__basicsize__ + g.gc_header
    state.reserve(amount)  # also before creating the bound-method temporary
    # The real typed discovery classmethod remains suspended through this
    # entire traversal. Its code geometry is a distinct live frame owner.
    method = selection_type.discover_under_admission
    discovery_code = method.__func__.__code__
    discovery_slots = (discovery_code.co_nlocals + len(discovery_code.co_cellvars)
                       + len(discovery_code.co_freevars) + discovery_code.co_stacksize)
    discovery_amount = (types.FrameType.__basicsize__ + g.gc_header
                        + discovery_slots * (g.pointer + max(g.unicode_bytes(1),
                                                   g.int_base + 3 * g.int_digit)))
    state.reserve(discovery_amount)
    amount += discovery_amount
    retained_owner_state(state, (tos_root, query_store_path, selectors),
                         maximum_objects=maximum_owner_objects)
    walker = _Paths(state)
    try:
        for name in selectors:
            state.visit()
            if type(name) is not str:
                raise TypeError('native discovery selector names must be strings')
            known = False
            for item in _SOURCE_SELECTORS:
                state.visit()
                if name == item[0]:
                    known = True
                    break
            if not known:
                raise TypeError('unknown source carrier selector')
        selected = tos_root or walker.env('TOS_DATA_ROOT')
        if selected:
            root = walker.absolute(selected)
            if root != walker.resolve(root):
                raise ValueError('data root may not contain symlinks')
            error, _, _ = walker.observe(walker.join(root, 'manifest.json'))
            if not error:
                data = walker.join(root, 'data')
                error, mode, _ = walker.observe(data, follow=True)
                if not error and stat.S_ISDIR(mode):
                    root = data
                elif error and sys.version_info[:2] in ((3, 11), (3, 12)) and error not in (
                        errno.ENOENT, errno.ENOTDIR, errno.EBADF, errno.ELOOP):
                    raise OSError(error, 'native bounded selected data directory failed')
        elif walker.env('TOS_RELEASE_ROOT'):
            raise ValueError('managed release discovery requires its native release owner')
        else:
            root = walker.join(walker.text(PACKAGE_ROOT), 'runtime_data')
        walker.hold(g.dict_bytes(7))
        fields = {}
        for name, env, relative in _SOURCE_SELECTORS:
            state.visit()
            selected = selectors.get(name) or walker.env(env)
            if selected:
                path = walker.absolute(selected, root)
            else:
                path = walker.join(root, walker.text(relative))
            fields[name] = walker.path(walker.resolve(path))
        ambient = walker.env('TOS_QUERY_STORE_PATH')
        configured = query_store_path is not None or bool(ambient)
        selected = (query_store_path if query_store_path is not None
                    else ambient if ambient else None)
        store = (walker.absolute(selected, root) if selected is not None
                 else walker.join(root, walker.text(QUERY_STORE_RELATIVE_PATH)))
        if source_root_only:
            error, _, _ = walker.observe(store, follow=True)
            if configured or not error:
                raise ValueError('selected QueryStore requires its genuine native Store profile')
        # Existing dataclass init/post-init makes bounded Path copies/caches;
        # admit those real owners before invoking it, without semantic changes.
        walker.hold(selection_type.__basicsize__ + g.gc_header + g.dict_bytes(11)
                    + 2 * g.dict_bytes(11) + g.tuple_base + 11 * g.pointer)
        root_path, store_path = walker.path(root), walker.path(store)
        for path in (root_path, *fields.values(), store_path):
            raw = walker.text(path)
            before = state._retained
            _path_workspace(raw, state)
            walker.reserved += state._retained - before
        scalar = max(g.unicode_bytes(1), g.int_base + 3 * g.int_digit,
                     g.tuple_base + 11 * g.pointer)
        for function in (selection_type.__init__, selection_type.__post_init__, _absolute_path):
            code = function.__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            walker.hold(types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar))
        walker.hold(g.bytes_base + 4 * len(store))
        result = selection_type(tos_root=root_path, **fields,
                                query_store_path=store_path,
                                query_store_configured=configured)
        retained_owner_state(state, (result,), maximum_objects=maximum_owner_objects)
        state.active()
        return result
    finally:
        # New returned owners were independently measured/identity registered.
        # This releases only conservative transient preallocation, no fabricated
        # caller baseline/alias ownership or independent whole-state allowance.
        if sys.exception() is None:
            state.release(walker.reserved + amount)
        # Failure is terminal: traceback retains this frame/walker/buffers.
        # Preserve the original reservation until the receiving owner ends.


class PreparedSourceDiscovery:
    """Prepared OS mechanics only; not an admission or caller-configurable grant."""
    __slots__ = ('thread_id', '_token')

    def __init__(self, thread_id, token):
        if token is not _COOKIE:
            raise TypeError('native discovery must be prepared by its runtime owner')
        self.thread_id, self._token = thread_id, _COOKIE

    def discover(self, selection_type, tos_root, query_store_path, selectors,
                 state, *, maximum_owner_objects, source_root_only=False):
        if (self._token is not _COOKIE or self.thread_id != threading.get_ident()
                or type(selectors) is not dict):
            raise TypeError('native discovery requires its prepared owner and selector dict')
        retained_owner_state(state, (self,), maximum_objects=maximum_owner_objects)
        return bounded_source_selection(selection_type, tos_root, query_store_path,
            selectors, state, maximum_owner_objects=maximum_owner_objects,
            source_root_only=source_root_only)


def prepare_owned_discovery():
    """Call before ReceiverState, inside the original setup/bootstrap envelope.

    This prepares the existing trusted OS runtime bindings, without reading
    ToS data, changing process state, selecting software or issuing a grant.
    Other platforms keep their existing Reference constructor unaffected.
    """
    if (sys.platform != 'linux' or struct.calcsize('P') != 8
            or os.uname().machine != 'x86_64'):
        raise ValueError('bounded native source discovery requires its Linux x86_64 owner')
    # The errobj capsule/two-int allocation and thread-state registry entry are
    # genuine preexisting runtime owners. Prepare on this exact SDK thread
    # before ReceiverState, not on the first already-admitted syscall.
    ctypes.get_errno()
    return PreparedSourceDiscovery(threading.get_ident(), _COOKIE)
