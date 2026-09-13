"""Explicit bounded text delivery, separate from source-catalog verification.

The existing native binding resolver owns exact closure and source meaning.
This read-only delivery helper neither changes its processor fingerprint nor
accepts content, grants rights or discovers another source.
"""
from __future__ import annotations

import json
import hashlib
import os
from datetime import datetime, timezone
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker
from jsonschema.exceptions import ValidationError, SchemaError
from source_owner_context import _absolute, _read, _object, _canonical

from native_text_binding import NativeTextBindingError, _json


LOCAL_SCHEMA = 'ToS/contracts/native-local-text-read.schema.json'


class LocalTextReadError(NativeTextBindingError):
    """The selected local conditions are absent, unsafe, expired or changed."""


class LocalTextReadSelection:
    """Finite owner-selected local delivery conditions, not source authority.

    Configuration must be a protected 0600 file owned by this account. Its
    exact bytes, mandate and schema are pinned and rechecked on every return.
    Editing or revoking it requires explicit reselection, never hot fallback.
    No Python is loaded from this configuration or its source root.
    """

    def __init__(self, path, source_root):
        try:
            self.path = _absolute(path)
            self.source_root = _absolute(source_root)
            self.raw = _read(self.path, 262_144, confidential_file=True)
            self.config = _object(self.raw)
            self.schema_path = Path(__file__).resolve().parents[1] / LOCAL_SCHEMA
            self.schema_raw = _read(self.schema_path, 65_536)
            schema = _object(self.schema_raw)
            if schema.get('$id') != 'https://tree-of-sophia.local/' + LOCAL_SCHEMA:
                raise ValueError('local reading schema identity differs')
            Draft202012Validator(schema, format_checker=FormatChecker()).validate(self.config)
            if (self.config['source_root'] != str(self.source_root)
                    or self.config['owner_uid'] != os.getuid()
                    or len({row['binding_sha256'] for row in self.config['selections']}) != len(self.config['selections'])):
                raise ValueError('local reading selection owner or subjects differ')
            self.mandate_path = _absolute(self.config['mandate']['path'])
            self.verify()
        except (ValueError, OSError, KeyError, ValidationError, SchemaError) as error:
            raise LocalTextReadError('local text selection is invalid or unavailable') from error

    def verify(self):
        try:
            issued = datetime.fromisoformat(self.config['issued_at'].replace('Z', '+00:00'))
            expires = datetime.fromisoformat(self.config['expires_at'].replace('Z', '+00:00'))
            now = datetime.now(timezone.utc)
            if (issued.tzinfo is None or expires.tzinfo is None or not issued <= now < expires
                    or (expires - issued).total_seconds() > 86_400):
                raise ValueError('local reading selection is not current or exceeds one day')
            if (_read(self.path, 262_144, confidential_file=True) != self.raw
                    or _read(self.schema_path, 65_536) != self.schema_raw
                    or hashlib.sha256(_read(self.mandate_path, 1_048_576)).hexdigest() != self.config['mandate']['sha256']):
                raise ValueError('local reading selection changed')
        except (ValueError, OSError, KeyError, TypeError) as error:
            raise LocalTextReadError('local text selection is expired, revoked or changed') from error

    def select(self, resolver, binding, rights_refs):
        self.verify()
        if resolver.root != self.source_root:
            raise LocalTextReadError('local text selection source differs')
        key = hashlib.sha256(_canonical(binding)).hexdigest()
        selected = next((row for row in self.config['selections'] if row['binding_sha256'] == key), None)
        if selected is None or selected['rights_record_refs'] != rights_refs:
            raise LocalTextReadError('local text selection does not cover exact binding and rights')
        # Notices may include root LICENSE/NOTICE, outside native witness paths.
        # Keep this new bounded reader separate from catalog normalization.
        notices = []
        remaining = 32_768
        for row in selected['notices']:
            ref = Path(row['ref'])
            if (ref.is_absolute() or '..' in ref.parts or ref.as_posix() != row['ref']
                    or '\\' in row['ref'] or '\x00' in row['ref']
                    or not (row['ref'] in ('LICENSE', 'NOTICE', 'NOTICE.md', 'README.md')
                            or ref.is_relative_to('ToS/review-ledger') and ref.suffix == '.md')):
                raise LocalTextReadError('local text notice is outside public notice surfaces')
            try:
                raw = _read(self.source_root / ref, remaining)
                if hashlib.sha256(raw).hexdigest() != row['sha256']:
                    raise ValueError('notice digest differs')
                notices.append({**row, 'text': raw.decode('utf-8')})
                remaining -= len(raw)
            except (ValueError, OSError) as error:
                raise LocalTextReadError('local text notice is unavailable or changed') from error
        if not {'license', 'attribution'} <= {row['role'] for row in notices}:
            raise LocalTextReadError('local text selection must preserve license and attribution')
        return {'selection_sha256': hashlib.sha256(self.raw).hexdigest(),
                'expires_at': self.config['expires_at'],
                'condition_review': selected['condition_review'], 'notices': notices}


