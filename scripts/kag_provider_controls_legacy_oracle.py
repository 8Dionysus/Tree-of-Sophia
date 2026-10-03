"""Materialize the bounded provider-home contract in an independent release.

The checked-in template describes routes. Only a verified, published release
binds them to a particular corpus export; it is never an index of this checkout.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

from corpus_store import CorpusStoreError, canonical, digest_file, regular

TEMPLATE = Path(__file__).resolve().parents[1] / 'kag/provider-template.json'
PATHS = (
    'kag/AGENTS.md', 'kag/README.md', 'kag/edges/source-return.json',
    'kag/indexes/source-routes.json', 'kag/manifest.json',
    'kag/nodes/export-route.json', 'kag/nodes/source-export.json',
    'kag/projections/source-return.json', 'kag/receipts/publication-route.json',
)
MAX_BYTES = 64 * 1024


def _template() -> dict[str, bytes]:
    if regular(TEMPLATE).st_size > MAX_BYTES:
        raise CorpusStoreError('provider template exceeds its bounded byte limit')
    def pairs(items):
        value = {}
        for key, item in items:
            if key in value:
                raise CorpusStoreError('provider template contains a duplicate field')
            value[key] = item
        return value
    try:
        template = json.loads(TEMPLATE.read_bytes(), object_pairs_hook=pairs)
        canonical(template)
    except (ValueError, UnicodeError) as error:
        raise CorpusStoreError('provider template must contain finite JSON') from error
    if not isinstance(template, dict):
        raise CorpusStoreError('provider template must be an object')
    if (set(template) != {'schema_version', 'files'}
            or template['schema_version'] != 'tos_kag_provider_template_v1'
            or not isinstance(template['files'], dict)
            or set(template['files']) != set(PATHS)):
        raise CorpusStoreError('provider template has an unexpected file contract')
    result = {}
    for path in PATHS:
        value = template['files'][path]
        if path.endswith('.md'):
            if not isinstance(value, str) or not value.strip():
                raise CorpusStoreError('provider route card must contain text')
            result[path] = value.encode('utf-8')
        else:
            if not isinstance(value, dict):
                raise CorpusStoreError('provider control must be a JSON object')
            result[path] = canonical(value)
    if sum(map(len, result.values())) > MAX_BYTES:
        raise CorpusStoreError('provider control closure exceeds its byte limit')
    return result


def verify_provider_controls(root: Path, entries: list[dict]) -> None:
    if [entry.get('path') for entry in entries] != list(PATHS):
        raise CorpusStoreError('provider control membership differs')
    for entry in entries:
        path = root / entry['path']
        if (regular(path).st_size != entry['size_bytes']
                or digest_file(path) != entry['sha256']):
            raise CorpusStoreError('consumer changed a provider control: ' + entry['path'])


def materialize_provider_controls(root: Path) -> list[dict]:
    root = Path(root).absolute()
    if root.resolve() != root or not root.is_dir():
        raise CorpusStoreError('provider output must be an explicit regular directory')
    files = _template()
    entries = []
    for relative, raw in files.items():
        path = root / relative
        if path.exists() or path.is_symlink() or path.resolve() != path.absolute():
            raise CorpusStoreError('provider control output must be new')
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as stream:
            stream.write(raw)
        entries.append({'path': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
                        'size_bytes': len(raw)})
    verify_provider_controls(root, entries)
    if files != _template():
        raise CorpusStoreError('provider template changed during materialization')
    return entries
