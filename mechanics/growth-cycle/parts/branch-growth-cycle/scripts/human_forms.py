"""Materialize exact-bound human forms, without authoring or executing prose.

Source adapters supply the current subject, mandatory context, access decision,
trusted templates and authenticated assessment inputs separately from a form.
The result is disposable; a ready form is not an admitted historical assertion.
"""
from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
import json
from pathlib import Path
from typing import Any, Iterable, Sequence

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from knowledge_assessment import AssessmentEngine, Record, RequiredAdmission, SubjectContext, Submission


MAX_OUTPUT_BYTES = 65_536
MAX_INPUT_BYTES = 8_388_608
MAX_SOURCE_RECORDS = 512
MAX_TEMPLATES = 64
MAX_PRIOR_FORMS = 256


def compile_source_form_validators(schemas):
    """Compile one caller-verified source grammar; perform no IO or caching.

    Protected owner adapters choose and hash the same four schema bodies for
    source-copy history and materialization. A runtime-global cached renderer
    cannot override a different context's freshly selected grammar.
    """
    names = ('knowledge-assessment', 'human-form', 'human-form-set', 'human-form-template')
    selected = {name: schemas[name] for name in names}
    for name, schema in selected.items():
        if schema.get('$id') != 'https://treeofsophia.local/ToS/contracts/' + name + '.schema.json':
            raise ValueError('source form grammar has another schema identity')
        Draft202012Validator.check_schema(schema)
    registry = Registry().with_resources((schema['$id'], Resource.from_contents(schema))
                                         for schema in selected.values())
    form = selected['human-form']
    return (Draft202012Validator(selected['human-form-set'], registry=registry),
            (Draft202012Validator(form, registry=registry),
             Draft202012Validator(selected['human-form-template'], registry=registry),
             Draft202012Validator({'$ref': form['$id'] + '#/$defs/languageContext'}, registry=registry)))


@dataclass(frozen=True)
class SourceBinding:
    record: Record
    pointer: str

    @property
    def ref(self):
        return {"record": self.record.ref, "pointer": self.pointer}


@dataclass(frozen=True)
class FormScope:
    subject: Record
    required_context: tuple[SourceBinding, ...]
    maker_id: str
    risk: str
    languages: tuple[str, ...]
    requested_use: str
    access_allowed: bool = False
    source_languages: tuple[tuple[SourceBinding, str | None, str | None], ...] = ()
    language_context: SourceBinding | None = None
    required_sources: tuple[Record, ...] = ()
    required_admissions: tuple[RequiredAdmission, ...] = ()


@lru_cache(maxsize=8)
def _validators(root: Path):
    schemas = [json.loads((root / 'ToS/contracts' / name).read_text()) for name in
               ('knowledge-assessment.schema.json', 'human-form.schema.json', 'human-form-template.schema.json')]
    for schema in schemas:
        Draft202012Validator.check_schema(schema)
    registry = Registry().with_resources((schema['$id'], Resource.from_contents(schema)) for schema in schemas)
    return (*tuple(Draft202012Validator(schema, registry=registry) for schema in schemas[1:]),
            Draft202012Validator({'$ref': schemas[1]['$id'] + '#/$defs/languageContext'}, registry=registry))


def _canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(',', ':'))


def _index(records: Iterable[Record]):
    indexed = {}
    for record in records:
        if record.id in indexed and indexed[record.id].ref != record.ref:
            raise ValueError('conflicting current form input records')
        indexed[record.id] = record
    return indexed


def _pointer(payload, pointer: str):
    """JSON Pointer only. No attributes, expressions, wildcards or tool calls."""
    value = payload
    for token in pointer.split('/')[1:] if pointer else ():
        token = token.replace('~1', '/').replace('~0', '~')
        if isinstance(value, dict):
            value = value[token]
        elif isinstance(value, list) and token.isascii() and token.isdigit() and (token == '0' or not token.startswith('0')):
            value = value[int(token)]
        else:
            raise KeyError(pointer)
    return value


