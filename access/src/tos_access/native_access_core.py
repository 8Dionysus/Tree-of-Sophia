"""Native imported access factory; selections never activate a reference engine.

The adapter owns absolute argv/path framing. The native selected executor owns
all authority, query validation and data compatibility. There is no graph build
or Python SourceReadService execution in this factory.
"""
from __future__ import annotations

from pathlib import Path
from typing import Any

from .native_core import NativeCore


def _selected_path(value: str | Path, name: str) -> Path:
    path = Path(value).expanduser()
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError(f'{name} requires an explicit absolute path')
    return path


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
                 reading_max_total_file_bytes: int | None = None):
        prefix = _selected_path(native_prefix, 'native_prefix')
        self.native_prefix = prefix
        self.tos_root = None if tos_root is None else _selected_path(tos_root, 'tos_root')
        self.release_root = None if release_root is None else _selected_path(release_root, 'release_root')
        pair = (published_read_model_path, published_read_model_binding_path)
        if (pair[0] is None) != (pair[1] is None):
            raise ValueError('prepared reader requires the model and owner-selected binding paths')
        if self.release_root is not None and pair[0] is not None:
            raise ValueError('select one generic release or prepared reader')
        if published_exploration_checkpoint_path is not None and pair[0] is None:
            raise ValueError('exploration checkpoints require an explicitly selected prepared reader')
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
        self._has_prepared_checkpoints = published_exploration_checkpoint_path is not None
        super().__init__(prefix, arguments)
        selectors = (reading_analysis_root, reading_max_file_bytes, reading_max_total_file_bytes)
        if self.tos_root is None:
            if any(value is not None for value in selectors):
                raise ValueError('native reading selectors require an explicit source root')
            self._reading_core = NativeCore(prefix)
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
            self._reading_core = NativeCore(prefix, reading_arguments)
            # Word observes the source root, never an independently selected
            # Reading output root or its file budget.
        self._word_core = NativeCore(prefix, ['--root', str(self.tos_root)]) if self.tos_root is not None else NativeCore(prefix)

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
        return self._word_core.zarathustra_word_analysis_task(
            query, language, rank, include_semantic_neighbors)

    def zarathustra_reading_search(self, query: str, language: str = 'ru',
                                   limit: int = 20,
                                   include_semantic_neighbors: bool = False,
                                   group_by: list[str] | None = None) -> dict:
        return self._reading_core.zarathustra_reading_search(
            query, language, limit, include_semantic_neighbors, group_by)
