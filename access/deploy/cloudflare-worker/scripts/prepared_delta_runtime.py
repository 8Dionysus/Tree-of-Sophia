"""Offline bibliographic prepared transition to revision-guarded D1 delta SQL.

The caller admits the exact full D1/prepared predecessor pair and holds all
three SQLite snapshots. No source commands, graph build, D1 write or deployment
occurs here. Source/currentness and remote admission remain with their owners.
"""
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path

import build_runtime as full
import lens_auxiliary_runtime as auxiliary
from incremental_runtime import DeltaRecorder, REGISTERED_KEYS as PRIMARY_KEYS, plan_search_addresses_transaction, _search_address_revision
from tos_access.prepared_source_binding import PreparedSourceInputs
from tos_access.published_read_model import _json
from tos_access.published_read_metadata import (
    TOP_KEY, CATALOG_KEY, LENS_META_KEY, _compact, published_snapshot_binding,
    published_reader_metadata, published_row_digest_key, emitted_row_digest, lens_order_row,
)
from tos_access.portable_paths import normalize_paths
from tos_access.search_read_model import SQLiteKnowledgeSearchReadModel as Search

SCHEMA = 'tos_prepared_bibliographic_d1_delta_v1'
NODE_COLUMNS = ('id', 'entity_id', 'native_id', 'source_graph', 'kind_id', 'type_id',
                'title_text', 'summary_text', 'search_text', 'json')
RELATION_COLUMNS = ('id', 'native_id', 'source_graph', 'from_id', 'to_id', 'predicate_id',
                    'relation_type_id', 'label_text', 'explanation_text', 'search_text', 'json')
DOCUMENT_COLUMNS = ('kind', 'position', 'id', 'source_graph', 'kind_id', 'predicate_id', 'id_lower',
                    'native_id_lower', 'identity_values', 'visible_values', 'document_chars', 'document_digest')
COLUMNS = {'edge_meta': ('key', 'part', 'json_chunk'), 'knowledge_nodes': NODE_COLUMNS,
    'knowledge_relations': RELATION_COLUMNS, 'knowledge_search_documents': DOCUMENT_COLUMNS,
    'knowledge_search_grams': ('kind', 'n', 'gram', 'position'),
    'knowledge_search_gram_stats': ('kind', 'n', 'gram', 'postings'),
    'knowledge_lens_order': ('kind', 'id', 'sort_key', 'from_id', 'to_id')}
COLUMNS.update(auxiliary.COLUMNS)


@dataclass(frozen=True)
class PreparedD1DeltaLimits:
    max_changes: int = 512
    max_row_bytes: int = 4 * 1024**2
    max_metadata_bytes: int = 32 * 1024**2
    max_read_bytes: int = 128 * 1024**2
    max_rows: int = 200_000
    max_retained_bytes: int = 128 * 1024**2
    max_sql_bytes: int = 128 * 1024**2
    max_postings: int = 100_000

    def __post_init__(self):
        if any(type(n) is not int or n < 1 for n in vars(self).values()):
            raise ValueError('positive prepared D1 delta budgets required')


