"""Addressed catalog addition of an exact committed initial Claim package.

The source writer creates and authorizes the package separately. This observer
stages immutable catalog parts only; it neither replays the command nor scans
the corpus, selects a root, normalizes a graph or grants admission.
"""
from contextlib import contextmanager
from pathlib import Path

import source_catalog_projection as catalog
import source_claim_commands as claims
import source_commands as source
from source_record_profiles import SourceClaimProfiles
from source_metadata_snapshot import PublicationSnapshot
from tos_access.projection_mutation import (
    MutationLimits, ProjectionChange, ProjectionHeaderChange, stage_projection_snapshot_changes,
)
from tos_access.projection_store import canonical_bytes


class ClaimCatalogAddition:
    """A current source-lock-scoped candidate, not durable commit authority."""

    def __init__(self, owner_config, before, *, expected_receipt_sha256,
                 expected_request_digest, limits, mutation_limits, target_part_bytes):
        self.active = True
        self.owner_config = Path(owner_config)
        self.config, self.configuration, self.path = source._configuration(self.owner_config)
        if self.config['schema_version'] != source.CLAIM_CONFIG:
            raise PermissionError('initial identity-only Claim creation delegation required')
        self.root = Path(self.config['source_root'])
        self.before = before
        self.capture = catalog._Capture(self.root, limits)
        self.execution = catalog._Capture(catalog.EXECUTION_ROOT, limits)
        self.publication = PublicationSnapshot(self.root)
        header = before.header
        if (not header.get('claims_addressed') or header['source_publication'] != {
                'protocol': catalog.PUBLICATION_PROTOCOL, 'token': self.publication.token,
                'generation': self.publication.generation}):
            raise catalog.SourceCatalogError('addressed Claim baseline or selected metadata publication differs')
        # An explicit addition is not permission to silently migrate an existing
        # bootstrap's schema/processor profile or claim global source currentness.
        for observer, bindings in ((self.capture, header['profile_bindings']['source']),
                                   (self.execution, header['profile_bindings']['execution'])):
            for ref, expected in bindings.items():
                raw = observer.read(ref)
                if {'sha256': catalog._sha(raw), 'bytes': len(raw)} != expected:
                    raise catalog.SourceCatalogRequiresBootstrap('catalog source/execution profile changed')
        self.execution.read('scripts/source_claim_catalog.py')
        self.execution.read(claims.MODULE_REF)
        self.relative = self.config['source_path']
        package = Path(self.relative).parent
        request_raw = self.capture.read(str(package / 'source-create-request.json'))
        receipt_raw = self.capture.read(str(package / 'source-create-receipt.json'))
        if catalog._sha(receipt_raw) != catalog._digest(expected_receipt_sha256):
            raise catalog.SourceCatalogError('selected creation receipt bytes differ')
        self.request = source._json_object(request_raw)
        if source._digest(source._canonical(self.request)) != expected_request_digest:
            raise catalog.SourceCatalogError('selected creation command differs')
        source.command_handler(self.config['schema_version']).validate_request(self.request)
        if (self.request['operation'] != 'claims.create'
                or self.request['expected_revision'] is not None
                or self.request['expected_configuration'] != self.configuration):
            raise PermissionError('creation request does not match the selected current delegation')
        profiles = SourceClaimProfiles(self.root)
        claims._scope(self.config, self.request['claims'], profiles=profiles)
        self.receipt = claims._replay(self.path.parent, self.config, self.request)
        if (self.receipt['authority_ref'] != self.config['authority_ref']
                or self.receipt != source._json_object(receipt_raw)):
            raise PermissionError('creation receipt authority or bytes differ')
        raw = self.capture.read(self.relative)
        if raw != b''.join(source._canonical(claim) + b'\n' for claim in self.request['claims']):
            raise catalog.SourceCatalogError('only unchanged initial Claim versions may be added')
        if len(self.request['claims']) > limits.max_claims:
            raise catalog.SourceCatalogBudgetExceeded('Claim addition count budget exceeded')
        for name in claims.PACKAGE_FILES:
            self.capture.read(str(package / name))
        # Selected endpoint versions must already belong to the admitted
        # predecessor. Source bytes, not the catalog alone, establish currentness.
        bindings = self.request['expected_inputs']
        if set(bindings) != {'objects', 'evidence'}:
            raise catalog.SourceCatalogRequiresBootstrap('this addition profile requires plain identity Claims')
        identities = {identity for claim in self.request['claims'] for identity in profiles.identity_refs(claim)}
        if identities != bindings['objects'].keys():
            raise catalog.SourceCatalogError('creation endpoint binding set differs')
        objects = {}
        for identity in sorted(identities):
            selected = before.get(identity)
            binding = bindings['objects'][identity]
            raw = self.capture.read(binding['source_ref'])
            record = source._json_object(raw)
            if (selected.source['source_ref'] != binding['source_ref']
                    or selected.source['raw_sha256'] != catalog._sha(raw)
                    or selected.source['record_ref'] != source.metadata_subject(record).ref
                    or binding != {'source_ref': selected.source['source_ref'],
                        'source_sha256': source._digest(raw),
                        'canonical_record_sha256': source._digest(source._canonical(record)),
                        'schema_version': record.get('schema_version'),
                        'record_version': record.get('record_version')}):
                raise catalog.SourceCatalogError('selected endpoint source version differs')
            objects[identity] = selected.entry
        evidence_refs = {ref for claim in self.request['claims']
                         for ref in (*claim['evidence_refs'], *claim.get('counterevidence_refs', []))}
        if evidence_refs != bindings['evidence'].keys():
            raise catalog.SourceCatalogError('creation evidence binding set differs')
        for ref in sorted(evidence_refs):
            binding = bindings['evidence'][ref]
            # More elaborate native/reference evidence uses the existing owner
            # readers in its future explicit addition profile, not guessed paths.
            if (binding.get('evidence_kind') != 'repo_path' or binding.get('source_ref') != ref
                    or binding.get('source_line') is not None or not ref.startswith('ToS/')
                    or any(part in {'payload', 'local-content', 'private', 'owner-local'} for part in Path(ref).parts)):
                raise catalog.SourceCatalogRequiresBootstrap('addition requires exact public metadata path evidence')
            if source._digest(self.capture.read(ref)) != binding['source_sha256']:
                raise catalog.SourceCatalogError('selected evidence bytes differ')
        entries = []
        for number, claim in enumerate(self.request['claims'], start=1):
            if (profiles.profiles[claim['predicate']]['reader'] != 'identity-relation-v1'
                    or claim.get('alternative_claim_refs') or claim.get('supersedes_claim_ref')
                    or claim.get('assessment_refs') or claim['claim_version'] != 1):
                raise catalog.SourceCatalogRequiresBootstrap('initial identity-relation addition profile exceeded')
            profiles.validate(claim, objects)
            identity = claim['claim_id']
            if before.lookup_claim(identity) is not None or before.lookup(identity) is not None:
                raise catalog.SourceCatalogError('new Claim identity already belongs to predecessor')
            entry = catalog.legacy.render_claim_catalog_entry(claim, self.relative, number,
                source_schema_ref=profiles.schema_routes[claim['predicate'], claim['schema_version']]['schema_ref'])
            entries.append(entry)
        event_ref = str(package / 'source-create-provenance.jsonl')
        claim_rows, slots = catalog._source_slot_rows(entries, {event_ref}, self.capture, limits)
        event_ids = {slot['identity'] for slot in slots if slot['kind'] == 'provenance_event'}
        if event_ids != {self.config['provenance_event_id']}:
            raise catalog.SourceCatalogError('creation provenance slot closure differs')
        for slot in slots:
            if before.lookup_slot(slot['kind'], slot['identity']) is not None:
                raise catalog.SourceCatalogError('new typed source slot already belongs to predecessor')
        for ref, expected in profiles.input_digests.items():
            if catalog._sha(self.capture.read(ref)) != expected:
                raise catalog.SourceCatalogError('new Claim profile changed during validation')
        # This is not a selected-metadata transaction: do not invent a token or
        # masquerade as the previous Agent transaction. The detached addition
        # receipt below binds both roots and the exact creation request instead.
        successor = {**header,
                     'profile_bindings': {**header['profile_bindings'],
                         'source': {**header['profile_bindings']['source'],
                                    **{ref: self.capture.observed[ref] for ref in profiles.input_digests}}},
                     'claim_count': header['claim_count'] + len(claim_rows),
                     'source_slot_count': header['source_slot_count'] + len(slots),
                     'last_transition': None}
        catalog._schema('header').validate(successor)
        changes = []
        for collection, key, rows, definition in (
                ('claims', 'claim_id', claim_rows, 'claimRow'),
                ('source_slots', 'source_slot_key', slots, 'slotRow')):
            for row in rows:
                catalog._schema(definition).validate(row)
                changes.append(ProjectionChange(collection, row[key], False, None, True, row))
        self.verify_current()
        staged = stage_projection_snapshot_changes(before.view,
            expected_before_sha256=before.root_sha256, trusted_baseline_sha256=before.root_sha256,
            changes=changes,
            header_change=ProjectionHeaderChange(catalog._sha(canonical_bytes(header)), successor),
            limits=mutation_limits, target_part_bytes=target_part_bytes)
        self.verify_current()
        self.candidate = catalog.SourceCatalogCandidate(staged.namespace_path, staged.root_bytes,
            before.root_sha256, staged.created_parts, canonical_bytes({
                'mode': 'committed-initial-identity-claim-addition',
                'creation_request_digest': expected_request_digest,
                'creation_receipt_sha256': expected_receipt_sha256,
                'claim_ids': [entry['claim_id'] for entry in entries],
                'claims_added': len(claim_rows), 'source_slots_added': len(slots),
                'source_read_bytes': self.capture.read_bytes,
                'projection_accounting': dict(staged.accounting),
                'global_source_currentness_verified': False, 'source_reference_closure_verified': False,
                'prepared_reader_updated': False, 'consumer_switched': False, 'grants_admission': False}))

    def verify_current(self):
        if not self.active:
            raise catalog.SourceCatalogError('Claim addition source scope is closed')
        config, digest, path = source._configuration(self.owner_config)
        if config != self.config or digest != self.configuration or path != self.path:
            raise PermissionError('Claim addition delegation changed')
        self.publication.verify_current()
        claims._scope(config, self.request['claims'])
        if claims._replay(path.parent, config, self.request) != self.receipt:
            raise catalog.SourceCatalogError('selected initial package changed')
        self.capture.verify()
        self.execution.verify()
        self.publication.verify_current()


@contextmanager
def claim_catalog_addition(owner_config, before, *, expected_receipt_sha256,
                           expected_request_digest, limits=None, mutation_limits=None,
                           target_part_bytes=catalog.DEFAULT_PART_BYTES):
    """Hold the source writer lock while producing/verifying one unselected root.

    No source command, complete-source scan, prepared write or consumer switch.
    A later joined publisher must call verify_current before its own commit and
    roll back its complete transaction on any failure. No staged part deletion
    is authorized by failure or scope exit.
    """
    if not isinstance(before, catalog.SourceCatalogSnapshot):
        raise TypeError('explicit source catalog predecessor required')
    config, _, _ = source._configuration(Path(owner_config))
    with source._locked(Path(config['source_root']) / 'ToS/source-witnesses/historical-create'):
        addition = ClaimCatalogAddition(owner_config, before,
            expected_receipt_sha256=expected_receipt_sha256, expected_request_digest=expected_request_digest,
            limits=limits or catalog.CatalogLimits(max_claims=32, max_source_slots=33),
            mutation_limits=mutation_limits or MutationLimits(), target_part_bytes=target_part_bytes)
        try:
            yield addition
        finally:
            addition.active = False
