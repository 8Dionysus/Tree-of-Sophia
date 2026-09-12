"""Private addressed source-Claim dependencies in a selected prepared SQLite.

This is storage integrity under an explicit owner-produced baseline, not source
verification, semantic admission or graph-incidence discovery. Every operation
uses an existing caller transaction and an explicit temporary progress-handler
owner. No entry point commits, rolls back, closes, selects a runtime, or falls
back to scanning Claims/graph rows. Any failure requires whole caller rollback.
"""
from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
from pathlib import Path, PurePosixPath
import re
import sqlite3
from typing import Callable

from .projection_mutation import _json_bytes
from .projection_store import _strict_json, _digest
from .prepared_source_binding import PreparedSourceInputs
from .semantic_index import SemanticIndexLimits, _Budget as _SemanticBudget, _binding

SCHEMA = 'tos_prepared_source_dependencies_v1'
DECLARATION_SCHEMA = 'tos_source_claim_dependencies_v1'
CHECKSUM = 'tos_source_dependency_count_xor_sum_sha256_v1'
KINDS = frozenset(('identity', 'provenance_event', 'claim', 'path', 'unresolved'))
MAX_DECLARATION_BYTES = 16 * 1024 * 1024
MAX_DEPENDENCIES = 65536
MAX_ID_BYTES = 4096
MAX_ORDER = 9_007_199_254_740_991
_SHA = re.compile(r'[a-f0-9]{64}\Z')
_MODULUS = 1 << 256


def _sha(value):
    if type(value) is not str or not _SHA.fullmatch(value):
        raise ValueError('source dependency requires exact lowercase SHA256')
    return value


def _identifier(value):
    if (type(value) is not str or not value or len(value) > MAX_ID_BYTES
            or len(value.encode('utf-8')) > MAX_ID_BYTES):
        raise ValueError('source dependency requires a bounded exact identifier')
    return value


def _address(kind, ref):
    if type(kind) is not str or kind not in KINDS or (ref is None and kind != 'unresolved'):
        raise ValueError('explicit supported source dependency kind/ref required')
    if ref is not None:
        _identifier(ref)
    return kind, _json_bytes(ref, MAX_ID_BYTES * 6 + 8).decode('utf-8')


@dataclass(frozen=True, init=False)
class SourceClaimDependencies:
    """Detached exact SOURCE Claim declaration; endpoint metadata is separate.

    ``input_sha256`` is the source Claim digest named by its exact catalog
    entry, not a digest of the rendered endpoint cohort or whole publication.
    Thus a descriptive Agent change may preserve this declaration. Construction
    verifies representation consistency only, never the source bytes or whether
    the supplied dependency list is the complete enumerator output.
    """
    raw: bytes

    def __init__(self, *, claim_id, source_entry, input_sha256, dependencies):
        _identifier(claim_id)
        _sha(input_sha256)
        if type(source_entry) is not dict or type(dependencies) not in (list, tuple):
            raise ValueError('exact source entry and typed dependency sequence required')
        if len(dependencies) > MAX_DEPENDENCIES:
            raise ValueError('source declaration dependency format bound exceeded')
        # Bounded encoding precedes structural traversal and detaches caller data.
        entry_raw = _json_bytes(source_entry, MAX_DECLARATION_BYTES)
        entry = _strict_json(entry_raw)
        if (entry.get('claim_id') != claim_id or entry.get('claim_sha256') != input_sha256
                or type(entry.get('source_claim_line')) is not int
                or not 1 <= entry['source_claim_line'] <= MAX_ORDER):
            raise ValueError('source declaration Claim identity/digest/slot differs')
        source_ref = _identifier(entry.get('source_claim_file_ref'))
        path = PurePosixPath(source_ref)
        if not source_ref.startswith('ToS/') or path.as_posix() != source_ref or '..' in path.parts:
            raise ValueError('source declaration requires canonical owned Claim locator')
        deps = _strict_json(_json_bytes(list(dependencies), MAX_DECLARATION_BYTES))
        keys = []
        for row in deps:
            if type(row) is not dict or set(row) != {'kind', 'ref', 'field_paths', 'reasons'}:
                raise ValueError('typed dependency declaration shape differs')
            _address(row['kind'], row['ref'])
            keys.append((row['kind'], row['ref'] or ''))
            for name in ('field_paths', 'reasons'):
                values = row[name]
                if type(values) is not list or not values:
                    raise ValueError('dependency needs explicit field paths and reasons')
                for value in values:
                    _identifier(value)
                    if name == 'field_paths' and not value.startswith('/'):
                        raise ValueError('dependency field path must be a JSON pointer')
                if values != sorted(set(values)):
                    raise ValueError('dependency reasons/paths must be sorted distinct')
        if keys != sorted(set(keys)):
            raise ValueError('dependency addresses must be sorted distinct')
        if ('claim', claim_id) not in keys or ('path', source_ref) not in keys:
            raise ValueError('declaration must include its own Claim and source slot')
        value = {'schema': DECLARATION_SCHEMA, 'claim_id': claim_id, 'source_entry': entry,
                 'source_entry_sha256': _digest(entry_raw), 'input_sha256': input_sha256,
                 'dependencies': deps}
        object.__setattr__(self, 'raw', _json_bytes(value, MAX_DECLARATION_BYTES))

    @property
    def digest(self):
        return _digest(self.raw)

    @property
    def claim_id(self):
        return self.value()['claim_id']

    def value(self):
        return _strict_json(self.raw)

    @classmethod
    def parse(cls, raw):
        if type(raw) is not bytes or len(raw) > MAX_DECLARATION_BYTES:
            raise ValueError('source declaration raw byte bound exceeded')
        value = _strict_json(raw)
        if (type(value) is not dict or set(value) != {'schema', 'claim_id', 'source_entry',
                'source_entry_sha256', 'input_sha256', 'dependencies'} or value['schema'] != DECLARATION_SCHEMA):
            raise ValueError('source declaration schema differs')
        result = cls(claim_id=value['claim_id'], source_entry=value['source_entry'],
                     input_sha256=value['input_sha256'], dependencies=value['dependencies'])
        if result.raw != raw:
            raise ValueError('source declaration canonical bytes/digest differ')
        return result


