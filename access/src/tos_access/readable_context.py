"""Finite source-owned presentation of governing context, never assessment.

This companion describes already returned source contexts. It does not change a
HumanForm, classify source truth, translate prose, read files or grant access.
Only an exact owner rule may move a known mechanical field into technical
details. Unclassified values remain source-visible, including null and false.
"""
from __future__ import annotations

import copy
import hashlib
import json
import re
from typing import Any, Callable

REGISTRY_REF = 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
OWNER_REF = 'ToS/doctrine/HUMAN_FORMS.md'
SCHEMA = 'tos_readable_context_v1'
VOCABULARY_SCHEMA = 'tos_context_presentation_v1'
MAX_BYTES = 32_768
MAX_ENTRIES = 256
MAX_CONTEXTS = 64
MAX_POINTER = 2048
_POINTER = re.compile(r'(?:/(?:[^~/]|~[01])*)*')
_HASH = re.compile(r'[a-f0-9]{64}')
_EXACT_HASH = re.compile(r'sha256:[a-f0-9]{64}')
_LANGUAGE = re.compile(r'(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|[iIxX](?:-[A-Za-z0-9]{1,8})+)')
_TARGETS = {'record', 'assertion', 'language-context', 'subject-assessment', 'assessment-snapshot'}
# The owner vocabulary can explain these finite mechanical keys, but cannot
# declare negation, source language, unknown qualifiers or arbitrary prose to
# be technical merely by changing a presentation category.
TECHNICAL_FIELDS = frozenset({'schema_version', 'record_id', 'record_version',
    'claim_id', 'claim_version', 'source_record_digest', 'source_sha256',
    'record_sha256', 'journal_revision', 'owner_snapshot', 'journal_batches'})


class ReadableContextError(ValueError):
    """The supplied presentation or its exact raw bindings cannot be trusted."""


class _OverBudget(Exception):
    pass


def _json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(',', ':'), allow_nan=False).encode('utf-8')


def vocabulary_digest(value: dict) -> str:
    return 'sha256:' + hashlib.sha256(_json(value)).hexdigest()


def presentation_catalog(registry: dict | None) -> dict | None:
    """Publish the exact read-only owner vocabulary, never UI-private wording."""
    registry = registry or {}
    errors = validate_vocabulary(registry)
    if errors:
        raise ReadableContextError('; '.join(errors))
    value = registry.get('context_presentation')
    if value is None:
        return None
    return {'id': value['presentation_id'], 'version': value['presentation_version'],
            'source_ref': REGISTRY_REF, 'digest': vocabulary_digest(value),
            'payload': copy.deepcopy(value)}


def _exact(value: Any) -> bool:
    return (isinstance(value, dict) and set(value) == {'id', 'version', 'digest'}
        and isinstance(value['id'], str) and bool(value['id'])
        and type(value['version']) is int and 1 <= value['version'] <= 9_007_199_254_740_991
        and isinstance(value['digest'], str) and _EXACT_HASH.fullmatch(value['digest']) is not None)


def _pointer(value: Any) -> bool:
    return isinstance(value, str) and len(value) <= MAX_POINTER and _POINTER.fullmatch(value) is not None


def _escape(value: str) -> str:
    return value.replace('~', '~0').replace('/', '~1')


def _at(value: Any, pointer: str) -> Any:
    if not _pointer(pointer):
        raise ReadableContextError('invalid-context-pointer')
    for part in pointer.split('/')[1:]:
        key = part.replace('~1', '/').replace('~0', '~')
        if isinstance(value, list) and re.fullmatch(r'0|[1-9][0-9]*', key):
            index = int(key)
            if index >= len(value):
                raise ReadableContextError('unresolved-context-pointer')
            value = value[index]
        elif isinstance(value, dict) and key in value:
            value = value[key]
        else:
            raise ReadableContextError('unresolved-context-pointer')
    return value


def _labels(value: Any, languages: set[str]) -> bool:
    return (isinstance(value, dict) and set(value) == languages
        and all(isinstance(text, str) and bool(text.strip()) and len(text) <= 1024
                for text in value.values()))


