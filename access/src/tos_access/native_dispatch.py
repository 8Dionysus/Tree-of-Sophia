"""Explicit installed software association for the maintained module caller.

The selected manifest supplies integrity, not admission or runtime acceptance.
Data selection never chooses this executable. Imported reference APIs remain
independent of this opt-in process replacement.
"""
from __future__ import annotations

import hashlib
import _hashlib
import sys
import types
import json
import os
from pathlib import Path
import re
import stat
import tomllib
import time
import math
from contextlib import contextmanager

_PROGRAM = 'access/src/tos_access/tos-access'
_PROOF_KEYS = {'schema_version', 'sha256', 'size_bytes', 'target', 'source_commit',
               'source_tree', 'lock_sha256', 'toolchain', 'profile'}


def _identity(fd):
    s = os.fstat(fd)
    return s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns


def _active(deadline):
    if deadline is not None and time.monotonic() >= deadline:
        raise TimeoutError("native installed code selection original deadline expired")


def _directory(path, deadline):
    """Hold a no-symlink absolute directory walk, never blocking on a FIFO."""
    current = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        for part in path.parts[1:]:
            _active(deadline)
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=current)
            os.close(current)
            current = child
        return current
    except BaseException:
        os.close(current)
        raise


def _open(root_fd, relative, cap):
    parts = relative.split('/')
    current = os.dup(root_fd)
    try:
        for part in parts[:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=current)
            os.close(current)
            current = child
        fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=current)
        s = os.fstat(fd)
        if not stat.S_ISREG(s.st_mode) or s.st_size > cap:
            os.close(fd)
            raise ValueError('native installation member is not bounded regular software')
        return fd
    finally:
        os.close(current)


def _bytes(fd, cap, deadline=None, receiving_state=None):
    if receiving_state is not None:
        # One exact file-sized buffer, immutable copy and one bounded read
        # chunk coexist. The held regular file stamp still fences EOF.
        size = os.fstat(fd).st_size
        if not 0 <= size <= cap:
            raise ValueError('native installation metadata exceeds cap')
        g = receiving_state.geometry
        receiving_state.reserve(bytearray.__basicsize__ + size + 1
                                + g.bytes_base + size + g.bytes_base + min(65536, size + 1)
                                + memoryview.__basicsize__ + g.gc_header)
        os.lseek(fd, 0, os.SEEK_SET)
        result = bytearray(size)
        offset = 0
        while offset < size:
            receiving_state.active()
            _active(deadline)
            chunk = os.read(fd, min(65536, size - offset))
            if not chunk:
                raise ValueError('native installation metadata shortened during selection')
            result[offset:offset + len(chunk)] = chunk
            offset += len(chunk)
        if os.read(fd, 1):
            raise ValueError('native installation metadata grew during selection')
        receiving_state.active()
        return bytes(result)

    os.lseek(fd, 0, os.SEEK_SET)
    result = bytearray()
    _active(deadline)
    while chunk := os.read(fd, min(65536, cap + 1 - len(result))):
        _active(deadline)
        result.extend(chunk)
        if len(result) > cap:
            raise ValueError('native installation metadata exceeds cap')
    return bytes(result)


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate native installation metadata member')
        result[key] = value
    return result


def _proof(proof, source, receiving_state=None):
    if (type(proof) is not dict or len(proof) != len(_PROOF_KEYS) or any(k not in _PROOF_KEYS for k in proof)
            or proof['schema_version'] != 'tos_native_access_build_v1'
            or proof['target'] != 'x86_64-unknown-linux-gnu'
            or proof['source_commit'] != source or proof['profile'] not in ('debug', 'release')
            or type(proof['size_bytes']) is not int or not 64 <= proof['size_bytes'] <= 512 * 1024 * 1024):
        raise ValueError('native installed build receipt differs')
    for key, pattern in (('sha256', r'[0-9a-f]{64}'), ('lock_sha256', r'[0-9a-f]{64}'),
                         ('source_commit', r'[0-9a-f]{40}|[0-9a-f]{64}'),
                         ('source_tree', r'[0-9a-f]{40}|[0-9a-f]{64}'),
                         ('toolchain', r'[0-9]+\.[0-9]+\.[0-9]+')):
        if type(proof[key]) is not str or not _identity_text(proof[key], key, receiving_state):
            raise ValueError('native installed build identity invalid')


