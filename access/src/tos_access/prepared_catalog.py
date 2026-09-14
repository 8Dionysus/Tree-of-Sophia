"""Offline catalog maintenance joined to an explicitly selected prepared file.

The caller supplies dependency-complete normalized changes and the new owner
header, including its semantic report. This joins existing catalog and prepared
storage operations; it neither assembles sources nor admits their meaning.
All entry points require an existing caller transaction and require whole
rollback on any exception. There is no request-time bootstrap or selection.
"""
from __future__ import annotations

from dataclasses import replace
import sqlite3

from . import knowledge as k
from .catalog_index import CatalogIndex, CatalogLimits
from .catalog_semantics import (CANONICAL_ORDER, CatalogInputs, CatalogRow,
    CatalogChange, catalog_digest, finalized_header)
from .prepared_publication import (PublicationLimits, PreparedChange, SCHEMA,
    _COLUMNS, _cap, _hash, _metadata, _row, apply_prepared_delta_transaction)
from .published_read_metadata import (CATALOG_KEY, TOP_KEY, emitted_row_digest,
    published_row_digest_key, published_snapshot_binding)
from .published_read_model import _json


def _selected(db, expected, inputs, limits):
    if not db.in_transaction:
        raise ValueError('catalogued publication requires a caller-owned transaction')
    if not isinstance(inputs, CatalogInputs):
        raise ValueError('explicit CatalogInputs required')
    top = _metadata(db, TOP_KEY)
    clock = db.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()
    if (top.get('read_model_schema') != SCHEMA or clock is None
            or published_snapshot_binding(top, clock[0]) != expected
            or _metadata(db, 'data_revision') != {'sha256': top['data_revision']}):
        raise ValueError('stale or foreign catalogued publication')
    state = db.execute('SELECT max_pages,CASE WHEN length(CAST(descriptor AS BLOB))<=? '
                       'THEN descriptor ELSE NULL END FROM prepared_state WHERE singleton=1',
                       (limits.max_metadata_bytes,)).fetchone()
    if state is None or state[1] is None:
        raise ValueError('prepared descriptor absent or over maintenance budget')
    descriptor = _json(state[1])
    if (not isinstance(descriptor, dict) or _hash(descriptor) != top['data_revision']
            or catalog_digest(descriptor.get('header')) != inputs.header_digest):
        raise ValueError('catalog input header differs from selected prepared descriptor')
    normalization = inputs.header.get('normalization_binding', {})
    if (normalization != top['normalization_binding']
            or normalization.get('entity_registry_digest') != k._stable_digest(inputs.entity_type_registry)
            or normalization.get('relation_registry_digest') != k._stable_digest(inputs.relation_type_registry)):
        raise ValueError('catalog registries differ from selected normalization binding')
    catalog = _metadata(db, CATALOG_KEY)
    if _hash(catalog) != top['catalog_sha256']:
        raise ValueError('selected prepared catalog digest differs')
    _cap(db, limits, state[0])
    return catalog


def _body(db, kind, identifier, limits):
    # Only an exact key is selected. A size guard precedes Python allocation.
    columns = _COLUMNS[kind]
    found = db.execute(f'SELECT {",".join(columns)},'
        f'CASE WHEN length(CAST(json AS BLOB))<=? THEN json ELSE NULL END '
        f'FROM knowledge_{kind}s WHERE id=?', (limits.max_row_bytes, identifier)).fetchone()
    if found is None:
        raise ValueError('selected prepared maintenance row is absent')
    raw = found[-1]
    if not isinstance(raw, str):
        raise ValueError('selected prepared maintenance row exceeds byte budget')
    if _metadata(db, published_row_digest_key(kind, identifier)) != emitted_row_digest(raw):
        raise ValueError('selected prepared maintenance row checksum differs')
    item = _json(raw)
    if (not isinstance(item, dict)
            or any(str(item.get(key) or '') != value for key, value in zip(columns, found))):
        raise ValueError('selected prepared maintenance identity columns differ')
    return item


def _source_order(inputs, item, token):
    return ((str(item.get('source_graph')), str(item.get('id')))
            if inputs.source_order_profile == CANONICAL_ORDER else token)


