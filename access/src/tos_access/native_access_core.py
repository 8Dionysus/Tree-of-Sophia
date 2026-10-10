"""Native imported access factory; selections never activate a reference engine.

The adapter owns absolute argv/path framing. The native selected executor owns
all authority, query validation and data compatibility. There is no graph build
or a Python source reader in this factory.
"""
from __future__ import annotations

from pathlib import Path
import os
import stat
import json
import hashlib
import shutil
from functools import wraps
import weakref
import tempfile
import time
import math
from contextlib import contextmanager
from threading import Condition, RLock
from typing import Any

from .native_core import NativeCore

# Defaults for the installed Rust owner's optional ordinary-search selector.
# These wire-profile numbers stay here as SDK framing; no Python SQLite index
# or query implementation is retained.
SEARCH_READ_MODEL_MAX_POSTINGS = 10_000_000
SEARCH_READ_MODEL_MAX_VERIFY_CHARS = 16_000_000


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


def _bounded_binding_json(value: dict[str, Any]) -> bytes:
    if type(value) is not dict:
        raise TypeError('published read-model expected binding must be an object')
    encoder = json.JSONEncoder(ensure_ascii=True, allow_nan=False,
                               separators=(',', ':'), sort_keys=True)
    chunks = []
    size = 0
    for piece in encoder.iterencode(value):
        if len(piece) > 65536 - size:
            raise ValueError('published read-model expected binding exceeds 65536 bytes')
        chunk = piece.encode('ascii')
        size += len(chunk)
        chunks.append(chunk)
    return b''.join(chunks)


def _write_private_binding(state: _OwnedState, value: dict[str, Any]) -> Path:
    path = Path(state.name) / 'published-read-model-binding.json'
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_CLOEXEC', 0)
    flags |= getattr(os, 'O_NOFOLLOW', 0)
    descriptor = os.open(path, flags, 0o600)
    try:
        os.fchmod(descriptor, 0o600)
        identity = os.fstat(descriptor)
        if (not stat.S_ISREG(identity.st_mode) or stat.S_IMODE(identity.st_mode) != 0o600
                or identity.st_uid != os.geteuid()):
            raise ValueError('published binding file ownership or mode differs')
        payload = _bounded_binding_json(value)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError('published binding write made no progress')
            offset += written
    finally:
        os.close(descriptor)
    return path


def _cleanup_binding_on_init_failure(initializer):
    @wraps(initializer)
    def guarded(self, *args, **kwargs):
        try:
            return initializer(self, *args, **kwargs)
        except BaseException:
            owned = getattr(self, '_owned_binding_state', None)
            if owned is not None:
                self._owned_binding_state = None
                owned.cleanup()
            raise
    return guarded


def _reference_release_selection(native_prefix, release_root):
    """Resolve only native release metadata; the native guard owns semantics."""
    from .native_dispatch import selected_native_prefix
    from .native_io import native_packets
    prefix = selected_native_prefix(native_prefix)
    release = _selected_path(release_root, 'release_root')
    environment = dict(os.environ)
    environment['TOS_RELEASE_ROOT'] = os.fspath(release)
    packets = native_packets(
        ['reference-release-root', '--release-root', os.fspath(release)],
        prefix=prefix, input_cap=4096, frame_cap=65536, env=environment)
    try:
        packet = next(packets, None)
        if type(packet) is not dict or next(packets, None) is not None:
            raise ValueError('native release resolver requires one object packet')
    finally:
        packets.close()
    if set(packet) != {'schema_version', 'root', 'snapshot_root', 'reference_release_guard'} or packet['schema_version'] != 'tos_reference_release_root_v1':
        raise ValueError('native release resolver packet shape differs')
    root = _selected_path(packet['root'], 'release data root')
    snapshot = _selected_path(packet['snapshot_root'], 'snapshot root')
    guard = packet['reference_release_guard']
    if (root != snapshot / 'data' or type(guard) is not str or len(guard) != 64
            or any(c not in '0123456789abcdef' for c in guard)):
        raise ValueError('native release resolver pairing or receipt differs')
    return prefix, root, snapshot, guard


