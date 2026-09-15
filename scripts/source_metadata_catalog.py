"""Addressed catalog addition of one exact initial source metadata record.

The source command remains the owner of creation and authorization.  This
module only observes that already-created package, stages its immutable
catalog row and its native provenance slot, and returns an unselected
candidate.  It does not run a source command, scan the source tree, add
Claims, revise metadata, grant admission, or switch a prepared reader.
"""
from __future__ import annotations

from contextlib import contextmanager
import copy
from pathlib import Path

import source_catalog_projection as catalog
import source_commands as source
from source_metadata_snapshot import PublicationSnapshot
from source_record_profiles import SourceRecordProfiles
from tos_access.projection_mutation import (
    MutationLimits,
    ProjectionChange,
    ProjectionHeaderChange,
    stage_projection_snapshot_changes,
)
from tos_access.projection_store import canonical_bytes


ALLOWED_CONFIGS = frozenset({
    source.PROFILE_CONFIG,
    source.CORPUS_CONFIG,
    source.CORPUS_COLLECTION_CONFIG,
})
HISTORY = 'source-revision-history.json'


def _binding(raw: bytes) -> dict:
    return {'sha256': catalog._sha(raw), 'bytes': len(raw)}


def _receipt_binding(raw: bytes) -> dict:
    return {'sha256': source._digest(raw), 'bytes': len(raw)}


