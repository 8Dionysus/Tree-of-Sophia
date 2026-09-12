"""Private source-root selection in the same transaction as prepared rows.

This is storage pairing, not source observation, closure verification or an
authority grant. The assembler admits source inputs, retains immutable parts,
and holds source-owner locks/guards through its eventual SQLite commit.
"""
from dataclasses import dataclass, replace
from pathlib import Path
import re

from .projection_mutation import ProjectionSnapshotView, _json_bytes
from .projection_store import _strict_json, _digest
from .prepared_publication import PublicationLimits, SCHEMA, _metadata, _hash, _cap
from .published_read_metadata import TOP_KEY, published_snapshot_binding
from .prepared_semantics import apply_semantic_prepared_delta_transaction

SCHEMA_VERSION = 'tos_prepared_source_inputs_v1'
MAX_STATE_BYTES = 1_048_576
MAX_ROOTS = 16
MAX_DEPENDENCIES = 1024
_SHA = re.compile(r'[a-f0-9]{64}\Z')
_NAME = re.compile(r'[a-z][a-z0-9_.-]{0,127}\Z')


def _sha(value):
    if type(value) is not str or not _SHA.fullmatch(value):
        raise ValueError('exact lowercase source binding digest required')
    return value


@dataclass(frozen=True, init=False)
class PreparedSourceInputs:
    """Detached roots and dependency digests supplied by the source assembler.

    Exact root bytes live here, never in a second mutable current-root file.
    Namespace paths locate retained parts; constructing this object does not
    read those parts, validate source coverage or establish a current epoch.
    """
    raw: bytes

    def __init__(self, *, source_revision, source_publication, dependencies, roots):
        _sha(source_revision)
        if source_publication is not None:
            if type(source_publication) is not str or not source_publication.startswith('sha256:'):
                raise ValueError('exact participating source publication token required')
            _sha(source_publication[7:])
        if (type(dependencies) is not dict or len(dependencies) > MAX_DEPENDENCIES
                or any(type(key) is not str or not key or len(key.encode('utf-8')) > 4096
                       for key in dependencies)):
            raise ValueError('bounded named source dependency digests required')
        for digest in dependencies.values():
            _sha(digest)
        if type(roots) is not dict or not 1 <= len(roots) <= MAX_ROOTS:
            raise ValueError('bounded explicit source projection roots required')
        selected = {}
        for name, view in roots.items():
            if type(name) is not str or not _NAME.fullmatch(name) or type(view) is not ProjectionSnapshotView:
                raise ValueError('named immutable projection snapshot required')
            path = view.namespace_path
            if not path.is_absolute() or '..' in path.parts or len(str(path).encode('utf-8')) > 4096:
                raise ValueError('exact bounded projection namespace locator required')
            selected[name] = {'namespace_path': path.as_posix(), 'root_json': view.root_bytes.decode('utf-8'),
                              'snapshot_sha256': view.snapshot_digest}
        value = {'schema': SCHEMA_VERSION, 'source_revision': source_revision,
                 'source_publication': source_publication, 'dependencies': dependencies, 'roots': selected}
        object.__setattr__(self, 'raw', _json_bytes(value, MAX_STATE_BYTES))

    @property
    def digest(self):
        return _digest(self.raw)

    def value(self):
        return _strict_json(self.raw)

    def roots(self):
        return {name: ProjectionSnapshotView(item['root_json'].encode('utf-8'), Path(item['namespace_path']))
                for name, item in self.value()['roots'].items()}

    @classmethod
    def parse(cls, raw):
        if type(raw) is not bytes or len(raw) > MAX_STATE_BYTES:
            raise ValueError('source selection exceeds byte budget')
        value = _strict_json(raw)
        if (type(value) is not dict or set(value) != {'schema', 'source_revision', 'source_publication', 'dependencies', 'roots'}
                or value['schema'] != SCHEMA_VERSION or type(value['roots']) is not dict
                or not 1 <= len(value['roots']) <= MAX_ROOTS):
            raise ValueError('invalid private source selection')
        roots = {}
        for name, item in value['roots'].items():
            if (type(item) is not dict or set(item) != {'namespace_path', 'root_json', 'snapshot_sha256'}
                    or type(item['namespace_path']) is not str or type(item['root_json']) is not str
                    or not Path(item['namespace_path']).is_absolute()):
                raise ValueError('invalid retained source projection root')
            view = ProjectionSnapshotView(item['root_json'].encode('utf-8'), Path(item['namespace_path']))
            if view.snapshot_digest != _sha(item['snapshot_sha256']):
                raise ValueError('source root bytes differ from retained digest')
            roots[name] = view
        result = cls(source_revision=value['source_revision'], source_publication=value['source_publication'],
                     dependencies=value['dependencies'], roots=roots)
        if result.raw != raw:
            raise ValueError('source selection is not exact canonical storage')
        return result