@dataclass(frozen=True)
class SourceDependencyChange:
    operation: str
    claim_id: str
    before_digest: str | None = None
    declaration: SourceClaimDependencies | None = None


@dataclass(frozen=True)
class SourceDependencyLimits(SemanticIndexLimits):
    max_claims: int = 4096
    max_dependencies_per_claim: int = 4096
    max_vm_steps: int = 20_000_000
    max_state_bytes: int = 8 * 1024 * 1024
    max_change_bytes: int = 16 * 1024 * 1024

    def __post_init__(self):
        super().__post_init__()
        if (any(value > MAX_ORDER for value in vars(self).values())
                or self.max_dependencies_per_claim > MAX_DEPENDENCIES
                or self.max_row_bytes > MAX_DECLARATION_BYTES):
            raise ValueError('source dependency limits exceed portable declaration bounds')


@dataclass(frozen=True)
class ProgressHandlerOwner:
    """Explicitly declare the prior handler; sqlite3 cannot discover it.

    With no previous handler, the caller asserts the connection's handler is
    unmanaged. Otherwise that known callback is also invoked at its requested
    interval while this lane runs, then restored on every exit. The caller
    must not replace handlers or reenter this lane during the operation.
    """
    previous_handler: Callable[[], int] | None = None
    previous_interval: int = 0

    def __post_init__(self):
        if (type(self.previous_interval) is not int
                or (self.previous_handler is None and self.previous_interval != 0)
                or (self.previous_handler is not None and (not callable(self.previous_handler)
                    or type(self.previous_interval) is not int or self.previous_interval < 1))):
            raise ValueError('explicit prior progress callback and interval required')


class _Budget(_SemanticBudget):
    def __init__(self, db, limits):
        self.steps = 0
        self.exhausted = False
        super().__init__(db, limits)

    def progress(self):
        self.steps += 100
        self.exhausted = self.steps > self.limits.max_vm_steps
        return int(self.exhausted)

    def execute(self, sql, args=()):
        if not self.db.in_transaction:
            raise ValueError('source dependency caller transaction ended during operation')
        # Charge the tail of every short statement as well as actual callbacks.
        self.steps += 100
        if self.steps > self.limits.max_vm_steps:
            self.exhausted = True
            raise ValueError('source dependency SQLite work budget exceeded')
        return super().execute(sql, args)

    def rows_from(self, sql, args=()):
        # Do not change a caller's row_factory; normalize bounded result rows.
        for row in super().rows_from(sql, args):
            yield tuple(row)