def validate_vocabulary(registry: dict, previous: dict | None = None) -> list[str]:
    """Validate the singleton and its version boundary, not meaning or quality."""
    value = registry.get('context_presentation')
    old = (previous or {}).get('context_presentation')
    if value is None:
        return ['context presentation cannot remove its historical owner contract'] if old is not None else []
    expected = {'schema_version', 'presentation_id', 'presentation_version', 'owner_ref', 'purpose',
        'default_language', 'languages', 'max_output_bytes', 'max_entries', 'record_schema_versions', 'field_rules', 'unclassified'}
    errors = []
    if (not isinstance(value, dict) or set(value) != expected
            or value.get('schema_version') != VOCABULARY_SCHEMA
            or value.get('presentation_id') != 'tos.context-presentation.governing'
            or value.get('owner_ref') != OWNER_REF
            or value.get('purpose') != 'source-context-reading-not-assessment'
            or type(value.get('presentation_version')) is not int or value['presentation_version'] < 1
            or type(value.get('max_output_bytes')) is not int or not 1024 <= value['max_output_bytes'] <= MAX_BYTES
            or type(value.get('max_entries')) is not int or not 1 <= value['max_entries'] <= MAX_ENTRIES):
        return ['context presentation violates its finite owner contract']
    languages = value['languages']
    if (not isinstance(languages, list) or not 1 <= len(languages) <= 8
            or any(not isinstance(language, str) or not _LANGUAGE.fullmatch(language)
                   or language.casefold() in {'default', 'original', 'auto'} for language in languages)
            or len({language.casefold() for language in languages}) != len(languages)
            or value['default_language'] not in languages):
        return ['context presentation has invalid label languages']
    language_set = set(languages)
    schemas = value['record_schema_versions']
    if (not isinstance(schemas, list) or not 1 <= len(schemas) <= 128
            or any(not isinstance(schema, str) or not re.fullmatch(r'tos_[a-z0-9_]+_v[0-9]+', schema) for schema in schemas)
            or len(set(schemas)) != len(schemas)):
        return ['context presentation has invalid source-schema selectors']
    unknown = value['unclassified']
    if (not isinstance(unknown, dict) or set(unknown) != {'label', 'explanation'}
            or not all(_labels(unknown.get(key), language_set) for key in ('label', 'explanation'))):
        return ['context presentation has invalid unclassified explanation']
    rules, seen = value['field_rules'], set()
    if not isinstance(rules, list) or not 1 <= len(rules) <= 128:
        return ['context presentation field rules are not bounded']
    for rule in rules:
        if (not isinstance(rule, dict) or set(rule) != {'field', 'targets', 'category', 'label', 'explanation', 'value_labels'}
                or not isinstance(rule['field'], str) or not re.fullmatch(r'[a-z][a-z0-9_]{0,95}', rule['field'])
                or not isinstance(rule['targets'], list) or not rule['targets']
                or any(target not in _TARGETS for target in rule['targets'])
                or len(set(rule['targets'])) != len(rule['targets'])
                or rule['category'] not in {'governing', 'technical'}
                or not _labels(rule['label'], language_set)
                or (rule['explanation'] is not None and not _labels(rule['explanation'], language_set))
                or (rule['value_labels'] is not None and (not isinstance(rule['value_labels'], dict)
                    or not rule['value_labels'] or len(rule['value_labels']) > 64
                    or any(not isinstance(key, str) or not key or len(key) > 128
                           or not _labels(label, language_set) for key, label in rule['value_labels'].items())))):
            errors.append('context presentation has an invalid field rule')
            continue
        if rule['category'] == 'technical' and rule['field'] not in TECHNICAL_FIELDS:
            errors.append('context presentation cannot hide a nonmechanical field')
        for target in rule['targets']:
            key = target, rule['field']
            if key in seen:
                errors.append('context presentation has ambiguous field rules')
            seen.add(key)
    if isinstance(old, dict):
        if _json(old) != _json(value) and value['presentation_version'] <= old.get('presentation_version', 0):
            errors.append('changed context presentation must increase presentation_version')
        for key in ('schema_version', 'presentation_id', 'owner_ref', 'purpose'):
            if old.get(key) != value[key]:
                errors.append('context presentation cannot repurpose its owner identity')
    return errors


