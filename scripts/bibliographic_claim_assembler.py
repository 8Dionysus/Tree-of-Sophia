"""Concrete addressed source assembly, never reverse selection or publication.

The explicit source catalog bootstrap owns membership and JSONL locations.
Current source slots and exact metadata history are verified by their real
owner readers. Exact collection-order membership versions are resolved by the
owner's bounded ClaimVersionReader; this module performs only selected forward
lookups and shares the full builders' renderers. It cannot admit source, select
a prepared root, or prove incident closure.
"""
from __future__ import annotations

import copy
from dataclasses import dataclass, fields, is_dataclass
import hashlib
import os
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

import source_catalog_projection as catalog
from source_catalog_projection import SourceCatalogSnapshot, SourceCatalogSourceReader, SourceSlotLimits
from metadata_version_reader import MetadataVersionReader
import source_commands as source
import source_record_profiles as profiles
import source_witness_bibliographic_graph_common as graph
import source_witness_human_forms as forms
from source_object_link_read import (SCHEMA_REF as LINK_SCHEMA, legacy_object_link_validator,
    validate_legacy_object_link, render_legacy_object_link_context)
from human_forms import compile_source_form_validators
from tos_corpus_index_common import (SourceNavigationRecordInput, project_source_navigation_record)


class ClaimAssemblyError(ValueError):
    """No complete selected source cohort is available."""


class ClaimAssemblyUnsupported(ClaimAssemblyError):
    """This producer has no exact bounded transport for the requested scope."""


class ClaimAssemblyBudgetExceeded(ClaimAssemblyError):
    pass


def _json_size_value(value):
    """Account detached input bytes as well as its expanded pure projection."""
    if is_dataclass(value):
        return {field.name: _json_size_value(getattr(value, field.name)) for field in fields(value)}
    if isinstance(value, bytes):
        return {'raw_utf8': value.decode('utf-8')}
    if isinstance(value, dict):
        return {key: _json_size_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set, frozenset)):
        return [_json_size_value(item) for item in value]
    return value


@dataclass(frozen=True)
class ClaimAssemblyLimits:
    max_claims: int = 64
    max_metadata_records: int = 256
    max_addressed_lookups: int = 4096
    max_files: int = 256
    max_file_bytes: int = 2 * 1024 * 1024
    max_read_bytes: int = 16 * 1024 * 1024
    max_output_bytes: int = 16 * 1024 * 1024

    def __post_init__(self):
        if any(type(value) is not int or value < 0 for value in vars(self).values()):
            raise ValueError('assembly limits must be nonnegative integers')


@dataclass(frozen=True)
class AssembledBibliographicClaim:
    """Owned detached input; pure projection is not a verification receipt."""
    inputs: graph.BibliographicClaimInput
    dependencies: tuple[dict, ...]
    bindings: dict

    def project(self):
        return graph.project_bibliographic_claim(copy.deepcopy(self.inputs))


@dataclass(frozen=True)
class AssembledAgentRecord:
    navigation_inputs: SourceNavigationRecordInput
    bibliographic_inputs: graph.BibliographicIdentityInput
    bindings: dict

    def project_navigation(self):
        return project_source_navigation_record(copy.deepcopy(self.navigation_inputs))

    def project_bibliographic(self):
        return graph.project_bibliographic_identity(copy.deepcopy(self.bibliographic_inputs))