def materialize_form(root: Path, form: Record, scope: FormScope, records: Sequence[Record], *,
                     templates: Sequence[Record] = (), engine: AssessmentEngine | None = None,
                     prior_forms: Sequence[Record] = (),
                     reviews: Sequence[Submission] = (), trusted_history: Sequence[Submission] = (),
                     now: str | None = None, validators=None) -> dict[str, Any]:
    """Bounded pure rendering with explicit stale/missing/assessment states.

Trusted templates are admitted by their source owner before this call, not by
including a template in the submitted form. Freeform needs a fresh assessment
policy result bound to this exact form and current dependency snapshot.
"""
    base = {'schema_version': 'tos_human_form_materialization_v1', 'form': form.ref,
              'subject': scope.subject.ref, 'state': 'invalid', 'display_text': None,
              'context': [], 'issues': [], 'admission': None, 'performs_semantic_assessment': False}
    result = {**base, 'context': []}

    def stop(state, issue):
        packet = {**base, 'state': state, 'issues': [issue], 'admission': result['admission']}
        if len(_canonical(packet).encode('utf-8')) > MAX_OUTPUT_BYTES:
            packet.update(state='over-budget', admission=None,
                          issues=['form.output-budget-exceeded-use-separate-assessment-inspection'])
        if len(_canonical(packet).encode('utf-8')) > MAX_OUTPUT_BYTES:
            raise ValueError('form identity exceeds output budget')
        return packet

    if scope.access_allowed is not True:
        return stop('restricted', 'access.not-authorized')
    if (not isinstance(scope.required_sources, tuple)
            or any(not isinstance(item, Record) or item.id == form.id for item in scope.required_sources)):
        return stop('invalid', 'form.required-source-scope')
    if (not isinstance(scope.required_admissions, tuple) or len(scope.required_admissions) > 64
            or any(not isinstance(item, RequiredAdmission) or not isinstance(item.basis, Record)
                   or type(item.can_use) is not bool for item in scope.required_admissions)):
        return stop('invalid', 'form.required-admission-scope')
    if any(not item.can_use for item in scope.required_admissions):
        return stop('needs-assessment', 'source-quality.not-admitted')
    if (len(records) > MAX_SOURCE_RECORDS or len(templates) > MAX_TEMPLATES
            or len(prior_forms) > MAX_PRIOR_FORMS or len(scope.required_context) > 256
            or len(scope.source_languages) > 256 or len(scope.required_sources) > 256
            or form.size_bytes + sum(record.size_bytes for record in (*records, *templates, *prior_forms)) > MAX_INPUT_BYTES):
        return stop('over-budget', 'form.input-budget-exceeded-narrow-snapshot')
    form_validator, template_validator, language_context_validator = (
        validators if validators is not None else _validators(root))
    payload = form.payload
    if not form_validator.is_valid(payload):
        return stop('invalid', 'form.schema')
    if (payload['form_id'] != form.id or payload['form_version'] != form.version
            or payload['subject'] != scope.subject.ref or form.id == scope.subject.id
            or payload['creator_id'] != scope.maker_id):
        return stop('invalid', 'form.identity-or-subject')
    if payload['language'] is not None and payload['language'].casefold() not in {language.casefold() for language in scope.languages}:
        return stop('invalid', 'form.language-outside-scope')
    if payload.get('language_context') != (scope.language_context.ref if scope.language_context else None):
        return stop('invalid', 'language-context.outside-owner-scope')
    if form.version == 1:
        if payload['revises'] is not None:
            return stop('invalid', 'form.initial-version-has-predecessor')
    else:
        previous = next((record for record in prior_forms if record.ref == payload['revises']), None)
        if previous is None:
            return stop('unavailable', 'form.predecessor-unavailable')
        old = previous.payload
        if (not form_validator.is_valid(old) or previous.id != form.id or previous.version != form.version - 1
                or old['form_id'] != previous.id or old['form_version'] != previous.version
                or old['subject']['id'] != payload['subject']['id']):
            return stop('invalid', 'form.incompatible-identity-reuse')
    current = _index(records)
    if scope.subject.id not in current:
        return stop('unavailable', 'subject.unavailable')
    if current[scope.subject.id].ref != scope.subject.ref:
        return stop('stale', 'subject.changed')
    values = {}
    source_payloads = {}
    dependencies = [scope.subject.ref]
    for dependency in _index((*scope.required_sources, *(item.basis for item in scope.required_admissions))).values():
        if dependency.id not in current:
            return stop('unavailable', 'required-source.unavailable')
        if current[dependency.id].ref != dependency.ref:
            return stop('stale', 'required-source.changed')
        dependencies.append(dependency.ref)
    for slot, binding in payload['bindings'].items():
        source = current.get(binding['record']['id'])
        if source is None:
            return stop('unavailable', 'binding.unavailable:' + slot)
        if source.ref != binding['record']:
            return stop('stale', 'binding.changed:' + slot)
        try:
            if source.id not in source_payloads:
                source_payloads[source.id] = source.payload
            values[slot] = _pointer(source_payloads[source.id], binding['pointer'])
        except (KeyError, IndexError):
            return stop('invalid', 'binding.pointer:' + slot)
        dependencies.append(source.ref)
    binding_keys = {_canonical(binding): slot for slot, binding in payload['bindings'].items()}
    required_context = [*scope.required_context,
                        *(SourceBinding(item.basis, '') for item in scope.required_admissions)]
    if scope.language_context is not None:
        metadata_slot = binding_keys.get(_canonical(scope.language_context.ref))
        if metadata_slot is None:
            return stop('invalid', 'context.omitted')
        metadata = values[metadata_slot]
        if not language_context_validator.is_valid(metadata):
            return stop('invalid', 'language-context.schema')
        if (metadata['language'], metadata['script']) != (payload['language'], payload['script']):
            return stop('invalid', 'language-context.language-or-script-mismatch')
        required_context.append(scope.language_context)
        if metadata['source'] is not None:
            source = metadata['source']
            if (source['record']['id'] == form.id or
                    (payload['content']['kind'] == 'source-copy' and
                     source == payload['bindings'].get(payload['content']['slot']))):
                return stop('invalid', 'language-context.self-derivation')
            source_slot = binding_keys.get(_canonical(source))
            if source_slot is None:
                return stop('invalid', 'context.omitted')
            original = values[source_slot]
            if not isinstance(original, str) or not original.strip():
                return stop('invalid', 'language-context.source-requires-complete-nonempty-string')
            required_context.append(SourceBinding(current[source['record']['id']], source['pointer']))
        result['language_context'] = {'binding': scope.language_context.ref, 'value': metadata}
    if len(required_context) > 256:
        return stop('over-budget', 'form.input-budget-exceeded-narrow-snapshot')
    context_bytes = 0
    for required in required_context:
        slot = binding_keys.get(_canonical(required.ref))
        if slot is None:
            return stop('invalid', 'context.omitted')
        entry = {'slot': slot, 'binding': required.ref, 'value': values[slot]}
        context_bytes += len(_canonical(entry).encode('utf-8'))
        if context_bytes > MAX_OUTPUT_BYTES:
            return stop('over-budget', 'form.output-budget-exceeded-do-not-truncate')
        result['context'].append(entry)
    content = payload['content']
    if content['kind'] == 'source-copy':
        wording = values.get(content['slot'])
        if not isinstance(wording, str) or not wording.strip():
            return stop('invalid', 'source-copy.requires-complete-nonempty-string')
        selected = payload['bindings'][content['slot']]
        languages = {(language, script) for binding, language, script in scope.source_languages if binding.ref == selected}
        if len(languages) > 1:
            return stop('invalid', 'source-copy.conflicting-language-metadata')
        actual = next(iter(languages), (None, None))
        if (payload['language'], payload['script']) != actual:
            return stop('invalid', 'source-copy.language-not-bound-to-source')
        # The whole selected field is copied; this operation cannot crop a
        # negation or choose an unreviewed substring from a sentence.
    elif content['kind'] == 'template':
        template = _index(templates).get(content['template']['id'])
        if template is None:
            return stop('unavailable', 'template.not-admitted-by-owner')
        if template.ref != content['template']:
            return stop('stale', 'template.changed')
        body = template.payload
        if (not template_validator.is_valid(body) or body['template_id'] != template.id
                or body['template_version'] != template.version
                or body['language'] != payload['language'] or body['script'] != payload['script']
                or payload['role'] not in body['roles']):
            return stop('invalid', 'template.contract')
        parts, used, wording_bytes = [], set(), 0
        for segment in body['segments']:
            if 'literal' in segment:
                part = segment['literal']
            else:
                slot = segment['slot']
                if slot not in values:
                    return stop('invalid', 'template.unbound-slot:' + slot)
                used.add(slot)
                value = values[slot]
                if segment['format'] == 'text' and not isinstance(value, str):
                    return stop('invalid', 'template.text-requires-string:' + slot)
                part = value if segment['format'] == 'text' else _canonical(value)
            wording_bytes += len(part.encode('utf-8'))
            if wording_bytes + context_bytes > MAX_OUTPUT_BYTES:
                return stop('over-budget', 'form.output-budget-exceeded-do-not-truncate')
            parts.append(part)
        used_bindings = {_canonical(payload['bindings'][slot]) for slot in used}
        # Semantic guards must be rendered. Linguistic provenance is retained
        # in the output context, not pasted as JSON into the human wording.
        if any(_canonical(required.ref) not in used_bindings for required in scope.required_context):
            return stop('invalid', 'template.context-not-rendered')
        wording = ''.join(parts)
        dependencies.append(template.ref)
    else:
        if engine is None or now is None:
            return stop('needs-assessment', 'freeform.current-assessment-required')
        # A historical receipt or an engine observing another source snapshot
        # must not authorize a revised form or stale source wording.
        for dependency in [form.ref, *dependencies]:
            record = engine.records.get(dependency['id'])
            if record is None or record.ref != dependency:
                return stop('stale', 'assessment.snapshot-differs')
        context = SubjectContext(form, 'human_projection', scope.risk, scope.languages,
                                 scope.maker_id, scope.requested_use, access_allowed=True,
                                 required_sources=scope.required_sources,
                                 required_admissions=scope.required_admissions)
        result['admission'] = engine.evaluate(context, reviews, now=now, trusted_history=trusted_history)
        if not result['admission']['can_use']:
            return stop('needs-assessment', 'freeform.not-admitted')
        wording = content['text']
    if not wording.strip():
        return stop('invalid', 'form.empty')
    # Field bindings/context remain separate and exact. Dependencies identify
    # records, not uses of those records: repetition must not consume delivery
    # budget or imply independent support. Different IDs/versions/digests stay.
    dependencies = list({_canonical(ref): ref for ref in dependencies}.values())
    result.update(state='ready', display_text=wording, role=payload['role'], language=payload['language'],
                  script=payload['script'], derivation=content['kind'], dependencies=dependencies,
                  standalone_reading=not result['context'])
    if len(_canonical(result).encode('utf-8')) > MAX_OUTPUT_BYTES:
        return stop('over-budget', 'form.output-budget-exceeded-do-not-truncate')
    return result