def _language(record: dict | None, key: str) -> tuple[str | None, str | None]:
    """Only explicit source declarations; UI and vocabulary language are unrelated."""
    if not isinstance(record, dict):
        return None, None
    fields = record.get('field_languages')
    declared = fields.get(key) if isinstance(fields, dict) else None
    value = record.get(key)
    if not isinstance(declared, dict) and isinstance(value, dict):
        declared = value
    if not isinstance(declared, dict):
        return None, None
    language, script = declared.get('language'), declared.get('script')
    return (language if isinstance(language, str) and _LANGUAGE.fullmatch(language) else None,
            script if isinstance(script, str) and re.fullmatch(r'[A-Za-z]{4}', script) else None)


class ReadableContextCompiler:
    """One build/update snapshot; no module cache or mutable request state."""

    def __init__(self, registry: dict, *, digest: Callable[[Any], str]):
        violations = validate_vocabulary(registry)
        if violations:
            raise ReadableContextError('; '.join(violations))
        self.registry = {'context_presentation': copy.deepcopy(registry.get('context_presentation'))}
        self.digest = digest
        vocabulary = self.registry['context_presentation']
        self.dependency = copy.deepcopy(vocabulary)
        self.rules = ({(target, rule['field']): rule for rule in vocabulary['field_rules'] for target in rule['targets']}
                      if vocabulary is not None else {})
        self.vocabulary_ref = ({'id': vocabulary['presentation_id'], 'version': vocabulary['presentation_version'],
                               'source_ref': REGISTRY_REF, 'digest': vocabulary_digest(vocabulary)}
                              if vocabulary is not None else None)

    def build(self, item: dict) -> dict | None:
        return build_readable_context(item, self.registry, digest=self.digest, _compiler=self)