def _identity_text(value, key, state):
    # The same original ASCII regex language, traversed cooperatively without
    # an opaque regex scratch allocation on the admitted SDK path.
    if key == 'toolchain':
        dots = 0
        digits = 0
        for character in value:
            if state is not None:
                state.visit()
            if character == '.':
                if not digits:
                    return False
                dots += 1
                digits = 0
            elif '0' <= character <= '9':
                digits += 1
            else:
                return False
        return dots == 2 and digits > 0
    if len(value) not in ((40, 64) if key in ('source_commit', 'source_tree') else (64,)):
        return False
    for character in value:
        if state is not None:
            state.visit()
        if character not in '0123456789abcdef':
            return False
    return True


def _resolver_workspace(prefix, state):
    g = state.geometry
    # Exactly six held FDs, six five-scalar file stamps and one image label.
    # Function/frame metadata comes from actual maintained code objects.
    scalar = g.int_base + 3 * g.int_digit
    total = (2 * g.list_bytes(6) + 2 * g.dict_bytes(6)
             + 6 * (g.tuple_base + 5 * g.pointer + 6 * scalar)
             + _hashlib.HASH.__basicsize__ + 2 * (g.bytes_base + 65536)
             + g.bytes_base + 64 + g.unicode_bytes(64))
    parts = getattr(prefix, '_tail_cached', None)
    if parts is None:
        parts = getattr(prefix, '_parts', None)
    if type(parts) is not list:
        raise ValueError('native resolver original parsed Path cache unavailable')
    length = len(parts) + 2 + len('/software/' + _PROGRAM)
    for part in parts:
        state.visit()
        length += len(part)
    total += 2 * g.unicode_bytes(length)
    # Prefix path owner is already in caller census; these are new joined
    # Path/list owners and static-relative split strings used by the resolver.
    total += 2 * (type(prefix).__basicsize__ + g.gc_header + g.list_bytes(len(parts) + 4))
    total += g.list_bytes(4) + 4 * g.unicode_bytes(len(_PROGRAM))
    for fn in (_directory, _open, _bytes, _identity, _active, _proof,
               _identity_text, _resolver_workspace, verified_image.__wrapped__):
        code = fn.__code__
        slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
        total += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
    # Generator context owner and hold closure survive through child terminal.
    code = verified_image.__wrapped__.__code__
    total += (types.GeneratorType.__basicsize__ + g.gc_header
              + (code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize) * g.pointer
              + types.FunctionType.__basicsize__ + g.gc_header
              + g.tuple_base + 2 * g.pointer
              + 2 * (types.CellType.__basicsize__ + g.gc_header))
    state.reserve(total)