class Capture:
    def __init__(self, limits):
        self.limits, self.read_bytes = limits, 0

    def take(self, raw):
        self.read_bytes += len(raw.encode('utf-8'))
        if self.read_bytes > self.limits.max_read_bytes:
            raise ValueError('prepared D1 capture read budget exceeded')
        return raw

    def one(self, db, expression, tail, args=(), maximum=None):
        cap = min(maximum or self.limits.max_row_bytes, self.limits.max_read_bytes - self.read_bytes)
        row = db.execute(f'SELECT CASE WHEN length(CAST(({expression}) AS BLOB))<=? '
                         f'THEN ({expression}) END {tail}', (cap, *args)).fetchone()
        if row is None:
            return None
        if not isinstance(row[0], str):
            raise ValueError('prepared D1 selected value missing or oversized')
        return _json(self.take(row[0]))

    def metadata(self, db, key):
        cap = min(self.limits.max_metadata_bytes, self.limits.max_read_bytes - self.read_bytes)
        rows = db.execute('WITH framed AS (SELECT part,json_chunk, '
            'sum(length(CAST(json_chunk AS BLOB))) OVER (ORDER BY part) AS bytes '
            'FROM edge_meta WHERE key=? ORDER BY part LIMIT 257) '
            'SELECT part,CASE WHEN bytes<=? AND length(CAST(json_chunk AS BLOB))<=131072 '
            'THEN json_chunk END FROM framed ORDER BY part', (key, cap)).fetchall()
        if (not rows or len(rows) > 256 or [r[0] for r in rows] != list(range(len(rows)))
                or any(not isinstance(r[1], str) for r in rows)):
            raise ValueError('prepared D1 metadata missing, incomplete or oversized: ' + key)
        raw = self.take(''.join(r[1] for r in rows))
        value = _json(raw)
        if _compact(value) != raw:
            raise ValueError('prepared D1 metadata framing differs')
        return value, [(key, part, chunk) for part, chunk in rows]

    def tuple(self, db, table, key):
        columns = COLUMNS[table]
        row = self.one(db, 'json_array(' + ','.join(columns) + ')',
            f'FROM {table} WHERE ' + ' AND '.join(name + ' IS ?' for name in PRIMARY_KEYS[table]), key)
        return None if row is None else tuple(row)

    def local(self, db, binding):
        if not db.in_transaction:
            raise ValueError('caller-owned prepared snapshot required')
        top = self.metadata(db, TOP_KEY)[0]
        clock = db.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()
        if (clock is None or published_snapshot_binding(top, clock[0]) != binding
                or top['read_model_schema'] != 'tos_local_prepared_read_model_v1'
                or self.metadata(db, 'data_revision')[0] != {'sha256': binding['data_revision']}):
            raise ValueError('selected prepared binding differs')
        descriptor = self.one(db, 'descriptor', 'FROM prepared_state WHERE singleton=1',
                              maximum=self.limits.max_metadata_bytes)
        if descriptor is None or _sha(_compact(descriptor)) != binding['data_revision']:
            raise ValueError('prepared descriptor digest differs')
        paired = self.one(db, 'json_array(binding,inputs,sha256)', 'FROM prepared_source_state WHERE singleton=1')
        if paired is None or _json(paired[0]) != binding or _sha(paired[1]) != paired[2]:
            raise ValueError('prepared source pairing differs')
        source = PreparedSourceInputs.parse(paired[1].encode('utf-8'))
        if source.value()['source_revision'] != binding['source_revision']:
            raise ValueError('paired source revision differs')
        return top, descriptor, source

    def item(self, db, kind, identifier):
        row = self.one(db, 'json', f'FROM knowledge_{kind}s WHERE id=?', (identifier,))
        if row is not None:
            digest = self.metadata(db, published_row_digest_key(kind, identifier))[0]
            if row.get('id') != identifier or digest != emitted_row_digest(_compact(row)):
                raise ValueError('selected prepared row digest differs')
            if normalize_paths(row, full.REPO_ROOT) != row:
                raise ValueError('prepared row requires a portable-path migration')
        return row


def _sha(text):
    return hashlib.sha256(text.encode('utf-8')).hexdigest()


def _literal(value):
    if isinstance(value, str):
        return full.sql_text(value)
    if type(value) is int:
        return str(value)
    if value is None:
        return 'NULL'
    raise ValueError('unexpected D1 row value type')