def build_readable_context(item: dict, registry: dict, *, digest: Callable[[Any], str],
                           _compiler: ReadableContextCompiler | None = None) -> dict | None:
    """Compile one bounded sidecar from exact existing contexts.

    ``digest`` is the existing normalized-source framing digest, not a new
    record identity algorithm. A referenced assertion retains its original
    context binding; it is not re-labelled as a newly verified source record.
    """
    vocabulary = registry.get('context_presentation')
    if vocabulary is None:
        return None
    if _compiler is None:
        return ReadableContextCompiler(registry, digest=digest).build(item)
    attributes = item.get('attributes') or {}
    if not isinstance(attributes, dict) or not isinstance(item.get('semantics', {}), dict):
        raise ReadableContextError('invalid-context-carrier')
    known_record = attributes.get('source_record') or attributes.get('source_claim')
    contexts = item.get('semantics', {}).get('assertion_contexts', [])
    forms = attributes.get('human_forms', [])
    if contexts == [] and forms == [] and known_record is None:
        return None
    result = {'schema_version': SCHEMA, 'state': 'complete', 'reason': 'all-returned-context-covered',
        'vocabulary': copy.deepcopy(_compiler.vocabulary_ref),
        'contexts': [], 'exact_context_pointers': [], 'exact_materials': [],
        'coverage': {'input_contexts': 0, 'returned_contexts': 0, 'entries': 0, 'unclassified_entries': 0},
        'performs_semantic_assessment': False, 'performs_translation': False}
    rules = _compiler.rules
    count = 0
    materials = {}

    def source_record_reference(record):
        if not isinstance(record, dict):
            raise ReadableContextError('missing-context-source-record')
        identity = next((record[key] for key in ('record_id', 'claim_id', 'artifact_id', 'composite_id') if key in record), None)
        reference = {'id': identity, 'version': record.get('claim_version' if 'claim_id' in record else 'record_version'),
                     'digest': 'sha256:' + hashlib.sha256(_json(record)).hexdigest()}
        if not _exact(reference):
            raise ReadableContextError('invalid-context-source-record')
        if attributes.get('source_sha256') is not None and reference['digest'] != 'sha256:' + str(attributes['source_sha256']):
            raise ReadableContextError('context-source-record-digest-mismatch')
        return reference

    def check_budget():
        if count > vocabulary['max_entries'] or len(_json(result)) > vocabulary['max_output_bytes']:
            raise _OverBudget()

    def add_material(pointer, value):
        """Preserve existing canonical numeric representation across JSON readers.

        This digest hashes the existing canonical JSON bytes, independently of
        the normalized graph's IEEE-754 framing digest. Origins are actual raw
        carrier locations. The same record can occur in several form contexts.
        """
        canonical = _json(value)
        if _json(_at(item, pointer)) != canonical:
            raise ReadableContextError('exact-material-origin-mismatch')
        material_digest = 'sha256:' + hashlib.sha256(canonical).hexdigest()
        material = materials.get(material_digest)
        if material is None:
            if len(materials) >= MAX_ENTRIES:
                raise _OverBudget()
            material = {'digest': material_digest, 'canonical_json': canonical.decode('utf-8'),
                        'origin_pointers': []}
            materials[material_digest] = material
            result['exact_materials'].append(material)
        if pointer not in material['origin_pointers']:
            if len(material['origin_pointers']) >= MAX_ENTRIES:
                raise _OverBudget()
            material['origin_pointers'].append(pointer)
        check_budget()

    def add_context(pointer, form=None):
        if not _pointer(pointer):
            raise ReadableContextError('invalid-context-origin')
        if len(result['exact_context_pointers']) >= MAX_CONTEXTS:
            raise _OverBudget()
        result['exact_context_pointers'].append(pointer)
        context = {'origin_pointer': pointer, 'form': copy.deepcopy(form), 'entries': []}
        result['contexts'].append(context)
        result['coverage']['returned_contexts'] += 1
        return context

    def add_entry(context, key, value, pointer, binding, target, record=None, force_unknown=False):
        nonlocal count
        if not _pointer(pointer) or not isinstance(key, str):
            raise ReadableContextError('invalid-context-value-pointer')
        if _json(_at(item, pointer)) != _json(value):
            raise ReadableContextError('context-value-pointer-mismatch')
        rule = None if force_unknown else rules.get((target, key))
        enum_labels = rule['value_labels'] if rule else None
        unknown_enum = enum_labels is not None and (not isinstance(value, str) or value not in enum_labels)
        category = rule['category'] if rule and not unknown_enum else 'unclassified'
        language, script = _language(record, key)
        entry = {'key': key, 'category': category,
            'label': copy.deepcopy(rule['label'] if rule else vocabulary['unclassified']['label']),
            'explanation': copy.deepcopy(rule['explanation'] if rule and not unknown_enum else vocabulary['unclassified']['explanation']),
            'value_mode': ('exact-reference' if category == 'technical' else
                           'vocabulary-value' if enum_labels is not None and not unknown_enum else 'source-value'),
            'value_label': copy.deepcopy(enum_labels[value]) if enum_labels is not None and not unknown_enum else None,
            'language': language, 'script': script,
            'binding': copy.deepcopy(binding), 'value_pointer': pointer}
        if category != 'technical':
            entry['value'] = copy.deepcopy(value)
        context['entries'].append(entry)
        count += 1
        result['coverage']['entries'] += 1
        if category == 'unclassified':
            result['coverage']['unclassified_entries'] += 1
        check_budget()

    try:
        if attributes.get('source_record') is not None and attributes.get('source_claim') is not None:
            raise ReadableContextError('ambiguous-context-source-record')
        if not isinstance(contexts, list) or not isinstance(forms, list):
            raise ReadableContextError('invalid-context-collection')
        ready_forms = sum(isinstance(packet, dict) and packet.get('state') == 'ready' for packet in forms)
        result['coverage']['input_contexts'] = len(contexts) + ready_forms + int(isinstance(known_record, dict) and not ready_forms)
        if isinstance(known_record, dict):
            key = 'source_record' if attributes.get('source_record') is not None else 'source_claim'
            add_material('/attributes/' + key, known_record)
        # Without a ready form, ordinary metadata reading still needs its exact
        # declared scope. The raw source record is not changed or wrapped into a
        # newly asserted Claim. Unknown top-level source members stay visible.
        if isinstance(known_record, dict) and not any(isinstance(p, dict) and p.get('state') == 'ready' for p in forms):
            key = 'source_record' if attributes.get('source_record') is not None else 'source_claim'
            pointer = '/attributes/' + key
            context = add_context(pointer)
            reference = source_record_reference(known_record)
            for field, value in known_record.items():
                add_entry(context, field, value, pointer + '/' + _escape(field),
                    {'kind': 'record', 'record': reference, 'source_pointer': '/' + _escape(field)},
                    'record', known_record,
                    force_unknown=known_record.get('schema_version') not in vocabulary['record_schema_versions'])
        for index, raw in enumerate(contexts):
            pointer = f'/semantics/assertion_contexts/{index}'
            context = add_context(pointer)
            if (not isinstance(raw, dict) or raw.get('schema_version') != 'tos_assertion_context_v1'
                    or not isinstance(raw.get('source_record_digest'), str)
                    or not _HASH.fullmatch(raw['source_record_digest'])
                    or not isinstance(raw.get('fields'), dict) or not isinstance(raw.get('conflicts'), list)):
                raise ReadableContextError('invalid-assertion-context')
            own = item.get('source_record')
            source = own.get('payload') if isinstance(own, dict) and own.get('digest') == raw['source_record_digest'] else None
            if (raw.get('binding_role') == 'carrier' and isinstance(own, dict)
                    and own.get('digest') != raw['source_record_digest']):
                raise ReadableContextError('assertion-carrier-digest-mismatch')
            if source is not None and digest(source) != raw['source_record_digest']:
                raise ReadableContextError('assertion-source-digest-mismatch')
            add_material(pointer, raw)
            for key, field in raw['fields'].items():
                if not isinstance(field, dict) or set(field) != {'value', 'source_pointer'} or not _pointer(field['source_pointer']):
                    raise ReadableContextError('invalid-assertion-field')
                if source is not None and _json(_at(source, field['source_pointer'])) != _json(field['value']):
                    raise ReadableContextError('assertion-source-pointer-mismatch')
                binding = {'kind': 'assertion-context', 'source_record_digest': raw['source_record_digest'],
                           'source_pointer': field['source_pointer']}
                value_pointer = pointer + '/fields/' + _escape(key) + '/value'
                if key == 'record' and isinstance(field['value'], dict):
                    for name, value in field['value'].items():
                        add_entry(context, name, value, value_pointer + '/' + _escape(name),
                            {**binding, 'source_pointer': field['source_pointer'] + '/' + _escape(name)},
                            'record', field['value'], force_unknown=field['value'].get('schema_version') not in vocabulary['record_schema_versions'])
                else:
                    add_entry(context, key, field['value'], value_pointer, binding, 'assertion', known_record)
            # A conflict is never replaced by its higher-priority convenience.
            if raw['conflicts']:
                add_entry(context, 'conflicts', raw['conflicts'], pointer + '/conflicts',
                    {'kind': 'assertion-context', 'source_record_digest': raw['source_record_digest'],
                     'source_pointer': ''}, 'assertion', force_unknown=True)
        for index, packet in enumerate(forms):
            if not isinstance(packet, dict) or packet.get('schema_version') != 'tos_human_form_materialization_v1':
                raise ReadableContextError('invalid-form-context-packet')
            if packet.get('state') != 'ready':
                # Non-ready contexts never acquire readable wording from this route.
                if packet.get('context') != [] or packet.get('display_text') is not None:
                    raise ReadableContextError('nonready-form-context-has-wording')
                continue
            if not _exact(packet.get('form')) or not _exact(packet.get('subject')) or not isinstance(packet.get('context'), list):
                raise ReadableContextError('invalid-form-context-binding')
            packet_pointer = f'/attributes/human_forms/{index}'
            pointer = packet_pointer + '/context'
            context = add_context(pointer, packet['form'])
            records = {}
            if isinstance(known_record, dict):
                reference = source_record_reference(known_record)
                if reference != packet['subject']:
                    raise ReadableContextError('form-source-version-or-digest-mismatch')
                records[_json(reference)] = known_record
            for ordinal, entry in enumerate(packet['context']):
                binding = entry.get('binding') if isinstance(entry, dict) else None
                if (not isinstance(binding, dict) or set(binding) != {'record', 'pointer'}
                        or not _exact(binding['record']) or not _pointer(binding['pointer']) or 'value' not in entry):
                    raise ReadableContextError('invalid-form-source-binding')
                if binding['pointer'] == '':
                    if 'sha256:' + hashlib.sha256(_json(entry['value'])).hexdigest() != binding['record']['digest']:
                        raise ReadableContextError('form-context-record-digest-mismatch')
                    value = entry['value']
                    if not isinstance(value, dict):
                        raise ReadableContextError('form-context-root-is-not-record')
                    identity = next((value[key] for key in ('record_id', 'claim_id', 'form_id', 'artifact_id', 'composite_id') if key in value), None)
                    version = value.get('claim_version' if 'claim_id' in value else 'form_version' if 'form_id' in value else 'record_version')
                    if identity != binding['record']['id'] or type(version) is not int or version != binding['record']['version']:
                        raise ReadableContextError('form-context-record-version-mismatch')
                    records[_json(binding['record'])] = entry['value']
                    add_material(pointer + '/' + str(ordinal) + '/value', entry['value'])
            for ordinal, entry in enumerate(packet['context']):
                binding = entry['binding']
                record = records.get(_json(binding['record']))
                if record is None or _json(_at(record, binding['pointer'])) != _json(entry['value']):
                    raise ReadableContextError('form-context-pointer-or-version-mismatch')
                value_pointer = pointer + '/' + str(ordinal) + '/value'
                source_binding = {'kind': 'record', 'record': binding['record'], 'source_pointer': binding['pointer']}
                if binding['pointer'] == '' and isinstance(entry['value'], dict):
                    for key, value in entry['value'].items():
                        add_entry(context, key, value, value_pointer + '/' + _escape(key),
                            {**source_binding, 'source_pointer': '/' + _escape(key)}, 'record', record,
                            force_unknown=record.get('schema_version') not in vocabulary['record_schema_versions'])
                else:
                    key = binding['pointer'].rsplit('/', 1)[-1].replace('~1', '/').replace('~0', '~')
                    # Only direct source fields use record rules. A nested name
                    # such as qualifiers/schema_version is not mechanical.
                    direct = binding['pointer'].count('/') == 1
                    add_entry(context, key or entry.get('slot', ''), entry['value'], value_pointer,
                        source_binding, 'record', record,
                        force_unknown=not direct or record.get('schema_version') not in vocabulary['record_schema_versions'])
            # Parent admission, withdrawal limits and linguistic derivation are
            # mandatory materialization context too, not just context[] cells.
            for key in ('subject_assessment', 'assessment_snapshot', 'language_context'):
                if key not in packet:
                    continue
                add_material(packet_pointer + '/' + key, packet[key])
                add_entry(context, key, packet[key], packet_pointer + '/' + key,
                    {'kind': 'form-materialization', 'form': packet['form'], 'subject': packet['subject'],
                     'packet_digest': digest(packet), 'source_pointer': '/' + key}, 'assertion')
        check_budget()
    except _OverBudget:
        result['state'], result['reason'] = 'requires-exact-context', 'context-presentation-budget'
        result['contexts'] = []
        result['exact_materials'] = []
        result['coverage'].update(returned_contexts=0, entries=0, unclassified_entries=0)
        # One bounded exact-root reference covers the entire remainder, even if
        # enumerating each origin itself exhausted the budget. No partial ready.
        result['exact_context_pointers'] = [pointer for pointer, present in (
            ('/semantics/assertion_contexts', bool(contexts)), ('/attributes/human_forms', bool(forms)),
            ('/attributes/source_record', attributes.get('source_record') is not None),
            ('/attributes/source_claim', attributes.get('source_claim') is not None)) if present] or ['/semantics']
    except (ReadableContextError, TypeError, KeyError, ValueError, RecursionError) as error:
        result['state'], result['reason'] = 'unavailable', str(error)[:128]
        result['contexts'] = []
        result['exact_materials'] = []
        result['coverage'].update(returned_contexts=0, entries=0, unclassified_entries=0)
        result['exact_context_pointers'] = [pointer for pointer, present in (
            ('/semantics/assertion_contexts', bool(contexts)), ('/attributes/human_forms', bool(forms)),
            ('/attributes/source_record', attributes.get('source_record') is not None),
            ('/attributes/source_claim', attributes.get('source_claim') is not None)) if present] or ['/semantics']
    return result


def validate_sidecar(item: dict, registry: dict, *, digest: Callable[[Any], str]) -> None:
    """Refuse a supplied sidecar which does not match the exact current input."""
    expected = build_readable_context(item, registry, digest=digest)
    if _json(item.get('readable_context')) != _json(expected):
        raise ReadableContextError('readable-context-sidecar-binding-mismatch')
