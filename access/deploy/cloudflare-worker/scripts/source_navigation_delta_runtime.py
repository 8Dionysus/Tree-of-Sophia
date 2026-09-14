"""Maintain the native navigation product for an exact prepared source delta.

The admitted D1/prepared baseline remains a caller prerequisite. An absent
native product stays absent, never becoming a partial, apparently complete
source dossier. Installed products preserve all owned row bytes and rights.
"""
from tos_access.projection_diff import DiffLimits, diff_projection_snapshots
from tos_access.projection_store import canonical_bytes, _strict_json
from tos_access.portable_paths import normalize_paths

import source_navigation_rows as projection


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
        columns = ','.join(projection.COLUMNS[payload_table])
        payload = capture.one(db, 'json_group_array(json(row))',
            f'FROM (SELECT json_array({columns}) AS row FROM {payload_table} '
            'WHERE id=? ORDER BY part LIMIT 257)', (identifier,),
            maximum=capture.limits.max_metadata_bytes)
        if len(payload) > 256 or [row[1] for row in payload] != list(range(len(payload))):
            raise ValueError('native navigation predecessor payload framing differs')
        old_value = None
        if actual is not None:
            encoded = actual[projection.COLUMNS[table].index('json')]
            if (encoded == '') != bool(payload):
                raise ValueError('native navigation predecessor payload selection differs')
            old_value = _strict_json(encoded if encoded else ''.join(row[2] for row in payload))
            if canonical_bytes(old_value) != canonical_bytes(old['row']):
                raise ValueError('native navigation predecessor source bytes differ')
        elif payload:
            raise ValueError('native navigation predecessor has orphan payload')
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
        expected = previous.get(table, [])
        if expected != ([] if actual is None else [actual]):
            raise ValueError('native navigation predecessor serving row differs')
        expected_payload = previous.get(payload_table, [])
        if payload != [list(row) for row in expected_payload] or len(payload) > 256:
            raise ValueError('native navigation predecessor payload differs')
        for rows, destination in ((previous, before_rows), (successor, after_rows)):
            for selected_table, tuples in rows.items():
                for values in tuples:
                    destination.put(selected_table, values)
    next_top = {**top, 'counts': {**top['counts'], **new_counts}}
    for row in chunks:
        before_rows.put('edge_meta', row)
    return {'state': 'maintained', 'changed_rows': len(changes), 'top': next_top}
