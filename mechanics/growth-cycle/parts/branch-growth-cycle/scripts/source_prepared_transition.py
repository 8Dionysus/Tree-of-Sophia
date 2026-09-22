"""Read an exact committed selected-metadata transition for derived assembly.

Source commands remain the only mutation entry point. This observer neither
publishes a projection nor broadens their delegation. It does not acquire a
lease over source files, the assessment journal or the caller's prepared store.
"""
from dataclasses import dataclass
from pathlib import Path

import source_commands as source
import source_revisions as revisions
import source_selected_revisions as selected
import source_metadata_transactions as transactions
from source_metadata_snapshot import PublicationSnapshot


@dataclass(frozen=True)
class SelectedPreparedTransition:
    """Detached exact bytes and bindings for the selected local transition.

    Only immutable byte/string/tuple values are retained. Decode a copy when
    handing a record to a projector. Root and owner are private local locators;
    do not export this object as a public knowledge packet.
    """
    root: Path
    owner: Path
    transaction_id: str
    manifest_sha256: str
    configuration: str
    dependencies: str
    source_path: str
    record_type: str
    before_publication: str | None
    after_publication: str
    before_files: tuple[tuple[str, bytes], ...]
    after_files: tuple[tuple[str, bytes], ...]
    receipt_bytes: bytes

    def record(self, side):
        if side not in ('before', 'after'):
            raise ValueError('select exact before or after side')
        return source._json_object(dict(getattr(self, side + '_files'))[Path(self.source_path).name])

    def receipt(self):
        return source._json_object(self.receipt_bytes)


def capture_selected_prepared_transition(owner, transaction_id, *, expected_before_publication):
    """Observe the current exact committed transition under current delegation.

    Reconstruct the retained command through its existing adapter; verify its
    predecessor token, authority, dependencies, archived before bytes, exact
    current after package and source publication. Pending, rolled-back,
    historical/non-current and unrelated transitions refuse. No arbitrary
    replacement JSON or caller-supplied positive receipt is accepted.

    The caller must select the predecessor from its admitted derived baseline.
    This function does not establish that baseline, new graph memberships or
    normalization closure. Before a later derived commit, retain the existing
    source-owner lock/guard protocol and reverify this observation.
    """
    owner = Path(owner).absolute()
    config, configuration, path = source._configuration(owner)
    if config['schema_version'] not in (source.CORPUS_SELECTED_REVISION_CONFIG,
                                        source.CORPUS_COMPLETE_REVISION_CONFIG):
        raise PermissionError('prepared transition requires explicit selected native metadata owner')
    root = Path(config['source_root'])
    snapshot = PublicationSnapshot(root)
    retained = transactions.inspect_transaction(root, transaction_id)
    if (retained['status'] != 'committed' or not retained['is_current_publication']
            or retained['publication']['token'] != snapshot.token):
        raise source.JournalConflict('prepared transition is not the current committed source publication')
    if retained['manifest']['base_publication']['token'] != expected_before_publication:
        raise source.JournalConflict('prepared predecessor source publication differs')
    authorization, before_record, receipt = selected._pending_plan(config, path, retained)
    request = authorization['request']
    if (request['expected_configuration'] != configuration
            or request['expected_publication'] != expected_before_publication
            or receipt['publication']['transaction_id'] != transaction_id):
        raise source.JournalConflict('prepared transition configuration or predecessor differs')
    # The existing guard performs current owner/scope/dependency/archive checks,
    # even for a retained transaction whose earlier writer already succeeded.
    guard = selected._guard(owner, config, configuration, path, authorization, before_record, receipt)
    if guard(authorization, retained['manifest']['plan']) is not True:
        raise PermissionError('prepared transition lacks current source delegation')
    before = {Path(item['path']).name: item['before'] for item in retained['plan']['files']
              if item['before'] is not None}
    after = {Path(item['path']).name: item['after'] for item in retained['plan']['files']
             if item['after'] is not None}
    archived, _ = revisions._read_archive(root, config, receipt)
    if archived != before:
        raise source.JournalCorruption('retained transition and archive disagree on exact before bytes')
    current, record, subject, history = selected._inspect(config, path)
    if (current != after or subject.ref != receipt['source']
            or not history['receipts'] or history['receipts'][-1] != receipt):
        raise source.JournalConflict('source files differ from the exact committed successor')
    # No assessed journal is consulted here. This selected-revision adapter
    # produces source-copy forms and grants no admission; the downstream
    # assembler must not reuse an assessment for the new subject by implication.
    guard(authorization, retained['manifest']['plan'])
    if revisions._selected_package(path) != after:
        raise source.JournalConflict('selected source files changed during transition capture')
    snapshot.verify_current()
    return SelectedPreparedTransition(root, owner, transaction_id, retained['manifest_sha256'],
        configuration, request['expected_dependencies'], config['source_path'], config['record_type'],
        expected_before_publication, snapshot.token, tuple(sorted(before.items())),
        tuple(sorted(after.items())), source._canonical(receipt))


def verify_selected_prepared_transition_current(transition):
    """Repeat exact owner observation; never silently refresh a stale result."""
    if not isinstance(transition, SelectedPreparedTransition):
        raise ValueError('exact selected transition object required')
    current = capture_selected_prepared_transition(transition.owner, transition.transaction_id,
        expected_before_publication=transition.before_publication)
    if current != transition:
        raise source.JournalConflict('selected source transition changed after capture')
    return True
