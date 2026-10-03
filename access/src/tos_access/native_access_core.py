"""Native imported access factory; selections never activate a reference engine.

The adapter owns absolute argv/path framing. The native selected executor owns
all authority, query validation and data compatibility. There is no graph build
or Python SourceReadService execution in this factory.
"""
from __future__ import annotations

from pathlib import Path
import os
import stat
import shutil
import weakref
import tempfile
import time
import math
from contextlib import contextmanager
from threading import RLock
from typing import Any

from .native_core import NativeCore


def _selected_path(value: str | Path, name: str) -> Path:
    path = Path(value).expanduser()
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError(f'{name} requires an explicit absolute path')
    return path


def _cleanup_owned_state(path: str, identity: tuple[int, int, int]) -> bool:
    try:
        info = Path(path).lstat()
    except FileNotFoundError:
        return True
    if (not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700
            or (info.st_dev, info.st_ino, info.st_uid) != identity):
        return False
    # Python's descriptor-based rmtree protects descendants against symlink
    # substitution. The enclosing directory was created only for this Core.
    shutil.rmtree(path)
    return True


class _OwnedState:
    def __init__(self, root: Path):
        self.name = tempfile.mkdtemp(prefix='tos-native-core-', dir=root)
        info = Path(self.name).lstat()
        if not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700 or info.st_uid != os.geteuid():
            raise ValueError('native Core state directory ownership or mode differs')
        self._finalizer = weakref.finalize(self, _cleanup_owned_state, self.name,
                                          (info.st_dev, info.st_ino, info.st_uid))

    def cleanup(self):
        if self._finalizer.alive and not self._finalizer():
            raise RuntimeError('native Core owned state identity changed; preserved for its owner')