class Rows:
    def __init__(self, limits, accounting):
        self.limits, self.data, self.bytes, self.accounting = limits, {}, 0, accounting

    def put(self, table, values):
        values = tuple(values)
        columns = COLUMNS[table]
        if len(values) != len(columns):
            raise ValueError('D1 row shape differs')
        key = tuple(values[columns.index(name)] for name in PRIMARY_KEYS[table])
        selected = self.data.setdefault(table, {})
        if key in selected:
            if selected[key] != values:
                raise ValueError('conflicting captured D1 row')
            return
        size = len(_compact(values).encode('utf-8'))
        if (self.accounting[0] + size > self.limits.max_retained_bytes
                or self.accounting[1] >= self.limits.max_rows):
            raise ValueError('retained D1 delta row budget exceeded')
        self.bytes += size
        self.accounting[0] += size
        self.accounting[1] += 1
        selected[key] = values

    def statements(self, table, values):
        columns, output = COLUMNS[table], []
        selector = ' AND '.join(name + ' IS ' + _literal(values[columns.index(name)]) for name in PRIMARY_KEYS[table])
        chunked = {name: value for name, value in zip(columns, values)
                   if name not in PRIMARY_KEYS[table] and isinstance(value, str)}
        full.append_chunkable_insert(output, table + '_next', columns, tuple(map(_literal, values)),
            selector_sql=selector, chunked_text=chunked)
        return output

    def index(self, revision):
        index = {name: {} for name in PRIMARY_KEYS}
        for table, rows in self.data.items():
            for key, values in rows.items():
                literals = list(map(_literal, key))
                index[table][_compact(literals)] = {'digest': _sha('\n'.join(self.statements(table, values))), 'values': literals}
        return {'schema': full.READ_MODEL_SCHEMA_VERSION, 'revision': revision, 'rows': index}


def _node_row(kind, item):
    display = item.get('display') if isinstance(item.get('display'), dict) else {}
    text = Search._searchable(item)
    title, description = ('title', 'summary') if kind == 'node' else ('label', 'explanation')
    primary = display.get(title) if isinstance(display.get(title), dict) else {}
    secondary = display.get(description) if isinstance(display.get(description), dict) else {}
    columns = NODE_COLUMNS if kind == 'node' else RELATION_COLUMNS
    extras = {title + '_text': str(primary.get('default') or '').lower(),
              description + '_text': str(secondary.get('default') or ''), 'search_text': text, 'json': _compact(item)}
    return tuple(extras[name] if name in extras else str(item.get(name) or '') for name in columns)


def _document(kind, item, position):
    text = Search._searchable(item)
    return (kind + 's', position, item['id'], item['source_graph'], str(item.get('kind_id') or ''),
            str(item.get('predicate_id') or ''), *Search._rank_fields(item, relation=kind == 'relation'),
            len(text), _sha(text)), set(text[i:i + 3] for i in range(len(text) - 2))


def execution_profile():
    root = Path(__file__).resolve().parents[4]
    refs = ('access/deploy/cloudflare-worker/scripts/prepared_delta_runtime.py',
            'access/deploy/cloudflare-worker/scripts/incremental_runtime.py',
            'access/deploy/cloudflare-worker/scripts/build_runtime.py',
            'access/deploy/cloudflare-worker/scripts/lens_auxiliary_runtime.py',
            'access/src/tos_access/compact_lens_carrier.py',
            'access/src/tos_access/compact_lens_store.py',
            'access/src/tos_access/lens_membership_index.py',
            'access/src/tos_access/published_read_model.py',
            'access/src/tos_access/knowledge.py', 'access/src/tos_access/human_form_codec.py',
            'access/src/tos_access/search_read_model.py', 'access/src/tos_access/published_read_metadata.py',
            'access/src/tos_access/portable_paths.py')
    values = {}
    for ref in refs:
        with (root / ref).open('rb') as stream:
            raw = stream.read(1_048_577)
        if len(raw) > 1_048_576:
            raise ValueError('D1 publication implementation byte budget exceeded')
        values[ref] = hashlib.sha256(raw).hexdigest()
    return _sha(_compact(values))