def _selected(db, binding, limits):
    if not db.in_transaction:
        raise ValueError('source pairing requires caller-owned transaction')
    top = _metadata(db, TOP_KEY)
    clock = db.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()
    if (clock is None or top.get('read_model_schema') != SCHEMA
            or _json_bytes(published_snapshot_binding(top, clock[0]), limits.max_metadata_bytes)
               != _json_bytes(binding, limits.max_metadata_bytes)
            or _metadata(db, 'data_revision') != {'sha256': top['data_revision']}):
        raise ValueError('source pairing selected publication differs')
    state = db.execute('SELECT max_pages,CASE WHEN length(CAST(descriptor AS BLOB))<=? '
                       'THEN descriptor ELSE NULL END FROM prepared_state WHERE singleton=1',
                       (limits.max_metadata_bytes,)).fetchone()
    if state is None or not isinstance(state[1], str) or _hash(_strict_json(state[1])) != top['data_revision']:
        raise ValueError('source pairing prepared descriptor differs')
    _cap(db, limits, state[0])
    return top


def _receipt(binding, inputs, writes):
    return {'binding': binding.copy(), 'source_inputs_sha256': inputs.digest, 'sql_mutations': writes,
            'roots_paired_in_caller_transaction': True, 'source_transition_verified': False,
            'target_closure_verified': False, 'semantic_acceptance': False, 'consumer_switched': False}


def read_prepared_source_inputs_transaction(db, *, expected_binding, limits=None):
    """Read only the private state row under the caller's selected transaction."""
    limits = limits or PublicationLimits()
    _selected(db, expected_binding, limits)
    maximum = min(MAX_STATE_BYTES, limits.max_metadata_bytes)
    rows = db.execute('SELECT CASE WHEN typeof(binding)=\'text\' AND length(CAST(binding AS BLOB))<=? THEN binding END,'
                      'CASE WHEN typeof(inputs)=\'text\' AND length(CAST(inputs AS BLOB))<=? THEN inputs END,'
                      'CASE WHEN typeof(sha256)=\'text\' AND length(sha256)=64 THEN sha256 END '
                      'FROM prepared_source_state WHERE singleton=1 LIMIT 2', (maximum, maximum)).fetchall()
    if len(rows) != 1 or any(type(value) is not str for value in rows[0]):
        raise ValueError('private source selection missing, malformed or over budget')
    binding, raw, digest = rows[0]
    if binding.encode('utf-8') != _json_bytes(expected_binding, maximum):
        raise ValueError('private source selection binds another prepared publication')
    result = PreparedSourceInputs.parse(raw.encode('utf-8'))
    if result.digest != digest or result.value()['source_revision'] != expected_binding['source_revision']:
        raise ValueError('private source selection digest/revision differs')
    return result


