"""Concrete native file owners for joined catalog/semantic maintenance.

Reference Connection APIs remain in prepared_catalog/prepared_semantics. This
adapter sends owned JSON, never a Python Connection, and acknowledges the actual
caller's final source check before the native transaction can commit.
"""
from __future__ import annotations
import dataclasses
from pathlib import Path
import time

from .catalog_index import CatalogLimits
from .catalog_semantics import CatalogInputs, owner_json
from .prepared_native import HEADER_FRAME_BYTES, _exchange, select_publication_executor
from .prepared_publication import PreparedChange, PublicationLimits
from .semantic_index import SemanticIndexLimits


def _inputs(value):
    if not isinstance(value, CatalogInputs):
        raise ValueError('exact CatalogInputs required for native maintenance')
    return {'header': value.header, 'entity_registry': value.entity_type_registry,
            'relation_registry': value.relation_type_registry, 'lenses': value.lenses,
            'source_order_profile': value.source_order_profile}


def native_maintenance(path, *, operation, expected_binding, inputs=None,
                       before_inputs=None, after_inputs=None, changes=None,
                       ordered_rows=None, limits=None, catalog_limits=None,
                       semantic_limits=None, normalization_processor_sha256=None,
                       check_source_current=None, native_executable=None,
                       native_timeout=None):
    executable, timeout = select_publication_executor(native_executable, native_timeout)
    deadline = time.monotonic() + timeout
    limits = limits or PublicationLimits()
    catalog = catalog_limits or CatalogLimits()
    semantic = semantic_limits or SemanticIndexLimits()
    if not isinstance(limits, PublicationLimits) or not isinstance(catalog, CatalogLimits):
        raise ValueError('exact publication/catalog maintenance limits required')
    if not isinstance(semantic, SemanticIndexLimits):
        raise ValueError('exact semantic maintenance limits required')
    if not callable(check_source_current):
        raise ValueError('native maintenance requires the actual caller precommit source check')
    if operation not in ('maintenance-bootstrap', 'catalogued-delta', 'semantic-delta'):
        raise ValueError('unknown native maintenance operation')
    if operation != 'catalogued-delta' and (not isinstance(normalization_processor_sha256, str)
            or len(normalization_processor_sha256) != 64
            or any(c not in '0123456789abcdef' for c in normalization_processor_sha256)):
        raise ValueError('exact selected normalization processor digest required')
    if operation == 'maintenance-bootstrap':
        if changes is not None or before_inputs is not None or after_inputs is not None:
            raise ValueError('bootstrap cannot carry delta inputs')
        if ordered_rows is not None and not callable(ordered_rows):
            raise ValueError('explicit ordered maintenance row factory required')
    elif changes is None or ordered_rows is not None or inputs is not None:
        raise ValueError('delta requires changes and exact before/after inputs')
    check_source_current()
    control = {'operation': operation, 'path': str(Path(path).absolute()),
               'limits': dataclasses.asdict(limits), 'catalog_limits': dataclasses.asdict(catalog),
               'semantic_limits': dataclasses.asdict(semantic), 'expected_binding': expected_binding,
               'max_seconds': timeout, 'ordered_rows': ordered_rows is not None,
               'normalization_processor_sha256': normalization_processor_sha256}

    def encoded(value, cap):
        raw = owner_json(value).encode('utf-8')
        if len(raw) > cap:
            raise ValueError('native maintenance input frame byte budget exceeded')
        if time.monotonic() >= deadline:
            raise TimeoutError('native maintenance whole-operation deadline exceeded')
        return raw + b'\n'

    # Control and owner inputs use separate existing bounded frames: two delta
    # snapshots must not coexist inside one fixed-size control frame.
    first = encoded(control, HEADER_FRAME_BYTES)
    selected_inputs = ([('inputs', inputs)] if operation == 'maintenance-bootstrap'
                       else [('before_inputs', before_inputs), ('after_inputs', after_inputs)])
    captured = [(key, encoded({key: _inputs(value)}, HEADER_FRAME_BYTES))
                for key, value in selected_inputs]

    def frames():
        yield first
        for _, raw in captured:
            yield raw
        if operation == 'maintenance-bootstrap':
            if ordered_rows is not None:
                if not callable(ordered_rows):
                    raise ValueError('explicit ordered maintenance row factory required')
                count = 0
                for kind in ('node', 'relation'):
                    for row in ordered_rows(kind):
                        count += 1
                        if count > semantic.max_rows:
                            raise ValueError('native maintenance input row budget exceeded')
                        yield encoded({'row': row}, min(limits.max_row_bytes, semantic.max_row_bytes) + 32)
                    yield b'{"end":true}\n'
        else:
            size = 0
            for count, change in enumerate(changes, 1):
                if count > limits.max_changes or not isinstance(change, PreparedChange):
                    raise ValueError('exact bounded PreparedChange sequence required')
                value = {key: getattr(change, key) for key in
                         ('operation', 'kind', 'identifier', 'item', 'source_order')}
                raw = encoded(value, limits.max_row_bytes + 32768)
                size += len(raw)
                if size > limits.max_change_bytes + limits.max_metadata_bytes:
                    raise ValueError('native maintenance retained changes byte budget exceeded')
                yield raw
            yield b'{"end":true}\n'

    def precommit(report):
        if report != {'phase': 'maintenance_precommit', 'committed': False}:
            raise ValueError('invalid native maintenance precommit report')
        check_source_current()

    output_cap = limits.max_metadata_bytes + catalog.max_catalog_bytes + 2 * semantic.max_output_bytes + 65536
    result = _exchange(executable, timeout, deadline, frames, callback=precommit,
                       output_cap=output_cap, progress_phases={'maintenance_precommit'})
    if (not isinstance(result.get('binding'), dict) or result.get('consumer_switched') is not False
            or result.get('source_transition_verified') is not False
            or result.get('semantic_acceptance') is not False
            or type(result.get('sql_mutations')) is not int or result['sql_mutations'] < 0):
        raise ValueError('native maintenance returned invalid owner result')
    return result