def bootstrap_prepared_catalog_transaction(db: sqlite3.Connection, *, expected_binding: dict,
        inputs: CatalogInputs, limits: PublicationLimits | None = None,
        catalog_limits: CatalogLimits | None = None) -> dict:
    """Explicit one-time index attachment; existing publication stays identical.

    Stream the selected full rows once, retaining only the current row. This is
    full offline bootstrap, not an addressed update. The catalog must reproduce
    the already selected catalog and final owner header before the caller can
    commit. No row/search/header/clock is changed and no new binding is selected.
    """
    limits = limits or PublicationLimits()
    before_changes = db.total_changes
    expected_catalog = _selected(db, expected_binding, inputs, limits)

    def rows():
        for kind in _COLUMNS:
            # Sparse tokens are owner source order; numeric doc IDs are not.
            cursor = db.execute('SELECT id,source_order FROM prepared_documents '
                                'WHERE kind=? ORDER BY source_order,doc_id', (kind,))
            for identifier, token in cursor:
                item = _body(db, kind, identifier, limits)
                yield CatalogRow(kind, identifier, _source_order(inputs, item, token), item)
                if db.total_changes - before_changes > limits.max_mutations:
                    raise ValueError('joined catalog bootstrap mutation budget exceeded')

    catalog = CatalogIndex(db, catalog_limits).bootstrap(inputs, rows())
    if (catalog_digest(catalog) != catalog_digest(expected_catalog)
            or catalog_digest(finalized_header(inputs, catalog)) != inputs.header_digest):
        raise ValueError('catalog index does not reproduce the selected catalog/header')
    _cap(db, limits)
    if db.total_changes - before_changes > limits.max_mutations:
        raise ValueError('joined catalog bootstrap mutation budget exceeded')
    return {'binding': expected_binding.copy(), 'catalog_digest': catalog_digest(catalog),
            'sql_mutations': db.total_changes - before_changes,
            'publication_changed': False, 'consumer_switched': False}


def apply_catalogued_prepared_delta_transaction(db: sqlite3.Connection, *, expected_binding: dict,
        before_inputs: CatalogInputs, after_inputs: CatalogInputs, changes,
        limits: PublicationLimits | None = None,
        catalog_limits: CatalogLimits | None = None) -> dict:
    """Join exact catalog contributors, full carriers, lens and search atomically.

    Insertions still require the prepared owner's explicit sparse source-order
    token. Canonical catalog order is independently derived from (source,id);
    owner-sequence catalog order uses the same token as search. Neither profile
    infers source membership or verifies dependency-complete normalization.
    The after input is a draft only for the existing row-owned count fields.
    Its semantic report and unknown fields remain explicit caller-owned data.
    Use the returned final header, not the draft, for the next transition.
    """
    limits = limits or PublicationLimits()
    start = db.total_changes
    catalog_before = _selected(db, expected_binding, before_inputs, limits)
    if not isinstance(after_inputs, CatalogInputs):
        raise ValueError('explicit after CatalogInputs required')
    retained, catalog_changes, seen, total_bytes = [], [], set(), 0
    for change in changes:
        if len(retained) >= limits.max_changes:
            raise ValueError('joined catalog delta change budget exceeded')
        if (not isinstance(change, PreparedChange) or change.kind not in _COLUMNS
                or change.operation not in ('insert', 'update', 'delete')):
            raise ValueError('explicit prepared changes required')
        key = (change.kind, change.identifier)
        if not isinstance(change.identifier, str) or not change.identifier or key in seen:
            raise ValueError('duplicate or invalid joined catalog target')
        seen.add(key)
        stored = db.execute('SELECT source_order FROM prepared_documents WHERE kind=? AND id=?', key).fetchone()
        if (stored is None) != (change.operation == 'insert'):
            raise ValueError('joined catalog operation differs from selected row')
        old = None if stored is None else _body(db, *key, limits)
        if old is not None:
            total_bytes += len(_row(change.kind, old, limits).encode('utf-8'))
        if change.operation == 'delete':
            if change.item is not None or change.source_order is not None:
                raise ValueError('deletion cannot carry a replacement or order')
            item, order = None, None
        else:
            raw = _row(change.kind, change.item, limits)
            total_bytes += len(raw.encode('utf-8'))
            if change.item['id'] != change.identifier:
                raise ValueError('joined catalog replacement identity differs')
            # Copy the bounded input: an external iterator cannot mutate an
            # earlier item between the catalog and carrier write passes.
            item = _json(raw)
            token = change.source_order if change.source_order is not None else (stored[0] if stored else None)
            if type(token) is not int or not 0 <= token <= 9_007_199_254_740_991:
                raise ValueError('explicit valid insertion source order required')
            order = _source_order(after_inputs, item, token)
        if total_bytes > limits.max_change_bytes:
            raise ValueError('joined catalog delta selected byte budget exceeded')
        retained.append(PreparedChange(change.operation, change.kind, change.identifier, item, change.source_order))
        catalog_changes.append(CatalogChange(change.operation, change.kind, change.identifier,
            None if old is None else catalog_digest(old), item, order))
    catalog = CatalogIndex(db, catalog_limits).apply_delta(before_inputs, after_inputs, catalog_changes,
        expected_catalog_digest=catalog_digest(catalog_before))
    header = finalized_header(after_inputs, catalog)
    remaining = limits.max_mutations - (db.total_changes - start)
    if remaining < 1:
        raise ValueError('joined catalog delta mutation budget exceeded')
    binding = apply_prepared_delta_transaction(db, expected_binding=expected_binding,
        source_header=header, catalog=catalog, changes=retained,
        limits=replace(limits, max_mutations=remaining))
    if db.total_changes - start > limits.max_mutations:
        raise ValueError('joined catalog delta mutation budget exceeded')
    return {'binding': binding, 'source_header': header, 'catalog': catalog,
            'sql_mutations': db.total_changes - start,
            'source_transition_verified': False, 'semantic_acceptance': False,
            'consumer_switched': False}
