"""Selection and request guards for a verified, independently released snapshot."""
from __future__ import annotations

from contextvars import ContextVar
from functools import wraps
import hashlib
import inspect
import os
from pathlib import Path

from .data_snapshot import verify_data_snapshot
from .release_state import ReleaseStore, ReleaseStateError


class DataAccessUnavailable(RuntimeError):
    """The selected snapshot is incomplete, changed, incompatible or withdrawn."""


_ACTIVE_GUARD = ContextVar('tos_data_guard', default=None)


def _identity(path: Path):
    if path.is_symlink() or path.resolve() != path.absolute():
        raise DataAccessUnavailable('snapshot path contains a symlink')
    try:
        stat = path.stat()
    except OSError as error:
        raise DataAccessUnavailable('selected data member is unavailable') from error
    return stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns


def _hash(path: Path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def release_data_root(release_root: Path) -> Path:
    """Read a local release binding; selecting it never installs software."""
    try:
        selection = ReleaseStore(release_root, create=False).read_selection()
    except ReleaseStateError as error:
        raise DataAccessUnavailable(str(error)) from error
    candidate = Path(selection['bindings']['data_root']).absolute()
    # Full integrity/compatibility is checked once by DataGuard when the core
    # opens this root, before any request can be served.
    if candidate != candidate.resolve() or not (candidate / 'manifest.json').is_file():
        raise DataAccessUnavailable('release data binding is missing or linked')
    return candidate / 'data'


class DataGuard:
    def __init__(self, snapshot_root: Path):
        self.snapshot_root = snapshot_root.absolute()
        self.data_root = self.snapshot_root / 'data'
        manifest_path = self.snapshot_root / 'manifest.json'
        before = _identity(manifest_path)
        before_files = {path: _identity(path) for path in self.data_root.rglob('*') if path.is_file()}
        self.manifest = verify_data_snapshot(self.snapshot_root)
        self.files = {self.snapshot_root / row['path']: _identity(self.snapshot_root / row['path'])
                      for row in self.manifest['members']}
        if self.files != before_files:
            raise DataAccessUnavailable('snapshot members changed during selection')
        self.manifest_identity = _identity(manifest_path)
        if before != self.manifest_identity:
            raise DataAccessUnavailable('snapshot manifest changed during selection')
        self.release = None
        self.pair = None
        if configured := os.environ.get('TOS_RELEASE_ROOT'):
            try:
                self.release = ReleaseStore(Path(configured), create=False)
                selection = self.release.read_selection()
            except ReleaseStateError as error:
                raise DataAccessUnavailable(str(error)) from error
            self.pair = selection['pair']
            if (self.pair['data_revision'] != self.manifest['data_revision']
                    or self.pair['corpus_revision'] != self.manifest['corpus_revision']
                    or self.pair['data_manifest_sha256'] != _hash(manifest_path)
                    or Path(selection['bindings']['data_root']).absolute() != self.snapshot_root):
                raise DataAccessUnavailable('explicit data selection differs from the managed release')
        self.check()

    @classmethod
    def for_data_root(cls, root: Path):
        root = Path(root).absolute()
        if root.name == 'data' and os.path.lexists(root.parent / 'manifest.json'):
            return cls(root.parent)
        if os.environ.get('TOS_RELEASE_ROOT'):
            raise DataAccessUnavailable('managed serving requires a verified data snapshot')
        # Source/fixture inspection remains explicit. It has no release claim
        # and is never selected by a managed release pointer.
        return None

    def check(self):
        if _identity(self.snapshot_root / 'manifest.json') != self.manifest_identity:
            raise DataAccessUnavailable('selected snapshot manifest changed')
        if self.release is not None:
            try:
                self.release.assert_available(self.pair)
            except ReleaseStateError as error:
                raise DataAccessUnavailable(str(error)) from error

    def check_path(self, path: Path):
        self.check()
        path = path.absolute()
        if path not in self.files or _identity(path) != self.files[path]:
            raise DataAccessUnavailable('selected data member is missing, changed or undeclared')


def check_data_path(path: Path):
    """Guard the exact file being read; never scan the full corpus per query."""
    absolute = Path(path).absolute()
    guard = _ACTIVE_GUARD.get()
    if guard is not None and absolute.is_relative_to(guard.data_root):
        guard.check_path(absolute)


def guard_public_data_methods(cls):
    """Check withdrawal before execution and again before returning a packet."""
    def guarded(function):
        @wraps(function)
        def call(self, *args, **kwargs):
            guard = self._data_guard
            if guard is not None:
                guard.check()
            token = _ACTIVE_GUARD.set(guard)
            try:
                result = function(self, *args, **kwargs)
                if guard is not None:
                    guard.check()
                return result
            finally:
                _ACTIVE_GUARD.reset(token)
        return call
    for name, function in list(vars(cls).items()):
        if not name.startswith('_') and inspect.isfunction(function):
            setattr(cls, name, guarded(function))
    return cls