class _Files:
    """Protected exact files and observed absences; no directory discovery."""
    def __init__(self, root, limits):
        self.root, self.limits = root, limits
        self.values, self.observed = {}, {}
        self.read_bytes = 0

    def _path(self, ref):
        path = Path(ref) if isinstance(ref, str) else Path()
        if (not isinstance(ref, str) or path.is_absolute() or path.as_posix() != ref
                or '\\' in ref or '\x00' in ref or not path.parts
                or any(part.startswith('.') or part in {'payload', 'private', 'owner-local', 'local-content'}
                       for part in path.parts)):
            raise ClaimAssemblyError('assembly file is not exact public owner metadata')
        return self.root / path

    def read(self, ref, limit=None, *, optional=False):
        path = self._path(ref)
        cap = min(self.limits.max_file_bytes, limit if limit is not None else self.limits.max_file_bytes)
        if ref in self.values:
            raw = self.values[ref]
            if raw is not None and len(raw) > cap:
                raise ClaimAssemblyBudgetExceeded('selected file exceeds caller byte bound')
            if raw is None and not optional:
                raise ClaimAssemblyError('required source input is absent')
            return raw
        if len(self.observed) >= self.limits.max_files:
            raise ClaimAssemblyBudgetExceeded('selected file-count bound exceeded')
        if optional and not os.path.lexists(path):
            # The protected parent prevents an absent-path symlink escape.
            descriptor = source._owned_path(path.parent, directory=True)
            os.close(descriptor)
            self.observed[ref] = None
            self.values[ref] = None
            return None
        descriptor = source._owned_path(path)
        try:
            before = catalog.SourceCatalogSourceReader._signature(os.fstat(descriptor))
            size = before[4]
            if size > cap or self.read_bytes + size > self.limits.max_read_bytes:
                raise ClaimAssemblyBudgetExceeded('selected file byte/read bound exceeded')
            self.read_bytes += size
            raw = os.read(descriptor, size + 1)
            after = catalog.SourceCatalogSourceReader._signature(os.fstat(descriptor))
        finally:
            os.close(descriptor)
        if before != after or len(raw) != size:
            raise ClaimAssemblyError('selected owner file changed during read')
        self.observed[ref], self.values[ref] = after, raw
        return raw

    def json(self, ref, digests=None):
        if not (ref.startswith('ToS/contracts/') or ref in {profiles.REGISTRY_REF, profiles.CLAIM_REGISTRY_REF}):
            raise ClaimAssemblyError('schema/registry read is outside exact owner contracts')
        raw = self.read(ref, profiles.MAX_RECORD_BYTES)
        value = catalog._strict_json(raw)
        if not isinstance(value, dict):
            raise ClaimAssemblyError('owner JSON input must be an object')
        if digests is not None:
            digests[ref] = hashlib.sha256(raw).hexdigest()
        return value

    def verify(self):
        for ref, observed in self.observed.items():
            path = self._path(ref)
            if observed is None:
                descriptor = source._owned_path(path.parent, directory=True)
                os.close(descriptor)
                if os.path.lexists(path):
                    raise ClaimAssemblyError('an absent adjacent source form appeared during assembly')
            else:
                descriptor = source._owned_path(path)
                try:
                    current = catalog.SourceCatalogSourceReader._signature(os.fstat(descriptor))
                finally:
                    os.close(descriptor)
                if current != observed:
                    raise ClaimAssemblyError('selected owner file changed during assembly')

    def bindings(self):
        return {ref: (None if raw is None else {'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw)})
                for ref, raw in sorted(self.values.items())}


class _Objects:
    def __init__(self, assembler):
        self.assembler = assembler

    def get(self, identity, default=None):
        value = self.assembler._object(identity)
        return default if value is None else value

    def __contains__(self, identity):
        return self.get(identity) is not None

    def __getitem__(self, identity):
        value = self.get(identity)
        if value is None:
            raise ClaimAssemblyError('declared metadata endpoint is absent from the exact catalog')
        return value


class _Slots:
    def __init__(self, assembler, kind):
        self.assembler, self.kind = assembler, kind
        self.values = {}

    def get(self, identity, default=None):
        if identity not in self.values:
            self.assembler._lookup()
            slot = self.assembler.catalog_snapshot.lookup_slot(self.kind, identity)
            if slot is None:
                self.values[identity] = None
            else:
                read = self.assembler.source_reader.read_slot(self.kind, identity,
                                                            expected_row_sha256=slot.row_sha256)
                self.assembler._slots[slot.source_slot_key] = read.provenance
                self.values[identity] = {'payload': read.payload,
                    'source_ref': slot.source['source_ref'], 'source_line': slot.source['source_line'],
                    'source_sha256': slot.source['canonical_sha256']}
        return default if self.values[identity] is None else self.values[identity]

    def __contains__(self, identity):
        return self.get(identity) is not None

    def __getitem__(self, identity):
        value = self.get(identity)
        if value is None:
            raise ClaimAssemblyError('declared source slot is absent from the exact catalog')
        return value


class BibliographicClaimAssembler:
    """Single-snapshot concrete forward assembly with separate owner budgets.

    ``limits`` bounds this adapter's selected files/lookups/materializations;
    ``slot_limits`` bounds real current source rows/profile verification; the
    catalog retains its caller-selected MutationLimits, while the
    MetadataVersionReader and lazy ClaimVersionReader retain their own bounded
    aggregate/per-record/history owner limits. None is a caller-attested
    verification flag. Reuse the assembler, then call verify_current
    immediately before the stronger owner's guarded publication.
    """
    def __init__(self, root: Path, *, catalog_snapshot: SourceCatalogSnapshot,
                 limits: ClaimAssemblyLimits | None = None, slot_limits: SourceSlotLimits | None = None):
        self.root = Path(root)
        self.limits = ClaimAssemblyLimits() if limits is None else limits
        if not isinstance(self.limits, ClaimAssemblyLimits):
            raise TypeError('assembly limits must be ClaimAssemblyLimits')
        self.source_reader = SourceCatalogSourceReader(self.root, catalog_snapshot=catalog_snapshot,
                                                      limits=slot_limits)
        self.catalog_snapshot = catalog_snapshot
        self.metadata_reader = MetadataVersionReader(self.root, catalog_snapshot=catalog_snapshot)
        # The historical Claim reader is intentionally lazy: ordinary Claims
        # must not pay for or imply historical transport. When a native
        # collection-order Claim opts into the exact basis adapter below, this
        # is the same owner resolver used by the full bibliographic builder.
        self._claim_version_reader = None
        # The full graph builder grounds this basis with its independent exact
        # metadata reader (without an addressed snapshot). Keep that provenance
        # shape identical while current endpoint nodes remain catalog-backed by
        # ``self.metadata_reader``.
        self._collection_metadata_reader = None
        self.files = _Files(self.root, self.limits)
        self._objects, self._metadata, self._slots, self._claim_rows = {}, {}, {}, {}
        self._lookups = self._claims = self._output_bytes = 0
        self._profiles = None
        self._form_validators = None
        self.objects = _Objects(self)
        self.anchors, self.events = _Slots(self, 'anchor'), _Slots(self, 'provenance_event')
        self.navigation_registry = graph.load_claim_navigation_registry(self.root, read_json=self.files.json)
        self.entity_registry = self.files.json(profiles.REGISTRY_REF)
        self.verify_current()

    def _lookup(self):
        if self._lookups >= self.limits.max_addressed_lookups:
            raise ClaimAssemblyBudgetExceeded('addressed lookup bound exceeded')
        self._lookups += 1

    def _forms(self, entry, record, *, claim=False):
        source_path = self.root / entry['source_claim_file_ref' if claim else 'source_record_ref']
        if claim:
            if source_path.name not in {profiles.SOURCE_CLAIM_BASENAME, 'historical-claims.jsonl'}:
                return None
            path = forms.claim_forms_path(source_path, record['claim_id'])
        else:
            path = source_path.with_name(source_path.stem + '.human-forms.json')
        ref = path.relative_to(self.root).as_posix()
        raw = self.files.read(ref, forms.MAX_SET_BYTES, optional=True)
        if raw is None:
            return None
        if self._form_validators is None:
            schemas = {name: self.files.json('ToS/contracts/' + name + '.schema.json') for name in
                       ('knowledge-assessment', 'human-form', 'human-form-set', 'human-form-template')}
            set_validator, materializers = compile_source_form_validators(schemas)
            corpus = self.files.json('ToS/contracts/corpus-record.schema.json')
            registry = Registry().with_resource(corpus['$id'], Resource.from_contents(corpus))
            language = Draft202012Validator({'$ref': corpus['$id'] + '#/properties/field_languages'}, registry=registry)
            self._form_validators = (set_validator, materializers, language)
        validator, materializers, language = self._form_validators
        if claim:
            subject = forms.Record.from_payload(record['claim_id'], record['claim_version'], record)
            display = None
            if record.get('qualifiers', {}).get('display_fields', {}).get('schema_version') == forms.CLAIM_DISPLAY_VERSION:
                schema, corpus = self.files.json(forms.CLAIM_DISPLAY_SCHEMA), self.files.json(profiles.CORPUS_REF)
                registry = Registry().with_resources((item['$id'], Resource.from_contents(item)) for item in (schema, corpus))
                display = Draft202012Validator(schema, registry=registry)
            fields = forms.claim_field_catalog(record, validators=(language, display))
        else:
            subject = forms.metadata_subject(record)
            fields = forms.metadata_field_catalog(record, field_language_validator=language)
        material = forms._materialize_forms(subject, fields, catalog._strict_json(raw), access_allowed=True,
                                           validator=validator, materializer_validators=materializers)
        return ref, raw, material

    def _object(self, identity):
        if (not isinstance(identity, str) or catalog.IDENTITY.fullmatch(identity) is None
                or identity.startswith('tos.claim.')):
            return None
        if identity not in self._objects:
            self._lookup()
            row = self.catalog_snapshot.lookup(identity)
            if row is None:
                self._objects[identity] = None
                return None
            if len(self._metadata) >= self.limits.max_metadata_records:
                raise ClaimAssemblyBudgetExceeded('selected metadata record bound exceeded')
            resolved = self.metadata_reader.resolve(row.source['record_ref'])
            if resolved['status'] != 'available' or resolved['version_status'] != 'current':
                raise ClaimAssemblyError('exact current endpoint unavailable: ' + str(resolved['status']) + '/' + str(resolved['reason']))
            record, entry = resolved['record'], row.entry
            material = {**entry, '_source_record': record}
            selected_forms = self._forms(entry, record)
            if selected_forms is not None:
                ref, raw, packets = selected_forms
                material.update(_human_forms=packets, _human_forms_source_ref=ref,
                                _human_forms_sha256=hashlib.sha256(raw).hexdigest())
            self._metadata[identity] = {'row': row, 'resolved': resolved, 'forms': selected_forms}
            self._objects[identity] = material
        return self._objects[identity]

    def _claim_profiles(self):
        if self._profiles is None:
            self._profiles = profiles.SourceClaimProfiles(self.root, read_json=self.files.json)
        return self._profiles

    def _historical_claim_reader(self):
        """Return the exact owner resolver for declared Claim versions only."""
        if self._claim_version_reader is None:
            # Keep this import lazy so the ordinary current-only path remains
            # unchanged, and so the resolver's own version/digest checks stay
            # the authority for retained Claim bytes.
            import claim_version_reader
            self._claim_version_reader = claim_version_reader.ClaimVersionReader(self.root)
        return self._claim_version_reader

    def _collection_order_metadata_reader(self):
        """Return the full builder's bounded exact collection-version reader."""
        if self._collection_metadata_reader is None:
            self._collection_metadata_reader = MetadataVersionReader(self.root)
        return self._collection_metadata_reader

    def _validate(self, entry, claim):
        native = Path(entry['source_claim_file_ref']).name == profiles.SOURCE_CLAIM_BASENAME
        if entry['source_claim_file_ref'] == graph.OBJECT_LINK_CLAIM_REF:
            validate_legacy_object_link(claim, legacy_object_link_validator(self.files.json(LINK_SCHEMA)), self.objects)
            return render_legacy_object_link_context(entry['source_claim_line'], claim), None
        selected = self._claim_profiles() if native else None
        if (entry.get('claim_type') not in {'bibliographic', 'relation'}
                or not native and entry.get('claim_type') == 'relation'
                and entry.get('predicate') not in {'is_derivative_of', *graph.HISTORICAL_PREDICATES}):
            raise ClaimAssemblyUnsupported('Claim is outside the full bibliographic owner profile')
        if native:
            if claim.get('predicate') not in selected.profiles:
                raise ClaimAssemblyUnsupported('Claim has no declared native source profile')
            selected.validate(claim, self.objects)
            route = selected.schema_routes[claim['predicate'], claim['schema_version']]['schema_ref']
            if entry.get('source_schema_ref') != route:
                raise ClaimAssemblyError('Claim catalog schema route differs from its exact profile')
            basis_adapter = selected.profiles[claim['predicate']].get('object_reference_set', {}).get('basis_adapter')
            collection_order_basis = None
            if basis_adapter == 'collection-membership-versions-v1':
                # This is deliberately the shared owner grounding routine: it
                # resolves every exact collection and membership ref, retains
                # historical/current status and provenance, verifies the same
                # reader snapshots, and never substitutes a current Claim.
                collection_order_basis = profiles.ground_collection_order(
                    claim, self._collection_order_metadata_reader(), self._historical_claim_reader())
            return None, collection_order_basis
        else:
            if claim.get('assertion_layer') not in {'bibliographic_assertion', 'scholarly_report'}:
                raise ClaimAssemblyUnsupported('Claim assertion layer is outside the full owner profile')
            if claim.get('predicate') in graph.HISTORICAL_PREDICATES:
                contract = graph._historical_claim_contract(self.root, read_json=self.files.json)
                graph._validate_historical_claim(claim, self.objects, contract, read_json=self.files.json)
        return None, None

    def _bindings(self):
        return {'catalog_root_sha256': self.catalog_snapshot.root_sha256,
            'source_publication': self.catalog_snapshot.header['source_publication'],
            'claim_rows': copy.deepcopy(self._claim_rows), 'source_slots': copy.deepcopy(self._slots),
            'metadata': {identity: {'catalog': item['row'].provenance,
                'exact_ref': copy.deepcopy(item['resolved']['exact_ref']),
                'provenance': copy.deepcopy(item['resolved']['provenance'])}
                for identity, item in self._metadata.items()},
            'files': self.files.bindings(),
            'accounting': {'addressed_lookups': self._lookups, 'claims': self._claims,
                'metadata_records': len(self._metadata), 'files': len(self.files.observed),
                'read_bytes': self.files.read_bytes, 'output_bytes': self._output_bytes,
                'source_slots': self.source_reader.accounting, 'catalog': self.catalog_snapshot.accounting,
                'metadata_versions': self.metadata_reader.accounting,
                'collection_metadata_versions': (
                    self._collection_metadata_reader.accounting
                    if self._collection_metadata_reader is not None else None),
                'claim_versions': (self._claim_version_reader.accounting
                                   if self._claim_version_reader is not None else None)},
            'scope': 'selected-forward-source-assembly', 'reverse_claim_closure_verified': False,
            'source_admission': False, 'establishes_epoch': False,
            'historical_claim_transport': self._claim_version_reader is not None}

    def _charge_output(self, value):
        size = len(catalog.canonical_bytes(_json_size_value(value)))
        if self._output_bytes + size > self.limits.max_output_bytes:
            raise ClaimAssemblyBudgetExceeded('assembled cohort output bound exceeded')
        self._output_bytes += size

    def assemble(self, claim_id: str, *, expected_row_sha256: str) -> AssembledBibliographicClaim:
        self.verify_current()
        if self._claims >= self.limits.max_claims:
            raise ClaimAssemblyBudgetExceeded('selected Claim count bound exceeded')
        self._claims += 1
        self._lookup()
        read = self.source_reader.read_claim(claim_id, expected_row_sha256=expected_row_sha256)
        entry, claim = read.catalog_claim.entry, read.payload
        self._claim_rows[claim_id] = read.catalog_claim.provenance
        self._slots[read.slot.source_slot_key] = read.provenance
        context, collection_order_basis = self._validate(entry, claim)
        subject_node = graph._identity_node(self.objects[claim['subject_ref']])
        value = claim['object']
        if isinstance(value, str) and value in self.objects:
            object_node = graph._identity_node(self.objects[value])
        elif isinstance(value, str) and value.startswith('tos.'):
            raise ClaimAssemblyError('identity-like Claim object is unresolved')
        else:
            object_node = graph._literal_node(value, claim, entry)
        event = self.events.get(claim['provenance_event_ref'])
        if event is None:
            raise ClaimAssemblyError('Claim provenance event does not resolve')
        event_node = graph._event_node(event, repo_root=self.root, read_json=self.files.json)
        baseline = self.catalog_snapshot.header['legacy_baseline']['files'][graph.CLAIM_CATALOG_REF]['sha256']
        maker = graph._maker_node(claim['maker'], objects=self.objects, claim_catalog_sha256=baseline)
        maker_identity = (graph._identity_node(self.objects[claim['maker']['agent_ref']])
                          if maker['properties']['identity_node_id'] is not None else None)
        def evidence(refs):
            if not isinstance(refs, list):
                raise ClaimAssemblyError('evidence references must preserve the source list')
            return tuple(graph._evidence_node(str(ref), repo_root=self.root, anchors=self.anchors,
                objects=self.objects, events=self.events, citing_claim=claim, claim_entry=entry,
                read_bytes=self.files.read) for ref in refs)
        evidence_nodes = evidence(claim['evidence_refs'])
        counterevidence = evidence(claim.get('counterevidence_refs', []))
        native = Path(entry['source_claim_file_ref']).name == profiles.SOURCE_CLAIM_BASENAME
        selected = self._claim_profiles() if native else None
        member_nodes = tuple(graph._identity_node(self.objects[ref]) for ref in
                             (selected.reference_members(claim) if native else ()))
        identity_edges = graph._provision_identity_edges(claim, objects=self.objects)
        temporal = selected.is_temporal(claim) if native else claim['predicate'] == 'historical_dating'
        if temporal and claim['object'].get('relative'):
            identity_edges.append(('has_historical_date_anchor', claim['object']['relative']['anchor_ref']))
        alternatives = claim.get('alternative_claim_refs', [])
        if not isinstance(alternatives, list):
            raise ClaimAssemblyError('alternative Claim refs must be a list')
        existing = set()
        for identity in [*alternatives, *([claim['supersedes_claim_ref']] if claim.get('supersedes_claim_ref') else [])]:
            if identity not in existing:
                self._lookup()
                row = self.catalog_snapshot.get_claim(identity)
                alternate = self.source_reader.read_claim(identity, expected_row_sha256=row.row_sha256)
                self._claim_rows[identity] = row.provenance
                self._slots[alternate.slot.source_slot_key] = alternate.provenance
                existing.add(identity)
        inputs = graph.BibliographicClaimInput(entry, claim, subject_node, object_node, event_node, maker,
            evidence_nodes, counterevidence, frozenset(existing), self.navigation_registry, self.entity_registry,
            maker_identity_node=maker_identity, member_nodes=member_nodes,
            normalized_identity_edges=tuple((kind, graph._identity_node(self.objects[ref])) for kind, ref in identity_edges),
            forms=self._forms(entry, claim, claim=True), collection_order_basis=collection_order_basis,
            legacy_object_link_context=context)
        dependencies = graph.enumerate_bibliographic_claim_dependencies(inputs)
        projected = graph.project_bibliographic_claim(inputs)
        self._charge_output({'inputs': inputs, 'projection': projected,
                             'dependencies': dependencies, 'bindings': self._bindings()})
        self.verify_current()
        return AssembledBibliographicClaim(copy.deepcopy(inputs), copy.deepcopy(dependencies), self._bindings())

    def assemble_record(self, record_id: str, *, expected_row_sha256: str) -> AssembledAgentRecord:
        """Selected Agent only: current identity/forms plus exact retained versions."""
        self.verify_current()
        self._lookup()
        row = self.catalog_snapshot.get(record_id)
        if row.row_sha256 != catalog._digest(expected_row_sha256):
            raise ClaimAssemblyError('selected metadata catalog row digest differs')
        if row.entry['record_type'] != 'agent':
            raise ClaimAssemblyUnsupported('record cohort assembler is limited to selected Agent metadata')
        material = self.objects[record_id]
        item = self._metadata[record_id]
        history = self.metadata_reader.exact_refs(record_id)
        if history['status'] != 'available' or history['current_ref'] != row.source['record_ref']:
            raise ClaimAssemblyError('exact Agent history is unavailable or catalog binding drifted')
        versions = tuple((ref, self.metadata_reader.resolve(ref)) for ref in history['refs'])
        if any(resolved['status'] != 'available' for _, resolved in versions):
            raise ClaimAssemblyError('an exact Agent history version is unavailable')
        navigation = SourceNavigationRecordInput('agent', row.entry, material['_source_record'],
                                                item['forms'], history, versions)
        identity = graph.BibliographicIdentityInput(row.entry, material['_source_record'],
            material.get('_human_forms'), material.get('_human_forms_source_ref'))
        projected = project_source_navigation_record(navigation)
        self._charge_output({'navigation': navigation, 'identity_input': identity,
                             'projection': projected, 'identity': graph.project_bibliographic_identity(identity),
                             'bindings': self._bindings()})
        self.verify_current()
        return AssembledAgentRecord(copy.deepcopy(navigation), copy.deepcopy(identity), self._bindings())

    def verify_current(self):
        self.source_reader.verify_current()
        self.metadata_reader.verify_current()
        if self._collection_metadata_reader is not None:
            self._collection_metadata_reader.verify_current()
        if self._claim_version_reader is not None:
            self._claim_version_reader.verify_current()
        self.files.verify()
