"""Explicit installed software association for the maintained module caller.

The selected manifest supplies integrity, not admission or runtime acceptance.
Data selection never chooses this executable. Imported reference APIs remain
independent of this opt-in process replacement.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tomllib

_PROGRAM = 'access/src/tos_access/tos-access'
_PROOF_KEYS = {'schema_version', 'sha256', 'size_bytes', 'target', 'source_commit',
               'source_tree', 'lock_sha256', 'toolchain', 'profile'}


def _identity(fd):
    s = os.fstat(fd)
    return s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns


def _open(root_fd, relative, cap):
    parts = relative.split('/')
    current = os.dup(root_fd)
    try:
        for part in parts[:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=current)
            os.close(current)
            current = child
        fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW, dir_fd=current)
        s = os.fstat(fd)
        if not stat.S_ISREG(s.st_mode) or s.st_size > cap:
            os.close(fd)
            raise ValueError('native installation member is not bounded regular software')
        return fd
    finally:
        os.close(current)


def _bytes(fd, cap):
    os.lseek(fd, 0, os.SEEK_SET)
    result = bytearray()
    while chunk := os.read(fd, min(65536, cap + 1 - len(result))):
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


def _proof(proof, source):
    if (type(proof) is not dict or set(proof) != _PROOF_KEYS
            or proof['schema_version'] != 'tos_native_access_build_v1'
            or proof['target'] != 'x86_64-unknown-linux-gnu'
            or proof['source_commit'] != source or proof['profile'] not in ('debug', 'release')
            or type(proof['size_bytes']) is not int or not 64 <= proof['size_bytes'] <= 512 * 1024 * 1024):
        raise ValueError('native installed build receipt differs')
    for key, pattern in (('sha256', r'[0-9a-f]{64}'), ('lock_sha256', r'[0-9a-f]{64}'),
                         ('source_commit', r'[0-9a-f]{40}|[0-9a-f]{64}'),
                         ('source_tree', r'[0-9a-f]{40}|[0-9a-f]{64}'),
                         ('toolchain', r'[0-9]+\.[0-9]+\.[0-9]+')):
        if type(proof[key]) is not str or re.fullmatch(pattern, proof[key]) is None:
            raise ValueError('native installed build identity invalid')


def run(prefix: Path, arguments: list[str]):
    """Forward unchanged maintained argv to the exact verified installed ELF FD."""
    if not prefix.is_absolute() or '..' in prefix.parts:
        raise ValueError('--native-prefix requires an explicit absolute software prefix')
    if not arguments:
        raise ValueError('--native-prefix requires a native operation')
    held = []
    before = {}
    def hold(fd):
        held.append(fd)
        before[fd] = _identity(fd)
    try:
        prefix_fd = os.open(prefix, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        hold(prefix_fd)
        root_fd = os.open('software', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=prefix_fd)
        hold(root_fd)
        manifest_fd = _open(root_fd, 'software.manifest.json', 1048576)
        hold(manifest_fd)
        manifest = json.loads(_bytes(manifest_fd, 1048576), object_pairs_hook=_unique)
        if (type(manifest) is not dict or manifest.get('schema_version') != 'tos_software_bundle_manifest_v1'
                or manifest.get('data_included') is not False or manifest.get('source_dirty') is not False):
            raise ValueError('explicit prefix lacks native software-only manifest')
        proof = manifest.get('native_access')
        _proof(proof, manifest.get('software_ref'))
        members = manifest.get('members')
        if type(members) is not list or len(members) > 1024:
            raise ValueError('native installation member declaration exceeds cap')
        selected = [m for m in members if type(m) is dict and m.get('path') == _PROGRAM]
        if (len(selected) != 1 or selected[0].get('sha256') != proof['sha256']
                or selected[0].get('size_bytes') != proof['size_bytes']):
            raise ValueError('native installed member differs from its build receipt')
        lock_fd = _open(root_fd, 'Cargo.lock', 1048576)
        hold(lock_fd)
        if hashlib.sha256(_bytes(lock_fd, 1048576)).hexdigest() != proof['lock_sha256']:
            raise ValueError('native installed lock differs from build receipt')
        pin_fd = _open(root_fd, 'rust-toolchain.toml', 8192)
        hold(pin_fd)
        if tomllib.loads(_bytes(pin_fd, 8192).decode())['toolchain']['channel'] != proof['toolchain']:
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
        while chunk := os.read(image_fd, min(65536, proof['size_bytes'] + 1 - count)):
            count += len(chunk)
            if count > proof['size_bytes']:
                raise ValueError('native installed executable grew during selection')
            digest.update(chunk)
        if digest.hexdigest() != proof['sha256'] or any(_identity(fd) != value for fd, value in before.items()):
            raise ValueError('native installed software changed during selection')
        # fexecve keeps the verified inode as the process image; reopening its
        # pathname here would introduce a code selection race. Native custody
        # and final-disclosure guards remain in the selected Rust consumer.
        os.execve(image_fd, [str(prefix / 'software' / _PROGRAM), *arguments], os.environ.copy())
    finally:
        for fd in reversed(held):
            os.close(fd)