@contextmanager
def verified_image(prefix: Path, *, absolute_deadline=None, absolute_cleanup_deadline=None, receiving_state=None):
    """Borrow the exact installed ELF inode under an optional original cutoff.

    The same resolver serves process replacement and the owned SDK child
    launcher. Integrity supplies no stage, cgroup, or publication grant.
    """
    if absolute_deadline is not None:
        if type(absolute_deadline) not in (int, float) or not math.isfinite(absolute_deadline):
            raise ValueError("native installed image requires a finite original cutoff")
        _active(absolute_deadline)
    if absolute_cleanup_deadline is not None:
        if (absolute_deadline is None or type(absolute_cleanup_deadline) not in (int, float)
                or not math.isfinite(absolute_cleanup_deadline)
                or not absolute_deadline <= absolute_cleanup_deadline <= absolute_deadline + 5):
            raise ValueError('native image cleanup requires original bounded shutdown cutoff')
    if not prefix.is_absolute() or '..' in prefix.parts:
        raise ValueError('--native-prefix requires an explicit absolute software prefix')
    if receiving_state is not None:
        _resolver_workspace(prefix, receiving_state)
    held = []
    before = {}
    def hold(fd):
        held.append(fd)
        before[fd] = _identity(fd)
    try:
        prefix_fd = _directory(prefix, absolute_deadline)
        hold(prefix_fd)
        root_fd = os.open('software', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=prefix_fd)
        hold(root_fd)
        manifest_fd = _open(root_fd, 'software.manifest.json', 1048576)
        hold(manifest_fd)
        manifest_raw = _bytes(manifest_fd, 1048576, absolute_deadline, receiving_state)
        if receiving_state is None:
            manifest = json.loads(manifest_raw, object_pairs_hook=_unique)
        else:
            manifest = receiving_state.decode(memoryview(manifest_raw))
        del manifest_raw
        if (type(manifest) is not dict or manifest.get('schema_version') != 'tos_software_bundle_manifest_v1'
                or manifest.get('data_included') is not False or manifest.get('source_dirty') is not False):
            raise ValueError('explicit prefix lacks native software-only manifest')
        proof = manifest.get('native_access')
        _proof(proof, manifest.get('software_ref'), receiving_state)
        members = manifest.get('members')
        if type(members) is not list or len(members) > 1024:
            raise ValueError('native installation member declaration exceeds cap')
        selected = None
        for member in members:
            if receiving_state is not None:
                receiving_state.visit()
            if type(member) is dict and member.get('path') == _PROGRAM:
                if selected is not None:
                    raise ValueError('native installed executable declared twice')
                selected = member
        if (selected is None or selected.get('sha256') != proof['sha256']
                or selected.get('size_bytes') != proof['size_bytes']):
            raise ValueError('native installed member differs from its build receipt')
        lock_fd = _open(root_fd, 'Cargo.lock', 1048576)
        hold(lock_fd)
        if hashlib.sha256(_bytes(lock_fd, 1048576, absolute_deadline, receiving_state)).hexdigest() != proof['lock_sha256']:
            raise ValueError('native installed lock differs from build receipt')
        pin_fd = _open(root_fd, 'rust-toolchain.toml', 8192)
        hold(pin_fd)
        pin_raw = _bytes(pin_fd, 8192, absolute_deadline, receiving_state)
        if receiving_state is None:
            pin = tomllib.loads(pin_raw.decode())
        else:
            from .native_core_session_metadata import bounded_toolchain_toml
            pin = bounded_toolchain_toml(pin_raw, receiving_state)
        if pin['toolchain']['channel'] != proof['toolchain']:
            raise ValueError('native installed toolchain differs from build receipt')
        image_fd = _open(root_fd, _PROGRAM, proof['size_bytes'])
        hold(image_fd)
        if before[image_fd][2] != proof['size_bytes'] or not os.fstat(image_fd).st_mode & 0o111:
            raise ValueError('native installed executable size or mode differs')
        header = os.read(image_fd, 64)
        if len(header) != 64 or header[:7] != b'\x7fELF\x02\x01\x01' or header[18:20] != b'\x3e\x00':
            raise ValueError('native installed member is not Linux x86_64 ELF64')
        os.lseek(image_fd, 0, os.SEEK_SET)
        digest = hashlib.sha256()
        count = 0
        _active(absolute_deadline)
        while chunk := os.read(image_fd, min(65536, proof['size_bytes'] + 1 - count)):
            if receiving_state is not None:
                receiving_state.active()
            _active(absolute_deadline)
            count += len(chunk)
            if count > proof['size_bytes']:
                raise ValueError('native installed executable grew during selection')
            digest.update(chunk)
        if digest.hexdigest() != proof['sha256'] or any(_identity(fd) != value for fd, value in before.items()):
            raise ValueError('native installed software changed during selection')
        _active(absolute_deadline)
        yield image_fd, str(prefix / 'software' / _PROGRAM)
        _active(absolute_cleanup_deadline if absolute_cleanup_deadline is not None else absolute_deadline)
        if any(_identity(fd) != value for fd, value in before.items()):
            raise ValueError('native installed software changed across borrowed image lifetime')
    finally:
        for fd in reversed(held):
            os.close(fd)


def run(prefix: Path, arguments: list[str]):
    """Forward unchanged maintained argv to the exact verified installed ELF FD."""
    if not arguments:
        raise ValueError('--native-prefix requires a native operation')
    with verified_image(prefix) as (image_fd, label):
        # fexecve preserves the verified inode, never reopens an executable name.
        os.execve(image_fd, [label, *arguments], os.environ.copy())