class NativeAccessCore(NativeCore):
    """Imported native facade with independent generic and local source routes.

    A release root selects ManagedLocal; a prepared model/binding selects the
    declared local prepared profile. ``tos_root`` separately selects the private
    Reading/Word source root. Supplying only that root grants no generic query
    capability. Omitting data selection leaves native software metadata usable.
    """

    @_cleanup_binding_on_init_failure
    def __init__(self, native_prefix: str | Path | None = None, *,
                 tos_root: str | Path | None = None,
                 release_root: str | Path | None = None,
                 published_read_model_path: str | Path | None = None,
                 published_read_model_binding_path: str | Path | None = None,
                 published_read_model_expected: dict[str, Any] | None = None,
                 search_read_model_path: str | Path | None = None,
                 search_read_model_max_bytes: int | None = None,
                 search_read_model_max_postings: int | None = None,
                 search_read_model_max_verify_chars: int | None = None,
                 published_exploration_checkpoint_path: str | Path | None = None,
                 source_inputs_path: str | Path | None = None,
                 source_local_text_selection_path: str | Path | None = None,
                 reading_analysis_root: str | Path | None = None,
                 reading_max_file_bytes: int | None = None,
                 reading_max_total_file_bytes: int | None = None,
                 concept_max_file_bytes: int | None = None,
                 concept_max_total_file_bytes: int | None = None,
                 native_state_root: str | Path | None = None,
                 source_provider: Any | None = None,
                 core_snapshot_selection: Any | None = None,
                 core_snapshot_admission_provider: Any | None = None,
                 core_snapshot_native_owned: bool = False,
                 core_snapshot_snapshot_root: str | Path | None = None,
                 core_snapshot_release_root: str | Path | None = None,
                 core_snapshot_expected_reference_guard: str | None = None,
                 source_read_service: Any | None = None):
        if source_provider is not None and source_read_service is not None:
            raise ValueError('select source_provider or source_read_service, not both')
        if source_provider is None:
            source_provider = source_read_service
        self._lifetime_lock = RLock()
        self._closed = False
        self._ephemeral_state = None
        self._owned_binding_state = None
        self._native_state_root = native_state_root
        if type(core_snapshot_native_owned) is not bool:
            raise TypeError('core_snapshot_native_owned must be a boolean')
        from .native_dispatch import selected_native_prefix
        selected_prefix = selected_native_prefix(native_prefix)
        prefix = _selected_path(selected_prefix, 'native_prefix')
        self.native_prefix = prefix
        self._source_provider = None
        self._owns_source_provider = False
        self._selected_source_provider = False
        self._selected_calls_condition = Condition()
        self._selected_calls = 0
        if source_provider is not None:
            if source_inputs_path is not None or source_local_text_selection_path is not None:
                raise ValueError('select one persisted or embedded source owner')
            from .native_source_provider import NativeSourceProvider
            from .native_selected_source import NativeSelectedSourceProvider
            if isinstance(source_provider, (NativeSourceProvider, NativeSelectedSourceProvider)):
                if source_provider.prefix != prefix:
                    raise ValueError('embedded source provider uses a different native prefix')
                self._source_provider = source_provider
                self._selected_source_provider = isinstance(source_provider, NativeSelectedSourceProvider)
            else:
                raise TypeError('embedded source owner requires a native source provider')
        self.tos_root = None if tos_root is None else _selected_path(tos_root, 'tos_root')
        self.release_root = None if release_root is None else _selected_path(release_root, 'release_root')
        self._core_snapshot_release_root = (
            None if core_snapshot_release_root is None
            else _selected_path(core_snapshot_release_root, 'core_snapshot_release_root'))
        if self.release_root is not None and self._core_snapshot_release_root is not None:
            raise ValueError('generic and ReferenceRelease selectors are separate native routes')
        binding_path = published_read_model_binding_path
        if published_read_model_expected is not None:
            if binding_path is not None:
                raise ValueError('select a published binding object or an explicit binding path, not both')
            if published_read_model_path is None:
                raise ValueError('published binding object requires an explicit prepared model')
            root_value = (native_state_root or os.environ.get('TOS_NATIVE_STATE_ROOT')
                          or tempfile.gettempdir())
            state_root = _selected_path(root_value, 'native state root')
            canonical_state_root = state_root.resolve(strict=True)
            if canonical_state_root != state_root or not state_root.is_dir():
                raise ValueError('native state root must be an existing non-symlink directory')
            for protected in (prefix, self.release_root, self._core_snapshot_release_root, self.tos_root,
                              _selected_path(published_read_model_path, 'prepared model')):
                if protected is not None and state_root.is_relative_to(protected.resolve()):
                    raise ValueError('native state root must be outside selected software, release, source and model roots')
            owned_binding = _OwnedState(state_root)
            self._owned_binding_state = owned_binding
            binding_path = _write_private_binding(owned_binding, published_read_model_expected)
        pair = (published_read_model_path, binding_path)
        if (pair[0] is None) != (pair[1] is None):
            raise ValueError('prepared reader requires the model and owner-selected binding paths')
        if self.release_root is not None and pair[0] is not None:
            raise ValueError('select one generic release or prepared reader')
        if self._core_snapshot_release_root is not None and pair[0] is not None:
            raise ValueError('ReferenceRelease guarded source and the prepared reader are separate native routes')
        if published_exploration_checkpoint_path is not None and pair[0] is None and self.release_root is None:
            raise ValueError('exploration checkpoints require an explicitly selected prepared reader or release')
        if source_inputs_path is not None and (pair[0] is None or self.tos_root is None):
            raise ValueError('exact source inputs require an explicit source root and prepared pair')
        if source_local_text_selection_path is not None and source_inputs_path is None:
            raise ValueError('local text selection requires exact source inputs')
        arguments: list[str] = []
        if self.tos_root is not None and self.release_root is None and pair[0] is None:
            arguments += ['--root', str(self.tos_root)]
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
            for protected in (prefix, self.release_root, self._core_snapshot_release_root, self.tos_root):
                if protected is not None and checkpoint.is_relative_to(protected):
                    raise ValueError('exploration checkpoints require a path outside software, release and source roots')
        self._has_prepared_checkpoints = published_exploration_checkpoint_path is not None
        self._search_read_model_options = None
        if self.tos_root is not None:
            configured_path = (search_read_model_path or
                               os.environ.get('TOS_SEARCH_READ_MODEL_PATH'))
            if configured_path is None:
                digest = hashlib.sha256(self.tos_root.resolve().as_posix().encode('utf-8')).hexdigest()[:24]
                configured_path = Path(tempfile.gettempdir()) / 'tos-access-search' / f'{digest}.sqlite'
            else:
                configured_path = Path(configured_path).expanduser()
                if not configured_path.is_absolute():
                    configured_path = self.tos_root / configured_path
                configured_path = configured_path.resolve()
            configured_bytes = search_read_model_max_bytes
            if configured_bytes is None:
                raw_bytes = os.environ.get('TOS_SEARCH_READ_MODEL_MAX_BYTES')
                configured_bytes = int(raw_bytes) if raw_bytes else 512 * 1024 * 1024
            self._search_read_model_options = {
                'path': str(configured_path),
                'max_bytes': configured_bytes,
                'max_postings': (SEARCH_READ_MODEL_MAX_POSTINGS
                                 if search_read_model_max_postings is None
                                 else search_read_model_max_postings),
                'max_verify_chars': (SEARCH_READ_MODEL_MAX_VERIFY_CHARS
                                     if search_read_model_max_verify_chars is None
                                     else search_read_model_max_verify_chars),
            }
        super().__init__(prefix, arguments, inherit_data_selection=False)
        self._core_snapshot_client = None
        if core_snapshot_native_owned and core_snapshot_admission_provider is not None:
            raise ValueError('native-owned SourceRoot cannot be combined with an external admission provider')
        self._core_snapshot_native_owned = core_snapshot_native_owned
        self._legacy_query_store_selected = bool(
            getattr(core_snapshot_selection, 'query_store_configured', False))
        if (core_snapshot_selection is not None or core_snapshot_admission_provider is not None
                or core_snapshot_native_owned or self._core_snapshot_release_root is not None):
            if core_snapshot_selection is None or (
                    core_snapshot_admission_provider is None and not core_snapshot_native_owned):
                raise ValueError('native whole-Core selection requires a captured selector and its declared owner route')
            from .native_core_snapshot import NativeCoreSnapshotClient, NativeCoreSnapshotSelection
            if not isinstance(core_snapshot_selection, NativeCoreSnapshotSelection):
                raise TypeError('native whole-Core selection must be captured by NativeCoreSnapshotSelection')
            if self.tos_root is None or core_snapshot_selection.tos_root != self.tos_root:
                raise ValueError('native whole-Core selection must use the exact selected tos_root')
            if pair[0] is not None:
                raise ValueError('native whole-Core bridge requires the raw source route, not a prepared reader')
            if self.release_root is not None:
                raise ValueError('generic ManagedRelease and SourceRoot are separate native routes')
            if self._core_snapshot_release_root is not None and (
                    not core_snapshot_native_owned or core_snapshot_snapshot_root is None
                    or core_snapshot_admission_provider is not None):
                raise ValueError('ReferenceRelease requires the native-owned paired snapshot route')
            if core_snapshot_native_owned:
                self._core_snapshot_client = NativeCoreSnapshotClient(
                    prefix, core_snapshot_selection, None,
                    search_read_model=self._search_read_model_options,
                    snapshot_root=core_snapshot_snapshot_root,
                    release_root=self._core_snapshot_release_root,
                    expected_reference_release_guard=core_snapshot_expected_reference_guard)
            else:
                self._core_snapshot_client = NativeCoreSnapshotClient(
                    prefix, core_snapshot_selection, core_snapshot_admission_provider)
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
        concept_file, concept_total = concept_max_file_bytes, concept_max_total_file_bytes
        if (concept_file is None) != (concept_total is None):
            raise ValueError('native concept file budgets require a pair')
        if concept_file is not None:
            if self.tos_root is None:
                raise ValueError('native concept file budgets require an explicit source root')
            if (type(concept_file) is not int or type(concept_total) is not int
                    or not 0 < concept_file <= concept_total <= 2**64 - 1):
                raise ValueError('invalid explicit native concept file budgets')
        word_arguments = ['--root', str(self.tos_root)] if self.tos_root is not None else []
        if concept_file is not None:
            word_arguments += ['--concept-max-file-bytes', str(concept_file),
                               '--concept-max-total-file-bytes', str(concept_total)]
        self._word_core = NativeCore(prefix, word_arguments, inherit_data_selection=False)

    @classmethod
    def owned_ordinary_source_session(cls, native_prefix, selection, transport, state, *,
                                      cancelled, maximum_owner_objects, config=None):
        # Delegate to the existing native-owned ordinary session factory.
        from .native_core_session_factory import owned_native_ordinary_source_session
        return owned_native_ordinary_source_session(prefix=native_prefix,
            selection=selection, transport=transport, state=state,
            cancelled=cancelled, config=config,
            maximum_owner_objects=maximum_owner_objects)

    @classmethod
    def owned_source_session(cls, native_prefix, selection, admission, state, *,
                             cancelled, maximum_owner_objects, config=None):
        """Enter the ordinary Linux child-owned SourceRoot session.

        This exact capability scope returns its typed session client. The
        original receiving-state owner belongs to the maintained SDK setup
        envelope; it does not issue a Stage grant. Supported methods come from
        the actual native ready receipt, currently GraphViews query/read and
        paired native render when explicitly advertised. Other Core methods
        remain outside this scope, and the public default alias is unchanged.
        """
        from .native_core_session_factory import owned_native_source_session
        return owned_native_source_session(prefix=native_prefix, selection=selection,
            admission=admission, state=state, cancelled=cancelled, config=config,
            maximum_owner_objects=maximum_owner_objects)

    @classmethod
    def owned_discovered_source_session(cls, native_prefix, prepared, admission, state,
            *, cancelled, carrier_selectors, maximum_owner_objects, tos_root=None,
            query_store_path=None, config=None, selected_probes=False, lazy_selected=False):
        """Use ordinary selectors inside the original owned Linux SDK scope.

        Prepare OS discovery before creating state. Existing paths and captured
        Root/probe callbacks remain native-owned. This returns an owned context,
        not a default alias replacement or a per-method renewed admission.
        """
        from .native_core_session_factory import owned_native_discovered_source_session
        return owned_native_discovered_source_session(prefix=native_prefix,
            prepared=prepared, admission=admission, state=state, cancelled=cancelled,
            carrier_selectors=carrier_selectors,
            maximum_owner_objects=maximum_owner_objects, tos_root=tos_root,
            query_store_path=query_store_path, config=config,
            selected_probes=selected_probes, lazy_selected=lazy_selected)

    @classmethod
    def from_source_root(cls, native_prefix: str | Path, selection,
                         admission_provider, **independent_routes):
        """Bind seven exact paths and a borrowed per-operation owner profile.

        Selection is explicit; this constructor reads no source payload and
        does not infer a stage ticket, quota, deadline, or publication grant.
        """
        from .native_core_snapshot import NativeCoreSnapshotSelection
        if not isinstance(selection, NativeCoreSnapshotSelection):
            raise TypeError('SourceRoot requires NativeCoreSnapshotSelection')
        return cls(native_prefix, tos_root=selection.tos_root,
                   core_snapshot_selection=selection,
                   core_snapshot_admission_provider=admission_provider,
                   **independent_routes)

    @classmethod
    def from_legacy_query_store(cls, native_prefix: str | Path, selection,
                                admission_provider, **independent_routes):
        """Select an existing immutable store with five native input bindings.

        No database or source payload is read here. Each call borrows its original
        admission; the native owner authenticates the selected store and input
        digests, and retains the store/process/resource fences through delivery.
        Graph/Snapshot exports use the weak zero-state-FD QueryStore profile.
        """
        from .native_core_snapshot import (
            NativeCoreSnapshotSelection, NativeCoreSnapshotAdmission,
            NativeCoreQueryStoreLimits, _SOURCE_OPERATION)
        if not isinstance(selection, NativeCoreSnapshotSelection):
            raise TypeError('LegacyQueryStore requires NativeCoreSnapshotSelection')
        if selection.query_store_configured is not True:
            raise ValueError('LegacyQueryStore requires an explicitly configured existing store')
        if not callable(admission_provider):
            raise TypeError('LegacyQueryStore requires its per-call admission provider')
        separate_source_operations = frozenset(_SOURCE_OPERATION.values()) - {
            'tos_knowledge_header', 'tos_corpus_header'}
        def store_admission(operation):
            if operation in separate_source_operations or operation in {
                    'tos_knowledge_snapshot_once', 'tos_native_resource_read'}:
                raise ValueError('LegacyQueryStore does not select a captured SourceRoot operation')
            admission = admission_provider(operation)
            if not isinstance(admission, NativeCoreSnapshotAdmission):
                raise TypeError('LegacyQueryStore admission must be NativeCoreSnapshotAdmission')
            if not isinstance(admission.query_store_limits, NativeCoreQueryStoreLimits):
                raise TypeError('LegacyQueryStore requires typed original per-call QueryStore limits')
            # Neither a deadline nor a quota is synthesized or renewed here.
            return admission
        core = cls(native_prefix, tos_root=selection.tos_root,
                   core_snapshot_selection=selection,
                   core_snapshot_admission_provider=store_admission,
                   **independent_routes)
        core._legacy_query_store_selected = True
        return core


    @classmethod
    def owned_selected_probe_session(cls, native_prefix, selection, admission, state,
                                     *, cancelled, maximum_owner_objects, config=None):
        """Observe four selected paths without a full graph/capture prerequisite.

        The same genuine child issuer and original typed limits own the scope.
        QueryStore is bypassed exactly for these four metadata observations.
        The explicit profile uses CPython3.14 Reference is_file semantics; no
        older-runtime error parity or public default replacement is implied.
        """
        from .native_core_session_factory import owned_native_source_session
        return owned_native_source_session(prefix=native_prefix, selection=selection,
            admission=admission, state=state, cancelled=cancelled, config=config,
            maximum_owner_objects=maximum_owner_objects, _selected_probe=True)

    @classmethod
    def owned_lazy_selected_session(cls, native_prefix, selection, admission, state,
                                    *, cancelled, maximum_owner_objects, config=None):
        """Keep one native selected-path/Store/carrier Driver under original owners.

        Exactly the native-declared nine lower methods are available. Source,
        data and exploration bindings are native-issued receipt fields. This
        does not replace the public Reference alias or mint admission.
        """
        from .native_core_session_factory import owned_native_source_session
        return owned_native_source_session(prefix=native_prefix, selection=selection,
            admission=admission, state=state, cancelled=cancelled, config=config,
            maximum_owner_objects=maximum_owner_objects, _selected_lazy=True)

    @classmethod
    def discover(cls, tos_root: str | Path | None = None, *,
                 native_prefix: str | Path | None = None,
                 core_snapshot_native_owned: bool = False,
                 **selection: Any) -> 'NativeAccessCore':
        """Select explicit native software; never infer it from CWD or data."""
        return cls(native_prefix, tos_root=tos_root,
                   core_snapshot_native_owned=core_snapshot_native_owned,
                   **selection)

    def knowledge_search_indexed(self, query: str = '', *, sources=None, kind_ids=None,
                                 predicate_ids=None, cursor=None, limit: int = 40) -> dict:
        if self._core_snapshot_native_owned:
            return self._packet('tos_knowledge_search_indexed_v2', {
                'query': query, 'sources': sources, 'kind_ids': kind_ids,
                'predicate_ids': predicate_ids, 'cursor': cursor, 'limit': limit,
            }, source_errors=False)
        # Explicit admission preserves its original request surface. In the
        # native-owned route, the full selector was captured by the client
        # above so its QueryStore owner can ignore path/build caps but honor
        # verify-char limits.
        explicit_provider_route = (self._core_snapshot_client is not None)
        search = None if explicit_provider_route else self._search_read_model_options
        return super().knowledge_search_indexed(query, sources=sources,
            kind_ids=kind_ids, predicate_ids=predicate_ids, cursor=cursor,
            limit=limit, search_read_model=search)

    def _embedded_source(self, operation, request):
        if self._selected_source_provider:
            # Retain the borrowed provider and count active native calls without
            # serializing its two nonqueued slots. Core close joins this lifetime.
            deadline = time.monotonic() + 50
            with self._selected_calls_condition:
                self._ensure_open()
                provider = self._source_provider
                self._selected_calls += 1
            try:
                packet = provider.call(operation, request, absolute_deadline=deadline)
                self._ensure_open()
                if time.monotonic() >= deadline:
                    raise TimeoutError('selected source deadline expired before disclosure')
                return packet
            finally:
                with self._selected_calls_condition:
                    self._selected_calls -= 1
                    self._selected_calls_condition.notify_all()
        # Explicit synchronous provider profile preserves owner callback latency;
        # it does not use the selected native profile's hard whole-call 50s clock.
        with self._lifetime_lock:
            self._ensure_open()
            return self._source_provider.call(operation, request)

    def source_read_capabilities(self):
        if self._source_provider is None:
            return super().source_read_capabilities()
        return self._embedded_source('capabilities', {})

    def source_handle_discover(self, request):
        if self._source_provider is None:
            return super().source_handle_discover(request)
        return self._embedded_source('discover', request)

    def source_read(self, request):
        if self._source_provider is None:
            return super().source_read(request)
        return self._embedded_source('read', request)

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


    def _packet(self, tool, request, *, absolute_deadline=None, source_errors=True):
        client = getattr(self, '_core_snapshot_client', None)
        # These independently selected exact-source and private Reading/Word
        # owners retain their existing native routes and lifetime contracts.
        independent = (tool.startswith('tos_source_handle_') or tool == 'tos_source_read'
                       or tool.startswith('tos_source_read_')
                       or tool.startswith('tos_zarathustra_')
                       # Software schemas are served by the selected native
                       # executor even when no corpus can be admitted.
                       or tool == 'tos_knowledge_exploration_contracts')
        if client is None or independent:
            return super()._packet(tool, request, absolute_deadline=absolute_deadline,
                                   source_errors=source_errors)
        # The borrowed admission supplies the original per-call cutoff; an
        # outer caller's earlier cutoff must remain an additional restriction.
        try:
            return self._core_snapshot_call('call', tool, request,
                                            absolute_deadline=absolute_deadline)
        except Exception as error:
            from .native_core_session import NativeSessionRefused
            if not isinstance(error, NativeSessionRefused):
                raise
            if source_errors:
                from .source_read_errors import SourceReadError
                wrapped = SourceReadError(str(error))
                wrapped.code = error.code
                raise wrapped from error
            from mcp.server.fastmcp.exceptions import ToolError
            wrapped = ToolError(str(error))
            wrapped.code = error.code
            raise wrapped from error

    def _core_snapshot_call(self, method, *args, **kwargs):
        with self._lifetime_lock:
            self._ensure_open()
            client = self._core_snapshot_client
            if client is None:
                raise RuntimeError('native whole-Core route requires an explicit source selection and caller admission')
            return getattr(client, method)(*args, **kwargs)

    def read_resource(self, uri: str) -> dict[str, Any]:
        if self._legacy_query_store_selected:
            raise ValueError('LegacyQueryStore resources require a separately selected captured SourceRoot')
        if self._core_snapshot_client is None:
            return super().read_resource(uri)
        return self._core_snapshot_call('read_resource', uri)

    def render_resource(self, uri: str) -> str:
        if self._legacy_query_store_selected:
            raise ValueError('LegacyQueryStore resources require a separately selected captured SourceRoot')
        if self._core_snapshot_client is None:
            return super().render_resource(uri)
        return self._core_snapshot_call('render_resource', uri)

    def index_exists(self) -> bool:
        return self._core_snapshot_call('index_exists')

    def index(self) -> dict[str, Any]:
        return self._core_snapshot_call('index')

    def source_navigation(self, *, bibliographic_only: bool = False) -> dict[str, Any]:
        return self._core_snapshot_call('source_navigation', bibliographic_only=bibliographic_only)

    def bibliographic_graph(self) -> dict[str, Any]:
        return self._core_snapshot_call('bibliographic_graph')

    def evidence_projection_exists(self) -> bool:
        return self._core_snapshot_call('evidence_projection_exists')

    def evidence_projection(self) -> dict[str, Any]:
        return self._core_snapshot_call('evidence_projection')

    def corpus_header(self) -> dict[str, Any]:
        return self._core_snapshot_call('corpus_header')

    def knowledge_header(self) -> dict[str, Any]:
        return self._core_snapshot_call('knowledge_header')

    def philosophy_projection_exists(self) -> bool:
        return self._core_snapshot_call('philosophy_projection_exists')

    def philosophy_projection(self) -> dict[str, Any]:
        if self._core_snapshot_client is None:
            return super().philosophy_projection()
        return self._core_snapshot_call('philosophy_projection')

    def philosophy_audit_exists(self) -> bool:
        return self._core_snapshot_call('philosophy_audit_exists')

    def philosophy_audit_payload(self) -> dict[str, Any]:
        return self._core_snapshot_call('philosophy_audit_payload')

    def knowledge_graph(self) -> dict[str, Any]:
        return self._core_snapshot_call('knowledge_graph')

    def knowledge_graph_addressed(self, previous_graph: dict[str, Any], source_graph: str,
                                  source_id: str, source_record: dict[str, Any], *,
                                  source_revision: str, expected_parent_revision: str | None = None,
                                  return_report: bool = False) -> dict[str, Any]:
        return self._core_snapshot_call(
            'knowledge_graph_addressed', previous_graph, source_graph, source_id,
            source_record, source_revision=source_revision,
            expected_parent_revision=expected_parent_revision, return_report=return_report)

    def knowledge_snapshot(self) -> dict[str, Any]:
        return self._core_snapshot_call('knowledge_snapshot')

    def knowledge_snapshot_once(self) -> dict[str, Any]:
        return self._core_snapshot_call('knowledge_snapshot_once')

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
            binding_state, self._owned_binding_state = self._owned_binding_state, None
            try:
                with self._selected_calls_condition:
                    while self._selected_calls:
                        self._selected_calls_condition.wait()
                if self._source_provider is not None and self._owns_source_provider:
                    self._source_provider.close()
                self._source_provider = None
                core_snapshot, self._core_snapshot_client = self._core_snapshot_client, None
                if core_snapshot is not None:
                    core_snapshot.close()
                owned, self._ephemeral_state = self._ephemeral_state, None
                if owned is not None:
                    owned.cleanup()
            finally:
                if binding_state is not None:
                    binding_state.cleanup()

    def __enter__(self):
        with self._lifetime_lock:
            self._ensure_open()
            return self

    def __exit__(self, *exc):
        self.close()
