"""Optional offline compact input store; never creates DDL during a request.

The publisher maintains both forms in one transaction. Base-table triggers
invalidate this store for older writers; checksums do not grant source authority.
"""
from dataclasses import dataclass

from .compact_lens_carrier import SCHEMA, MAX_ROW_BYTES, compact_lens_carrier
from .published_read_metadata import (_compact, emitted_row_digest, published_row_digest_key,
                                     published_snapshot_binding, TOP_KEY)
from .published_read_model import _json, PublishedReadModelError

TABLE = 'knowledge_compact_lens'
STATE = 'knowledge_compact_lens_state'


def schema_statements():
    """Owned optional-store DDL shared by explicit local and full D1 writers."""
    statements = [f'CREATE TABLE {TABLE}(kind TEXT NOT NULL,id TEXT NOT NULL,source_sha256 TEXT NOT NULL,'
                  'seed_sha256 TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id))',
                  f'CREATE TABLE {STATE}(singleton INTEGER PRIMARY KEY CHECK(singleton=1),'
                  'schema TEXT NOT NULL,binding TEXT NOT NULL,valid INTEGER NOT NULL CHECK(valid IN (0,1)))']
    for kind in ('node', 'relation'):
        for action in ('INSERT', 'UPDATE', 'DELETE'):
            statements.append(f'CREATE TRIGGER compact_lens_{kind}_{action.lower()} AFTER {action} ON knowledge_{kind}s '
                              f'BEGIN UPDATE {STATE} SET valid=0 WHERE singleton=1; END')
    return statements


@dataclass(frozen=True)
class CompactStoreLimits:
    max_rows: int = 200000
    max_source_bytes: int = 64 * 1024 * 1024
    max_seed_bytes: int = 32 * 1024 * 1024

    def __post_init__(self):
        if any(type(v) is not int or v < 1 for v in vars(self).values()):
            raise ValueError('compact store budgets must be positive integers')


def _exists(query):
    return bool(query("SELECT name FROM sqlite_master WHERE type='table' AND name=?", (STATE,)))


def validate_state(query, binding):
    """Return False only for an absent optional lane, never for a stale one."""
    if not _exists(query):
        return False
    rows = query(f'SELECT schema,binding,valid FROM {STATE} WHERE singleton=1 LIMIT 2')
    if (len(rows) != 1 or tuple(rows[0]) != (SCHEMA, _compact(binding), 1)):
        raise PublishedReadModelError('compact lens store is stale or incompatible')
    return True


def put(db, kind, identifier, raw):
    if raw is None:
        db.execute(f'DELETE FROM {TABLE} WHERE kind=? AND id=?', (kind, identifier))
        return 0
    carrier = compact_lens_carrier(kind, raw)
    if carrier.identifier != identifier:
        raise PublishedReadModelError('compact lens addressed identity differs')
    db.execute(f'INSERT OR REPLACE INTO {TABLE} VALUES (?,?,?,?,?)',
               (kind, identifier, carrier.source_sha256, carrier.seed_sha256, carrier.seed_json))
    return len(carrier.seed_json.encode('utf-8'))


def seal(db, binding):
    db.execute(f'UPDATE {STATE} SET binding=?,valid=1 WHERE singleton=1', (_compact(binding),))


def prepare_compact_lens_store_transaction(db, *, expected_binding, limits=None,
                                         expected_read_model_schema='tos_local_prepared_read_model_v1'):
    """Explicit, bounded complete bootstrap; caller MUST rollback on failure.

    Source bytes and digests are read together inside the caller's transaction.
    Existing stores are not silently rebuilt or adopted. Reserve disk capacity
    and configure SQLite's file cap separately before a large installation.
    """
    from .prepared_publication import _metadata, SCHEMA as PREPARED_SCHEMA
    if not db.in_transaction:
        raise ValueError('compact store preparation requires an explicit transaction')
    limits = limits or CompactStoreLimits()
    top = _metadata(db, TOP_KEY)
    epoch = db.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()[0]
    if (expected_read_model_schema not in {PREPARED_SCHEMA, 'tos_cloudflare_edge_read_model_v9'}
            or top['read_model_schema'] != expected_read_model_schema
            or published_snapshot_binding(top, epoch) != expected_binding):
        raise PublishedReadModelError('compact store publication binding differs')
    if _exists(lambda sql, args=(): db.execute(sql, args).fetchall()):
        raise ValueError('compact store already exists; explicit migration required')
    for statement in schema_statements():
        db.execute(statement)
    db.execute(f'INSERT INTO {STATE} VALUES(1,?,?,0)', (SCHEMA, _compact(expected_binding)))
    count = source_bytes = seed_bytes = 0
    for kind in ('node', 'relation'):
        for identifier, raw in db.execute(f'SELECT id,CASE WHEN length(CAST(json AS BLOB))<=? '
                                         f'THEN json ELSE NULL END FROM knowledge_{kind}s ORDER BY id', (MAX_ROW_BYTES,)):
            if not isinstance(raw, str):
                raise PublishedReadModelError('compact source exceeds row budget')
            count += 1
            source_bytes += len(raw.encode('utf-8'))
            if count > limits.max_rows or source_bytes > limits.max_source_bytes:
                raise PublishedReadModelError('compact store bootstrap exceeds input budget')
            if _metadata(db, published_row_digest_key(kind, identifier)) != emitted_row_digest(raw):
                raise PublishedReadModelError('compact source row checksum differs')
            seed_bytes += put(db, kind, identifier, raw)
            if seed_bytes > limits.max_seed_bytes:
                raise PublishedReadModelError('compact store bootstrap exceeds seed budget')
    seal(db, expected_binding)
    return {'rows': count, 'source_bytes': source_bytes, 'seed_bytes': seed_bytes}


def read_items(read, kind, ids, before_parse):
    """Read internal seeds only after transaction-local state admission."""
    from .prepared_publication import _COLUMNS
    columns = _COLUMNS[kind]
    indexed = ','.join(f'n.{key} AS indexed_{key}' for key in columns)
    rows = read.query(f'SELECT c.id,c.source_sha256,c.seed_sha256,c.json,{indexed} FROM {TABLE} c '
                      f'LEFT JOIN knowledge_{kind}s n ON n.id=c.id '
                      'WHERE c.kind=? AND c.id IN (SELECT value FROM json_each(?)) ORDER BY c.id LIMIT ?',
                      (kind, _compact(ids), len(ids) + 1))
    if len(rows) != len(ids):
        raise PublishedReadModelError('compact lens selected row is missing')
    result = []
    for row in rows:
        raw = read.text(row['json'], MAX_ROW_BYTES)
        _, digest = read.metadata(published_row_digest_key(kind, row['id']), 1024)
        if (digest != {'sha256': row['source_sha256']}
                or emitted_row_digest(raw) != {'sha256': row['seed_sha256']}):
            raise PublishedReadModelError('compact lens source or seed checksum differs')
        before_parse(raw)
        item = _json(raw)
        if (not isinstance(item, dict) or item.get('id') != row['id']
                or any(str(item.get(key) or '') != row['indexed_' + key] for key in columns)):
            raise PublishedReadModelError('compact lens identity/index columns differ')
        result.append(item)
    return result
