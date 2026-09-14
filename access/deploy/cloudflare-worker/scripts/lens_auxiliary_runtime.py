"""D1 writer support for explicitly installed, independently bound lens stores.

Only derived rows change. The single publication trigger admits its exact
predecessor before mutation and seals the successor using the actual clock.
Reverse publication gets a new epoch, never a resurrected publication identity.
"""
from tos_access.compact_lens_carrier import compact_lens_carrier, SCHEMA as COMPACT_SCHEMA
from tos_access.lens_membership_index import membership_rows, SCHEMA as MEMBERSHIP_SCHEMA
from tos_access.published_read_metadata import _compact, published_snapshot_binding

STORES = {
    'knowledge_compact_lens': ('knowledge_compact_lens_state', COMPACT_SCHEMA),
    'knowledge_lens_memberships': ('knowledge_lens_membership_state', MEMBERSHIP_SCHEMA),
}
PRIMARY_KEYS = {'knowledge_compact_lens': ('kind', 'id'),
                'knowledge_lens_memberships': ('kind', 'field', 'value', 'id')}
COLUMNS = {'knowledge_compact_lens': ('kind', 'id', 'source_sha256', 'seed_sha256', 'json'),
           'knowledge_lens_memberships': ('kind', 'field', 'value', 'id', 'sort_key')}


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
    clock = '(SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1)'
    quote = lambda text: "'" + text.replace("'", "''") + "'"
    for table, (before_top, after_top) in bindings.items():
        state, schema = STORES[table]
        def expression(top):
            raw = _compact(published_snapshot_binding(top, 0))
            prefix, suffix = raw.split('"publication_epoch":0', 1)
            return quote(prefix + '"publication_epoch":') + '||' + clock + '||' + quote(suffix)
        guards.append(f"SELECT CASE WHEN typeof({clock})!='integer' OR {clock}<0 OR {clock}>9007199254740991 "
                      f"OR NOT EXISTS(SELECT 1 FROM {state} WHERE singleton=1 AND schema={quote(schema)} "
                      f"AND binding=({expression(before_top)}) AND valid=1) THEN RAISE(ABORT,'stale lens auxiliary publication') END;")
        seals.append(f"SELECT CASE WHEN typeof({clock})!='integer' OR {clock}<0 OR {clock}>9007199254740991 "
                     "THEN RAISE(ABORT,'invalid auxiliary successor epoch') END;")
        seals.append(f'UPDATE {state} SET binding=({expression(after_top)}),valid=1 WHERE singleton=1;')
    return guards, seals
