"""Maintain the native navigation product for an exact prepared source delta.

The admitted D1/prepared baseline remains a caller prerequisite. An absent
native product stays absent, never becoming a partial, apparently complete
source dossier. Installed products preserve all owned row bytes and rights.
"""
from tos_access.projection_diff import DiffLimits, diff_projection_snapshots
from tos_access.projection_store import canonical_bytes, _strict_json
from tos_access.portable_paths import normalize_paths
from tos_access.published_read_metadata import (
    SOURCE_NAVIGATION_HEADER_DIGEST_KEY,
    emitted_row_digest,
    published_source_navigation_digest_key,
)

import source_navigation_rows as projection


def _capture_payload(db, capture, table, identifier):
    """Seek one addressed payload within the caller's byte and row budgets."""
    columns = ','.join(projection.COLUMNS[table])
    payload, retained_bytes = [], 0
    while True:
        tail = f'FROM {table} WHERE id=?'
        args = (identifier,)
        if payload:
            tail += ' AND part>?'
            args += (payload[-1][1],)
        tail += ' ORDER BY part LIMIT 1'
        remaining = min(capture.limits.max_row_bytes,
                        capture.limits.max_metadata_bytes - retained_bytes,
                        capture.limits.max_read_bytes - capture.read_bytes)
        if len(payload) >= capture.limits.max_rows or remaining <= 0:
            if db.execute('SELECT 1 ' + tail, args).fetchone() is not None:
                raise ValueError('native navigation payload capture budget exceeded')
            break
        before_bytes = capture.read_bytes
        row = capture.one(db, f'json_array({columns})', tail, args, maximum=remaining)
        if row is None:
            break
        if type(row[1]) is not int or row[1] != len(payload):
            raise ValueError('native navigation predecessor payload framing differs')
        retained_bytes += capture.read_bytes - before_bytes
        payload.append(row)
    return payload


