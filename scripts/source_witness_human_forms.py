"""Read adjacent metadata and declared Claim forms through the source materializer.

The default adapter copies already public metadata. Explicit local snapshots
can read protected source/journal inputs for current source-form policy admission.
Neither route performs substantive assessment or authorizes public release.
"""
from __future__ import annotations

from functools import lru_cache
import copy
import hashlib
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


class AssessedFormSnapshot:
    """Explicit local research input, never an implicit public-export source.

    The caller independently selects a protected assessment configuration and
    a bounded set of form IDs. Existing graph packets identify the exact
    source/form versions; they cannot provide grants or ready wording. A
    double collection checks observed source/configuration and journal drift.
    This captures committed heads, not a live runtime permission or a new
    cross-subject journal transaction. The source owner must keep its inputs
    stable for assembly, as for the underlying source-bound command.
    """

    def __init__(self, owner_config: Path, form_ids: list[str]):
        if (not isinstance(form_ids, list) or not 1 <= len(form_ids) <= 256
                or any(not isinstance(value, str) or not re.fullmatch(r'tos\.form\.[a-z0-9]+(?:[.-][a-z0-9]+)*', value)
                       for value in form_ids) or len(set(form_ids)) != len(form_ids)):
            raise ValueError('assessed graph input requires 1..256 distinct form IDs')
        self.owner_config = Path(owner_config)
        self.form_ids = frozenset(form_ids)
        self._snapshot = None
        self._observed = {}

    def _resolve(self, subject, form_ref, source_path, form_path):
        from assessment_journal import JournalConflict, run_public_source_command
        identity = form_ref['id']
        described = run_public_source_command(self.owner_config, {
            'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe', 'subject_id': identity})
        context = described['result']['command_context']
        snapshot = described['owner_snapshot']
        if self._snapshot is not None and snapshot != self._snapshot:
            raise JournalConflict('assessment graph inputs changed during assembly')
        selected = {row['record']['id']: row for row in context.get('source_records', [])}
        if (context['subject'] != form_ref or 'materialize-form' not in context['supported_operations']
                or selected.get(identity, {}).get('path') != form_path
                or selected.get(subject.id, {}).get('path') != source_path
                or selected.get(subject.id, {}).get('record') != subject.ref):
            raise JournalConflict('graph and assessment owner bind different source/form inputs')
        request = {'schema_version': 'tos_local_assessment_command_v1', 'operation': 'materialize-form',
                   'subject_id': identity, 'expected_subject': form_ref, 'expected_snapshot': snapshot}
        reply = run_public_source_command(self.owner_config, request)
        result = reply['result']
        packet = result['materialization']
        if packet['subject'] != subject.ref or packet['form'] != form_ref:
            raise JournalConflict('materialized form does not belong to the graph source')
        prior = self._observed.get(identity)
        if prior is not None and (prior['request'] != request or prior['reply'] != reply):
            raise JournalConflict('assessment graph form changed during assembly')
        self._snapshot = snapshot
        self._observed[identity] = {'request': request, 'reply': copy.deepcopy(reply)}
        return {**copy.deepcopy(packet), 'assessment_snapshot': {
            'owner_snapshot': snapshot, 'journal_revision': result['revision'],
            'journal_batches': result['batch_count'], 'publication_authorized': False,
            'current_runtime_grant': False,
            **({'subject_assessment_required': True} if 'subject_assessment' in packet else {})}}

    def verify_current(self):
        """Fail on observed change; do not silently rebuild only part of a graph."""
        from assessment_journal import JournalConflict, run_public_source_command
        if set(self._observed) != self.form_ids:
            raise ValueError('selected assessed forms are not all present in the graph')
        for identity in sorted(self._observed):
            observed = self._observed[identity]
            current = run_public_source_command(self.owner_config, observed['request'])
            if current != observed['reply']:
                raise JournalConflict('assessment graph snapshot changed before return')

    def materialize(self, nodes: list[dict]) -> list[dict]:
        """Replace selected forms on existing source carriers, without mutation.

        Both bibliographic and corpus-navigation carriers keep full source
        records in properties. No unrelated node, edge or source is created.
        The result is local research material: assessment limits and explicit
        source context have not received a separate public-safety clearance.
        """
        from assessment_journal import JournalConflict
        output, seen = [], set()
        for node in nodes:
            properties = node.get('properties', {})
            packets = properties.get('human_forms', [])
            selected = [packet for packet in packets if packet.get('form', {}).get('id') in self.form_ids]
            if not selected:
                output.append(node)
                continue
            source, claim = properties.get('source_record'), properties.get('source_claim')
            if (source is None) == (claim is None):
                raise ValueError('assessed forms require one exact source carrier')
            subject = (metadata_subject(source) if source is not None
                       else Record.from_payload(claim['claim_id'], claim['claim_version'], claim))
            digest = node.get('source_sha256', properties.get('source_sha256'))
            if digest != subject.ref['digest'].removeprefix('sha256:'):
                raise JournalConflict('assessed graph source body and digest disagree')
            replacement = copy.deepcopy(node)
            for index, packet in enumerate(packets):
                identity = packet.get('form', {}).get('id')
                if identity not in self.form_ids:
                    continue
                if identity in seen or packet.get('subject') != subject.ref:
                    raise ValueError('selected form must have one exact source carrier')
                seen.add(identity)
                replacement['properties']['human_forms'][index] = self._resolve(
                    subject, packet['form'], node.get('source_ref'), properties.get('human_forms_source_ref'))
                if len(json.dumps(replacement['properties']['human_forms'][index], ensure_ascii=False,
                                  separators=(',', ':')).encode()) > 65_536:
                    raise ValueError('assessed form with snapshot binding exceeds its output budget')
            if len(json.dumps(replacement['properties']['human_forms'], ensure_ascii=False,
                              separators=(',', ':')).encode()) > MAX_SET_OUTPUT_BYTES:
                raise ValueError('assessed form set exceeds its output budget')
            output.append(replacement)
        if seen != self.form_ids:
            raise ValueError('selected assessed forms are not all present in the graph')
        self.verify_current()
        return output