def build_prepared_delta_sql(db, before_db, after_db, target, *, expected_d1_revision,
                            before_binding, after_binding, limits=None, rollback_target=None):
    """Capture one committed bibliographic transition; emit, never apply, SQL.

    A trusted initial D1/prepared pair is an explicit caller prerequisite. Exact
    reader/catalog/lens headers and selected old rows are additionally checked.
    Frozen source-root/dependency checks prevent an unrelated legacy-carrier
    migration from hitchhiking on this knowledge-only publication profile.
    """
    limits = limits or PreparedD1DeltaLimits()
    target = Path(target)
    rollback_target = None if rollback_target is None else Path(rollback_target)
    paths = [target] + ([] if rollback_target is None else [rollback_target])
    all_paths = [p.resolve() for path in paths for p in (path, path.with_name(path.name + '.next'))]
    if len(set(all_paths)) != len(all_paths) or any(p.exists() for p in all_paths):
        raise ValueError('distinct fresh delta SQL targets required')
    _search_address_revision(db, expected_d1_revision)
    profile = execution_profile()
    capture = Capture(limits)
    before_top, before_descriptor, before_source = capture.local(before_db, before_binding)
    _, after_descriptor, after_source = capture.local(after_db, after_binding)
    if (after_descriptor.get('mode') != 'delta-history'
            or after_descriptor.get('parent_data_revision') != before_binding['data_revision']):
        raise ValueError('one exact committed prepared transition required')
    a, b = before_source.value(), after_source.value()
    if (set(a['roots']) != set(b['roots'])
            or any(a['roots'][key] != b['roots'][key] for key in a['roots'] if key not in ('source-catalog', 'bibliographic-claims'))
            or {k: v for k, v in a['dependencies'].items() if k != 'claim-publication-profile'}
               != {k: v for k, v in b['dependencies'].items() if k != 'claim-publication-profile'}):
        raise ValueError('nonparticipating source scope changed; broader D1 migration required')
    old_header, header = before_descriptor['header'], after_descriptor['header']
    if {k: v for k, v in old_header.items() if k not in ('source_revision', 'counts')} != {
            k: v for k, v in header.items() if k not in ('source_revision', 'counts')}:
        raise ValueError('prepared header profile changed; explicit migration required')
    d1_top = capture.metadata(db, TOP_KEY)[0]
    installed_auxiliary = auxiliary.admit(db, capture, d1_top)
    ignored = {'data_revision', 'read_model_schema'}
    if (d1_top['read_model_schema'] != full.READ_MODEL_SCHEMA_VERSION
            or {k: v for k, v in d1_top.items() if k not in ignored}
               != {k: v for k, v in before_top.items() if k not in ignored}):
        raise ValueError('D1 and admitted prepared predecessor headers differ')
    for key in (CATALOG_KEY, LENS_META_KEY):
        if capture.metadata(db, key)[0] != capture.metadata(before_db, key)[0]:
            raise ValueError('D1 and prepared predecessor metadata differ: ' + key)
    frames = after_descriptor.get('changes')
    if not isinstance(frames, list) or not 1 <= len(frames) <= limits.max_changes:
        raise ValueError('prepared transition change budget exceeded')
    selected, original, successors = {}, {}, {}
    for frame in frames:
        if not isinstance(frame, list) or len(frame) != 6:
            raise ValueError('invalid prepared change frame')
        operation, kind, identifier, _, _, digest = frame
        if operation not in ('insert', 'update', 'delete') or kind not in ('node', 'relation') or not isinstance(identifier, str):
            raise ValueError('invalid prepared change identity/operation')
        key = kind, identifier
        if key in selected:
            raise ValueError('duplicate prepared change')
        old, new = capture.item(before_db, *key), capture.item(after_db, *key)
        if ((operation == 'insert') != (old is None) or (operation == 'delete') != (new is None)
                or (emitted_row_digest(_compact(new))['sha256'] if new is not None else None) != digest):
            raise ValueError('prepared change frame differs from exact rows')
        selected[key], original[key], successors[key] = operation, old, new
    accounting = [0, 0]
    before_rows, after_rows, plans, adjustments = Rows(limits, accounting), Rows(limits, accounting), [], Counter()
    posting_count = 0
    for kind in ('node', 'relation'):
        groups, preview_members = {}, 0
        for lower in sorted({identifier.lower() for k, identifier in selected if k == kind}):
            # Read the complete old tie group through the planner, then join its
            # surviving members to exact successor source-order positions.
            empty = plan_search_addresses_transaction(db, expected_revision=expected_d1_revision,
                kind=kind + 's', successor_groups={lower: []}, max_members=max(1, limits.max_changes - preview_members))
            preview_members += len(empty['before'])
            if preview_members > limits.max_changes:
                raise ValueError('complete search tie closure exceeds member budget')
            capture.take(_compact(empty['before']))
            ids = set(empty['before'])
            for (k, identifier), operation in selected.items():
                if k == kind and identifier.lower() == lower:
                    ids.discard(identifier) if operation == 'delete' else ids.add(identifier)
            ranked = []
            for identifier in ids:
                row = after_db.execute('SELECT source_order FROM prepared_documents WHERE kind=? AND id=?', (kind, identifier)).fetchone()
                if row is None or type(row[0]) is not int:
                    raise ValueError('successor search tie member has no source order')
                ranked.append((row[0], identifier))
            groups[lower] = [identifier for _, identifier in sorted(ranked)]
        if not groups:
            continue
        plan = plan_search_addresses_transaction(db, expected_revision=expected_d1_revision,
            kind=kind + 's', successor_groups=groups, max_groups=limits.max_changes, max_members=limits.max_changes)
        plans.append(plan)
        affected = {identifier for k, identifier in selected if k == kind} | set(plan['changed_ids'])
        for identifier in sorted(affected):
            key = kind, identifier
            old = original[key] if key in original else capture.item(before_db, *key)
            new = successors[key] if key in successors else capture.item(after_db, *key)
            actual = capture.tuple(db, 'knowledge_' + kind + 's', (identifier,))
            expected = None if old is None else _node_row(kind, old)
            if actual != expected:
                raise ValueError('D1 predecessor row differs from admitted prepared source')
            if old is not None:
                before_rows.put('knowledge_' + kind + 's', actual)
            if new is not None:
                after_rows.put('knowledge_' + kind + 's', _node_row(kind, new))
            for table in installed_auxiliary:
                auxiliary.capture_change(db, capture, table, kind, identifier, old, new, before_rows, after_rows)
            for table, old_value, new_value in (
                ('knowledge_lens_order', None if old is None else lens_order_row(kind, old), None if new is None else lens_order_row(kind, new)),
            ):
                actual = capture.tuple(db, table, key)
                if actual != old_value:
                    raise ValueError('D1 predecessor lens order differs')
                if actual is not None:
                    before_rows.put(table, actual)
                if new_value is not None:
                    after_rows.put(table, new_value)
            digest_key = published_row_digest_key(kind, identifier)
            if old is not None:
                value, chunks = capture.metadata(db, digest_key)
                if value != emitted_row_digest(_compact(old)):
                    raise ValueError('D1 predecessor digest differs')
                for row in chunks:
                    before_rows.put('edge_meta', row)
            if new is not None:
                after_rows.put('edge_meta', (digest_key, 0, _compact(emitted_row_digest(_compact(new)))))
            for previous, item, position, rows, sign in ((True, old, plan['before'].get(identifier), before_rows, -1),
                                                        (False, new, plan['after'].get(identifier), after_rows, 1)):
                if item is None:
                    continue
                if position is None:
                    raise ValueError('missing exact search address')
                document, grams = _document(kind, item, position)
                if previous and capture.tuple(db, 'knowledge_search_documents', (kind + 's', position)) != document:
                    raise ValueError('D1 predecessor search carrier differs')
                rows.put('knowledge_search_documents', document)
                posting_count += len(grams)
                if posting_count > limits.max_postings:
                    raise ValueError('D1 changed posting budget exceeded')
                for gram in sorted(grams):
                    posting = kind + 's', 3, gram, position
                    if previous and capture.tuple(db, 'knowledge_search_grams', posting) != posting:
                        raise ValueError('D1 predecessor posting is missing')
                    rows.put('knowledge_search_grams', posting)
                    adjustments[posting[:3]] += sign
    for key, adjustment in sorted(adjustments.items()):
        if not adjustment:
            continue
        prior = capture.tuple(db, 'knowledge_search_gram_stats', key)
        count = 0 if prior is None else prior[3]
        if type(count) is not int or count < 0 or count + adjustment < 0:
            raise ValueError('D1 posting count transition is invalid')
        if prior is not None:
            before_rows.put('knowledge_search_gram_stats', prior)
        if count + adjustment:
            after_rows.put('knowledge_search_gram_stats', (*key, count + adjustment))
    lineage = {'schema': SCHEMA, 'implementation_sha256': profile, 'base_d1_revision': expected_d1_revision,
               'before_prepared_binding': before_binding, 'after_prepared_binding': after_binding,
               'before_source_inputs_sha256': before_source.digest, 'after_source_inputs_sha256': after_source.digest}
    revision = _sha(_compact(lineage))
    catalog = capture.metadata(after_db, CATALOG_KEY)[0]
    lens = capture.metadata(after_db, LENS_META_KEY)[0]
    metadata = published_reader_metadata(header, catalog, full.READ_MODEL_SCHEMA_VERSION, revision, lens_metadata=lens)
    metadata.update(data_revision={'sha256': revision}, knowledge_top=header,
        knowledge_exploration_top={'source_revision': header['source_revision'], 'authority_boundary': header['authority_boundary']},
        knowledge_search_top={'schema': full.SEARCH_READ_MODEL_SCHEMA_VERSION, 'source_revision': header['source_revision'],
            'ngram_size': 3, 'matching_counts': 'unknown-until-indexed-page-exhaustion'})
    for key, value in metadata.items():
        _, old_chunks = capture.metadata(db, key)
        for row in old_chunks:
            before_rows.put('edge_meta', row)
        for part, chunk in enumerate(full.chunk_text(_compact(value))):
            after_rows.put('edge_meta', (key, part, chunk))
    # A selected-row index is deliberately internal and never emitted as a
    # complete baseline companion. The existing trigger touches only these keys.
    recorders, sql_bytes = [], 0
    try:
        transitions = [(target, before_rows, after_rows, expected_d1_revision, revision)]
        if rollback_target is not None:
            transitions.append((rollback_target, after_rows, before_rows, revision, expected_d1_revision))
        for path, prior, successor, base, destination in transitions:
            path.parent.mkdir(parents=True, exist_ok=True)
            tops = (d1_top, metadata[TOP_KEY]) if path == target else (metadata[TOP_KEY], d1_top)
            recorder = DeltaRecorder(path, destination, full.READ_MODEL_SCHEMA_VERSION, prior.index(base),
                auxiliary_bindings={table: tops for table in installed_auxiliary})
            recorders.append(recorder)
            for table, rows in successor.data.items():
                for values in rows.values():
                    for statement in successor.statements(table, values):
                        recorder.observe(statement)
                    if sql_bytes + recorder.stream.tell() > limits.max_sql_bytes:
                        raise ValueError('D1 delta SQL byte budget exceeded')
            recorder.finish(publish=False)
            sql_bytes += recorder.pending_path.stat().st_size
            if sql_bytes > limits.max_sql_bytes:
                raise ValueError('D1 delta SQL byte budget exceeded')
        _search_address_revision(db, expected_d1_revision)
        if execution_profile() != profile:
            raise ValueError('D1 publication implementation changed during capture')
        # Publish recovery first. Only a successful return admits the pair;
        # a failed rename may leave a complete unadmitted SQL artifact.
        for recorder in reversed(recorders):
            recorder.publish()
    finally:
        for recorder in recorders:
            if not recorder.stream.closed:
                recorder.stream.close()
    return {**lineage, 'target_d1_revision': revision, 'source_revision': header['source_revision'],
            'changed_prepared_rows': len(frames), 'address_plans': plans, 'delta': recorders[0].summary(),
            'rollback': None if rollback_target is None else recorders[1].summary(),
            'read_bytes': capture.read_bytes, 'retained_bytes': before_rows.bytes + after_rows.bytes,
            'posting_rows_observed': posting_count, 'sql_bytes': target.stat().st_size,
            'rollback_sql_bytes': 0 if rollback_target is None else rollback_target.stat().st_size,
            'prepared_source_pairing_verified': True, 'global_source_currentness_verified': False,
            'maintained_auxiliary_stores': installed_auxiliary,
            'd1_applied': False, 'consumer_switched': False, 'semantic_acceptance': False}
