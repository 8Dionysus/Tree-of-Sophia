"""Bounded native navigation access within a held published-reader snapshot.

Consumes the existing D1 product, not normalized knowledge attributes or a
legacy corpus index. Missing native storage is an explicit unsupported read.
"""
from .published_read_model import PublishedReadModelError, PublishedReadBudgetExceeded, _compact, _json

_SPECS = {
    'nodes': ('node_id', ('node_kind', 'source_ref', 'label', 'identity_status'), 'properties_json', 'node'),
    'edges': ('edge_id', ('from_id', 'to_id', 'edge_kind', 'predicate_id', 'review_status'), 'source_refs_json', 'edge'),
    'rights': ('rights_id', (), 'scope_refs_json', 'rights'),
}
_SEEK_INDEXES = {'source_navigation_edges_from_seek_idx', 'source_navigation_edges_to_seek_idx'}
_BIBLIOGRAPHIC = ('has_expression', 'embodied_by', 'exemplified_by')
_LINKS = ('described_by', 'metadata_at', 'downloadable_at', 'rights_statement_at')


class NativeNavigationView:
    def __init__(self, read):
        self.read = read
        _, self.header = read.metadata('source_navigation_top')
        if (not isinstance(self.header, dict)
                or self.header.get('schema_version') != 'tos_source_navigation_v1'):
            raise PublishedReadModelError('selected publication has no supported native source-navigation product')
        indexes = read.query("SELECT name FROM sqlite_master WHERE type='index' AND name IN (?,?)",
                             tuple(sorted(_SEEK_INDEXES)))
        if {row['name'] for row in indexes} != _SEEK_INDEXES:
            raise PublishedReadModelError('native source-navigation seek indexes are unavailable')
        self._nodes = {}

    def _rows(self, kind, where, args=()):
        key, mirrors, hint, payload = _SPECS[kind]
        maximum = self.read.limits.max_row_bytes
        fields = (key, *mirrors)
        # Bound the body before transferring it out of SQLite, not after a
        # whole wide row has already been materialized in the Python process.
        rows = self.read.query(f'SELECT {",".join(fields)},CASE WHEN length(CAST({hint} AS BLOB))<=? '
            f'THEN {hint} END AS {hint},CASE WHEN length(CAST(json AS BLOB))<=? '
            f'THEN json END AS json FROM source_navigation_{kind} WHERE {where} '
            f'ORDER BY {key} LIMIT ?', (maximum, maximum, *args, self.read.limits.max_rows + 1))
        for row in rows:
            raw = row['json']
            if raw is None or row[hint] is None:
                raise PublishedReadBudgetExceeded('native navigation row exceeds its byte budget')
            if raw == '':
                chunks = self.read.query(f'SELECT part,CASE WHEN length(CAST(json_chunk AS BLOB))<=? '
                    f'THEN json_chunk END AS json_chunk FROM source_navigation_{payload}_payload '
                    'WHERE id=? ORDER BY part LIMIT 257', (min(maximum, 131072), row[key]))
                if not chunks or len(chunks) > 256 or [r['part'] for r in chunks] != list(range(len(chunks))):
                    raise PublishedReadModelError('native navigation payload framing is incomplete')
                if any(r['json_chunk'] is None for r in chunks):
                    raise PublishedReadBudgetExceeded('native navigation payload chunk exceeds its byte budget')
                raw = ''.join(r['json_chunk'] for r in chunks)
            raw = self.read.text(raw, maximum)
            value = _json(raw)
            if (not isinstance(value, dict) or not isinstance(value.get(key), str)
                    or not value[key] or value[key] != row[key]
                    or any(str(value.get(field) or '') != row[field] for field in mirrors)):
                raise PublishedReadModelError('native navigation selection columns differ from source JSON')
            if row[hint] != '':
                selected = _json(row[hint])
                if kind == 'nodes':
                    full = value.get('properties')
                    full = full if isinstance(full, dict) else {}
                    if (not isinstance(selected, dict) or any(
                            (name in selected) != (name in full)
                            or _compact(selected.get(name)) != _compact(full.get(name))
                            for name in ('packet_id', 'access_status'))):
                        raise PublishedReadModelError('native navigation property hint differs')
                else:
                    field = 'source_refs' if kind == 'edges' else 'scope_refs'
                    if not isinstance(selected, list) or selected != value.get(field, []):
                        raise PublishedReadModelError('native navigation reference hint differs')
            yield value

    def get(self, identifier, default=None):
        if identifier not in self._nodes:
            rows = list(self._rows('nodes', 'node_id=?', (identifier,)))
            if len(rows) > 1:
                raise PublishedReadModelError('duplicate native navigation identity')
            node = rows[0] if rows else None
            properties = node.get('properties') if node else None
            if isinstance(properties, dict) and properties.get('packet_id'):
                node = None  # Dense packet members use the indexed knowledge route.
            self._nodes[identifier] = node
        return self._nodes[identifier] if self._nodes[identifier] is not None else default

    def __contains__(self, identifier):
        return self.get(identifier) is not None

    def __getitem__(self, identifier):
        result = self.get(identifier)
        if result is None:
            raise KeyError(identifier)
        return result

    def edges(self, direction, *, semantic=False):
        if direction not in ('incoming', 'outgoing'):
            raise ValueError('explicit native navigation edge direction required')
        view = self
        class Adjacency:
            def get(self, identifier, default=None):
                endpoint = 'to_id' if direction == 'incoming' else 'from_id'
                where, args = endpoint + '=?', [identifier]
                if semantic:
                    where += " AND (edge_kind='authored_item_manifest' OR (edge_kind='evidence_claim' AND predicate_id IN (SELECT value FROM json_each(?))))"
                    args.append(_compact([*_BIBLIOGRAPHIC, *_LINKS]))
                # Cache node membership, not whole neighborhoods. Every row
                # and payload seek retains the surrounding snapshot budget.
                return [row for row in view._rows('edges', where, tuple(args))
                        if row['from_id'] in view and row['to_id'] in view]
        return Adjacency()

    def rights(self, identifiers):
        selected = set(identifiers)
        if not selected:
            return []
        where = "scope_refs_json='' OR EXISTS (SELECT 1 FROM json_each(scope_refs_json) WHERE value IN (SELECT value FROM json_each(?)))"
        result = []
        for row in self._rows('rights', where, (_compact(sorted(selected)),)):
            scope = row.get('scope_refs', [])
            if not isinstance(scope, list) or any(not isinstance(item, str) for item in scope):
                raise PublishedReadModelError('native navigation rights scope is invalid')
            if selected.intersection(scope):
                result.append(row)
        return result