@contextmanager
def _operation(db, limits, owner):
    limits = limits or SourceDependencyLimits()
    if not isinstance(limits, SourceDependencyLimits) or type(owner) is not ProgressHandlerOwner:
        raise ValueError('explicit source dependency limits/progress owner required')
    if not db.in_transaction:
        raise ValueError('source dependency index requires caller-owned transaction')
    budget = None
    prior_steps = 0
    def progress():
        nonlocal prior_steps
        stopped = budget.progress() if budget is not None else 0
        if owner.previous_handler is not None:
            prior_steps += 100
            while prior_steps >= owner.previous_interval:
                prior_steps -= owner.previous_interval
                stopped |= int(bool(owner.previous_handler()))
                if stopped:
                    break
        return stopped
    db.set_progress_handler(progress, 100)
    try:
        budget = _Budget(db, limits)
        yield budget
    except sqlite3.OperationalError as error:
        if budget is not None and budget.exhausted:
            raise ValueError('source dependency SQLite work budget exceeded; rollback required') from error
        raise
    finally:
        db.set_progress_handler(owner.previous_handler, owner.previous_interval)


_DDL = (
    'CREATE TABLE source_dependency_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),json TEXT NOT NULL,sha256 TEXT NOT NULL)',
    'CREATE TABLE source_dependency_claims(claim_id TEXT PRIMARY KEY,declaration TEXT NOT NULL,digest TEXT NOT NULL) WITHOUT ROWID',
    'CREATE TABLE source_dependency_refs(kind TEXT NOT NULL,ref_key TEXT NOT NULL,claim_id TEXT NOT NULL,declaration_digest TEXT NOT NULL,PRIMARY KEY(kind,ref_key,claim_id)) WITHOUT ROWID',
    'CREATE INDEX source_dependency_claim_refs ON source_dependency_refs(claim_id,kind,ref_key)',
    'CREATE TABLE source_dependency_heads(kind TEXT NOT NULL,ref_key TEXT NOT NULL,n INTEGER NOT NULL CHECK(n>0),xor_sha256 TEXT NOT NULL,sum_sha256 TEXT NOT NULL,seal TEXT NOT NULL,PRIMARY KEY(kind,ref_key)) WITHOUT ROWID',
    'CREATE TABLE source_dependency_pending(claim_id TEXT PRIMARY KEY,digest TEXT) WITHOUT ROWID',
)


def _column(name, maximum):
    # Names are fixed owner literals, never caller SQL. Bound before transfer
    # into Python even if a stored index/metadata column was damaged.
    return (f"CASE WHEN typeof({name})='text' AND length(CAST({name} AS BLOB))<={maximum} "
            f'THEN {name} END')


def source_dependency_processor_digest():
    """Conservative executable binding, separate from the source enumerator."""
    here = Path(__file__)
    digest = hashlib.sha256(SCHEMA.encode('utf-8'))
    for name in ('prepared_source_dependencies.py', 'semantic_index.py', 'projection_mutation.py',
                 'projection_store.py', 'published_read_metadata.py'):
        digest.update(here.with_name(name).read_bytes())
    return digest.hexdigest()


def _schema(b):
    expected = {statement.split()[2].split('(')[0]: statement for statement in _DDL}
    placeholders = ','.join('?' for _ in expected)
    actual = dict(b.rows_from(f'SELECT name,{_column("sql", 4096)} FROM sqlite_master '
                             f'WHERE name IN ({placeholders})', tuple(expected)))
    if actual != expected:
        raise ValueError('source dependency physical schema/index drift; explicit bootstrap required')
    tables = tuple(name for name in expected if name != 'source_dependency_claim_refs')
    placeholders = ','.join('?' for _ in tables)
    if b.one(f"SELECT name FROM sqlite_master WHERE type='trigger' AND tbl_name IN ({placeholders}) LIMIT 1", tables):
        raise ValueError('source dependency table triggers are not part of this write mask')


