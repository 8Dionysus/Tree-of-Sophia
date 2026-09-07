"""Read adjacent bibliographic forms through the source-owned materializer.

This adapter copies already public metadata. It does not authenticate growth
commands or authorize freeform wording, templates, private text or publication.
"""
from __future__ import annotations

from functools import lru_cache
import json
from pathlib import Path
import sys

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
if str(MECHANIC) not in sys.path:
    sys.path.insert(0, str(MECHANIC))
from human_forms import FormScope, SourceBinding, materialize_form
from knowledge_assessment import Record

MAX_SET_BYTES = 2_097_152
MAX_SET_OUTPUT_BYTES = 262_144


@lru_cache(maxsize=1)
def _validator():
    schemas = [json.loads((ROOT / 'ToS/contracts' / name).read_text()) for name in
               ('knowledge-assessment.schema.json', 'human-form.schema.json', 'human-form-set.schema.json')]
    registry = Registry().with_resources((schema['$id'], Resource.from_contents(schema)) for schema in schemas)
    return Draft202012Validator(schemas[-1], registry=registry)


@lru_cache(maxsize=1)
def _field_language_validator():
    schema = json.loads((ROOT / 'ToS/contracts/corpus-record.schema.json').read_text())
    registry = Registry().with_resource(schema['$id'], Resource.from_contents(schema))
    return Draft202012Validator({'$ref': schema['$id'] + '#/properties/field_languages'}, registry=registry)


def metadata_field_catalog(source: dict) -> list[dict]:
    """Semantic field selectors for this adapter; callers never guess pointers.

    Variant ordinals are snapshot-local, not stable name identities. An exact
    source ref must accompany prepared commands, so reordering is a conflict.
    """
    context = ['/' + key for key in ('identity_status', 'same_as_posture') if key in source]
    declarations = source.get('field_languages', {})
    if not _field_language_validator().is_valid(declarations):
        raise ValueError('source field-language declarations violate the source contract')
    if any(not isinstance(source.get(key), str) or not source[key].strip() for key in declarations):
        raise ValueError('source field-language declaration has no complete wording field')
    result = []
    for key, field_id, role in (('preferred_label', 'metadata.preferred-name', 'name'),
                                ('notes', 'metadata.source-note', 'hover')):
        if isinstance(source.get(key), str) and source[key].strip():
            declaration = declarations.get(key, {})
            result.append({'field_id': field_id, 'pointer': '/' + key, 'role': role,
                           'language': declaration.get('language'), 'script': declaration.get('script'),
                           'context': [*context, *(['/field_languages/' + key] if key in declarations else [])]})
    for index, variant in enumerate(source.get('variant_labels', [])):
        if isinstance(variant, dict) and isinstance(variant.get('value'), str) and variant['value'].strip():
            base = f'/variant_labels/{index}/'
            result.append({'field_id': f'metadata.variant-name:{index}', 'pointer': base + 'value',
                           'role': 'name', 'language': variant.get('language'), 'script': variant.get('script'),
                           'context': [*context, *(base + key.replace('~', '~0').replace('/', '~1')
                                                  for key in variant if key != 'value')]})
    return result


def materialize_metadata_forms(source: dict, form_set: dict, *, access_allowed: bool) -> list[dict]:
    """The caller supplies a current, public-metadata source record.

Only full names and notes are supported here. Unsupported forms remain
explicitly unavailable, not silently rendered under a more permissive role.
"""
    input_bytes = 0
    for chunk in json.JSONEncoder(ensure_ascii=False, allow_nan=False).iterencode(form_set):
        input_bytes += len(chunk.encode('utf-8'))
        if input_bytes > MAX_SET_BYTES:
            raise ValueError('human-form set exceeds input budget')
    if not _validator().is_valid(form_set):
        raise ValueError('human-form set schema is invalid')
    subject = Record.from_payload(source['record_id'], source['record_version'], source)
    if form_set['subject']['id'] != subject.id:
        raise ValueError('human-form set belongs to another source subject')
    stale_subject = form_set['subject'] != subject.ref
    prior = [Record.from_payload(value['form_id'], value['form_version'], value)
             for value in form_set['prior_forms']]
    fields = {(field['role'], field['pointer']): field for field in metadata_field_catalog(source)}
    seen, results, output_bytes = set(), [], 0
    for value in form_set['forms']:
        if value['form_id'] in seen:
            raise ValueError('duplicate current human-form identity')
        seen.add(value['form_id'])
        form = Record.from_payload(value['form_id'], value['form_version'], value)
        stale_form = stale_subject or (value['subject']['id'] == subject.id and value['subject'] != subject.ref)
        content = value['content']
        selected = value['bindings'].get(content.get('slot')) if content['kind'] == 'source-copy' else None
        pointer = selected['pointer'] if selected else None
        required = []
        language, script, supported = None, None, False
        if selected and selected['record'] == subject.ref:
            field = fields.get((value['role'], pointer))
            if field is not None:
                language, script, supported = field['language'], field['script'], True
                required = [SourceBinding(subject, pointer) for pointer in field['context']]
        scope = FormScope(subject, tuple(required), value['creator_id'], 'low',
                          (language,) if isinstance(language, str) else (), 'research',
                          access_allowed=access_allowed,
                          source_languages=((SourceBinding(subject, pointer), language, script),) if supported else ())
        if (stale_form or not supported) and access_allowed is True:
            result = {'schema_version': 'tos_human_form_materialization_v1', 'form': form.ref,
                      'subject': subject.ref, 'state': 'stale' if stale_form else 'unavailable', 'display_text': None,
                      'context': [], 'issues': ['metadata-adapter.subject-changed' if stale_form else
                                                'metadata-adapter.unsupported-role-or-production-mode'],
                      'admission': None, 'performs_semantic_assessment': False}
        else:
            result = materialize_form(ROOT, form, scope, [subject], prior_forms=prior)
        output_bytes += len(json.dumps(result, ensure_ascii=False, separators=(',', ':')).encode())
        if output_bytes > MAX_SET_OUTPUT_BYTES:
            raise ValueError('human-form set exceeds bounded metadata output')
        results.append(result)
    return results


def load_metadata_forms(repo_root: Path, source_ref: str, source: dict, *, access_allowed: bool):
    """Find only the adjacent <record-stem>.human-forms.json, never a submitted path."""
    root = repo_root.resolve()
    source_path = (root / source_ref).resolve()
    source_path.relative_to(root / 'ToS/source-witnesses')
    path = source_path.with_name(source_path.stem + '.human-forms.json')
    if not path.exists():
        return None
    resolved = path.resolve()
    resolved.relative_to(source_path.parent)
    with resolved.open('rb') as handle:
        raw = handle.read(MAX_SET_BYTES + 1)
    if len(raw) > MAX_SET_BYTES:
        raise ValueError('human-form set exceeds input budget')
    payload = json.loads(raw)
    return path.relative_to(root).as_posix(), raw, materialize_metadata_forms(source, payload, access_allowed=access_allowed)
