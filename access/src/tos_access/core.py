"""Python SDK adapter for the installed native Tree of Sophia access product."""
from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from .locations import data_root, source_carrier_paths
from .native_access_core import (
    NativeAccessCore as _NativeAccessCore,
    SEARCH_READ_MODEL_MAX_POSTINGS, SEARCH_READ_MODEL_MAX_VERIFY_CHARS,
)
from .native_core_errors import DataAccessUnavailable


def _discover_root(explicit: str | Path | None = None) -> Path:
    return data_root(explicit)




class NativeToSAccessCore(_NativeAccessCore):
    """Thin SDK facade over the installed native Core.

    It selects explicit paths and delegates reads, admission, and compatibility
    checks to the native product. It contains no query implementation.
    """

    _CARRIER_FIELDS = (
        'index_path', 'philosophy_graph_projection_path',
        'bibliographic_graph_path', 'entity_type_registry_path',
        'relation_type_registry_path', 'philosophy_post_planting_audit_path',
        'evidence_projection_path',
    )
    _CARRIER_ENV = (
        'TOS_CORPUS_INDEX_PATH', 'TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH',
        'TOS_BIBLIOGRAPHIC_GRAPH_PATH', 'TOS_ENTITY_TYPE_REGISTRY_PATH',
        'TOS_RELATION_TYPE_REGISTRY_PATH', 'TOS_PHILOSOPHY_POST_PLANTING_AUDIT_PATH',
        'TOS_EVIDENCE_PROJECTION_PATH',
    )

    @classmethod
    def _has_nondefault_carrier_override(cls, root: Path, selectors: dict[str, Any]) -> bool:
        if any(os.environ.get(name) for name in cls._CARRIER_ENV):
            return True
        defaults = source_carrier_paths(root)
        for name, value in selectors.items():
            if value is None:
                continue
            selected = Path(value).expanduser()
            if not selected.is_absolute():
                selected = root / selected
            if selected.resolve() != defaults[name]:
                return True
        return False

    def __init__(
        self,
        tos_root: str | Path | None = None,
        index_path: str | Path | None = None,
        philosophy_graph_projection_path: str | Path | None = None,
        bibliographic_graph_path: str | Path | None = None,
        entity_type_registry_path: str | Path | None = None,
        relation_type_registry_path: str | Path | None = None,
        philosophy_post_planting_audit_path: str | Path | None = None,
        evidence_projection_path: str | Path | None = None,
        search_read_model_path: str | Path | None = None,
        search_read_model_max_bytes: int | None = None,
        search_read_model_max_postings: int = SEARCH_READ_MODEL_MAX_POSTINGS,
        search_read_model_max_verify_chars: int = SEARCH_READ_MODEL_MAX_VERIFY_CHARS,
        published_read_model_path: str | Path | None = None,
        published_read_model_expected: dict[str, Any] | None = None,
        published_exploration_checkpoint_path: str | Path | None = None,
        source_provider: Any | None = None,
        native_prefix: str | Path | None = None,
        reading_analysis_root: str | Path | None = None,
        reading_max_file_bytes: int | None = None,
        reading_max_total_file_bytes: int | None = None,
        concept_max_file_bytes: int | None = None,
        concept_max_total_file_bytes: int | None = None,
        native_admission_provider: Any | None = None,
        query_store_path: str | Path | None = None,
        release_root: str | Path | None = None,
        source_read_service: Any | None = None,
    ):
        if source_provider is not None and source_read_service is not None:
            raise ValueError('select source_provider or source_read_service, not both')
        if source_provider is None:
            source_provider = source_read_service
        selectors = {
            'index_path': index_path,
            'philosophy_graph_projection_path': philosophy_graph_projection_path,
            'bibliographic_graph_path': bibliographic_graph_path,
            'entity_type_registry_path': entity_type_registry_path,
            'relation_type_registry_path': relation_type_registry_path,
            'philosophy_post_planting_audit_path': philosophy_post_planting_audit_path,
            'evidence_projection_path': evidence_projection_path,
        }
        selected_release = release_root if release_root is not None else os.environ.get('TOS_RELEASE_ROOT')
        explicit_source_root = tos_root is not None or bool(os.environ.get('TOS_DATA_ROOT'))
        reference_guard = None
        if selected_release is not None and not explicit_source_root:
            from .native_access_core import _reference_release_selection
            try:
                native_prefix, selected_root, snapshot_root, reference_guard = _reference_release_selection(
                    native_prefix, selected_release)
            except (OSError, ValueError, RuntimeError) as error:
                raise DataAccessUnavailable(str(error)) from error
        else:
            selected_root = _discover_root(tos_root)
        if selected_release is not None and selected_root is not None:
            if selected_root.name != 'data':
                raise DataAccessUnavailable(
                    'ReferenceRelease pairing requires an explicitly selected snapshot data root'
                )
            # This is path pairing only. The native ReferenceRelease guard opens
            # and validates both manifests and their member/revocation bindings.
            snapshot_root = selected_root.parent
        else:
            snapshot_root = (selected_root.parent if selected_root is not None
                             and selected_root.name == 'data'
                             and os.path.lexists(selected_root.parent / 'manifest.json') else None)
        if selected_release is not None and selected_root is not None and native_admission_provider is not None:
            raise ValueError('ReferenceRelease requires the native-owned ordinary operation route')
        if snapshot_root is not None and native_admission_provider is not None:
            raise ValueError('guarded snapshot selection requires the native-owned operation route')

        if selected_root is None:
            raise ValueError('native Core requires a selected source root')
        carrier_paths = source_carrier_paths(selected_root, **selectors)
        prepared_selected = (published_read_model_path is not None
                             or published_read_model_expected is not None)
        if selected_release is not None and (
                prepared_selected or published_exploration_checkpoint_path is not None):
            raise ValueError('ReferenceRelease guarded source and prepared/checkpoint readers are separate native routes')
        if prepared_selected and (query_store_path is not None
                                  or os.environ.get('TOS_QUERY_STORE_PATH')):
            raise ValueError('select a prepared reader or a QueryStore')
        if prepared_selected and self._has_nondefault_carrier_override(selected_root, selectors):
            raise ValueError('prepared reader does not accept independent Reference carrier overrides')
        if native_admission_provider is not None:
            if native_prefix is None:
                raise ValueError('native discovery admission requires an explicit native_prefix')
            if (search_read_model_path is not None or search_read_model_max_bytes is not None
                    or search_read_model_max_postings != SEARCH_READ_MODEL_MAX_POSTINGS
                    or search_read_model_max_verify_chars != SEARCH_READ_MODEL_MAX_VERIFY_CHARS
                    or os.environ.get('TOS_SEARCH_READ_MODEL_PATH')
                    or os.environ.get('TOS_SEARCH_READ_MODEL_MAX_BYTES')):
                raise ValueError('explicit-provider SourceRoot does not select the indexed-search sidecar')
            if prepared_selected or published_exploration_checkpoint_path is not None:
                raise ValueError('native SourceRoot discovery cannot combine a prepared reader or checkpoint')
        from .native_core_snapshot import NativeCoreSnapshotSelection
        if prepared_selected:
            selection = None
        else:
            selection = NativeCoreSnapshotSelection.discover(
                selected_root, query_store_path=query_store_path, **selectors)
        _NativeAccessCore.__init__(
            self, native_prefix, tos_root=selected_root,
            published_read_model_path=published_read_model_path,
            published_read_model_expected=published_read_model_expected,
            search_read_model_path=search_read_model_path,
            search_read_model_max_bytes=search_read_model_max_bytes,
            search_read_model_max_postings=search_read_model_max_postings,
            search_read_model_max_verify_chars=search_read_model_max_verify_chars,
            published_exploration_checkpoint_path=published_exploration_checkpoint_path,
            source_provider=source_provider,
            reading_analysis_root=reading_analysis_root,
            reading_max_file_bytes=reading_max_file_bytes,
            reading_max_total_file_bytes=reading_max_total_file_bytes,
            concept_max_file_bytes=concept_max_file_bytes,
            concept_max_total_file_bytes=concept_max_total_file_bytes,
            core_snapshot_selection=selection,
            core_snapshot_admission_provider=native_admission_provider,
            core_snapshot_native_owned=(selection is not None and native_admission_provider is None),
            core_snapshot_snapshot_root=snapshot_root,
            core_snapshot_release_root=selected_release,
            core_snapshot_expected_reference_guard=reference_guard,
        )
        root = self.tos_root
        query_path = None if selection is None else selection.query_store_path

        for name, path in carrier_paths.items():
            setattr(self, name, path)
        self.query_store_path = query_path
        self.search_read_model_path = (
            None if self._search_read_model_options is None
            else Path(self._search_read_model_options['path'])
        )
        self.search_read_model_max_bytes = (
            None if self._search_read_model_options is None
            else self._search_read_model_options['max_bytes']
        )
        self.search_read_model_max_postings = (
            None if self._search_read_model_options is None
            else self._search_read_model_options['max_postings']
        )
        self.search_read_model_max_verify_chars = (
            None if self._search_read_model_options is None
            else self._search_read_model_options['max_verify_chars']
        )
        self.published_read_model_path = (
            None if published_read_model_path is None else Path(published_read_model_path)
        )
        self.published_read_model_expected = published_read_model_expected
        self.published_exploration_checkpoint_path = (
            None if published_exploration_checkpoint_path is None
            else Path(published_exploration_checkpoint_path)
        )
        self.source_provider = source_provider
        self.reading_analysis_root = (
            None if reading_analysis_root is None else Path(reading_analysis_root)
        )
        self.reading_max_file_bytes = reading_max_file_bytes
        self.reading_max_total_file_bytes = reading_max_total_file_bytes
        self.concept_max_file_bytes = concept_max_file_bytes
        self.concept_max_total_file_bytes = concept_max_total_file_bytes

    @classmethod
    def discover(
        cls, tos_root: str | Path | None = None,
        index_path: str | Path | None = None,
        philosophy_graph_projection_path: str | Path | None = None,
        bibliographic_graph_path: str | Path | None = None,
        entity_type_registry_path: str | Path | None = None,
        relation_type_registry_path: str | Path | None = None,
        philosophy_post_planting_audit_path: str | Path | None = None,
        evidence_projection_path: str | Path | None = None,
        search_read_model_path: str | Path | None = None,
        search_read_model_max_bytes: int | None = None,
        search_read_model_max_postings: int = SEARCH_READ_MODEL_MAX_POSTINGS,
        search_read_model_max_verify_chars: int = SEARCH_READ_MODEL_MAX_VERIFY_CHARS,
        published_read_model_path: str | Path | None = None,
        published_read_model_expected: dict[str, Any] | None = None,
        published_exploration_checkpoint_path: str | Path | None = None,
        source_provider: Any | None = None,
        native_prefix: str | Path | None = None,
        reading_analysis_root: str | Path | None = None,
        reading_max_file_bytes: int | None = None,
        reading_max_total_file_bytes: int | None = None,
        concept_max_file_bytes: int | None = None,
        concept_max_total_file_bytes: int | None = None,
        native_admission_provider: Any | None = None,
        query_store_path: str | Path | None = None,
        release_root: str | Path | None = None,
        source_read_service: Any | None = None,
    ) -> 'NativeToSAccessCore':
        return cls(
            tos_root, index_path, philosophy_graph_projection_path,
            bibliographic_graph_path, entity_type_registry_path,
            relation_type_registry_path, philosophy_post_planting_audit_path,
            evidence_projection_path, search_read_model_path,
            search_read_model_max_bytes, search_read_model_max_postings,
            search_read_model_max_verify_chars, published_read_model_path,
            published_read_model_expected, published_exploration_checkpoint_path,
            source_provider, native_prefix, reading_analysis_root,
            reading_max_file_bytes, reading_max_total_file_bytes,
            concept_max_file_bytes, concept_max_total_file_bytes,
            native_admission_provider, query_store_path, release_root,
            source_read_service=source_read_service,
        )


# Public SDK name delegates every operation to the installed native Core.
ToSAccessCore = NativeToSAccessCore