def _selected(b, binding, source_digest):
    _sha(source_digest)
    selected = _binding(b, binding)
    descriptor = b.text('prepared_state', 'descriptor', 'singleton=1', (), b.limits.max_state_bytes)
    if type(descriptor) is not str or _digest(descriptor.encode('utf-8')) != selected['data_revision']:
        raise ValueError('source dependency selected prepared descriptor differs')
    stored_pages = b.one('SELECT max_pages FROM prepared_state WHERE singleton=1')
    if stored_pages is None or type(stored_pages[0]) is not int or stored_pages[0] < 1:
        raise ValueError('source dependency prepared page ceiling differs')
    ceiling = min(stored_pages[0], b.one('PRAGMA max_page_count')[0])
    if b.one('PRAGMA page_count')[0] > ceiling:
        raise ValueError('source dependency prepared file exceeds page budget')
    b.execute(f'PRAGMA max_page_count={ceiling}')
    raw = b.text('prepared_source_state', 'inputs', 'singleton=1', (), b.limits.max_state_bytes)
    stored_binding = b.text('prepared_source_state', 'binding', 'singleton=1', (), b.limits.max_state_bytes)
    digest = b.one(f'SELECT {_column("sha256", 64)} FROM prepared_source_state WHERE singleton=1')
    if (type(raw) is not str or type(stored_binding) is not str or digest != (source_digest,)
            or stored_binding.encode('utf-8') != _json_bytes(selected, b.limits.max_state_bytes)):
        raise ValueError('source dependency selected source-root binding differs')
    inputs = PreparedSourceInputs.parse(raw.encode('utf-8'))
    if inputs.digest != source_digest or inputs.value()['source_revision'] != binding['source_revision']:
        raise ValueError('source dependency source-root bytes/revision differ')
    return selected


def _state(b, profile):
    _sha(profile)
    _schema(b)
    raw = b.text('source_dependency_state', 'json', 'singleton=1', (), b.limits.max_state_bytes)
    digest = b.one(f'SELECT {_column("sha256", 64)} FROM source_dependency_state WHERE singleton=1')
    if type(raw) is not str or digest != (_digest(raw.encode('utf-8')),):
        raise ValueError('source dependency state digest differs')
    state = _strict_json(raw.encode('utf-8'))
    fields = {'schema', 'checksum', 'declaration_profile_sha256', 'implementation_sha256',
              'binding', 'source_inputs_sha256', 'claim_count', 'dependency_count', 'pending', 'pending_count'}
    if type(state) is dict and state.get('pending') is True:
        fields.update(('next_source_inputs_sha256', 'next_source_revision', 'next_epoch'))
    if (type(state) is not dict or set(state) != fields
            or state.get('schema') != SCHEMA or state.get('checksum') != CHECKSUM
            or state.get('declaration_profile_sha256') != profile
            or state.get('implementation_sha256') != source_dependency_processor_digest()
            or _json_bytes(state, b.limits.max_state_bytes).decode('utf-8') != raw
            or any(type(state.get(key)) is not int or not 0 <= state[key] <= MAX_ORDER
                   for key in ('claim_count', 'dependency_count', 'pending_count'))
            or type(state.get('pending')) is not bool or type(state.get('binding')) is not dict
            or (not state['pending'] and state['pending_count'] != 0)):
        raise ValueError('source dependency state/profile/framing differs')
    _sha(state['source_inputs_sha256'])
    if state['pending']:
        _sha(state['next_source_inputs_sha256'])
        _sha(state['next_source_revision'])
        if type(state['next_epoch']) is not int or not 1 <= state['next_epoch'] <= MAX_ORDER:
            raise ValueError('source dependency next epoch differs')
    return state


def _put_state(b, state, *, before_digest=None):
    raw = _json_bytes(state, b.limits.max_state_bytes).decode('utf-8')
    digest = _digest(raw.encode('utf-8'))
    if before_digest is None:
        b.execute('INSERT INTO source_dependency_state VALUES (1,?,?)', (raw, digest))
    elif b.execute('UPDATE source_dependency_state SET json=?,sha256=? WHERE singleton=1 AND sha256=?',
                   (raw, digest, before_digest)).rowcount != 1:
        raise ValueError('source dependency state compare-and-swap failed')


def _current(b, binding, source_digest, profile):
    _selected(b, binding, source_digest)
    state = _state(b, profile)
    if state['pending'] or state['binding'] != binding or state['source_inputs_sha256'] != source_digest:
        raise ValueError('source dependency pending/stale selection requires rollback or bootstrap')
    if b.one('SELECT 1 FROM source_dependency_pending LIMIT 1'):
        raise ValueError('source dependency orphan pending declaration')
    return state