def add_assessed_build_arguments(parser):
    """The two existing builders share one explicit, local-only CLI seam."""
    parser.add_argument('--assessment-owner-config', type=Path,
                        help='independently selected protected configuration for a local research candidate')
    parser.add_argument('--assessed-form-id', action='append', default=[],
                        help='exact source form ID to materialize; repeat for a bounded selection')
    parser.add_argument('--output', type=Path,
                        help='new local candidate JSON, outside source surfaces; required with assessment input')


def assessed_build_input(args, repo_root, standard_path):
    if args.assessment_owner_config is None:
        if args.assessed_form_id or args.output is not None:
            raise ValueError('assessed form/output selection requires --assessment-owner-config')
        return None, standard_path
    if args.output is None:
        raise ValueError('assessed build requires a separate --output; standard public export is not a target')
    target, root = args.output.resolve(), repo_root.resolve()
    if (target.suffix != '.json' or target == standard_path.resolve()
            or (target.is_relative_to(root) and not target.is_relative_to(root / '.git'))):
        raise ValueError('local assessed output must be JSON outside repository sources (or within .git)')
    return AssessedFormSnapshot(args.assessment_owner_config, args.assessed_form_id), target


def write_assessed_candidate(target, rendered, snapshot):
    """Publish one complete local candidate without replacing any existing file.

    This filesystem publication is not a public release or artifact admission.
    A final currentness check follows serialization and precedes file creation.
    """
    import os
    import tempfile
    snapshot.verify_current()
    target.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix='.tos-assessed-', dir=target.parent)
    try:
        with os.fdopen(descriptor, 'w', encoding='utf-8') as stream:
            stream.write(rendered)
            stream.flush()
            os.fsync(stream.fileno())
        os.link(temporary, target)  # Atomic complete visibility; never overwrite.
        directory = os.open(target.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        os.unlink(temporary)  # Only the exclusive staging file created above.

NATIVE_IDENTITIES = {
    'tos_scholarly_composite_witness_v1': 'composite_id',
    'tos_artifact_source_witness_v1': 'artifact_id',
    'tos_artifact_source_witness_v2': 'artifact_id',
}


def metadata_subject(source: dict) -> Record:
    """Bind the unchanged validated payload using its actual identity field."""
    identity = NATIVE_IDENTITIES.get(source.get('schema_version'), 'record_id')
    return Record.from_payload(source[identity], source['record_version'], source)


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
    native = NATIVE_IDENTITIES.get(source.get('schema_version'))
    if native is not None:
        # These original schemas declare no field language. Do not infer one
        # from a provider, script, territory or the reader's interface locale.
        pointers = (('/preferred_label', '/editorial_object/description') if native == 'composite_id'
                    else ('/custody/inventory_numbers/0', '/path_identity/note'))
        name_context = ['/identity_status' if native == 'composite_id' else '/custody',
                        '/layer_separation', '/authority', '/rights_ref']
        return [{'field_id': field_id, 'pointer': pointer, 'role': role,
                 'language': None, 'script': None, 'context': name_context if role == 'name' else ['']}
                for pointer, field_id, role in zip(pointers,
                    ('metadata.preferred-name', 'metadata.source-note'), ('name', 'hover'))]
    context = ['/' + key for key in ('identity_status', 'same_as_posture', 'semantic_scope', 'semantic_content',
                                   'form_identity', 'native_text_binding') if key in source]
    if source.get('record_type') == 'sign':
        # Birth evidence and limits travel with every source-copy wording.
        # They never supply this HumanForm's current semantic admission.
        context.append('/promotion_basis')
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
    subject = metadata_subject(source)
    if source.get('schema_version') in NATIVE_IDENTITIES:
        access_allowed = access_allowed is True and source.get('authority', {}).get('visibility') in {'public', 'public_metadata_only'}
    elif 'visibility' in source:
        access_allowed = access_allowed is True and source['visibility'] in {'public', 'public_metadata_only'}
    return _materialize_forms(subject, metadata_field_catalog(source), form_set, access_allowed=access_allowed)


def claim_field_catalog(source: dict) -> list[dict]:
    """Only the complete declared statement; the entire Claim guards its reading.

    A source profile validates the Claim before calling this adapter. No label,
    endpoint name, predicate or assessment is synthesized into a statement.
    """
    qualifiers = source.get('qualifiers') or {}
    statement = qualifiers.get('statement')
    if not isinstance(statement, str) or not statement.strip():
        return []
    language, script = qualifiers.get('statement_language'), qualifiers.get('statement_script')
    if not _field_language_validator().is_valid({'notes': {'language': language, 'script': script}}):
        raise ValueError('claim statement language/script violates the source-form contract')
    return [{'field_id': 'claim.statement', 'pointer': '/qualifiers/statement', 'role': 'statement',
             'language': language, 'script': script, 'context': ['']}]


def materialize_claim_forms(source: dict, form_set: dict, *, access_allowed: bool) -> list[dict]:
    """Render already validated public Claim metadata, never an admission."""
    subject = Record.from_payload(source['claim_id'], source['claim_version'], source)
    return _materialize_forms(subject, claim_field_catalog(source), form_set,
                              access_allowed=access_allowed is True and
                              source.get('visibility') in {'public', 'public_metadata_only'})


def source_copy_field(subject: Record, form: dict, field_catalog: list[dict]) -> dict | None:
    """Resolve one whole copied field by the source owner's actual catalogue.

    Shared by source-only readiness and the explicit assessed consumer. A form
    cannot grant an arbitrary pointer, role or language its own source scope.
    """
    content = form['content']
    selected = form['bindings'].get(content.get('slot')) if content['kind'] == 'source-copy' else None
    if selected is None or selected['record'] != subject.ref:
        return None
    matches = [field for field in field_catalog
               if (field['role'], field['pointer']) == (form['role'], selected['pointer'])]
    if len(matches) > 1:
        raise ValueError('source-copy field catalogue is ambiguous')
    return matches[0] if matches else None


def _materialize_forms(subject: Record, field_catalog: list[dict], form_set: dict, *, access_allowed: bool,
                       validator=None, materializer_validators=None):
    input_bytes = 0
    for chunk in json.JSONEncoder(ensure_ascii=False, allow_nan=False).iterencode(form_set):
        input_bytes += len(chunk.encode('utf-8'))
        if input_bytes > MAX_SET_BYTES:
            raise ValueError('human-form set exceeds input budget')
    if not (validator if validator is not None else _validator()).is_valid(form_set):
        raise ValueError('human-form set schema is invalid')
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
        stale_form = stale_subject or (value['subject']['id'] == subject.id and value['subject'] != subject.ref)
        content = value['content']
        selected = value['bindings'].get(content.get('slot')) if content['kind'] == 'source-copy' else None
        pointer = selected['pointer'] if selected else None
        required = []
        language, script, supported = None, None, False
        field = source_copy_field(subject, value, field_catalog)
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
            result = materialize_form(ROOT, form, scope, [subject], prior_forms=prior,
                                      validators=materializer_validators)
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
    return _load_forms(root, source_path, path, source, materialize_metadata_forms, access_allowed)


def claim_forms_path(source_path: Path, claim_id: str) -> Path:
    """Bounded adjacent filename, stable under row reorder; never a caller path."""
    if (source_path.name != 'source-claims.jsonl' or not isinstance(claim_id, str)
            or not re.fullmatch(r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*', claim_id)):
        raise ValueError('Claim forms require a declared source stream and stable Claim identity')
    suffix = hashlib.sha256(claim_id.encode('utf-8')).hexdigest()
    return source_path.with_name(f'source-claims.{suffix}.human-forms.json')


def load_claim_forms(repo_root: Path, source_ref: str, source: dict, *, access_allowed: bool):
    root = repo_root.resolve()
    source_path = (root / source_ref).resolve()
    source_path.relative_to(root / 'ToS/source-witnesses')
    path = claim_forms_path(source_path, source['claim_id'])
    return _load_forms(root, source_path, path, source, materialize_claim_forms, access_allowed)


def _load_forms(root, source_path, path, source, materializer, access_allowed):
    if not path.exists():
        return None
    resolved = path.resolve()
    resolved.relative_to(source_path.parent)
    with resolved.open('rb') as handle:
        raw = handle.read(MAX_SET_BYTES + 1)
    if len(raw) > MAX_SET_BYTES:
        raise ValueError('human-form set exceeds input budget')
    payload = json.loads(raw)
    return path.relative_to(root).as_posix(), raw, materializer(source, payload, access_allowed=access_allowed)
