"""Read adjacent bibliographic forms through the source-owned materializer.

This adapter copies already public metadata. It does not authenticate growth
commands or authorize freeform wording, templates, private text or publication.
"""
from __future__ import annotations

from functools import lru_cache
import json
from pathlib import Path
import re
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
    seen, results, output_bytes = set(), [], 0
    for value in form_set['forms']:
        if value['form_id'] in seen:
            raise ValueError('duplicate current human-form identity')
        seen.add(value['form_id'])
        form = Record.from_payload(value['form_id'], value['form_version'], value)
        content = value['content']
        selected = value['bindings'].get(content.get('slot')) if content['kind'] == 'source-copy' else None
        pointer = selected['pointer'] if selected else None
        required = [SourceBinding(subject, '/' + key) for key in ('identity_status', 'same_as_posture') if key in source]
        language, script, supported = None, None, False
        if selected and selected['record'] == subject.ref:
            if value['role'] == 'name' and pointer == '/preferred_label':
                supported = True
            elif value['role'] == 'hover' and pointer == '/notes':
                supported = True
            elif value['role'] == 'name' and (match := re.fullmatch(r'/variant_labels/(0|[1-9][0-9]*)/value', pointer)):
                index = int(match.group(1))
                variants = source.get('variant_labels', [])
                if index < len(variants) and isinstance(variants[index], dict):
                    variant = variants[index]
                    language, script = variant.get('language'), variant.get('script')
                    required.extend(SourceBinding(subject, f'/variant_labels/{index}/' + key)
                                    for key in variant if key != 'value')
                    supported = True
        scope = FormScope(subject, tuple(required), value['creator_id'], 'low',
                          (language,) if isinstance(language, str) else (), 'research',
                          access_allowed=access_allowed,
                          source_languages=((SourceBinding(subject, pointer), language, script),) if supported else ())
        if (stale_subject or not supported) and access_allowed is True:
            result = {'schema_version': 'tos_human_form_materialization_v1', 'form': form.ref,
                      'subject': subject.ref, 'state': 'stale' if stale_subject else 'unavailable', 'display_text': None,
                      'context': [], 'issues': ['metadata-adapter.subject-changed' if stale_subject else
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