def _declaration(b, value):
    if type(value) is not SourceClaimDependencies or len(value.raw) > b.limits.max_row_bytes:
        raise ValueError('bounded exact source Claim declaration required')
    b.input(value.value())
    result = SourceClaimDependencies.parse(value.raw)
    if len(result.value()['dependencies']) > b.limits.max_dependencies_per_claim:
        raise ValueError('source Claim dependency count budget exceeded')
    return result


def _claim(b, claim_id):
    _identifier(claim_id)
    raw = b.text('source_dependency_claims', 'declaration', 'claim_id=?', (claim_id,), b.limits.max_row_bytes)
    if raw is None:
        # An orphan reverse row cannot masquerade as a clean absent declaration.
        if b.one('SELECT 1 FROM source_dependency_refs INDEXED BY source_dependency_claim_refs WHERE claim_id=? LIMIT 1', (claim_id,)):
            raise ValueError('source dependency orphan reverse row')
        return None
    digest = b.one(f'SELECT {_column("digest", 64)} FROM source_dependency_claims WHERE claim_id=?', (claim_id,))
    value = SourceClaimDependencies.parse(raw.encode('utf-8'))
    if value.claim_id != claim_id or digest != (value.digest,):
        raise ValueError('source Claim declaration digest/identity differs')
    dependencies = value.value()['dependencies']
    if len(dependencies) > b.limits.max_dependencies_per_claim:
        raise ValueError('source Claim dependency count budget exceeded')
    found = list(b.rows_from(f'SELECT {_column("kind", 32)},{_column("ref_key", MAX_ID_BYTES * 6 + 8)},'
        f'{_column("declaration_digest", 64)} FROM source_dependency_refs '
        'INDEXED BY source_dependency_claim_refs WHERE claim_id=? ORDER BY kind,ref_key LIMIT ?',
        (claim_id, b.limits.max_dependencies_per_claim + 1)))
    expected = sorted((*_address(row['kind'], row['ref']), value.digest) for row in dependencies)
    if found != expected:
        raise ValueError('source Claim complete reverse declarations differ')
    return value


def _contribution(kind, ref_key, claim_id, digest):
    raw = _json_bytes([CHECKSUM, kind, ref_key, claim_id, digest], MAX_ID_BYTES * 24 + 1024)
    return int.from_bytes(hashlib.sha256(raw).digest(), 'big')


def _head_seal(kind, ref_key, n, xor, total):
    return _digest(_json_bytes([CHECKSUM, kind, ref_key, n, f'{xor:064x}', f'{total:064x}'], MAX_ID_BYTES * 8 + 1024))


def _head(b, key):
    row = b.one(f'SELECT n,{_column("xor_sha256", 64)},{_column("sum_sha256", 64)},'
                f'{_column("seal", 64)} FROM source_dependency_heads WHERE kind=? AND ref_key=?', key)
    if row is None:
        return 0, 0, 0
    n, xor, total, seal = row
    if type(n) is not int or not 1 <= n <= MAX_ORDER:
        raise ValueError('source dependency head count differs')
    xor, total = int(_sha(xor), 16), int(_sha(total), 16)
    if seal != _head_seal(*key, n, xor, total):
        raise ValueError('source dependency head seal differs')
    return n, xor, total


def _adjust(b, key, claim_id, digest, adjustment):
    # Constant addressed work, including a high-fanout Agent address. A complete
    # bounded fanout is verified only when that address is explicitly queried.
    n, xor, total = _head(b, key)
    if not n and adjustment == 1:
        # The new link is already present. A missing head may represent that
        # single first link, never an unverified preexisting broad address.
        links = list(b.rows_from(f'SELECT {_column("claim_id", MAX_ID_BYTES)} FROM source_dependency_refs '
                                'WHERE kind=? AND ref_key=? ORDER BY claim_id LIMIT 2', key))
        if links != [(claim_id,)]:
            raise ValueError('source dependency head missing for existing address')
    contribution = _contribution(*key, claim_id, digest)
    n, xor, total = n + adjustment, xor ^ contribution, (total + adjustment * contribution) % _MODULUS
    if not 0 <= n <= MAX_ORDER or (n == 0 and (xor or total)):
        raise ValueError('source dependency aggregate underflow/drift')
    if n:
        b.execute('INSERT INTO source_dependency_heads VALUES (?,?,?,?,?,?) '
            'ON CONFLICT(kind,ref_key) DO UPDATE SET n=excluded.n,xor_sha256=excluded.xor_sha256,'
            'sum_sha256=excluded.sum_sha256,seal=excluded.seal',
            (*key, n, f'{xor:064x}', f'{total:064x}', _head_seal(*key, n, xor, total)))
    else:
        if b.one('SELECT 1 FROM source_dependency_refs WHERE kind=? AND ref_key=? LIMIT 1', key):
            raise ValueError('source dependency zero head has remaining address entries')
        b.execute('DELETE FROM source_dependency_heads WHERE kind=? AND ref_key=?', key)


