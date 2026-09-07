"""Execute declared source record/claim profiles from existing type registries.

This reader selects no executable and grants no write or semantic authority.
The source schema, exact metadata, and profile stay separate from disposable
catalogs. Native non-Corpus shapes (for example artifacts) keep their adapters.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource
from referencing.exceptions import Unresolvable

REGISTRY_REF = 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
CONTRACT_REF = 'ToS/contracts/semantic-entity-type-registry.schema.json'
CORPUS_REF = 'ToS/contracts/corpus-record.schema.json'
SOURCE_ROOT = Path('ToS/source-witnesses')
MAX_RECORD_BYTES = 1_048_576
SOURCE_CLAIM_BASENAME = 'source-claims.jsonl'
CLAIM_REGISTRY_REF = 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
CLAIM_CONTRACT_REF = 'ToS/contracts/semantic-relation-type-registry.schema.json'
CLAIM_BASE_REF = 'ToS/contracts/source-claim-record.schema.json'
TEMPORAL_VALUE_REF = 'ToS/contracts/historical-claim.schema.json'
STRUCTURED_VALUE_REF = 'ToS/contracts/source-structured-value.schema.json'
CLAIM_SHARED_REFS = ('ToS/contracts/claim-packet.schema.json',
                     'ToS/contracts/knowledge-assessment.schema.json', CLAIM_BASE_REF)
MAX_CLAIM_FILE_BYTES = 16_777_216
RESERVED_KINDS = {'agent', 'place', 'organization', 'work', 'expression', 'edition',
                  'collection', 'item', 'link', 'artifact', 'composite'}
RESERVED_BASENAMES = {kind + '.json' for kind in RESERVED_KINDS} | {'artifact-witness.json', 'composite-witness.json'}
RESERVED_CATALOGS = {kind + 's.jsonl' for kind in RESERVED_KINDS} | {'claims.jsonl'}
METADATA_LINK_FIELDS = (
    'work_ref', 'expression_claim_refs', 'responsibility_claim_refs', 'chronology_claim_refs',
    'embodiment_claim_refs', 'derivation_claim_refs', 'embodies_expression_refs',
    'publication_claim_refs', 'provision_activity_claim_refs', 'exemplar_claim_refs',
    'collection_ref', 'membership_claim_refs', 'item_manifest_ref', 'association_claim_refs',
)


class SourceProfileError(ValueError):
    """Source or profile cannot be read without inventing a mapping."""


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise SourceProfileError('profile input contains a duplicate JSON field')
        result[key] = value
    return result


def _nonfinite(_value):
    raise SourceProfileError('profile input contains a nonfinite JSON number')


def _read_json(root: Path, ref: str, digests: dict | None = None) -> dict:
    path = root / ref
    if path.is_symlink() or not path.is_file() or path.resolve() != path.absolute():
        raise SourceProfileError(f'{ref}: profile input must be a regular non-symlink file')
    with path.open('rb') as stream:
        raw = stream.read(MAX_RECORD_BYTES + 1)
    if len(raw) > MAX_RECORD_BYTES:
        raise SourceProfileError(f'{ref}: profile input exceeds 1 MiB')
    try:
        value = json.loads(raw, object_pairs_hook=_unique_object, parse_constant=_nonfinite)
    except (ValueError, UnicodeError) as error:
        raise SourceProfileError(f'{ref}: profile input is not JSON') from error
    if not isinstance(value, dict):
        raise SourceProfileError(f'{ref}: profile input must be an object')
    try:
        json.dumps(value, allow_nan=False)
    except ValueError as error:
        raise SourceProfileError(f'{ref}: profile input contains a nonfinite JSON value') from error
    if digests is not None:
        digests[ref] = hashlib.sha256(raw).hexdigest()
    return value


def _schema_route(root, route, digests, cache, shared_refs=()):
    """Exact local resources only; neither metadata nor a claim selects code."""
    refs = list(dict.fromkeys([*shared_refs, *route['schema_dependencies'], route['schema_ref']]))
    resources = {}
    for ref in refs:
        if ref not in cache:
            schema = _read_json(root, ref, digests)
            if schema.get('$id') not in {'https://tree-of-sophia.local/' + ref,
                                          'https://treeofsophia.local/' + ref}:
                raise SourceProfileError(f'{ref}: schema identity differs from its declared owner path')
            Draft202012Validator.check_schema(schema)
            cache[ref] = schema
        schema = cache[ref]
        if schema['$id'] in resources:
            raise SourceProfileError('duplicate schema resource identity')
        resources[schema['$id']] = Resource.from_contents(schema)
    registry = Registry().with_resources(resources.items())
    return Draft202012Validator(cache[route['schema_ref']], registry=registry,
                                format_checker=FormatChecker()), registry


def _type_ancestry(entities, type_id, visiting=frozenset()):
    if type_id not in entities or type_id in visiting:
        raise SourceProfileError('unknown or cyclic source type hierarchy')
    result = {type_id}
    for parent in entities[type_id]['parent_type_ids']:
        result.update(_type_ancestry(entities, parent, visiting | {type_id}))
    return result


class SourceRecordProfiles:
    """One bounded registry/schema snapshot per source build, never global cache."""

    def __init__(self, root: Path):
        self.root = root
        self.input_digests = {}
        self.registry = _read_json(root, REGISTRY_REF, self.input_digests)
        contract = _read_json(root, CONTRACT_REF, self.input_digests)
        if not Draft202012Validator(contract).is_valid(self.registry):
            raise SourceProfileError('entity registry violates its source contract')
        descriptor_contract = contract['$defs']['sourceRecordProfile']
        validator = Draft202012Validator(descriptor_contract)
        entities = {entry['type_id']: entry for entry in self.registry['types']}
        if len(entities) != len(self.registry['types']):
            raise SourceProfileError('duplicate entity type identity')
        self.profiles = {}
        self.validators = {}
        self.metadata_validators = {}
        self.schema_routes = {}
        self.schemas = {}
        seen = {key: set() for key in ('record_type', 'id_prefix', 'source_basename', 'catalog_filename')}
        for entry in self.registry.get('types', []):
            profile = entry.get('source_record_profile')
            if profile is None:
                continue
            if not validator.is_valid(profile):
                raise SourceProfileError('source-record profile violates its declared contract')
            kind = profile['record_type']
            retained_composite = (kind == 'composite' and entry['type_id'] == 'tos.entity.composite'
                and profile.get('retained_native_adapter') == 'scholarly-composite-v1'
                and profile['reader'] == 'corpus-metadata-v1'
                and profile['catalog_filename'] == 'composites.jsonl')
            if 'retained_native_adapter' in profile and not retained_composite:
                raise SourceProfileError(f'{kind}: incompatible retained native adapter')
            role, family = {'corpus-metadata-v1': ('identity', 'tos.entity.identity'),
                            'semantic-metadata-v1': ('semantic', 'tos.entity.semantic-object')}[profile['reader']]
            if (entry.get('abstract') is not False or entry.get('object_role') != role
                    or family not in _type_ancestry(entities, entry['type_id'])
                    or entry['type_id'] == family
                    or (not retained_composite and (kind in RESERVED_KINDS
                        or profile['source_basename'] in RESERVED_BASENAMES
                        or profile['catalog_filename'] in RESERVED_CATALOGS))
                    or profile['id_prefix'] != f'tos.{kind}.'
                    or profile['source_basename'] != kind + '.json'):
                raise SourceProfileError(f'{kind}: source-record profile identity or adapter collision')
            mappings = entry.get('source_mappings', [])
            if any(sum(mapping.get('source_graph') == graph and mapping.get('source_kind_id') == kind
                       for mapping in mappings) != 1 for graph in ('source-claims', 'source-navigation')):
                raise SourceProfileError(f'{kind}: source-record profile must map both readers exactly once')
            for other in self.registry['types']:
                if other is not entry and any(mapping.get('source_kind_id') == kind
                                              and mapping.get('source_graph') in {'source-claims', 'source-navigation'}
                                              for mapping in other.get('source_mappings', [])):
                    raise SourceProfileError(f'{kind}: source profile mapping has another type owner')
            for key, values in seen.items():
                if profile[key] in values:
                    raise SourceProfileError(f'{kind}: duplicate source-record profile {key}')
                values.add(profile[key])
            self.profiles[kind] = profile
            for route in profile['schemas']:
                key = kind, route['schema_version']
                if key in self.schema_routes:
                    raise SourceProfileError(f'{kind}: duplicate source schema-version route')
                self.schema_routes[key] = route

    @property
    def catalog_files(self) -> dict[str, str]:
        return {kind: profile['catalog_filename'] for kind, profile in self.profiles.items()}

    @property
    def source_basenames(self) -> dict[str, str]:
        return {kind: profile['source_basename'] for kind, profile in self.profiles.items()}

    def validator(self, kind: str, schema_version: str) -> Draft202012Validator:
        key = kind, schema_version
        if key not in self.schema_routes:
            raise SourceProfileError(f'{kind}: unsupported source schema version')
        if key not in self.validators:
            route = self.schema_routes[key]
            self.validators[key], registry = _schema_route(self.root, route, self.input_digests,
                                                           self.schemas, [CORPUS_REF])
            fields = ('preferred_label', 'variant_labels', 'field_languages', 'identity_status',
                      'source_refs', 'external_identifiers', 'same_as_posture', 'record_version', 'notes')
            common = {'type': 'object',
                      'required': ['preferred_label', 'identity_status', 'source_refs',
                                   'external_identifiers', 'same_as_posture', 'record_version'],
                      'properties': {field: {'$ref': self.schemas[CORPUS_REF]['$id'] + '#/properties/' + field}
                                     for field in fields}}
            self.metadata_validators[key] = Draft202012Validator(common, registry=registry,
                                                                 format_checker=FormatChecker())
        return self.validators[key]

    def validate_path(self, kind: str, ref: str) -> None:
        """Apply the same owner-home guard to prospective writes and reads."""
        path = Path(ref)
        if (path.is_absolute() or '..' in path.parts or path.as_posix() != ref
                or not path.is_relative_to(SOURCE_ROOT) or 'catalog' in path.parts
                or any(part in {'payload', 'local-content'} for part in path.parts)
                or path.name != self.profiles[kind]['source_basename']
                or (kind == 'composite' and (len(path.parts) < 7
                    or not path.is_relative_to(SOURCE_ROOT / 'scholarly-composites')))):
            raise SourceProfileError('source-record profile path is outside its metadata home')

    def load(self, kind: str, ref: str) -> dict:
        self.validate_path(kind, ref)
        source = _read_json(self.root, ref)
        self.validate(kind, source)
        return source

    def validate(self, kind: str, source: dict) -> None:
        profile = self.profiles[kind]
        if source.get('visibility') not in {'public', 'public_metadata_only'}:
            raise SourceProfileError(f'{kind}: source visibility is outside public metadata')
        if (not isinstance(source.get('schema_version'), str) or source.get('record_type') != kind
                or not isinstance(source.get('record_id'), str)
                or not re.fullmatch(re.escape(profile['id_prefix']) + r'[a-z0-9]+(?:[.-][a-z0-9]+)*', source['record_id'])
                or type(source.get('record_version')) is not int or source['record_version'] < 1
                or source.get('identity_status') not in {'provisional', 'verified', 'disputed', 'superseded'}
                or not isinstance(source.get('preferred_label'), str) or not source['preferred_label'].strip()):
            raise SourceProfileError(f'{kind}: invalid source identity or incompatible metadata')
        try:
            key = kind, source['schema_version']
            if not self.validator(*key).is_valid(source) or not self.metadata_validators[key].is_valid(source):
                raise SourceProfileError(f'{kind}: source record violates its exact profile schema or shared metadata contract')
        except Unresolvable as error:
            raise SourceProfileError(f'{kind}: source schema has an undeclared dependency') from error

    def catalog_entry(self, kind: str, source: dict, ref: str) -> dict:
        self.validate(kind, source)
        digest = hashlib.sha256(json.dumps(source, ensure_ascii=False, sort_keys=True, allow_nan=False,
                                           separators=(',', ':')).encode('utf-8')).hexdigest()
        return {'schema_version': 'tos_source_witness_catalog_entry_v1',
                'source_schema_ref': self.schema_routes[kind, source['schema_version']]['schema_ref'],
                **{key: source[key] for key in ('record_id', 'record_type', 'preferred_label', 'identity_status')},
                'source_record_ref': ref, 'record_sha256': digest,
                'links': {field: source[field] for field in METADATA_LINK_FIELDS if field in source}}

    def verify_entry(self, kind: str, entry: dict) -> dict:
        source = self.load(kind, entry['source_record_ref'])
        if entry != self.catalog_entry(kind, source, entry['source_record_ref']):
            raise SourceProfileError(f'{kind}: catalog/source profile mapping drifted')
        return source


class SourceClaimProfiles:
    """Read declared evidence-bearing source relations, without admission.

    One shared source-claims.jsonl stream format serves new predicates. Legacy
    bibliographic/historical files retain their existing adapters and schemas.
    """

    def __init__(self, root: Path):
        self.root, self.input_digests = root, {}
        self.registry = _read_json(root, CLAIM_REGISTRY_REF, self.input_digests)
        contract = _read_json(root, CLAIM_CONTRACT_REF, self.input_digests)
        if not Draft202012Validator(contract).is_valid(self.registry):
            raise SourceProfileError('relation registry violates its source contract')
        entity_registry = _read_json(root, REGISTRY_REF, self.input_digests)
        entity_contract = _read_json(root, CONTRACT_REF, self.input_digests)
        if not Draft202012Validator(entity_contract).is_valid(entity_registry):
            raise SourceProfileError('entity registry violates its source contract')
        self.entities = {entry['type_id']: entry for entry in entity_registry['types']}
        if len(self.entities) != len(entity_registry['types']):
            raise SourceProfileError('duplicate entity type identity')
        self.mappings, self.profiles, self.relations = {}, {}, {}
        self.schema_routes, self.schemas, self.validators, self.base_validators = {}, {}, {}, {}
        self.temporal_validators = {}
        self.value_validators = {}
        if len({entry['relation_type_id'] for entry in self.registry['relations']}) != len(self.registry['relations']):
            raise SourceProfileError('duplicate relation type identity')
        for entry in entity_registry['types']:
            for mapping in entry['source_mappings']:
                if mapping['source_graph'] != 'source-claims':
                    continue
                kind = mapping['source_kind_id']
                if kind in self.mappings:
                    raise SourceProfileError('source kind has more than one identity mapping')
                self.mappings[kind] = entry['type_id']
        for entry in self.registry['relations']:
            profile = entry.get('source_claim_profile')
            if profile is None:
                continue
            mappings = [mapping for mapping in entry['source_mappings']
                        if mapping['source_graph'] == 'source-claims' and mapping['scope'] == 'claim-predicate']
            if (len(mappings) != 1 or entry['abstract'] or entry['assertion_mode'] != 'reified-claim'
                    or not entry['evidence_required']):
                raise SourceProfileError('source claim profile requires one reified evidence-bearing predicate mapping')
            predicate = mappings[0]['source_predicate_id']
            if any(other is not entry and any(mapping['source_graph'] == 'source-claims'
                    and mapping['scope'] == 'claim-predicate' and mapping['source_predicate_id'] == predicate
                    for mapping in other['source_mappings']) for other in self.registry['relations']):
                raise SourceProfileError('source predicate has another relation owner')
            semantic_endpoint = False
            for endpoint, type_id in ((endpoint, type_id) for endpoint in ('domain_type_ids', 'range_type_ids')
                                      for type_id in entry[endpoint]):
                if type_id in {'tos.entity.thing', 'tos.entity.identity', 'tos.entity.semantic-object',
                               'tos.entity.unmapped', 'tos.entity.unresolved-endpoint'}:
                    raise SourceProfileError('source relation profile requires a specific domain and range')
                ancestry = self.ancestry(type_id)
                if profile['reader'] == 'historical-temporal-v1':
                    expected = ('tos.entity.historical-situation' if endpoint == 'domain_type_ids'
                                else 'tos.entity.temporal-assertion')
                    if expected not in ancestry:
                        raise SourceProfileError('historical temporal profile requires historical domain and temporal value range')
                elif profile['reader'] == 'structured-value-v1':
                    if endpoint == 'domain_type_ids':
                        if not ancestry.intersection({'tos.entity.identity', 'tos.entity.semantic-object'}):
                            raise SourceProfileError('structured value subject requires a specific identity or semantic family')
                    elif (len(entry['range_type_ids']) != 1 or type_id == 'tos.entity.literal'
                            or 'tos.entity.literal' not in ancestry
                            or self.entities[type_id]['abstract'] or self.entities[type_id]['object_role'] != 'literal'
                            or self.mappings.get(profile['value_kind']) != type_id):
                        raise SourceProfileError('structured value range requires one concrete mapped literal subtype')
                elif profile['reader'] == 'identity-relation-v1':
                    if 'tos.entity.identity' not in ancestry:
                        raise SourceProfileError('identity relation endpoint domain/range must be an identity family')
                else:
                    semantic_endpoint |= 'tos.entity.semantic-object' in ancestry
                    if not ancestry.intersection({'tos.entity.identity', 'tos.entity.semantic-object'}):
                        raise SourceProfileError('semantic relation endpoint must be a specific semantic or identity family')
            if profile['reader'] == 'semantic-relation-v1' and not semantic_endpoint:
                raise SourceProfileError('semantic relation profile requires a semantic endpoint')
            self.profiles[predicate], self.relations[predicate] = profile, entry
            for route in profile['schemas']:
                key = predicate, route['schema_version']
                if key in self.schema_routes:
                    raise SourceProfileError('duplicate claim schema-version route')
                self.schema_routes[key] = route

    def ancestry(self, type_id, visiting=frozenset()):
        return _type_ancestry(self.entities, type_id, visiting)

    def is_temporal(self, claim):
        return self.profiles[claim['predicate']]['reader'] == 'historical-temporal-v1'

    def is_value(self, claim):
        return self.profiles[claim['predicate']]['reader'] in {'historical-temporal-v1', 'structured-value-v1'}

    def identity_refs(self, claim):
        """Identity dependencies of a validated Claim; values never become IDs."""
        refs = {claim['subject_ref']}
        if self.is_temporal(claim):
            if claim['object']['kind'] == 'relative-order':
                refs.add(claim['object']['relative']['anchor_ref'])
        elif not self.is_value(claim):
            refs.add(claim['object'])
        return refs

    def validate(self, claim, objects=None):
        if (not isinstance(claim, dict) or not isinstance(claim.get('predicate'), str)
                or not isinstance(claim.get('schema_version'), str)):
            raise SourceProfileError('source claim must declare a string predicate and schema version')
        predicate = claim.get('predicate')
        key = predicate, claim.get('schema_version')
        if key not in self.schema_routes:
            raise SourceProfileError('unrecognized source claim predicate or schema version')
        if (claim.get('claim_type') != 'relation' or not isinstance(claim.get('subject_ref'), str)
                or not isinstance(claim.get('object'), dict if self.is_value(claim) else str)
                or claim.get('assertion_layer') not in self.profiles[predicate]['assertion_layers']
                or claim.get('visibility') not in {'public', 'public_metadata_only'}
                or claim.get('claim_id') in (claim.get('subject_ref'), claim.get('object'))):
            raise SourceProfileError('source claim identity, endpoints, layer or visibility violates its profile')
        if key not in self.validators:
            shared_refs = (*CLAIM_SHARED_REFS, *([TEMPORAL_VALUE_REF] if self.is_temporal(claim) else []))
            if self.profiles[predicate]['reader'] == 'structured-value-v1':
                shared_refs = (*shared_refs, CORPUS_REF, STRUCTURED_VALUE_REF)
            self.validators[key], registry = _schema_route(self.root, self.schema_routes[key], self.input_digests,
                                                           self.schemas, shared_refs)
            self.base_validators[key] = Draft202012Validator(self.schemas[CLAIM_BASE_REF], registry=registry,
                                                             format_checker=FormatChecker())
            if self.is_temporal(claim):
                self.temporal_validators[key] = Draft202012Validator(
                    {'$ref': self.schemas[TEMPORAL_VALUE_REF]['$id'] + '#/$defs/historicalDate'},
                    registry=registry, format_checker=FormatChecker())
            if self.profiles[predicate]['reader'] == 'structured-value-v1':
                self.value_validators[key] = Draft202012Validator(self.schemas[STRUCTURED_VALUE_REF],
                    registry=registry, format_checker=FormatChecker())
        try:
            if not self.validators[key].is_valid(claim) or not self.base_validators[key].is_valid(claim):
                raise SourceProfileError('source claim violates its exact schema or shared record contract')
            if self.is_temporal(claim) and not self.temporal_validators[key].is_valid(claim['object']):
                raise SourceProfileError('source claim violates the shared historical temporal value contract')
            if key in self.value_validators and (not self.value_validators[key].is_valid(claim['object'])
                    or claim['object']['kind'] != self.profiles[predicate]['value_kind']):
                raise SourceProfileError('source claim violates the shared structured value contract or declared kind')
        except Unresolvable as error:
            raise SourceProfileError('source claim schema has an undeclared dependency') from error
        if objects is not None:
            relation = self.relations[predicate]
            endpoints = [('subject_ref', claim['subject_ref'], relation['domain_type_ids'])]
            if self.is_temporal(claim):
                if claim['object']['kind'] == 'relative-order':
                    endpoints.append(('relative anchor', claim['object']['relative']['anchor_ref'],
                                      ['tos.entity.historical-situation']))
            elif not self.is_value(claim):
                endpoints.append(('object', claim['object'], relation['range_type_ids']))
            for field, identity, allowed in endpoints:
                record = objects.get(identity)
                kind = self.mappings.get(record['record_type']) if record else None
                if kind is None or not self.ancestry(kind).intersection(allowed):
                    raise SourceProfileError(f'source claim {field} violates registry domain/range or is unresolved')

    def read_rows(self, ref):
        path = Path(ref)
        if (path.is_absolute() or '..' in path.parts or path.as_posix() != ref
                or not path.is_relative_to(SOURCE_ROOT) or path.name != SOURCE_CLAIM_BASENAME
                or any(part in {'catalog', 'payload', 'local-content'} for part in path.parts)):
            raise SourceProfileError('source claim path is outside its metadata home')
        target = self.root / path
        if target.is_symlink() or not target.is_file() or target.resolve() != target.absolute():
            raise SourceProfileError('source claims must be a regular non-symlink file')
        with target.open('rb') as stream:
            raw = stream.read(MAX_CLAIM_FILE_BYTES + 1)
        if len(raw) > MAX_CLAIM_FILE_BYTES:
            raise SourceProfileError('source claim file exceeds 16 MiB')
        for number, line in enumerate(raw.splitlines(), start=1):
            if not line.strip():
                continue
            if len(line) > MAX_RECORD_BYTES:
                raise SourceProfileError('source claim exceeds 1 MiB')
            try:
                claim = json.loads(line, object_pairs_hook=_unique_object, parse_constant=_nonfinite)
                if not isinstance(claim, dict):
                    raise SourceProfileError('source claim must be an object')
                json.dumps(claim, allow_nan=False)
            except (ValueError, UnicodeError) as error:
                raise SourceProfileError('source claim is not strict JSON') from error
            self.validate(claim)
            yield number, claim
