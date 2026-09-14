"""Exact list-membership selection over admitted normalized view/layer fields.

This optional offline index never interprets arbitrary JSON or changes query
semantics. Unsupported filters retain the bounded native plan.
"""
from dataclasses import dataclass

from .published_read_metadata import _compact, emitted_row_digest, published_row_digest_key, published_snapshot_binding, TOP_KEY
from .published_read_model import _json, PublishedReadModelError

SCHEMA = 'tos_lens_membership_index_v1'
TABLE = 'knowledge_lens_memberships'
STATE = 'knowledge_lens_membership_state'
FIELDS = ('view_ids', 'graph_layers')
ORDER_INDEX = 'knowledge_lens_memberships_order'
# Expanded Boolean SQL repeats each condition in every ordered driver. Bound
# that product before constructing SQL; overflow keeps the native budget path.
MAX_PLAN_TERMS = 32
MAX_PLAN_DRIVERS = 16
MAX_PLAN_BINDINGS = 2048


def validate_state(query, binding):
    if not query("SELECT name FROM sqlite_master WHERE type='table' AND name=?", (STATE,)):
        return False
    rows = query(f'SELECT schema,binding,valid FROM {STATE} WHERE singleton=1 LIMIT 2')
    if len(rows) != 1 or tuple(rows[0]) != (SCHEMA, _compact(binding), 1):
        raise PublishedReadModelError('lens membership index is stale or incompatible')
    return True


def put(db, kind, identifier, raw):
    db.execute(f'DELETE FROM {TABLE} WHERE kind=? AND id=?', (kind, identifier))
    if raw is None:
        return 0
    item = _json(raw)
    if item.get('id') != identifier:
        raise PublishedReadModelError('membership index identity differs')
    rows = []
    for field in FIELDS:
        values = item.get(field)
        if (not isinstance(values, list) or len(values) > 256
                or any(not isinstance(value, str) or len(value.encode('utf-8')) > 4096 for value in values)):
            raise PublishedReadModelError('membership index requires bounded normalized string arrays')
        rows.extend((kind, field, value, identifier, identifier.lower()) for value in sorted(set(values)))
    db.executemany(f'INSERT INTO {TABLE} VALUES(?,?,?,?,?)', rows)
    return len(rows)


def seal(db, binding):
    db.execute(f'UPDATE {STATE} SET binding=?,valid=1 WHERE singleton=1', (_compact(binding),))


def prepare_membership_index_transaction(db, *, expected_binding, max_rows=200000,
                                         max_source_bytes=64*1024**2, max_entries=1000000):
    """Bounded complete installation; reserve storage and rollback on failure."""
    from .prepared_publication import _metadata, SCHEMA as PREPARED_SCHEMA
    if not db.in_transaction:
        raise ValueError('membership preparation requires an explicit transaction')
    if any(type(value) is not int or value < 1 for value in (max_rows, max_source_bytes, max_entries)):
        raise ValueError('membership budgets must be positive integers')
    top = _metadata(db, TOP_KEY)
    epoch = db.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()[0]
    if top['read_model_schema'] != PREPARED_SCHEMA or published_snapshot_binding(top, epoch) != expected_binding:
        raise PublishedReadModelError('membership publication binding differs')
    if db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name=?", (STATE,)).fetchone():
        raise ValueError('membership index already exists; explicit migration required')
    db.execute(f'CREATE TABLE {TABLE}(kind TEXT NOT NULL,field TEXT NOT NULL,value TEXT NOT NULL,'
               'id TEXT NOT NULL,sort_key TEXT NOT NULL,PRIMARY KEY(kind,field,value,id))')
    db.execute(f'CREATE INDEX {ORDER_INDEX} ON {TABLE}(kind,field,value,sort_key,id)')
    db.execute(f'CREATE INDEX knowledge_lens_memberships_row ON {TABLE}(kind,id)')
    db.execute(f'CREATE TABLE {STATE}(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema TEXT NOT NULL,'
               'binding TEXT NOT NULL,valid INTEGER NOT NULL CHECK(valid IN (0,1)))')
    db.execute(f'INSERT INTO {STATE} VALUES(1,?,?,0)', (SCHEMA, _compact(expected_binding)))
    for table in ('knowledge_nodes', 'knowledge_relations', TABLE):
        for action in ('INSERT', 'UPDATE', 'DELETE'):
            db.execute(f'CREATE TRIGGER membership_{table}_{action.lower()} AFTER {action} ON {table} '
                       f'BEGIN UPDATE {STATE} SET valid=0 WHERE singleton=1; END')
    count = source_bytes = entries = 0
    for kind in ('node', 'relation'):
        for identifier, raw in db.execute(f'SELECT id,CASE WHEN length(CAST(json AS BLOB))<=1048576 '
                                         f'THEN json ELSE NULL END FROM knowledge_{kind}s ORDER BY id'):
            if not isinstance(raw, str):
                raise PublishedReadModelError('membership source exceeds row budget')
            count += 1
            source_bytes += len(raw.encode('utf-8'))
            if count > max_rows or source_bytes > max_source_bytes:
                raise PublishedReadModelError('membership preparation exceeds source budget')
            if _metadata(db, published_row_digest_key(kind, identifier)) != emitted_row_digest(raw):
                raise PublishedReadModelError('membership source checksum differs')
            entries += put(db, kind, identifier, raw)
            if entries > max_entries:
                raise PublishedReadModelError('membership preparation exceeds entry budget')
    seal(db, expected_binding)
    return {'rows': count, 'source_bytes': source_bytes, 'entries': entries}