def _insert(b, declaration):
    value, digest = declaration.value(), declaration.digest
    claim_id = value['claim_id']
    b.execute('INSERT INTO source_dependency_claims VALUES (?,?,?)', (claim_id, declaration.raw.decode('utf-8'), digest))
    for row in value['dependencies']:
        key = _address(row['kind'], row['ref'])
        b.execute('INSERT INTO source_dependency_refs VALUES (?,?,?,?)', (*key, claim_id, digest))
        _adjust(b, key, claim_id, digest, 1)


def _delete(b, declaration):
    value, digest = declaration.value(), declaration.digest
    claim_id = value['claim_id']
    for row in value['dependencies']:
        key = _address(row['kind'], row['ref'])
        if b.execute('DELETE FROM source_dependency_refs WHERE kind=? AND ref_key=? AND claim_id=? AND declaration_digest=?',
                     (*key, claim_id, digest)).rowcount != 1:
            raise ValueError('source dependency reverse-row compare-and-swap differs')
        _adjust(b, key, claim_id, digest, -1)
    if b.execute('DELETE FROM source_dependency_claims WHERE claim_id=? AND digest=?', (claim_id, digest)).rowcount != 1:
        raise ValueError('source Claim declaration compare-and-swap differs')


def _receipt(b, binding, source_digest, **values):
    result = {'binding': binding.copy(), 'source_inputs_sha256': source_digest,
              'sql_mutations': b.db.total_changes - b.initial_writes,
              'budget_usage': {'rows': b.rows, 'queries': b.queries, 'read_bytes': b.read_bytes,
                               'vm_steps_upper_bound': b.steps},
              'source_completeness_verified': False, 'source_transition_verified': False,
              'source_verification_performed': False, 'target_closure_verified': False,
              'semantic_acceptance': False, 'consumer_switched': False, **values}
    b.diagnostic(result)
    return result


def bootstrap_source_dependency_index_transaction(db, *, expected_binding, source_inputs_sha256,
        declaration_profile_sha256, claims, progress_owner, limits=None):
    """Explicit full supplied Claim stream; no discovery or completeness claim."""
    with _operation(db, limits, progress_owner) as b:
        binding = _selected(b, expected_binding, source_inputs_sha256)
        _sha(declaration_profile_sha256)
        for statement in _DDL:
            b.execute(statement)  # No IF NOT EXISTS or adoption of foreign state.
        count = dependencies = 0
        for supplied in claims:
            if count >= b.limits.max_claims:
                raise ValueError('source dependency bootstrap Claim budget exceeded')
            declaration = _declaration(b, supplied)
            _insert(b, declaration)
            count += 1
            dependencies += len(declaration.value()['dependencies'])
        state = {'schema': SCHEMA, 'checksum': CHECKSUM, 'implementation_sha256': source_dependency_processor_digest(),
                 'declaration_profile_sha256': declaration_profile_sha256, 'binding': binding,
                 'source_inputs_sha256': source_inputs_sha256, 'claim_count': count,
                 'dependency_count': dependencies, 'pending': False, 'pending_count': 0}
        _put_state(b, state)
        _current(b, binding, source_inputs_sha256, declaration_profile_sha256)
        return _receipt(b, binding, source_inputs_sha256, claim_count=count,
                        dependency_count=dependencies, publication_changed=False)