def capture_transition(db, capture, before, after, before_rows, after_rows, *,
                       repo_root, max_changes):
    if before.snapshot_digest == after.snapshot_digest:
        return {'state': 'unchanged', 'changed_rows': 0}
    old_root, new_root = _strict_json(before.root_bytes), _strict_json(after.root_bytes)
    if old_root['logical_schema'] not in ('tos_source_navigation_v1', 'tos_agent_source_navigation_rows_v1'):
        raise ValueError('unsupported raw source-navigation profile')
    for root in (old_root, new_root):
        for name, spec in root['collections'].items():
            table = 'source_navigation_' + name
            if table not in projection.COLUMNS:
                raise ValueError('unsupported raw source-navigation collection')
            key = projection.COLUMNS[table][0]
            if spec['key_field'] != key or spec['order_fields'] != [key]:
                raise ValueError('raw source-navigation identity/order profile differs')
    remaining = capture.limits.max_read_bytes - capture.read_bytes
    packet = diff_projection_snapshots(before, after,
        expected_before_sha256=before.snapshot_digest,
        expected_after_sha256=after.snapshot_digest,
        trusted_baseline_sha256=before.snapshot_digest,
        limits=DiffLimits(max_opened_parts=min(256, max_changes * 4),
            max_decoded_bytes=remaining, max_keys=max_changes * 16,
            max_output_bytes=min(remaining, capture.limits.max_retained_bytes)),
        include_rows=True)
    capture.read_bytes += packet['accounting']['decoded_bytes']
    changes = packet['changes']
    if not 1 <= len(changes) <= max_changes:
        raise ValueError('native source-navigation change budget exceeded')
    old_manifest, new_manifest = before.metadata(), after.metadata()
    without_counts = lambda header: {key: value for key, value in header.items() if key != 'counts'}
    if canonical_bytes(without_counts(old_manifest)) != canonical_bytes(without_counts(new_manifest)):
        raise ValueError('native navigation header policy changed; explicit migration required')
    if any(change['collection'] not in ('nodes', 'edges', 'rights') for change in changes):
        raise ValueError('unhandled source-navigation collection changed')
    top, chunks = capture.metadata(db, 'source_navigation_top')
    if top == {}:
        # The full producer explicitly emits {} for an unavailable navigation
        # product. Empty metadata with retained rows is inconsistent, not an
        # optional capability that can safely be ignored.
        if any(db.execute(f'SELECT 1 FROM {table} LIMIT 1').fetchone()
               for table in projection.COLUMNS):
            raise ValueError('unavailable native navigation contains serving rows')
        return {'state': 'unavailable', 'changed_rows': len(changes),
                'native_product_created': False}
    try:
        header_digest, _ = capture.metadata(db, SOURCE_NAVIGATION_HEADER_DIGEST_KEY)
    except ValueError as exc:
        raise ValueError(
            'native navigation header digest requires explicit product migration'
        ) from exc
    header_raw = ''.join(row[2] for row in chunks)
    if header_digest != emitted_row_digest(header_raw):
        raise ValueError(
            'native navigation header digest differs; explicit product migration required'
        )
    if top.get('schema_version') != 'tos_source_navigation_v1':
        raise ValueError('unsupported native source-navigation product')
    # Metadata headers are already bounded by Capture. Counts certify no new
    # corpus scan: applicability still comes from the admitted baseline pair.
    old_counts = {key: spec['root']['count'] for key, spec in old_root['collections'].items()}
    new_counts = {key: spec['root']['count'] for key, spec in new_root['collections'].items()}
    if not isinstance(top.get('counts'), dict) or any(
            top['counts'].get(key) != value for key, value in old_counts.items()):
        raise ValueError('native navigation and admitted raw predecessor counts differ')
    for change in changes:
        collection, identifier = change['collection'], change['key']
        table = 'source_navigation_' + collection
        payload_table = {'nodes': 'source_navigation_node_payload',
                         'edges': 'source_navigation_edge_payload',
                         'rights': 'source_navigation_rights_payload'}[collection]
        actual = capture.tuple(db, table, (identifier,))
        old, new = change['before'], change['after']
        if old['present'] != (actual is not None):
            raise ValueError('native navigation predecessor identity differs')
        payload = _capture_payload(db, capture, payload_table, identifier)
        old_value = None
        predecessor_digest_chunks = []
        if actual is not None:
            encoded = actual[projection.COLUMNS[table].index('json')]
            if (encoded == '') != bool(payload):
                raise ValueError('native navigation predecessor payload selection differs')
            old_raw = encoded if encoded else ''.join(row[2] for row in payload)
            old_value = _strict_json(old_raw)
            if canonical_bytes(old_value) != canonical_bytes(old['row']):
                raise ValueError('native navigation predecessor source bytes differ')
            digest_key = published_source_navigation_digest_key(collection, identifier)
            try:
                predecessor_digest, predecessor_digest_chunks = capture.metadata(db, digest_key)
            except ValueError as exc:
                raise ValueError(
                    'native navigation predecessor digest requires explicit product migration'
                ) from exc
            if predecessor_digest != emitted_row_digest(old_raw):
                raise ValueError(
                    'native navigation predecessor digest differs; explicit product migration required'
                )
        elif payload:
            raise ValueError('native navigation predecessor has orphan payload')
        elif db.execute('SELECT 1 FROM edge_meta WHERE key=? LIMIT 1',
                (published_source_navigation_digest_key(collection, identifier),)).fetchone():
            raise ValueError('native navigation predecessor has orphan digest')
        # ord is retained legacy storage, not source-navigation query order.
        # Native readers order by stable IDs; insertion never renumbers O(N)
        # unrelated rows merely to recreate a full emitter's positional field.
        ordinal = 0 if actual is None else actual[projection.COLUMNS[table].index('ord')]
        projected = []
        for index, side in enumerate((old, new)):
            if not side['present']:
                projected.append({})
                continue
            # Preserve the predecessor's JSON serialization for exact reversal,
            # after checking its value against the immutable raw source above.
            value = old_value if index == 0 else side['row']
            if normalize_paths(value, repo_root) != value:
                raise ValueError('native source row requires portable-path migration')
            projected.append(projection.project_rows(collection, ordinal, value, repo_root=repo_root))
        previous, successor = projected
        if actual is not None:
            # Preserve the admitted predecessor's exact metadata framing. The
            # successor companion is produced from the successor's emitted
            # JSON by ``project_rows`` below; never backfill a missing old
            # checksum from the row itself.
            previous['edge_meta'] = [tuple(row) for row in predecessor_digest_chunks]
        expected = previous.get(table, [])
        if expected != ([] if actual is None else [actual]):
            raise ValueError('native navigation predecessor serving row differs')
        expected_payload = previous.get(payload_table, [])
        if payload != [list(row) for row in expected_payload]:
            raise ValueError('native navigation predecessor payload differs')
        for rows, destination in ((previous, before_rows), (successor, after_rows)):
            for selected_table, tuples in rows.items():
                for values in tuples:
                    destination.put(selected_table, values)
    next_top = {**top, 'counts': {**top['counts'], **new_counts}}
    for row in chunks:
        before_rows.put('edge_meta', row)
    return {'state': 'maintained', 'changed_rows': len(changes), 'top': next_top}
