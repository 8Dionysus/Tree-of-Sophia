"""Execute declared source-metadata profiles from the existing type registry.

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
RESERVED_KINDS = {'agent', 'place', 'organization', 'work', 'expression', 'edition',
                  'collection', 'item', 'link', 'artifact'}
RESERVED_BASENAMES = {kind + '.json' for kind in RESERVED_KINDS} | {'artifact-witness.json'}
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
            if (entry.get('abstract') is not False or entry.get('object_role') != 'identity'
                    or kind in RESERVED_KINDS or profile['source_basename'] in RESERVED_BASENAMES
                    or profile['catalog_filename'] in RESERVED_CATALOGS
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
            refs = [CORPUS_REF, *route['schema_dependencies'], route['schema_ref']]
            for ref in dict.fromkeys(refs):
                if ref not in self.schemas:
                    schema = _read_json(self.root, ref, self.input_digests)
                    if schema.get('$id') not in {'https://tree-of-sophia.local/' + ref,
                                                  'https://treeofsophia.local/' + ref}:
                        raise SourceProfileError(f'{kind}: source schema identity differs from its declared owner path')
                    self.schemas[ref] = schema
            schemas = [self.schemas[ref] for ref in dict.fromkeys(refs)]
            resources = {}
            for schema in schemas:
                Draft202012Validator.check_schema(schema)
                if schema['$id'] in resources:
                    raise SourceProfileError(f'{kind}: duplicate schema resource identity')
                resources[schema['$id']] = Resource.from_contents(schema)
            registry = Registry().with_resources(resources.items())
            self.validators[key] = Draft202012Validator(self.schemas[route['schema_ref']], registry=registry,
                                                         format_checker=FormatChecker())
            fields = ('preferred_label', 'variant_labels', 'field_languages', 'identity_status',
                      'source_refs', 'external_identifiers', 'same_as_posture', 'record_version', 'notes')
            common = {'type': 'object',
                      'required': ['preferred_label', 'identity_status', 'source_refs',
                                   'external_identifiers', 'same_as_posture', 'record_version'],
                      'properties': {field: {'$ref': schemas[0]['$id'] + '#/properties/' + field}
                                     for field in fields}}
            self.metadata_validators[key] = Draft202012Validator(common, registry=registry,
                                                                 format_checker=FormatChecker())
        return self.validators[key]

    def load(self, kind: str, ref: str) -> dict:
        path = Path(ref)
        if (path.is_absolute() or '..' in path.parts or path.as_posix() != ref
                or not path.is_relative_to(SOURCE_ROOT) or 'catalog' in path.parts
                or any(part in {'payload', 'local-content'} for part in path.parts)
                or path.name != self.profiles[kind]['source_basename']):
            raise SourceProfileError('source-record profile path is outside its metadata home')
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
