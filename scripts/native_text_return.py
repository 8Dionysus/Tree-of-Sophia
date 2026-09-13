"""Explicit bounded text delivery, separate from source-catalog verification.

The existing native binding resolver owns exact closure and source meaning.
This read-only delivery helper neither changes its processor fingerprint nor
accepts content, grants rights or discovers another source.
"""
from __future__ import annotations

import json

from native_text_binding import NativeTextBindingError, _json


def read_public_unit(resolver, binding: dict, *, max_return_bytes=65_536) -> dict:
    """Return ordered exact public spans, never a private-text capability.

    Gates precede content reads. Conditional rights need another owner's
    explicit route, not an inference that free-text terms were satisfied.
    No implicit context, concatenation, normalization or truncation.
    """
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
    if any(row['redistribution_posture'] != 'authorized'
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
    if len(json.dumps(result, ensure_ascii=False, allow_nan=False,
                      separators=(',', ':')).encode('utf-8')) > max_return_bytes:
        raise NativeTextBindingError('native public unit return exceeds its output-byte budget')
    return result