def bootstrap_prepared_source_inputs_transaction(db, *, expected_binding, inputs, limits=None):
    """Explicit one-time pairing; no source closure is inferred from a root."""
    limits = limits or PublicationLimits()
    top = _selected(db, expected_binding, limits)
    if type(inputs) is not PreparedSourceInputs or PreparedSourceInputs.parse(inputs.raw) != inputs:
        raise ValueError('exact prepared source inputs required')
    if inputs.value()['source_revision'] != top['source_revision'] or len(inputs.raw) > limits.max_metadata_bytes:
        raise ValueError('source inputs exceed metadata cap or name another revision')
    start = db.total_changes
    # Deliberately no IF NOT EXISTS: attachment to any prior selection is not
    # an idempotent bootstrap or a silent adoption of an existing table.
    db.execute('CREATE TABLE prepared_source_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),'
               'binding TEXT NOT NULL, inputs TEXT NOT NULL, sha256 TEXT NOT NULL)')
    db.execute('INSERT INTO prepared_source_state VALUES (1,?,?,?)',
               (_json_bytes(expected_binding, limits.max_metadata_bytes).decode('utf-8'),
                inputs.raw.decode('utf-8'), inputs.digest))
    _cap(db, limits)
    if db.total_changes - start > limits.max_mutations:
        raise ValueError('source pairing mutation budget exceeded')
    if read_prepared_source_inputs_transaction(db, expected_binding=expected_binding, limits=limits) != inputs:
        raise ValueError('source pairing readback differs')
    return _receipt(expected_binding, inputs, db.total_changes - start)


def apply_source_bound_prepared_delta_transaction(db, *, expected_binding, before_source_inputs,
        after_source_inputs, before_inputs, after_inputs, changes, limits=None,
        catalog_limits=None, semantic_limits=None):
    """Pair immutable roots with all normalized lanes under one caller commit.

    Any exception requires whole rollback. Source guards run outside this
    kernel immediately before caller commit; it cannot create cross-filesystem
    atomicity, establish source completeness, admit arbitrary replacement rows
    or make an unverified COW candidate trustworthy.
    """
    limits = limits or PublicationLimits()
    if type(before_source_inputs) is not PreparedSourceInputs or type(after_source_inputs) is not PreparedSourceInputs:
        raise ValueError('exact before/after source selection required')
    if read_prepared_source_inputs_transaction(db, expected_binding=expected_binding, limits=limits) != before_source_inputs:
        raise ValueError('source selection compare-and-swap predecessor differs')
    after = PreparedSourceInputs.parse(after_source_inputs.raw)
    old_value, new_value = before_source_inputs.value(), after.value()
    if (set(old_value['roots']) != set(new_value['roots'])
            or any(old_value['roots'][key]['namespace_path'] != new_value['roots'][key]['namespace_path']
                   for key in old_value['roots'])):
        raise ValueError('source root set/namespace change requires explicit bootstrap')
    for key in old_value['roots']:
        old_root = _strict_json(old_value['roots'][key]['root_json'])
        new_root = _strict_json(new_value['roots'][key]['root_json'])
        def identity(root):
            return {name: value for name, value in root.items() if name not in ('header', 'collections')} | {
                'collections': {name: {field: value for field, value in spec.items() if field != 'root'}
                                for name, spec in root['collections'].items()}}
        if _json_bytes(identity(old_root), MAX_STATE_BYTES) != _json_bytes(identity(new_root), MAX_STATE_BYTES):
            raise ValueError('source projection logical identity requires explicit bootstrap')
    if (new_value['source_revision'] != after_inputs.header.get('source_revision')
            or len(after.raw) > limits.max_metadata_bytes or limits.max_mutations < 2):
        raise ValueError('source successor revision or pairing budget differs')
    start = db.total_changes
    result = apply_semantic_prepared_delta_transaction(db, expected_binding=expected_binding,
        before_inputs=before_inputs, after_inputs=after_inputs, changes=changes,
        limits=replace(limits, max_mutations=limits.max_mutations - 1),
        catalog_limits=catalog_limits, semantic_limits=semantic_limits)
    db.execute('UPDATE prepared_source_state SET binding=?,inputs=?,sha256=? WHERE singleton=1 AND sha256=?',
        (_json_bytes(result['binding'], limits.max_metadata_bytes).decode('utf-8'),
         after.raw.decode('utf-8'), after.digest, before_source_inputs.digest))
    if read_prepared_source_inputs_transaction(db, expected_binding=result['binding'], limits=limits) != after:
        raise ValueError('paired source successor readback differs')
    if db.total_changes - start > limits.max_mutations:
        raise ValueError('all-lane source pairing mutation budget exceeded')
    return {**result, **_receipt(result['binding'], after, db.total_changes - start)}