def read_public_unit(resolver, binding: dict, *, max_return_bytes=65_536) -> dict:
    """Return ordered exact public spans, never a private-text capability.

    Gates precede content reads. Conditional rights need another owner's
    explicit route, not an inference that free-text terms were satisfied.
    No implicit context, concatenation, normalization or truncation.
    """
    return _read_unit(resolver, binding, max_return_bytes=max_return_bytes)


def read_local_unit(resolver, binding: dict, selection: LocalTextReadSelection, *, max_return_bytes=65_536) -> dict:
    """Exact local spans with reviewed conditions, never private/public fallback."""
    if not isinstance(selection, LocalTextReadSelection):
        raise LocalTextReadError('local text selection must be explicit')
    return _read_unit(resolver, binding, max_return_bytes=max_return_bytes, selection=selection)


def _read_unit(resolver, binding, *, max_return_bytes, selection=None):
    if type(max_return_bytes) is not int or not 1 <= max_return_bytes <= 1_048_576:
        raise NativeTextBindingError('public unit return needs a bounded positive byte limit')
    # Detach the request before user-supplied I/O callbacks can change it.
    binding = _json(json.dumps(binding, allow_nan=False).encode('utf-8'))
    summary = resolver.resolve(binding)
    if not summary['public_content_declared']:
        raise NativeTextBindingError('native public unit return requires public content authority')
    layer = resolver._record(binding['text_layer']['record_ref'],
                         expected=binding['text_layer']['record_sha256'])
    rep = layer['representation']
    exact_scope = {layer['layer_id'], rep['content_file_id']}
    rights = [resolver._record(row['ref'], expected=row['sha256']) for row in rep['rights_record_refs']]
    applicable = [row for row in rights if exact_scope.intersection(row['scope_refs'])] or rights
    local_conditions = None
    if selection is not None:
        if any(row['redistribution_posture'] not in ('authorized', 'authorized_with_conditions')
               or row['derivative_posture'] not in ('allowed', 'allowed_with_conditions') for row in applicable):
            raise LocalTextReadError('local text selection cannot override recorded rights')
        local_conditions = selection.select(resolver, binding, rep['rights_record_refs'])
    elif any(row['redistribution_posture'] != 'authorized'
             or row['derivative_posture'] != 'allowed' for row in applicable):
        raise NativeTextBindingError('native public unit return requires unconditional recorded rights')
    summary = resolver.resolve(binding, verify_content=True)
    packet = resolver._record(binding['packet_ref'], expected=binding['packet_sha256'])
    text = resolver._read(rep['content_ref'], expected=rep['content_sha256'], content=True).decode('utf-8')
    anchors = {row['anchor_ref']: row for row in packet['anchors']}
    spans = []
    for ref in binding['ordered_anchor_refs']:
        anchor = anchors[ref]
        selector = anchor['selector']
        spans.append({'anchor_ref': ref, 'selector': dict(selector),
                      'exact_sha256': anchor['exact_sha256'],
                      'text': text[selector['start']:selector['end']]})
    result = {'schema_version': 'tos_native_public_unit_return_v1',
              'summary': summary,
              'packet': {'id': binding['packet_id'], 'version': binding['packet_version'],
                         'sha256': binding['packet_sha256']},
              'layer_record_sha256': binding['text_layer']['record_sha256'],
              'representation_sha256': rep['content_sha256'],
              'spans': spans, 'closure_fingerprint': resolver.snapshot()}
    if selection is not None:
        if selection.select(resolver, binding, rep['rights_record_refs']) != local_conditions:
            raise LocalTextReadError('local text conditions changed during return')
        result.update(schema_version='tos_native_local_unit_return_v1', local_conditions=local_conditions)
    if len(json.dumps(result, ensure_ascii=False, allow_nan=False,
                      separators=(',', ':')).encode('utf-8')) > max_return_bytes:
        raise NativeTextBindingError('native public unit return exceeds its output-byte budget')
    return result