class NativeAccessCore(NativeCore):
    """Imported native facade with independent generic and local source routes.

    A release root selects ManagedLocal; a prepared model/binding selects the
    declared local prepared profile. ``tos_root`` separately selects the private
    Reading/Word source root. Supplying only that root grants no generic query
    capability. Omitting data selection leaves native software metadata usable.
    """

    def __init__(self, native_prefix: str | Path, *,
                 tos_root: str | Path | None = None,
                 release_root: str | Path | None = None,
                 published_read_model_path: str | Path | None = None,
                 published_read_model_binding_path: str | Path | None = None,
                 published_exploration_checkpoint_path: str | Path | None = None,
                 source_inputs_path: str | Path | None = None,
                 source_local_text_selection_path: str | Path | None = None,
                 reading_analysis_root: str | Path | None = None,
                 reading_max_file_bytes: int | None = None,
                 reading_max_total_file_bytes: int | None = None,
                 native_state_root: str | Path | None = None):
        self._lifetime_lock = RLock()
        self._closed = False
        self._ephemeral_state = None
        self._native_state_root = native_state_root
        prefix = _selected_path(native_prefix, 'native_prefix')
        self.native_prefix = prefix
        self.tos_root = None if tos_root is None else _selected_path(tos_root, 'tos_root')
        self.release_root = None if release_root is None else _selected_path(release_root, 'release_root')
        pair = (published_read_model_path, published_read_model_binding_path)
        if (pair[0] is None) != (pair[1] is None):
            raise ValueError('prepared reader requires the model and owner-selected binding paths')
        if self.release_root is not None and pair[0] is not None:
            raise ValueError('select one generic release or prepared reader')
        if published_exploration_checkpoint_path is not None and pair[0] is None and self.release_root is None:
            raise ValueError('exploration checkpoints require an explicitly selected prepared reader or release')
        if source_inputs_path is not None and (pair[0] is None or self.tos_root is None):
            raise ValueError('exact source inputs require an explicit source root and prepared pair')
        if source_local_text_selection_path is not None and source_inputs_path is None:
            raise ValueError('local text selection requires exact source inputs')
        arguments: list[str] = []
        if self.release_root is not None:
            arguments += ['--release-root', str(self.release_root)]
        if pair[0] is not None:
            arguments += ['--prepared-read-model', str(_selected_path(pair[0], 'prepared model')),
                          '--prepared-binding', str(_selected_path(pair[1], 'prepared binding'))]
            if self.tos_root is not None:
                arguments += ['--root', str(self.tos_root)]
        for flag, value in (
            ('--exploration-checkpoints', published_exploration_checkpoint_path),
            ('--source-inputs', source_inputs_path),
            ('--source-local-text-selection', source_local_text_selection_path),
        ):
            if value is not None:
                arguments += [flag, str(_selected_path(value, flag))]
        if published_exploration_checkpoint_path is not None:
            checkpoint = _selected_path(published_exploration_checkpoint_path, 'exploration checkpoints')
            for protected in (prefix, self.release_root, self.tos_root):
                if protected is not None and checkpoint.is_relative_to(protected):
                    raise ValueError('exploration checkpoints require a path outside software, release and source roots')
        self._has_prepared_checkpoints = published_exploration_checkpoint_path is not None
        super().__init__(prefix, arguments, inherit_data_selection=False)
        selectors = (reading_analysis_root, reading_max_file_bytes, reading_max_total_file_bytes)
        if self.tos_root is None:
            if any(value is not None for value in selectors):
                raise ValueError('native reading selectors require an explicit source root')
            self._reading_core = NativeCore(prefix, inherit_data_selection=False)
        else:
            reading_arguments = ['--root', str(self.tos_root)]
            if reading_analysis_root is not None:
                reading_arguments += ['--reading-analysis-root', str(_selected_path(reading_analysis_root, 'reading analysis'))]
            file_bytes, total_bytes = reading_max_file_bytes, reading_max_total_file_bytes
            if (file_bytes is None) != (total_bytes is None):
                raise ValueError('native reading file budgets require a pair')
            if file_bytes is not None:
                if (type(file_bytes) is not int or type(total_bytes) is not int
                        or not 0 < file_bytes <= total_bytes <= 2**64 - 1):
                    raise ValueError('invalid explicit native reading file budgets')
                reading_arguments += ['--reading-max-file-bytes', str(file_bytes),
                                      '--reading-max-total-file-bytes', str(total_bytes)]
            self._reading_core = NativeCore(prefix, reading_arguments, inherit_data_selection=False)
            # Word observes the source root, never an independently selected
            # Reading output root or its file budget.
        self._word_core = NativeCore(prefix, ['--root', str(self.tos_root)], inherit_data_selection=False) if self.tos_root is not None else NativeCore(prefix, inherit_data_selection=False)

    @classmethod
    def discover(cls, tos_root: str | Path | None = None, *,
                 native_prefix: str | Path | None = None, **selection: Any) -> 'NativeAccessCore':
        """Select explicit native software; never infer it from CWD or data."""
        if native_prefix is None:
            raise ValueError('native imported Core requires an explicit native_prefix')
        return cls(native_prefix, tos_root=tos_root, **selection)

    def zarathustra_word_analysis_task(self, query: str, language: str = 'ru',
                                      rank: int = 1,
                                      include_semantic_neighbors: bool = False) -> dict:
        deadline = time.monotonic() + 50
        with self._call_lifetime(deadline):
            return self._word_core.zarathustra_word_analysis_task(
                query, language, rank, include_semantic_neighbors, absolute_deadline=deadline)

    def zarathustra_reading_search(self, query: str, language: str = 'ru',
                                   limit: int = 20,
                                   include_semantic_neighbors: bool = False,
                                   group_by: list[str] | None = None) -> dict:
        deadline = time.monotonic() + 50
        with self._call_lifetime(deadline):
            return self._reading_core.zarathustra_reading_search(
                query, language, limit, include_semantic_neighbors, group_by, absolute_deadline=deadline)


    def _ensure_open(self):
        if self._closed:
            raise RuntimeError('native imported Core is closed')

    @contextmanager
    def _call_lifetime(self, deadline):
        if type(deadline) not in (int, float) or not math.isfinite(deadline):
            raise ValueError('Native Core deadline must be finite')
        remaining = deadline - time.monotonic()
        if remaining <= 0 or not self._lifetime_lock.acquire(timeout=remaining):
            raise TimeoutError('Native Core deadline expired waiting for its lifetime')
        try:
            self._ensure_open()
            if time.monotonic() >= deadline:
                raise TimeoutError('Native Core deadline expired before setup')
            yield
            if time.monotonic() >= deadline:
                raise TimeoutError('Native Core deadline expired before returning the packet')
        finally:
            self._lifetime_lock.release()

    def _native_result(self, operation: str, arguments, *, absolute_deadline=None):
        # Capture before waiting: close cannot remove state during the child,
        # and serialization never renews the original call allowance.
        deadline = time.monotonic() + 50 if absolute_deadline is None else absolute_deadline
        with self._call_lifetime(deadline):
            return super()._native_result(operation, arguments, absolute_deadline=deadline)

    def knowledge_explore(self, request: dict[str, Any]) -> dict:
        deadline = time.monotonic() + 50
        with self._call_lifetime(deadline):
            if not self._has_prepared_checkpoints:
                # A software-only instance keeps the native unavailable result;
                # no disposable store can manufacture a selected publication.
                selected = self.release_root is not None or '--prepared-read-model' in self._server.arguments
                if selected:
                    root = _selected_path(self._native_state_root or os.environ.get('TOS_NATIVE_STATE_ROOT')
                                          or tempfile.gettempdir(), 'native state root')
                    if root.resolve(strict=True) != root or not root.is_dir():
                        raise ValueError('native state root must be an existing non-symlink directory')
                    for protected in (self.native_prefix, self.release_root, self.tos_root):
                        if protected is not None and root.is_relative_to(protected):
                            raise ValueError('native state root must be outside software, release and source roots')
                    owned = _OwnedState(root)
                    try:
                        info = Path(owned.name).lstat()
                        if not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700 or info.st_uid != os.geteuid():
                            raise ValueError('native Core state directory ownership or mode differs')
                        from .native_mcp import NativeMCPServer
                        self._server = NativeMCPServer(self.native_prefix,
                            [*self._server.arguments, '--exploration-checkpoints', str(Path(owned.name) / 'checkpoints.sqlite')], inherit_data_selection=False)
                    except BaseException:
                        owned.cleanup()
                        raise
                    self._ephemeral_state = owned
                    self._has_prepared_checkpoints = True
            if time.monotonic() >= deadline:
                raise TimeoutError('Native Core deadline expired during checkpoint setup')
            return self._packet('tos_knowledge_explore', {'request': request},
                                absolute_deadline=deadline, source_errors=False)

    def close(self):
        """End this Core lifetime; explicit checkpoint paths remain user-owned."""
        with self._lifetime_lock:
            if self._closed:
                return
            self._closed = True
            owned, self._ephemeral_state = self._ephemeral_state, None
            if owned is not None:
                owned.cleanup()

    def __enter__(self):
        with self._lifetime_lock:
            self._ensure_open()
            return self

    def __exit__(self, *exc):
        self.close()
