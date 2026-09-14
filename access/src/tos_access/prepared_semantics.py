"""One caller transaction for semantic checks and prepared catalog maintenance.

These are mechanical reports over normalized rows, never source admission.
Source assembly, dependency-complete normalization and consumer activation stay
with their existing owners. Every exception requires complete caller rollback.
"""
from dataclasses import replace

from .addressed_replacement import _json_size
from .catalog_semantics import CatalogInputs
from .prepared_catalog import (bootstrap_prepared_catalog_transaction,
    apply_catalogued_prepared_delta_transaction)
from .prepared_publication import PreparedChange, PublicationLimits, _row
from .published_read_model import _json
from .semantic_index import (SemanticIndexLimits, bootstrap_semantic_index_transaction,
    apply_semantic_delta_transaction, verify_semantic_index_binding_transaction)


def _remaining(db, start, limits):
    remaining = limits.max_mutations - (db.total_changes - start)
    if remaining < 1:
        raise ValueError('semantic/catalog/prepared combined mutation budget exceeded')
    return remaining


def _semantic_limits(db, start, limits, semantic):
    return replace(semantic, max_bytes=min(semantic.max_bytes, limits.max_bytes),
                   max_writes=min(semantic.max_writes, _remaining(db, start, limits)))


def _capture(changes, limits):
    """Detach ONCE before passing identical native JSON to all three lanes."""
    retained, size, seen = [], 0, set()
    for change in changes:
        if len(retained) >= limits.max_changes:
            raise ValueError('joined semantic change count budget exceeded')
        if (not isinstance(change, PreparedChange) or change.kind not in ('node', 'relation')
                or change.operation not in ('insert', 'update', 'delete')
                or not isinstance(change.identifier, str) or not 1 <= len(change.identifier) <= 4096):
            raise ValueError('joined semantic delta requires exact PreparedChange targets')
        key = (change.kind, change.identifier)
        if key in seen:
            raise ValueError('joined semantic delta duplicate target')
        seen.add(key)
        size += _json_size([change.operation, change.kind, change.identifier,
                           change.source_order, change.item], limits.max_change_bytes - size)
        if change.operation == 'delete':
            if change.item is not None or change.source_order is not None:
                raise ValueError('joined semantic deletion cannot carry replacement/order')
            item = None
        else:
            item = _json(_row(change.kind, change.item, limits))
            if item['id'] != change.identifier:
                raise ValueError('joined semantic replacement identity differs')
        retained.append(PreparedChange(change.operation, change.kind, change.identifier,
                                       item, change.source_order))
    return retained


def bootstrap_prepared_maintenance_transaction(db, *, expected_binding, inputs,
        limits=None, catalog_limits=None, semantic_limits=None, ordered_rows=None):
    """Attach both indexes to an existing exact publication without changing it.

    This is explicit full offline bootstrap, not request-time repair. Its
    recomputed semantic report and catalog must equal the selected header and
    catalog. A missing/false report is not filled in by trusting its valid bit.
    """
    if not db.in_transaction or not isinstance(inputs, CatalogInputs):
        raise ValueError('caller transaction and exact CatalogInputs required')
    limits, semantic = limits or PublicationLimits(), semantic_limits or SemanticIndexLimits()
    start = db.total_changes
    report = bootstrap_semantic_index_transaction(db, binding=expected_binding,
        entity_registry=inputs.entity_type_registry, relation_registry=inputs.relation_type_registry,
        ordered_rows=ordered_rows, limits=_semantic_limits(db, start, limits, semantic))
    catalog = bootstrap_prepared_catalog_transaction(db, expected_binding=expected_binding,
        inputs=inputs, limits=replace(limits, max_mutations=_remaining(db, start, limits)),
        catalog_limits=catalog_limits)
    # Bootstrap itself leaves the semantic binding final. It does not need a
    # second finalization write, nor advance the prepared publication epoch.
    if db.total_changes - start > limits.max_mutations:
        raise ValueError('semantic/catalog/prepared combined mutation budget exceeded')
    return {**catalog, 'semantic_report': report, 'sql_mutations': db.total_changes - start,
            'source_transition_verified': False, 'semantic_acceptance': False}


def apply_semantic_prepared_delta_transaction(db, *, expected_binding, before_inputs,
        after_inputs, changes, limits=None, catalog_limits=None, semantic_limits=None):
    """Validate overlay, render catalog, publish identical changes, verify all.

    Replaces only after_header.counts.semantic_validation with the exact
    mechanical report. Other unknown header/count fields remain owner inputs.
    A report with violations remains a report; no acceptance or authority is
    inferred from either a valid or invalid report here.
    """
    if (not db.in_transaction or not isinstance(before_inputs, CatalogInputs)
            or not isinstance(after_inputs, CatalogInputs)):
        raise ValueError('caller transaction and exact before/after CatalogInputs required')
    limits, semantic = limits or PublicationLimits(), semantic_limits or SemanticIndexLimits()
    start = db.total_changes
    header = after_inputs.header
    if 'counts' in header and not isinstance(header['counts'], dict):
        raise ValueError('semantic publication requires an object counts header')
    retained = _capture(changes, limits)
    report = apply_semantic_delta_transaction(db, expected_binding=expected_binding,
        new_source_revision=header.get('source_revision'), changes=retained,
        entity_registry=after_inputs.entity_type_registry,
        relation_registry=after_inputs.relation_type_registry,
        limits=_semantic_limits(db, start, limits, semantic))
    header.setdefault('counts', {})['semantic_validation'] = report
    after = CatalogInputs(header, after_inputs.entity_type_registry,
        after_inputs.relation_type_registry, after_inputs.lenses,
        source_order_profile=after_inputs.source_order_profile)
    result = apply_catalogued_prepared_delta_transaction(db, expected_binding=expected_binding,
        before_inputs=before_inputs, after_inputs=after, changes=retained,
        limits=replace(limits, max_mutations=_remaining(db, start, limits)),
        catalog_limits=catalog_limits)
    receipt = verify_semantic_index_binding_transaction(db, result['binding'],
        limits=_semantic_limits(db, start, limits, semantic))
    if db.total_changes - start > limits.max_mutations:
        raise ValueError('semantic/catalog/prepared combined mutation budget exceeded')
    return {**result, 'semantic_report': report,
            'semantic_report_sha256': receipt['semantic_report_sha256'],
            'sql_mutations': db.total_changes - start}
