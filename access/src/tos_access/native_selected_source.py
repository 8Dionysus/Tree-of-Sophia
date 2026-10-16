"""Retained selected-source bytes and two nonqueued native operation slots.

Rust parses the vector, admits the installed schema worker and checks source
currentness and text rights. This module only retains bytes, paths and FDs.
"""
from __future__ import annotations

import fcntl
import math
import os
from pathlib import Path
import stat
import threading
import time

from .native_io import native_packets
from .source_read_errors import SourceReadError, SourceReadBudgetExceeded


def _absolute(value, name):
    path = Path(value)
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError(f'{name} requires an explicit absolute path')
    return path


class NativeSelectedSourceProvider:
    """Initialize a selected owner without Python vector or epoch rules."""

    def __init__(self, prefix, source_root, inputs_path, *, expected_revision,
                 local_text_selection=None):
        deadline = time.monotonic() + 50
        path = _absolute(inputs_path, 'source inputs')
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode) or before.st_size > 1048576:
                raise SourceReadBudgetExceeded('source vector host byte budget exceeded')
            raw = bytearray()
            while len(raw) <= 1048576:
                if time.monotonic() >= deadline:
                    raise TimeoutError('selected source initialization deadline expired')
                part = os.read(descriptor, min(65536, 1048577 - len(raw)))
                if not part:
                    break
                raw.extend(part)
            after = os.fstat(descriptor)
            fields = ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_uid', 'st_gid',
                      'st_size', 'st_mtime_ns', 'st_ctime_ns')
            if len(raw) > 1048576:
                raise SourceReadBudgetExceeded('source vector host byte budget exceeded')
            if len(raw) != before.st_size or any(getattr(before, key) != getattr(after, key) for key in fields):
                raise SourceReadError('source input bytes changed during retention')
        finally:
            os.close(descriptor)
        self._initialize(prefix, source_root, bytes(raw), expected_revision,
                         local_text_selection, deadline)

    @classmethod
    def from_retained_bytes(cls, prefix, source_root, raw, *, expected_revision,
                            local_text_selection=None):
        """Retain actual immutable vector bytes; native code validates them."""
        deadline = time.monotonic() + 50
        if type(raw) is not bytes or len(raw) > 1048576:
            raise ValueError('selected source requires bounded immutable vector bytes')
        provider = cls.__new__(cls)
        provider._initialize(prefix, source_root, raw, expected_revision,
                             local_text_selection, deadline)
        return provider

    def _initialize(self, prefix, root, raw, revision, local, deadline):
        self.prefix = _absolute(prefix, 'native prefix')
        self.source_root = _absolute(root, 'source root')
        if type(revision) is not str or len(revision) > 65536:
            raise ValueError('selected source revision requires bounded owner text')
        self.expected_revision = revision
        self.local_text_selection = None if local is None else _absolute(local, 'local text selection')
        self._slots = threading.BoundedSemaphore(2)
        self._condition = threading.Condition()
        self._active = 0
        self._closed = False
        self._vector = None
        descriptor = os.memfd_create('tos-selected-source-inputs', os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING)
        try:
            view = memoryview(raw)
            offset = 0
            while offset < len(view):
                if time.monotonic() >= deadline:
                    raise TimeoutError('selected source initialization deadline expired')
                count = os.write(descriptor, view[offset:])
                if count <= 0:
                    raise OSError('selected source descriptor write failed')
                offset += count
            fcntl.fcntl(descriptor, fcntl.F_ADD_SEALS,
                        fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL)
            self._vector = descriptor
            descriptor = None
            self.initial_capabilities = self.call('capabilities', {}, absolute_deadline=deadline)
        except BaseException:
            self.close()
            raise
        finally:
            if descriptor is not None:
                os.close(descriptor)

    def call(self, operation, request, *, absolute_deadline=None, cancelled=None):
        deadline = time.monotonic() + 50
        if absolute_deadline is not None:
            if type(absolute_deadline) not in (int, float) or not math.isfinite(absolute_deadline):
                raise ValueError('selected source deadline must be finite')
            deadline = min(deadline, absolute_deadline)
        if operation not in ('capabilities', 'contracts', 'discover', 'read'):
            raise ValueError('unknown selected source operation')
        if not self._slots.acquire(blocking=False):
            raise SourceReadError('source reader concurrency budget exhausted')
        admitted = False
        try:
            with self._condition:
                if self._closed:
                    raise SourceReadError('selected source provider is closed')
                self._active += 1
                admitted = True
                descriptor = self._vector
            arguments = ['source', 'selected-owner', '--root', str(self.source_root),
                         '--inputs-fd', str(descriptor), '--revision', self.expected_revision]
            if self.local_text_selection is not None:
                arguments += ['--local-text-selection', str(self.local_text_selection)]
            arguments.append(operation)
            packets = native_packets(arguments, request, prefix=self.prefix,
                input_cap=65536, frame_cap=2097152, absolute_deadline=deadline,
                cancelled=cancelled, pass_fds=(descriptor,), env={
                    key: value for key, value in os.environ.items()
                    if key not in {'TOS_RELEASE_ROOT', 'TOS_DATA_ROOT'}
                })
            try:
                packet = next(packets, None)
                if packet is None or next(packets, None) is not None:
                    raise SourceReadError('selected source returned an invalid packet count')
            finally:
                packets.close()
        finally:
            if admitted:
                with self._condition:
                    self._active -= 1
                    self._condition.notify_all()
            self._slots.release()
        with self._condition:
            if self._closed:
                raise SourceReadError('selected source provider is closed')
        if time.monotonic() >= deadline:
            raise TimeoutError('selected source deadline expired before disclosure')
        if cancelled is not None and cancelled.is_set():
            raise SourceReadError('selected source operation cancelled')
        return packet

    def capabilities(self):
        return self.call('capabilities', {})

    def discover(self, request):
        return self.call('discover', request)

    def read(self, request):
        return self.call('read', request)

    def close(self):
        with self._condition:
            self._closed = True
            while self._active:
                self._condition.wait()
            if self._vector is not None:
                os.close(self._vector)
                self._vector = None
