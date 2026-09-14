"""Pure compact-lens input projection, subordinate to the complete source row.

This is an internal rendering/query seed, not a replacement normalized record.
It cannot serve full inspection or answer filters on omitted source fields.
Publication, index completeness and snapshot admission belong to the caller.
"""
from dataclasses import dataclass
import hashlib

from . import knowledge as k
from .published_read_model import _json
from .published_read_metadata import _compact, PublishedReadModelError

SCHEMA = 'tos_compact_lens_carrier_v1'
MAX_ROW_BYTES = 1_048_576
_OMITTED = ('attributes', 'source_record', 'readable_context', 'semantics.claim.source_canonical_json')


@dataclass(frozen=True)
class CompactLensCarrier:
    kind: str
    identifier: str
    source_sha256: str
    seed_sha256: str
    seed_json: str

    def render(self, language='auto'):
        """Deliver only compact output through the existing form/language law."""
        seed = _json(self.seed_json)
        if hashlib.sha256(self.seed_json.encode('utf-8')).hexdigest() != self.seed_sha256:
            raise PublishedReadModelError('compact lens carrier checksum differs')
        if seed.get('id') != self.identifier:
            raise PublishedReadModelError('compact lens carrier identity differs')
        return k._lens_carrier(seed, 'compact', language=language)


def compact_lens_carrier(kind, raw, *, max_source_bytes=MAX_ROW_BYTES):
    """Project exact admitted row bytes without touching the source or its digest.

    Keep form inputs intact, including malformed/over-budget collections: their
    existing selector must produce the same honest refusal or missing state.
    Unknown top-level/semantic fields survive; only declared compact omissions
    are removed. No inference, translation or semantic admission happens here.
    """
    if kind not in {'node', 'relation'} or not isinstance(raw, str):
        raise ValueError('exact normalized node/relation JSON required')
    source = raw.encode('utf-8')
    if type(max_source_bytes) is not int or not 1 <= max_source_bytes <= 8 * 1024 * 1024:
        raise ValueError('compact source budget must be between 1 byte and 8 MiB')
    if len(source) > max_source_bytes:
        raise PublishedReadModelError('compact lens source row exceeds byte budget')
    item = _json(raw)
    if (not isinstance(item, dict) or not isinstance(item.get('id'), str)
            or not 1 <= len(item['id']) <= 4096 or not isinstance(item.get('attributes'), dict)):
        raise PublishedReadModelError('compact lens requires a normalized source row')
    seed = k._lens_carrier(item, 'compact')
    # These inputs remain internal. render() always removes attributes from
    # the public carrier after the unchanged human-form selector has read them.
    if 'human_forms' in item['attributes']:
        seed['attributes'] = item['attributes']
    encoded = _compact(seed)
    if len(encoded.encode('utf-8')) > MAX_ROW_BYTES:
        raise PublishedReadModelError('compact lens seed exceeds byte budget')
    return CompactLensCarrier(kind, item['id'], hashlib.sha256(source).hexdigest(),
                              hashlib.sha256(encoded.encode('utf-8')).hexdigest(), encoded)


def supports_compact_lens_carrier(spec):
    """Conservative capability check on an already bound/normalized LensSpec.

    Missing projected source data must never become a negative match. Full-text
    seed search uses the original whole row, so it also requires another plan.
    """
    if spec['detail'] != 'compact' or spec['seed']['text_query']:
        return False
    fields = list(spec['composition']['group_by'])
    fields.extend(rule['field'] for kind in ('nodes', 'relations')
                  for rule in spec['composition']['sort_' + kind])
    groups = [spec['node_query'], spec['relation_query']]
    groups.extend(step[kind + '_query'] for path in spec['path_query'] for step in path['steps']
                  for kind in ('node', 'relation'))
    for group in groups:
        for rule in group['filters']:
            field = rule.get('field')
            if not isinstance(field, str) or not field:
                return False
            fields.append(field)
    return all(not any(field == omitted or field.startswith(omitted + '.')
                       or omitted.startswith(field + '.') for omitted in _OMITTED) for field in fields)