def read_source_claim_dependencies_transaction(db, *, expected_binding, source_inputs_sha256,
        declaration_profile_sha256, claim_id, progress_owner, limits=None):
    with _operation(db, limits, progress_owner) as b:
        _current(b, expected_binding, source_inputs_sha256, declaration_profile_sha256)
        value = _claim(b, claim_id)
        declaration = None if value is None else {'digest': value.digest, **value.value()}
        return _receipt(b, expected_binding, source_inputs_sha256, claim_id=claim_id, declaration=declaration)


def lookup_source_dependencies_transaction(db, *, expected_binding, source_inputs_sha256,
        declaration_profile_sha256, kind, ref, progress_owner, limits=None):
    """Return the complete exact address fanout or refuse, never truncate."""
    with _operation(db, limits, progress_owner) as b:
        key = _address(kind, ref)
        _current(b, expected_binding, source_inputs_sha256, declaration_profile_sha256)
        expected = _head(b, key)
        if expected[0] > b.limits.max_claims:
            raise ValueError('source dependency addressed fanout budget exceeded')
        rows = b.rows_from(f'SELECT {_column("claim_id", MAX_ID_BYTES)},{_column("declaration_digest", 64)} '
                          'FROM source_dependency_refs '
                          'WHERE kind=? AND ref_key=? ORDER BY claim_id LIMIT ?', (*key, b.limits.max_claims + 1))
        count = xor = total = 0
        declarations = []
        for claim_id, digest in rows:
            count += 1
            if count > b.limits.max_claims:
                raise ValueError('source dependency addressed fanout budget exceeded')
            if count > expected[0]:
                raise ValueError('source dependency complete addressed aggregate differs')
            declaration = _claim(b, claim_id)
            if declaration is None or declaration.digest != digest:
                raise ValueError('source dependency fanout declaration differs')
            contribution = _contribution(*key, claim_id, digest)
            xor, total = xor ^ contribution, (total + contribution) % _MODULUS
            value = {'digest': digest, **declaration.value()}
            b.diagnostic(value)
            declarations.append(value)
        if (count, xor, total) != expected:
            raise ValueError('source dependency complete addressed aggregate differs')
        # Charge IDs/framing without encoding all retained declarations again;
        # null markers conservatively cover the declaration-array separators.
        claim_ids = [row['claim_id'] for row in declarations]
        b.diagnostic({'claim_ids': claim_ids, 'declarations': [None] * len(declarations)})
        result = _receipt(b, expected_binding, source_inputs_sha256, kind=kind, ref=ref,
                          complete_stored_address_verified=True, checksum=CHECKSUM)
        result.update(claim_ids=claim_ids, declarations=declarations)
        return result


