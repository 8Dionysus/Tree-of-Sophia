"""Wire-only provider control adapter; installed native owner is mandatory.

Original mechanics remain in kag_provider_controls_legacy_oracle.py solely as
an explicit cold oracle. Maintained imports never execute or fall back to it.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
from corpus_store import CorpusStoreError

TEMPLATE = Path(__file__).resolve().parents[1] / 'kag/provider-template.json'
PATHS = (
    'kag/AGENTS.md', 'kag/README.md', 'kag/edges/source-return.json',
    'kag/indexes/source-routes.json', 'kag/manifest.json',
    'kag/nodes/export-route.json', 'kag/nodes/source-export.json',
    'kag/projections/source-return.json', 'kag/receipts/publication-route.json',
)
MAX_BYTES = 64 * 1024


def _invoke(action: str, root=None, entries=None):
    binary = os.environ.get('TOS_KAG_PROVIDER_CONTROLS_BIN') or shutil.which('tos-kag-provider-controls')
    if not binary:
        raise CorpusStoreError('installed tos-kag-provider-controls is required; set TOS_KAG_PROVIDER_CONTROLS_BIN')
    command = [str(Path(binary).absolute()), action, '--template', str(TEMPLATE)]
    if root is not None:
        command += ['--root', str(Path(root).absolute())]
    raw = json.dumps(entries, ensure_ascii=False, allow_nan=False).encode() if entries is not None else b''
    if len(raw) > MAX_BYTES:
        raise CorpusStoreError('provider control request exceeds its byte limit')
    try:
        child = subprocess.run(command, input=raw, capture_output=True, timeout=120, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CorpusStoreError('native provider controls failed: ' + str(error)) from error
    if child.returncode:
        raise CorpusStoreError(child.stderr.decode(errors='replace').strip() or 'native provider controls failed')
    if len(child.stdout) > 8 * MAX_BYTES:
        raise CorpusStoreError('native provider control response exceeds its byte limit')
    try:
        return json.loads(child.stdout)
    except (ValueError, UnicodeError) as error:
        raise CorpusStoreError('invalid native provider control response') from error


def _template() -> dict[str, bytes]:
    return {path: bytes(raw) for path, raw in _invoke('template').items()}


def verify_provider_controls(root: Path, entries: list[dict]) -> None:
    _invoke('verify', root, entries)


def materialize_provider_controls(root: Path) -> list[dict]:
    return _invoke('materialize', root)
