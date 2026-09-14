"""D1 writer support for explicitly installed, independently bound lens stores.

Only derived rows change. The single publication trigger admits its exact
predecessor before mutation and seals the successor using the actual clock.
Reverse publication gets a new epoch, never a resurrected publication identity.
"""
from tos_access.compact_lens_carrier import compact_lens_carrier, SCHEMA as COMPACT_SCHEMA
from tos_access.lens_membership_index import membership_rows, SCHEMA as MEMBERSHIP_SCHEMA
from tos_access.published_read_metadata import _compact, published_snapshot_binding
from tos_access.compact_lens_store import schema_statements as compact_schema
from tos_access.lens_membership_index import schema_statements as membership_schema
from tos_access.published_read_model import _json

STORES = {
    'knowledge_compact_lens': ('knowledge_compact_lens_state', COMPACT_SCHEMA),
    'knowledge_lens_memberships': ('knowledge_lens_membership_state', MEMBERSHIP_SCHEMA),
}
PRIMARY_KEYS = {'knowledge_compact_lens': ('kind', 'id'),
                'knowledge_lens_memberships': ('kind', 'field', 'value', 'id')}
COLUMNS = {'knowledge_compact_lens': ('kind', 'id', 'source_sha256', 'seed_sha256', 'json'),
           'knowledge_lens_memberships': ('kind', 'field', 'value', 'id', 'sort_key')}
PUBLICATION_SCHEMA = 'tos_d1_lens_auxiliary_publication_v1'
MAX_PUBLICATION_BYTES = 131072
CLOCK = '(SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1)'


def quote(text):
    return "'" + text.replace("'", "''") + "'"


def binding_expression(top):
    raw = _compact(published_snapshot_binding(top, 0))
    prefix, suffix = raw.split('"publication_epoch":0', 1)
    return quote(prefix + '"publication_epoch":') + '||' + CLOCK + '||' + quote(suffix)


def publication_descriptor(top):
    published_snapshot_binding(top, 0)  # Validate the declared owner header.
    value = {'schema': PUBLICATION_SCHEMA, 'stores': {table: schema for table, (_, schema) in STORES.items()},
             'reader_top': top}
    raw = _compact(value)
    if len(raw.encode('utf-8')) > MAX_PUBLICATION_BYTES:
        raise ValueError('auxiliary baseline publication exceeds byte budget')
    return _json(raw)


def baseline_publication_top(previous):
    value = previous.get('auxiliary_publication') if isinstance(previous, dict) else previous.auxiliary_publication
    if value is None:
        return None  # Historical base-only baseline: explicit initial migration.
    revision = previous['revision'] if isinstance(previous, dict) else previous.revision
    schema = previous['schema'] if isinstance(previous, dict) else previous.schema
    if (not isinstance(value, dict) or set(value) != {'schema', 'stores', 'reader_top'}
            or not isinstance(value['reader_top'], dict)):
        raise ValueError('invalid auxiliary baseline publication')
    top = value['reader_top']
    if value != publication_descriptor(top) or top['data_revision'] != revision or top['read_model_schema'] != schema:
        raise ValueError('auxiliary baseline publication identity differs')
    return top


def staging_schema():
    return [f'DROP TABLE IF EXISTS {table}_next;' for table in STORES] + [
        statement.replace(table, table + '_next', 1) + ';'
        for statement in compact_schema() + membership_schema()
        for table in STORES if statement.startswith(f'CREATE TABLE {table}(')]


def bootstrap_finish(top):
    statements = [f'DROP TABLE IF EXISTS {state};' for state, _ in STORES.values()]
    statements += [statement + ';' for statement in compact_schema() + membership_schema()
                   if not any(statement.startswith(f'CREATE TABLE {table}(') for table in STORES)]
    # Outside a publication trigger, the state table CHECK is the SQL guard.
    # A missing/noninteger/unsafe clock fails the bootstrap, not a silent seal.
    valid = f"CASE WHEN typeof({CLOCK})='integer' AND {CLOCK}>=0 AND {CLOCK}<=9007199254740991 THEN 1 ELSE -1 END"
    statements += [f'INSERT INTO {state} VALUES(1,{quote(schema)},({binding_expression(top)}),{valid});'
                   for state, schema in STORES.values()]
    return statements


def projected_rows(table, kind, identifier, raw):
    if raw is None:
        return []
    if table == 'knowledge_lens_memberships':
        return membership_rows(kind, identifier, raw)
    if table != 'knowledge_compact_lens':
        raise ValueError('unknown lens auxiliary store')
    seed = compact_lens_carrier(kind, raw)
    if seed.identifier != identifier:
        raise ValueError('compact auxiliary identity differs')
    return [(kind, identifier, seed.source_sha256, seed.seed_sha256, seed.seed_json)]


def admit(db, capture, top):
    installed = []
    clock = db.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()
    binding = published_snapshot_binding(top, None if clock is None else clock[0])
    for table, (state, schema) in STORES.items():
        found = {r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name IN (?,?)", (table, state))}
        if not found:
            continue
        if found != {table, state}:
            raise ValueError('incomplete lens auxiliary installation')
        row = capture.one(db, 'json_array(schema,binding,valid)', f'FROM {state} WHERE singleton=1')
        if row != [schema, _compact(binding), 1]:
            raise ValueError('stale lens auxiliary publication')
        installed.append(table)
    return installed


def capture_change(db, capture, table, kind, identifier, old, new, before_rows, after_rows):
    expected = projected_rows(table, kind, identifier, None if old is None else _compact(old))
    positions = [COLUMNS[table].index(key) for key in PRIMARY_KEYS[table]]
    expected.sort(key=lambda row: tuple(row[p] for p in positions))
    # One selected normalized source has at most 512 distinct memberships.
    # Frame the aggregate in SQL before transfer; capture enforces byte limits.
    columns = ','.join(COLUMNS[table])
    actual = capture.one(db, 'json_group_array(json(row))',
        f'FROM (SELECT json_array({columns}) AS row FROM {table} WHERE kind=? AND id=? '
        f'ORDER BY ' + ','.join(PRIMARY_KEYS[table]) + ' LIMIT 513)', (kind, identifier))
    if actual != [list(row) for row in expected]:
        raise ValueError('D1 predecessor lens auxiliary rows differ')
    for row in expected:
        before_rows.put(table, row)
    for row in projected_rows(table, kind, identifier, None if new is None else _compact(new)):
        after_rows.put(table, row)


def publication_operations(bindings):
    """Trusted producer inputs only; return pre-mutation guards and final seals."""
    guards, seals = [], []
    clock = CLOCK
    for table, (before_top, after_top) in bindings.items():
        state, schema = STORES[table]
        guards.append(f"SELECT CASE WHEN typeof({clock})!='integer' OR {clock}<0 OR {clock}>9007199254740991 "
                      f"OR NOT EXISTS(SELECT 1 FROM {state} WHERE singleton=1 AND schema={quote(schema)} "
                      f"AND binding=({binding_expression(before_top)}) AND valid=1) THEN RAISE(ABORT,'stale lens auxiliary publication') END;")
        seals.append(f"SELECT CASE WHEN typeof({clock})!='integer' OR {clock}<0 OR {clock}>9007199254740991 "
                     "THEN RAISE(ABORT,'invalid auxiliary successor epoch') END;")
        seals.append(f'UPDATE {state} SET binding=({binding_expression(after_top)}),valid=1 WHERE singleton=1;')
    return guards, seals