def apply_source_dependency_delta_transaction(db, *, expected_binding, before_source_inputs_sha256,
        after_source_inputs_sha256, new_source_revision, declaration_profile_sha256,
        changes, progress_owner, limits=None):
    """Stage exact Claim deltas before the caller's all-lane publisher.

    The selected source/graph binding stays the predecessor while this index
    is pending. Ordinary lookup refuses pending state. Even an empty delta must
    be finalized after the source and prepared binding advance in this SAME
    transaction; committing a pending index is a caller error, not a success.
    """
    with _operation(db, limits, progress_owner) as b:
        state = _current(b, expected_binding, before_source_inputs_sha256, declaration_profile_sha256)
        state_digest = _digest(_json_bytes(state, b.limits.max_state_bytes))
        _sha(after_source_inputs_sha256)
        _sha(new_source_revision)
        if expected_binding['publication_epoch'] >= MAX_ORDER:
            raise ValueError('source dependency publication epoch exhausted')
        frames, seen, selected_bytes = [], set(), 0
        for change in changes:
            if len(frames) >= b.limits.max_changes:
                raise ValueError('source dependency delta Claim count budget exceeded')
            if type(change) is not SourceDependencyChange or change.operation not in ('insert', 'update', 'delete'):
                raise ValueError('explicit source dependency change required')
            claim_id = _identifier(change.claim_id)
            if claim_id in seen:
                raise ValueError('duplicate source dependency delta target')
            seen.add(claim_id)
            before = _claim(b, claim_id)
            if change.operation == 'insert':
                if before is not None or change.before_digest is not None:
                    raise ValueError('source dependency insertion predecessor differs')
            elif before is None or before.digest != _sha(change.before_digest):
                raise ValueError('source dependency stale Claim precondition')
            if change.operation == 'delete':
                if change.declaration is not None:
                    raise ValueError('source dependency deletion cannot replace declaration')
                after = None
            else:
                after = _declaration(b, change.declaration)
                if after.claim_id != claim_id:
                    raise ValueError('source dependency replacement Claim identity differs')
            selected_bytes += (0 if before is None else len(before.raw)) + (0 if after is None else len(after.raw))
            if selected_bytes > b.limits.max_change_bytes:
                raise ValueError('source dependency selected delta byte budget exceeded')
            frames.append((claim_id, before, after))
        # A caller iterator may have yielded from mutable external code. Recheck
        # both selected bindings and the state CAS before the first lane write.
        if _current(b, expected_binding, before_source_inputs_sha256, declaration_profile_sha256) != state:
            raise ValueError('source dependency selection changed during delta capture')
        for claim_id, before, after in frames:
            if before is not None:
                _delete(b, before)
                state['claim_count'] -= 1
                state['dependency_count'] -= len(before.value()['dependencies'])
            if after is not None:
                _insert(b, after)
                state['claim_count'] += 1
                state['dependency_count'] += len(after.value()['dependencies'])
            b.execute('INSERT INTO source_dependency_pending VALUES (?,?)', (claim_id, None if after is None else after.digest))
        state.update(pending=True, pending_count=len(frames), next_source_inputs_sha256=after_source_inputs_sha256,
                     next_source_revision=new_source_revision, next_epoch=expected_binding['publication_epoch'] + 1)
        _put_state(b, state, before_digest=state_digest)
        return _receipt(b, expected_binding, before_source_inputs_sha256, pending=True,
                        changed_claims=len(frames), next_source_inputs_sha256=after_source_inputs_sha256,
                        finalize_sql_mutations_upper_bound=len(frames) + 1)


def verify_source_dependency_binding_transaction(db, *, new_binding, source_inputs_sha256,
        declaration_profile_sha256, progress_owner, limits=None):
    """Finalize only after both source-root and prepared rows are caller-paired."""
    with _operation(db, limits, progress_owner) as b:
        _selected(b, new_binding, source_inputs_sha256)
        state = _state(b, declaration_profile_sha256)
        old_digest = _digest(_json_bytes(state, b.limits.max_state_bytes))
        if not state['pending']:
            if state['binding'] != new_binding or state['source_inputs_sha256'] != source_inputs_sha256:
                raise ValueError('source dependency final binding differs')
            return _receipt(b, new_binding, source_inputs_sha256, pending=False)
        if (state['next_source_inputs_sha256'] != source_inputs_sha256
                or state['next_source_revision'] != new_binding['source_revision']
                or state['next_epoch'] != new_binding['publication_epoch']
                or state['binding']['normalization_binding'] != new_binding['normalization_binding']):
            raise ValueError('source dependency pending final binding differs')
        if state['pending_count'] > b.limits.max_changes:
            raise ValueError('source dependency pending Claim budget exceeded')
        pending = list(b.rows_from(f'SELECT {_column("claim_id", MAX_ID_BYTES)},digest IS NULL,'
                                  f'{_column("digest", 64)} '
                                  'FROM source_dependency_pending ORDER BY claim_id LIMIT ?',
                                  (b.limits.max_changes + 1,))) if state['pending_count'] else []
        if not state['pending_count'] and b.one('SELECT 1 FROM source_dependency_pending LIMIT 1'):
            raise ValueError('source dependency orphan pending declaration')
        if len(pending) != state['pending_count']:
            raise ValueError('source dependency pending declarations incomplete')
        for claim_id, missing, digest in pending:
            if not missing:
                _sha(digest)
            value = _claim(b, claim_id)
            if (None if value is None else value.digest) != digest:
                raise ValueError('source dependency final declaration differs')
            b.execute('DELETE FROM source_dependency_pending WHERE claim_id=?', (claim_id,))
        state.update(binding=new_binding, source_inputs_sha256=source_inputs_sha256, pending=False, pending_count=0)
        for key in ('next_source_inputs_sha256', 'next_source_revision', 'next_epoch'):
            del state[key]
        _put_state(b, state, before_digest=old_digest)
        return _receipt(b, new_binding, source_inputs_sha256, pending=False, finalized_claims=len(pending))
