"""Pair supplied source dependencies, roots and prepared lanes in one transaction.

This offline owner composition neither observes sources nor computes their
affected closure. The assembler supplies those facts and keeps its source locks
and guards through commit. Any failure requires complete caller rollback.
"""
from dataclasses import replace

from .prepared_publication import PublicationLimits
from .prepared_source_binding import (
    PreparedSourceInputs, apply_source_bound_prepared_delta_transaction,
    read_prepared_source_inputs_transaction, bootstrap_prepared_source_root_extension_transaction,
)
from .prepared_source_dependencies import (
    ProgressHandlerOwner, SourceDependencyLimits,
    apply_source_dependency_delta_transaction,
    verify_source_dependency_binding_transaction,
)


def apply_dependency_bound_prepared_delta_transaction(db, *, expected_binding,
        before_source_inputs, after_source_inputs, before_inputs, after_inputs,
        changes, dependency_changes, declaration_profile_sha256, progress_owner,
        limits=None, dependency_limits=None, catalog_limits=None, semantic_limits=None):
    """Pair supplied declarations and rows; caller owns guards and full rollback."""
    return _apply_dependency_pair(db, expected_binding=expected_binding,
        before_source_inputs=before_source_inputs, after_source_inputs=after_source_inputs,
        before_inputs=before_inputs, after_inputs=after_inputs, changes=changes,
        dependency_changes=dependency_changes, declaration_profile_sha256=declaration_profile_sha256,
        progress_owner=progress_owner, limits=limits, dependency_limits=dependency_limits,
        catalog_limits=catalog_limits, semantic_limits=semantic_limits,
        publisher=apply_source_bound_prepared_delta_transaction)


def bootstrap_dependency_bound_source_extension_transaction(db, *, expected_binding,
        before_source_inputs, after_source_inputs, added_root, before_inputs, after_inputs,
        declaration_profile_sha256, progress_owner, limits=None, dependency_limits=None,
        catalog_limits=None, semantic_limits=None):
    """Pair one explicit additional root and unchanged dependency declarations.

    Root membership admission and any source-owner context index remain the
    caller's responsibility. No source or normalized/declaration row changes
    can be supplied to this route; all errors require caller rollback.
    """
    def publish(connection, **options):
        options.pop('changes')  # The internal composition below supplies ().
        return bootstrap_prepared_source_root_extension_transaction(connection,
            added_root=added_root, **options)
    return _apply_dependency_pair(db, expected_binding=expected_binding,
        before_source_inputs=before_source_inputs, after_source_inputs=after_source_inputs,
        before_inputs=before_inputs, after_inputs=after_inputs, changes=(), dependency_changes=(),
        declaration_profile_sha256=declaration_profile_sha256, progress_owner=progress_owner,
        limits=limits, dependency_limits=dependency_limits, catalog_limits=catalog_limits,
        semantic_limits=semantic_limits, publisher=publish)


def _apply_dependency_pair(db, *, expected_binding, before_source_inputs, after_source_inputs,
        before_inputs, after_inputs, changes, dependency_changes, declaration_profile_sha256,
        progress_owner, limits, dependency_limits, catalog_limits, semantic_limits, publisher):
    """Stage declarations, publish all rows/roots, finalize their exact binding.

    ``limits.max_mutations`` covers this entire call, excluding earlier caller
    writes. The declaration finalizer's conservative allowance is reserved
    before the row/root publisher runs. Per-lane work limits remain independent;
    there is no new whole-operation VM or elapsed-time guarantee. No entry point
    here begins, commits, rolls back, closes, repairs or activates a consumer.
    """
    limits = limits or PublicationLimits()
    dependencies = dependency_limits or SourceDependencyLimits()
    if (not db.in_transaction or type(limits) is not PublicationLimits
            or type(dependencies) is not SourceDependencyLimits
            or type(progress_owner) is not ProgressHandlerOwner
            or type(before_source_inputs) is not PreparedSourceInputs
            or type(after_source_inputs) is not PreparedSourceInputs):
        raise ValueError('explicit caller transaction, source inputs, limits and progress owner required')
    if read_prepared_source_inputs_transaction(db, expected_binding=expected_binding,
            limits=limits) != before_source_inputs:
        raise ValueError('dependency publication source predecessor differs')
    after = PreparedSourceInputs.parse(after_source_inputs.raw)
    # At least two row/root writes plus one empty finalization write must remain.
    # The actual nonempty finalizer bound is known after bounded stage capture.
    if limits.max_mutations <= 3:
        raise ValueError('dependency publication combined mutation budget exceeded')
    start = db.total_changes
    staged = apply_source_dependency_delta_transaction(db, expected_binding=expected_binding,
        before_source_inputs_sha256=before_source_inputs.digest,
        after_source_inputs_sha256=after.digest,
        new_source_revision=after.value()['source_revision'],
        declaration_profile_sha256=declaration_profile_sha256,
        changes=dependency_changes, progress_owner=progress_owner,
        limits=replace(dependencies, max_bytes=min(dependencies.max_bytes, limits.max_bytes),
                       max_writes=min(dependencies.max_writes, limits.max_mutations - 3)))
    staged_writes = db.total_changes - start
    reserve = staged['finalize_sql_mutations_upper_bound']
    remaining = limits.max_mutations - staged_writes - reserve
    if remaining < 2:
        raise ValueError('dependency publication finalizer reservation exceeds combined mutation budget')
    published = publisher(db,
        expected_binding=expected_binding, before_source_inputs=before_source_inputs,
        after_source_inputs=after, before_inputs=before_inputs, after_inputs=after_inputs,
        changes=changes, limits=replace(limits, max_mutations=remaining),
        catalog_limits=catalog_limits, semantic_limits=semantic_limits)
    remaining = limits.max_mutations - (db.total_changes - start)
    if remaining < reserve:
        raise ValueError('dependency publication finalizer allowance was consumed')
    finalized = verify_source_dependency_binding_transaction(db, new_binding=published['binding'],
        source_inputs_sha256=after.digest, declaration_profile_sha256=declaration_profile_sha256,
        progress_owner=progress_owner,
        limits=replace(dependencies, max_bytes=min(dependencies.max_bytes, limits.max_bytes),
                       max_writes=min(dependencies.max_writes, remaining)))
    total = db.total_changes - start
    if finalized['sql_mutations'] > reserve or total > limits.max_mutations:
        raise ValueError('dependency publication combined mutation budget exceeded; rollback required')
    return {**published, 'sql_mutations': total,
            'source_dependency_stage': staged, 'source_dependency_finalization': finalized,
            'source_dependencies_paired_in_caller_transaction': True,
            'source_transition_verified': False, 'source_completeness_verified': False,
            'target_closure_verified': False, 'semantic_acceptance': False, 'consumer_switched': False}
