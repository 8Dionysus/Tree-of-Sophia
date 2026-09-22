"""Read adjacent metadata and declared Claim forms through the source materializer.

The default adapter copies already public metadata. Explicit local snapshots
can read protected source/journal inputs for current source-form policy admission.
Substantive assessment and public-release authorization follow their
respective owner decision routes.
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
from validate_tree_node_contracts import node_consistency_issues, parse_node_json

MAX_SET_BYTES = 2_097_152
MAX_SET_OUTPUT_BYTES = 262_144
CLAIM_DISPLAY_SCHEMA = 'ToS/contracts/claim-display-fields.schema.json'
CLAIM_DISPLAY_VERSION = 'tos_claim_display_fields_v1'
CLAIM_FORM_FIELDS = {
    'claim.statement': ('statement', '/qualifiers/statement'),
    **{f'claim.{role}': (role, f'/qualifiers/display_fields/{role}/text')
       for role in ('name', 'caption', 'hover')},
}


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
        self._reader = None
        self._reader_selected = False

    def _bind_request(self, subject, form_ref, source_path, form_path, described):
        from assessment_journal import JournalConflict
        identity = form_ref['id']
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
        return {'schema_version': 'tos_local_assessment_command_v1', 'operation': 'materialize-form',
                'subject_id': identity, 'expected_subject': form_ref, 'expected_snapshot': snapshot}

    def _remember_reply(self, subject, form_ref, request, reply):
        from assessment_journal import JournalConflict
        identity, snapshot = form_ref['id'], request['expected_snapshot']
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

    def _resolve(self, subject, form_ref, source_path, form_path):
        # The pre-existing v1/v3 route keeps its single-command semantics.
        from assessment_journal import run_public_source_command
        described = run_public_source_command(self.owner_config, {
            'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe', 'subject_id': form_ref['id']})
        request = self._bind_request(subject, form_ref, source_path, form_path, described)
        return self._remember_reply(subject, form_ref, request, run_public_source_command(self.owner_config, request))

    def _resolve_batch(self, selections):
        from assessment_journal import JournalConflict
        described = self._reader.read_batch([
            {'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe', 'subject_id': form_ref['id']}
            for subject, form_ref, source_path, form_path in selections])
        if len({reply['owner_snapshot'] for reply in described}) != 1:
            raise JournalConflict('assessment graph inputs changed during assembly')
        requests = [self._bind_request(*selection, reply)
                    for selection, reply in zip(selections, described, strict=True)]
        # No unverified per-form callback crosses this boundary. Both batches
        # have completed their source/configuration/journal checks before use.
        replies = self._reader.read_batch(requests)
        return [self._remember_reply(selection[0], selection[1], request, reply)
                for selection, request, reply in zip(selections, requests, replies, strict=True)]

    def verify_current(self):
        """Fail on observed change; do not silently rebuild only part of a graph."""
        from assessment_journal import JournalConflict, run_public_source_command
        if set(self._observed) != self.form_ids:
            raise ValueError('selected assessed forms are not all present in the graph')
        observations = [self._observed[identity] for identity in sorted(self._observed)]
        if self._reader is None:
            for observed in observations:
                if run_public_source_command(self.owner_config, observed['request']) != observed['reply']:
                    raise JournalConflict('assessment graph snapshot changed before return')
            return
        current_replies = self._reader.read_batch([item['request'] for item in observations])
        for observed, current in zip(observations, current_replies, strict=True):
            if current != observed['reply']:
                raise JournalConflict('assessment graph snapshot changed before return')

    def materialize(self, nodes: list[dict]) -> list[dict]:
        """Replace selected forms on existing source carriers, without mutation.

        Both bibliographic and corpus-navigation carriers keep full source
        records in properties. No unrelated node, edge or source is created.
        The result is local research material: assessment limits and explicit
        source context have not received a separate public-safety clearance.
        """
        from assessment_journal import JournalConflict, PublicSourceReadSession
        if not self._reader_selected:
            self._reader = PublicSourceReadSession.for_public_owner(self.owner_config, sorted(self.form_ids))
            self._reader_selected = True
        output, seen, destinations, selections = [], set(), [], []
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
                selection = (subject, packet['form'], node.get('source_ref'), properties.get('human_forms_source_ref'))
                if self._reader is None:
                    replacement['properties']['human_forms'][index] = self._resolve(*selection)
                else:
                    selections.append(selection)
                destinations.append((replacement, index))
            output.append(replacement)
        if seen != self.form_ids:
            raise ValueError('selected assessed forms are not all present in the graph')
        if self._reader is not None:
            for (node, index), packet in zip(destinations, self._resolve_batch(selections), strict=True):
                node['properties']['human_forms'][index] = packet
        for node, index in destinations:
            if len(json.dumps(node['properties']['human_forms'][index], ensure_ascii=False,
                              separators=(',', ':')).encode()) > 65_536:
                raise ValueError('assessed form with snapshot binding exceeds its output budget')
        for node in output:
            if any(packet.get('form', {}).get('id') in self.form_ids
                   for packet in node.get('properties', {}).get('human_forms', [])):
                if len(json.dumps(node['properties']['human_forms'], ensure_ascii=False,
                                  separators=(',', ':')).encode()) > MAX_SET_OUTPUT_BYTES:
                    raise ValueError('assessed form set exceeds its output budget')
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

    This publication creates the local candidate. Public release and artifact
    admission follow their separate owner routes.
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
        snapshot.verify_current()
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
CANONICAL_NODE_SCHEMA = 'tos_canonical_node_v1'
CANONICAL_NODE_SCHEMA_REF = 'ToS/contracts/tos-node-contract.schema.json'
CANONICAL_NODE_TYPES = frozenset({
    'source', 'concept', 'principle', 'lineage', 'event', 'state', 'support',
    'context', 'analogy', 'synthesis',
})
CANONICAL_IDENTITIES = {CANONICAL_NODE_SCHEMA: 'node_id'}


def metadata_subject(source: dict) -> Record:
    """Bind the unchanged validated payload using its actual identity field."""
    identity = (CANONICAL_IDENTITIES | NATIVE_IDENTITIES).get(source.get('schema_version'), 'record_id')
    if source.get('schema_version') == CANONICAL_NODE_SCHEMA:
        if 'record_id' in source or 'node_id' not in source or 'record_version' not in source:
            raise ValueError('canonical node forms require native node_id and explicit record_version')
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


@lru_cache(maxsize=1)
def _canonical_node_validator():
    """Validate the native canonical node contract, including its local defs."""
    schema = json.loads((ROOT / CANONICAL_NODE_SCHEMA_REF).read_text())
    registry = Registry().with_resource(schema['$id'], Resource.from_contents(schema))
    return Draft202012Validator(schema, registry=registry)


def _canonical_validator_for_schema(schema: dict):
    registry = Registry().with_resource(schema['$id'], Resource.from_contents(schema))
    return Draft202012Validator(schema, registry=registry)


def _validate_canonical_node(source: dict, *, schema: dict | None = None) -> dict:
    """Return a validated canonical node; never version an unversioned node."""
    if not isinstance(source, dict) or source.get('schema_version') != CANONICAL_NODE_SCHEMA:
        raise ValueError('canonical forms require an explicit tos_canonical_node_v1 source')
    if 'record_id' in source or 'node_id' not in source or 'record_version' not in source:
        raise ValueError('canonical node must use native node_id and explicit record_version')
    validator = _canonical_node_validator() if schema is None else _canonical_validator_for_schema(schema)
    errors = sorted(validator.iter_errors(source), key=lambda error: list(error.path))
    if errors:
        raise ValueError('canonical node violates its exact source contract')
    if source.get('node_type') not in CANONICAL_NODE_TYPES:
        raise ValueError('canonical node type is outside its source contract')
    if not source['node_id'].startswith(f"tos.{source['node_type']}."):
        raise ValueError('canonical node_id prefix does not match native node_type')
    if node_consistency_issues(source, location='canonical source'):
        raise ValueError('canonical node has inconsistent identity, relations or witness segments')
    return source


def _canonical_relative_path(root: Path, source_ref: str, source: dict | None = None) -> Path:
    """Validate the only canonical source route accepted by this adapter."""
    if not isinstance(source_ref, str) or not source_ref:
        raise PermissionError('canonical source path must be an explicit relative path')
    relative = Path(source_ref)
    if (relative.is_absolute() or relative.as_posix() != source_ref or '..' in relative.parts
            or relative.parts[:2] != ('ToS', 'canon') or relative.name != 'node.json'
            or len(relative.parts) < 4 or any(not part or part in {'.', '..'} for part in relative.parts)):
        raise PermissionError('canonical source path must be ToS/canon/.../node.json')
    if source is not None:
        if (not isinstance(source, dict) or source.get('schema_version') != CANONICAL_NODE_SCHEMA
                or not isinstance(source.get('node_type'), str) or not isinstance(source.get('node_id'), str)):
            raise ValueError('canonical source path binding requires native node_id and schema')
        if relative.parts[2] != source['node_type']:
            raise PermissionError('canonical source path and native node_type disagree')
        # Source nodes use the route directory prologue-1 while their native
        # id ends in prologue; retain that bounded historical path convention.
        slug = source['node_id'].rsplit('.', 1)[-1]
        if relative.parent.name != slug and not (
                source['node_type'] == 'source' and relative.parent.name.startswith(slug + '-')):
            raise PermissionError('canonical source path and native node_id disagree')
    root = Path(root)
    if not root.is_absolute():
        raise PermissionError('canonical source root must be absolute')
    return relative


def _reject_symlink_components(root: Path, relative: Path) -> None:
    """Reject source and adjacent form aliases before any resolved-path read."""
    current = root
    for part in relative.parts:
        current /= part
        if current.is_symlink():
            raise PermissionError('canonical source and form paths must not traverse symlinks')


def _read_canonical_source(root: Path, source_ref: str, source: dict | None = None):
    """Read and bind the native canonical file and its schema digest."""
    root = Path(root)
    if root.is_symlink():
        raise PermissionError('canonical source root must not be a symlink')
    root = root.resolve()
    relative = _canonical_relative_path(root, source_ref, source)
    _reject_symlink_components(root, relative)
    source_path = root / relative
    with source_path.open('rb') as handle:
        source_raw = handle.read(MAX_SET_BYTES + 1)
    if len(source_raw) > MAX_SET_BYTES:
        raise ValueError('canonical source exceeds input budget')
    schema_path = root / CANONICAL_NODE_SCHEMA_REF
    _reject_symlink_components(root, Path(CANONICAL_NODE_SCHEMA_REF))
    schema_raw = schema_path.read_bytes()
    schema = parse_node_json(schema_raw)
    if not isinstance(schema, dict):
        raise ValueError('canonical source schema must be a JSON object')
    actual = parse_node_json(source_raw)
    _validate_canonical_node(actual, schema=schema)
    if actual.get('schema_version') != schema.get('properties', {}).get('schema_version', {}).get('const'):
        raise ValueError('canonical source schema contract does not declare its native schema')
    # Use the immutable subject's typed JSON digest: Python equality merges
    # false/zero and integer/float values that have distinct source bindings.
    subject = metadata_subject(actual)
    if source is not None and subject.ref != metadata_subject(source).ref:
        raise ValueError('canonical source file content differs from the supplied source payload')
    # The command path may have been checked before reading when a caller
    # supplied a payload. Rebind it to the actual source bytes as well so a
    # path/type/slug mismatch cannot pass the source=None command route.
    bound_relative = _canonical_relative_path(root, source_ref, actual)
    if bound_relative != relative:
        raise PermissionError('canonical source path and native node disagree')
    return source_path, source_raw, actual, {CANONICAL_NODE_SCHEMA_REF: 'sha256:' + hashlib.sha256(schema_raw).hexdigest()}


def metadata_field_catalog(source: dict, *, field_language_validator=None) -> list[dict]:
    """Semantic field selectors for this adapter; callers never guess pointers.

    Variant ordinals are snapshot-local, not stable name identities. An exact
    source ref must accompany prepared commands, so reordering is a conflict.
    """
    if source.get('schema_version') == CANONICAL_NODE_SCHEMA:
        # Canonical nodes have a separate owner route. Do not let a generic
        # metadata consumer inherit native node fields or old grants.
        raise ValueError('canonical nodes require the canonical form adapter')
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
    if source.get('schema_version') == 'tos_source_link_v1':
        context.extend('/' + key for key in ('uri', 'provider_label', 'link_kind', 'access_status', 'observed_at',
                                            'observation_ref', 'association_claim_refs', 'provenance_event_ref'))
    if source.get('record_type') == 'sign':
        # Birth evidence and limits travel with every source-copy wording.
        # They never supply this HumanForm's current semantic admission.
        context.append('/promotion_basis')
    declarations = source.get('field_languages', {})
    if not (field_language_validator if field_language_validator is not None else _field_language_validator()).is_valid(declarations):
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


def canonical_field_catalog(source: dict) -> list[dict]:
    """Expose only native wording fields declared by a versioned canon node.

    Every wording form carries the complete node as mandatory context. The
    adapter never turns the distilled thesis into an assessment or derives a
    language from a neighbouring source witness.
    """
    source = _validate_canonical_node(source)
    declarations = source.get('field_languages') or {}
    if ('preferred_label' in declarations
            and (not isinstance(source.get('preferred_label'), str) or not source['preferred_label'].strip())):
        raise ValueError('canonical preferred-label language declaration has no wording field')
    result = []
    if isinstance(source.get('preferred_label'), str) and source['preferred_label'].strip():
        declaration = declarations.get('preferred_label') or {}
        result.append({'field_id': 'canonical.preferred-name', 'pointer': '/preferred_label', 'role': 'name',
                       'language': declaration.get('language'), 'script': declaration.get('script'),
                       'context': ['']})
    for index, variant in enumerate(source.get('variant_labels', [])):
        # The node schema has already checked these members. Re-checking the
        # value here keeps this catalog safe when called with a test double.
        if not isinstance(variant, dict) or not isinstance(variant.get('value'), str) or not variant['value'].strip():
            raise ValueError('canonical variant label has no complete wording value')
        result.append({'field_id': f'canonical.variant-name:{index}',
                       'pointer': f'/variant_labels/{index}/value', 'role': 'name',
                       'language': variant.get('language'), 'script': variant.get('script'),
                       'context': ['']})
    if not isinstance(source.get('distilled_thesis'), str) or not source['distilled_thesis'].strip():
        raise ValueError('canonical node thesis has no complete wording value')
    declaration = declarations.get('distilled_thesis') or {}
    result.append({'field_id': 'canonical.thesis', 'pointer': '/distilled_thesis', 'role': 'statement',
                   'language': declaration.get('language'), 'script': declaration.get('script'),
                   'context': ['']})
    return result


def materialize_canonical_forms(source: dict, form_set: dict, *, access_allowed: bool) -> list[dict]:
    """Render only source-copy forms bound to the exact native canon node."""
    source = _validate_canonical_node(source)
    subject = metadata_subject(source)
    # Canonical wording is never supplied by a freeform/template proposal.
    # Check retained history too so a later revision cannot launder an older
    # non-source form into this source-owned route.
    if isinstance(form_set, dict):
        all_forms = [*form_set.get('forms', []), *form_set.get('prior_forms', [])]
        if any(form.get('content', {}).get('kind') != 'source-copy' for form in all_forms):
            raise ValueError('canonical forms permit only exact source-copy content')
    return _materialize_forms(subject, canonical_field_catalog(source), form_set,
                              access_allowed=access_allowed is True)


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


@lru_cache(maxsize=1)
def _claim_display_validator():
    schemas = [json.loads((ROOT / ref).read_text()) for ref in
               (CLAIM_DISPLAY_SCHEMA, 'ToS/contracts/corpus-record.schema.json')]
    registry = Registry().with_resources((schema['$id'], Resource.from_contents(schema)) for schema in schemas)
    return Draft202012Validator(schemas[0], registry=registry)


def claim_field_catalog(source: dict, *, validators=None) -> list[dict]:
    """Whole authored fields only; the entire Claim guards every reading.

    A source profile validates the Claim before calling this adapter. No label,
    endpoint name, predicate or assessment is synthesized into a statement.
    """
    qualifiers = source.get('qualifiers') or {}
    display = qualifiers.get('display_fields')
    understood = isinstance(display, dict) and display.get('schema_version') == CLAIM_DISPLAY_VERSION
    if understood and not (validators[1] if validators is not None else _claim_display_validator()).is_valid(qualifiers):
        raise ValueError('Claim display fields violate their explicit source contract')
    statement = qualifiers.get('statement')
    if not isinstance(statement, str) or not statement.strip():
        return []
    language, script = qualifiers.get('statement_language'), qualifiers.get('statement_script')
    if not (validators[0] if validators is not None else _field_language_validator()).is_valid({'notes': {'language': language, 'script': script}}):
        raise ValueError('claim statement language/script violates the source-form contract')
    result = [{'field_id': 'claim.statement', 'pointer': '/qualifiers/statement', 'role': 'statement',
               'language': language, 'script': script, 'context': ['']}]
    if understood:
        for role in ('name', 'caption', 'hover'):
            if role in display:
                wording = display[role]
                result.append({'field_id': f'claim.{role}', 'pointer': CLAIM_FORM_FIELDS[f'claim.{role}'][1],
                               'role': role, 'language': wording['language'], 'script': wording['script'],
                               'context': ['']})
    return result


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


def load_canonical_forms(repo_root: Path, source_ref: str, source: dict, *, access_allowed: bool):
    """Read the adjacent canonical form set through the exact native node route.

    This route deliberately does not reuse ``load_metadata_forms``: canonical
    nodes live under ``ToS/canon/.../node.json``, require an explicit native
    version/schema, and must match the bytes' parsed source object before any
    human-form payload is considered.
    """
    root = Path(repo_root)
    source_path, source_raw, actual, _ = _read_canonical_source(root, source_ref, source)
    root = root.resolve()
    relative = Path(source_ref)
    path = source_path.with_name('node.human-forms.json')
    _reject_symlink_components(root, relative)
    _reject_symlink_components(root, path.relative_to(root))
    if not path.exists():
        return None
    # _load_forms reads the adjacent payload with its existing bounded JSON
    # materializer. The actual source object, not the caller's object, is used.
    loaded = _load_forms(root, source_path, path, actual, materialize_canonical_forms, access_allowed)
    # Preserve the source-byte check as an explicit currentness boundary even
    # though _read_canonical_source already parsed and validated those bytes.
    if source_path.read_bytes() != source_raw:
        raise ValueError('canonical source changed while reading adjacent forms')
    return loaded


def claim_forms_path(source_path: Path, claim_id: str) -> Path:
    """Bounded adjacent filename, stable under row reorder; never a caller path."""
    from source_historical_claims import is_path as historical_claim_path
    if ((source_path.name != 'source-claims.jsonl' and not historical_claim_path(source_path)) or not isinstance(claim_id, str)
            or not re.fullmatch(r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*', claim_id)):
        raise ValueError('Claim forms require a declared source stream and stable Claim identity')
    suffix = hashlib.sha256(claim_id.encode('utf-8')).hexdigest()
    return source_path.with_name(f'{source_path.stem}.{suffix}.human-forms.json')


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