@dataclass(frozen=True)
class MembershipPlan:
    kind: str
    mode: str
    terms: tuple  # (field, all|any, tuple of exact strings)

    @property
    def drivers(self):
        # Every result must belong to at least one driving posting range.
        terms = self.terms[:1] if self.mode == 'all' else self.terms
        return tuple(dict.fromkeys((field, value) for field, mode, values in terms
                                   for value in (values[:1] if mode == 'all' else values)))

    def condition(self, alias):
        groups, args = [], []
        for field, mode, values in self.terms:
            parts = []
            for value in values:
                parts.append(f'EXISTS (SELECT 1 FROM {TABLE} m WHERE m.kind=? AND m.field=? AND m.value=? AND m.id={alias}.id)')
                args.extend((self.kind, field, value))
            groups.append('(' + (' AND ' if mode == 'all' else ' OR ').join(parts) + ')')
        return '(' + (' AND ' if self.mode == 'all' else ' OR ').join(groups) + ')', args

    def count(self, read, where, args):
        branches = [f'SELECT id FROM {TABLE} WHERE kind=? AND field=? AND value=?' for _ in self.drivers]
        values = [component for field, value in self.drivers for component in (self.kind, field, value)]
        return read.query('WITH candidates AS (' + ' UNION '.join(branches) + ') '
            f'SELECT count(*) AS total FROM candidates c CROSS JOIN knowledge_{self.kind}s r ON r.id=c.id WHERE {where}',
            (*values, *args))[0]['total']

    def ordered(self, read, where, args, block):
        after = '', ''
        while True:
            branches, values = [], []
            for field, value in self.drivers:
                branches.append(f'SELECT id,sort_key FROM (SELECT m.id,m.sort_key FROM {TABLE} m INDEXED BY {ORDER_INDEX} '
                    f'CROSS JOIN knowledge_{self.kind}s r ON r.id=m.id '
                    'WHERE m.kind=? AND m.field=? AND m.value=? AND (m.sort_key,m.id)>(?,?) '
                    f'AND {where} ORDER BY m.sort_key,m.id LIMIT ?)')
                values.extend((self.kind, field, value, *after, *args, block))
            columns = ',r.from_id,r.to_id' if self.kind == 'relation' else ''
            rows = read.query('WITH candidates AS (' + ' UNION '.join(branches) + ') '
                f'SELECT c.id,c.sort_key{columns} FROM candidates c CROSS JOIN knowledge_{self.kind}s r ON r.id=c.id '
                'ORDER BY c.sort_key,c.id LIMIT ?', (*values, block))
            if not rows:
                return
            for row in rows:
                if row['sort_key'] != row['id'].lower():
                    raise PublishedReadModelError('membership ordering differs from native order')
                yield dict(row)
            after = rows[-1]['sort_key'], rows[-1]['id']


def compile_plan(kind, group):
    """Only exact positive membership groups; no inferred missing/negative state."""
    if not group['enabled'] or not group['filters']:
        return None
    terms = []
    for rule in group['filters']:
        if rule.get('_property_binding') or rule.get('field') not in FIELDS or rule['op'] not in ('eq', 'in', 'contains'):
            return None
        value = rule['value']
        if rule['op'] == 'eq' and not isinstance(value, str):
            return None  # Native equality of two arrays is not membership.
        values = value if isinstance(value, list) else [value]
        if not values or any(not isinstance(entry, str) for entry in values):
            return None  # Empty contains is universal, not an empty posting.
        terms.append((rule['field'], 'all' if rule['op'] == 'contains' else 'any', tuple(sorted(set(values)))))
    plan = MembershipPlan(kind, group['match'], tuple(terms))
    predicates = sum(len(values) for _, _, values in terms)
    if (predicates > MAX_PLAN_TERMS or len(plan.drivers) > MAX_PLAN_DRIVERS
            or len(plan.drivers) * (3 * predicates + 8) > MAX_PLAN_BINDINGS):
        return None
    return plan