class MetadataCatalogAddition:
    """One source-lock-scoped, detached initial metadata candidate."""

    def __init__(self, owner_config, before, *, expected_receipt_sha256,
                 expected_request_digest, limits, mutation_limits, target_part_bytes):
        self.active = True
        self.owner_config = Path(owner_config)
        self.config, self.configuration, self.path = source._configuration(self.owner_config)
        if self.config['schema_version'] not in ALLOWED_CONFIGS:
            raise PermissionError('initial metadata addition requires the explicit profile/corpus owner lane')
        self.before = before
        self.limits = limits
        self.mutation_limits = mutation_limits
        self.target_part_bytes = target_part_bytes
        self.root = Path(self.config['source_root'])
        self.capture = catalog._Capture(self.root, limits)
        self.execution = catalog._Capture(catalog.EXECUTION_ROOT, limits)
        self.publication = PublicationSnapshot(self.root)
        header = before.header
        if (not header.get('claims_addressed') or header['source_publication'] != {
                'protocol': catalog.PUBLICATION_PROTOCOL,
                'token': self.publication.token,
                'generation': self.publication.generation}):
            raise catalog.SourceCatalogError(
                'addressed metadata baseline or selected publication differs')
        # Preserve the existing catalog execution/source contract.  New source
        # profile inputs are added below only after their exact bytes are read.
        for observer, bindings in ((self.capture, header['profile_bindings']['source']),
                                   (self.execution, header['profile_bindings']['execution'])):
            for ref, expected in bindings.items():
                raw = observer.read(ref)
                if _binding(raw) != expected:
                    raise catalog.SourceCatalogRequiresBootstrap(
                        'catalog source/execution profile changed')
        self.execution.read('scripts/source_metadata_catalog.py')

        self.relative = self.config['source_path']
        package = self.path.parent
        request_ref = str(package.relative_to(self.root) / 'source-create-request.json')
        receipt_ref = str(package.relative_to(self.root) / 'source-create-receipt.json')
        request_raw = self.capture.read(request_ref)
        receipt_raw = self.capture.read(receipt_ref)
        if catalog._sha(receipt_raw) != catalog._digest(expected_receipt_sha256):
            raise catalog.SourceCatalogError('selected creation receipt bytes differ')
        self.request = source._json_object(request_raw)
        if source._digest(source._canonical(self.request)) != expected_request_digest:
            raise catalog.SourceCatalogError('selected creation command differs')
        source.command_handler(self.config['schema_version']).validate_request(self.request)
        if (self.request.get('operation') != 'source.create'
                or self.request.get('expected_revision') is not None
                or self.request.get('expected_source') is not None
                or self.request.get('expected_configuration') != self.configuration
                or self.request.get('claims', []) != []):
            raise PermissionError('metadata addition requires an exact standalone initial source request')
        source._creation_scope(self.config, self.request)

        self.receipt = source._json_object(receipt_raw)
        # A versioned correction/history package is a different owner route.
        # Reject its presence before invoking the replay reader so this lane
        # never interprets a successor as an initial metadata add.
        history_path = package / HISTORY
        if history_path.exists() or history_path.is_symlink():
            raise catalog.SourceCatalogRequiresBootstrap(
                'metadata addition accepts only an exact initial package without retained history')
        source._creation_replay(self.config, self.path, self.request, self.receipt)
        if (self.receipt.get('authority_ref') != self.config['authority_ref']
                or self.receipt.get('request_digest') != expected_request_digest
                or self.receipt.get('source_path') != self.relative
                or self.receipt.get('grants_admission') is not False):
            raise PermissionError('creation receipt authority or identity differs')

        source_raw = self.capture.read(self.relative)
        form_name = self.path.stem + '.human-forms.json'
        form_ref = str(package.relative_to(self.root) / form_name)
        forms_raw = self.capture.read(form_ref)
        self.record = source._json_object(source_raw)
        if self.request.get('record') != self.record:
            raise catalog.SourceCatalogError('creation request and current source record bytes differ')
        source._initial_source_record(self.config, self.record)
        subject = source.metadata_subject(self.record)
        forms = source._json_object(forms_raw)
        if (forms.get('subject') != subject.ref
                or forms.get('prior_forms') not in ([], None)
                or forms.get('growth_history') not in ([], None)):
            raise catalog.SourceCatalogRequiresBootstrap(
                'metadata addition requires an exact initial form set')

        expected_files = self.receipt.get('files')
        if not isinstance(expected_files, dict):
            raise catalog.SourceCatalogError('creation receipt file bindings are missing')
        for name, expected in expected_files.items():
            if not isinstance(name, str) or Path(name).name != name:
                raise catalog.SourceCatalogError('creation receipt contains an unsafe file binding')
            raw = self.capture.read(str(package.relative_to(self.root) / name))
            if expected != _receipt_binding(raw):
                raise catalog.SourceCatalogError('creation output bytes differ from receipt')
        if form_ref not in self.capture.observed or self.relative not in self.capture.observed:
            raise catalog.SourceCatalogError('creation package does not bind its source and forms')

        profiles = SourceRecordProfiles(self.root)
        if self.config['schema_version'] == source.PROFILE_CONFIG:
            configured_profiles, profile = source._configured_profile(self.config, profiles)
            kind = profile['record_type']
        else:
            configured_profiles, profile = profiles, source._configured_corpus_profile(self.config)
            kind = profile['record_type']
        if self.record.get('record_type') != kind:
            raise PermissionError('source record type differs from the declared owner profile')
        # The shared initial owner validator is the source contract for exact
        # identity/version/visibility.  It does not grant publication rights.
        if source.metadata_subject(self.record).ref != self.receipt['source']:
            raise catalog.SourceCatalogError('creation receipt source identity differs')

        if kind in configured_profiles.profiles:
            configured_profiles.validate_path(kind, self.relative)
            entry = configured_profiles.catalog_entry(kind, self.record, self.relative)
        else:
            entry = catalog.legacy.render_native_catalog_entry(self.record, self.relative)
        row = catalog._verified_row(entry, source_raw, configured_profiles, self.capture)
        self.entry, self.row = row['entry'], row
        record_id = row['record_id']
        # Stable identities are checked in every addressed predecessor space;
        # no catalog-wide iteration or caller-selected fallback is permitted.
        if self.before.lookup(record_id) is not None:
            raise catalog.SourceCatalogError('new metadata identity already belongs to predecessor')
        # A metadata identity cannot inhabit the Claim slot namespace unless
        # it has the Claim grammar; the typed event/anchor slots accept the
        # bounded stable identity grammar directly.
        slot_kinds = ('provenance_event', 'anchor')
        if catalog.CLAIM_IDENTITY.fullmatch(record_id):
            if self.before.lookup_claim(record_id) is not None:
                raise catalog.SourceCatalogError('new metadata identity already belongs to predecessor')
            slot_kinds = ('claim', *slot_kinds)
        for kind_name in slot_kinds:
            if self.before.lookup_slot(kind_name, record_id) is not None:
                raise catalog.SourceCatalogError('new metadata identity already belongs to predecessor')

        event_ref = str(package.relative_to(self.root) / 'source-create-provenance.jsonl')
        claim_rows, slots = catalog._source_slot_rows([], {event_ref}, self.capture, limits)
        if claim_rows:
            raise catalog.SourceCatalogError('metadata addition unexpectedly produced Claim rows')
        event_ids = {slot['identity'] for slot in slots if slot['kind'] == 'provenance_event'}
        if event_ids != {self.config['provenance_event_id']}:
            raise catalog.SourceCatalogError('creation provenance slot closure differs')
        for slot in slots:
            if self.before.lookup_slot(slot['kind'], slot['identity']) is not None:
                raise catalog.SourceCatalogError('new typed source slot already belongs to predecessor')

        # SourceRecordProfiles reads its declared registry/schema resources via
        # its owner reader. Capture and bind those exact bytes before staging.
        for ref, expected in configured_profiles.input_digests.items():
            raw = self.capture.read(ref)
            if catalog._sha(raw) != expected:
                raise catalog.SourceCatalogError('source profile changed during validation')

        successor = copy.deepcopy(header)
        successor['record_families'] = sorted(set(header['record_families']) | {kind})
        successor['record_count'] = header['record_count'] + 1
        successor['source_slot_count'] = header['source_slot_count'] + len(slots)
        successor['profile_bindings']['source'] = {
            **header['profile_bindings']['source'],
            **{ref: observed for ref, observed in self.capture.observed.items()
               if ref in configured_profiles.input_digests},
        }
        successor['last_transition'] = None
        catalog._schema('header').validate(successor)
        catalog._schema('row').validate(row)
        for slot in slots:
            catalog._schema('slotRow').validate(slot)

        changes = [ProjectionChange('records', record_id, False, None, True, row)]
        changes.extend(ProjectionChange('source_slots', slot['source_slot_key'], False, None, True, slot)
                       for slot in slots)
        self.verify_current()
        staged = stage_projection_snapshot_changes(
            before.view,
            expected_before_sha256=before.root_sha256,
            trusted_baseline_sha256=before.root_sha256,
            changes=changes,
            header_change=ProjectionHeaderChange(catalog._sha(canonical_bytes(header)), successor),
            limits=mutation_limits,
            target_part_bytes=target_part_bytes,
        )
        self.verify_current()
        verification = {
            'mode': 'committed-initial-metadata-addition',
            'record_id': record_id,
            'record_type': kind,
            'creation_request_digest': expected_request_digest,
            'creation_receipt_sha256': expected_receipt_sha256,
            'records_added': 1,
            'claims_added': 0,
            'source_slots_added': len(slots),
            'source_input_bytes': self.capture.input_bytes,
            'source_read_bytes': self.capture.read_bytes,
            'global_source_currentness_verified': False,
            'source_reference_closure_verified': False,
            'prepared_reader_updated': False,
            'consumer_switched': False,
            'grants_admission': False,
        }
        self.candidate = catalog.SourceCatalogCandidate(
            staged.namespace_path,
            staged.root_bytes,
            before.root_sha256,
            staged.created_parts,
            canonical_bytes(verification),
        )

    def verify_current(self):
        if not self.active:
            raise catalog.SourceCatalogError('metadata addition source scope is closed')
        config, digest, path = source._configuration(self.owner_config)
        if config != self.config or digest != self.configuration or path != self.path:
            raise PermissionError('metadata addition delegation changed')
        self.publication.verify_current()
        source.command_handler(config['schema_version']).validate_request(self.request)
        source._creation_scope(config, self.request)
        source._creation_replay(config, path, self.request, self.receipt)
        if (self.request.get('record') != source._json_object(self.capture.read(self.relative))
                or self.receipt.get('request_digest') != self.request_digest):
            raise catalog.SourceCatalogError('selected initial metadata package changed')
        if self.receipt.get('grants_admission') is not False:
            raise PermissionError('metadata creation receipt grants admission')
        self.capture.verify()
        self.execution.verify()
        self.publication.verify_current()

    @property
    def request_digest(self):
        return source._digest(source._canonical(self.request))


@contextmanager
def metadata_catalog_addition(owner_config, before, *, expected_receipt_sha256,
                              expected_request_digest, limits=None, mutation_limits=None,
                              target_part_bytes=catalog.DEFAULT_PART_BYTES):
    """Hold the ordinary source creation lock while staging one detached row."""
    if not isinstance(before, catalog.SourceCatalogSnapshot):
        raise TypeError('an explicit source catalog predecessor is required')
    limits = limits or catalog.CatalogLimits(max_records=1, max_claims=0, max_source_slots=2)
    mutation_limits = mutation_limits or MutationLimits()
    config, _, _ = source._configuration(Path(owner_config))
    if config['schema_version'] not in ALLOWED_CONFIGS:
        raise PermissionError('metadata addition requires an explicit profile/corpus owner lane')
    with source._locked(Path(config['source_root']) / 'ToS/source-witnesses/historical-create'):
        addition = MetadataCatalogAddition(
            owner_config,
            before,
            expected_receipt_sha256=expected_receipt_sha256,
            expected_request_digest=expected_request_digest,
            limits=limits,
            mutation_limits=mutation_limits,
            target_part_bytes=target_part_bytes,
        )
        try:
            yield addition
        finally:
            addition.active = False
