"""Confidential semantic source reading through an explicitly selected owner.

Composition reuses the public registry and exact record grammar, never its
catalog/export API. The context partitions storage; independently supplied
access binds source reading. Neither grants derivation, assessment, rights or
publication authority. Writers reserve identities and check applicable rights
before asking this reader to verify exact native content.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

from native_text_binding import NativeTextBindingError, NativeTextBindingResolver
from source_owner_context import (
    CONTEXT_SCHEMA_REF, OwnerLocalSourceContext, SourceOwnerContextError,
)
from source_record_profiles import (
    MAX_RECORD_BYTES, SourceProfileError, SourceRecordProfiles,
    _nonfinite, _unique_object,
)


MAX_SNAPSHOT_BYTES = 8_388_608
MAX_SNAPSHOT_FILES = 128


def _canonical(value):
    try:
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(',', ':'), allow_nan=False).encode('utf-8')
    except (TypeError, ValueError, UnicodeError, RecursionError) as error:
        raise SourceProfileError('owner-local profile input is not finite UTF-8 JSON') from error
    if len(raw) > MAX_RECORD_BYTES:
        raise SourceProfileError('owner-local profile input exceeds its record byte budget')
    return raw


def _object(raw):
    try:
        value = json.loads(raw, object_pairs_hook=_unique_object, parse_constant=_nonfinite)
        if not isinstance(value, dict):
            raise ValueError('not an object')
        _canonical(value)
        return value
    except (ValueError, UnicodeError, RecursionError) as error:
        raise SourceProfileError('owner-local source input is not a bounded JSON object') from error


def _hash(raw):
    return hashlib.sha256(raw).hexdigest()


class OwnerLocalSourceRecordProfiles:
    """One immutable transport/access snapshot, without a publication method.

    ``validate`` and ``load`` always verify metadata only. Exact content needs
    the distinct ``validate_native_binding(..., verify_content=True)`` call;
    the command owner must first check current rights and derivation scope.
    Non-native semantic profiles require no binding and metadata-only access.
    This reader does not reserve IDs or prove existence of arbitrary semantic
    claims/relations; the source command's bounded owner inventory does that.
    """

    def __init__(self, context, source_access, source_binding, source_reader=None):
        if not isinstance(context, OwnerLocalSourceContext):
            raise SourceProfileError('owner-local profiles need the explicit protected context')
        if source_reader is not None and not callable(source_reader):
            raise SourceProfileError('owner-local source reader must be an internal callable')
        access = _object(_canonical(source_access))
        if (set(access) != {'read_scope', 'access_allowed', 'authority_ref'}
                or access['read_scope'] not in {'metadata_only', 'exact_owner_local'}
                or access['access_allowed'] is not True
                or not isinstance(access['authority_ref'], str) or not access['authority_ref'].strip()):
            raise SourceProfileError('owner-local source reading needs an explicit bounded access scope')
        if source_binding is not None and not isinstance(source_binding, dict):
            raise SourceProfileError('owner-local native source binding must be an object or absent')
        self._context, self._reader = context, source_reader
        self._access = access
        self._binding_raw = _canonical(source_binding) if source_binding is not None else None
        self._binding = _object(self._binding_raw) if self._binding_raw is not None else None
        self._context_digest = self._context_snapshot()
        self._public = SourceRecordProfiles(context.public_root)
        self._public.input_digests[CONTEXT_SCHEMA_REF] = context.contract_digest
        self._profiles = {kind: copy.deepcopy(profile) for kind, profile in self._public.profiles.items()
                          if profile['reader'] == 'semantic-metadata-v1'}
        self._source_inputs = {}
        self._native_metadata, self._native_exact = None, None
        self.snapshot()

    @property
    def registry(self):
        return copy.deepcopy(self._public.registry)

    @property
    def profiles(self):
        return copy.deepcopy(self._profiles)

    @property
    def source_basenames(self):
        return {kind: profile['source_basename'] for kind, profile in self._profiles.items()}

    @property
    def input_digests(self):
        """Only public grammar; private records and native closure stay opaque."""
        return dict(self._public.input_digests)

    def _profile(self, kind):
        if not isinstance(kind, str) or kind not in self._profiles:
            raise SourceProfileError('owner-local reader requires a declared semantic metadata profile')
        return self._profiles[kind]

    def _context_snapshot(self):
        try:
            return self._context.snapshot()
        except (SourceOwnerContextError, OSError) as error:
            raise SourceProfileError('owner-local source context changed or became unsafe') from error

    def _read(self, ref, limit):
        try:
            return self._context.read_bytes(self._context.path(ref), limit, read_bytes=self._reader)
        except (SourceOwnerContextError, OSError, ValueError) as error:
            raise SourceProfileError('owner-local source dependency is absent, unsafe or changed') from error

    def validate_path(self, kind, ref):
        profile = self._profile(kind)
        try:
            path = Path(ref)
            target = self._context.path(ref)
            if (self._context.role(ref) != 'owner-local-root'
                    or not target.is_relative_to(self._context.private_root)
                    or path.name != profile['source_basename']
                    or len(path.parts) < len(Path(self._context.private_prefix).parts) + 2
                    or any(part.startswith('.') or part in {'payload', 'local-content', 'catalog'}
                           for part in path.parts)):
                raise SourceProfileError('owner-local record leaves its typed private metadata package')
        except (SourceOwnerContextError, TypeError, ValueError) as error:
            raise SourceProfileError('owner-local record requires its exact private source path') from error
        if self._context_snapshot() != self._context_digest:
            raise SourceProfileError('owner-local source context changed during path resolution')

    def _source(self, kind, payload):
        profile = self._profile(kind)
        source = _object(_canonical(payload))
        if source.get('visibility') != 'local_only':
            raise SourceProfileError('owner-local source record must remain local_only')
        self._public._validate_shape(kind, source)
        adapter = profile.get('native_binding_adapter')
        if adapter is None:
            if ('native_text_binding' in source or self._binding is not None
                    or self._access['read_scope'] != 'metadata_only'):
                raise SourceProfileError('non-native semantic metadata cannot acquire a native source binding')
        elif adapter != 'source-text-unit-v1' or kind != 'occurrence':
            raise SourceProfileError('owner-local native profile needs a separately understood adapter')
        elif self._binding_raw is None or _canonical(source.get('native_text_binding')) != self._binding_raw:
            raise SourceProfileError('owner-local occurrence differs from its complete delegated native binding')
        return source, adapter

    def validate(self, kind, payload):
        self.validate_native_binding(kind, payload)

    def validate_native_binding(self, kind, payload, *, verify_content=False):
        if type(verify_content) is not bool:
            raise SourceProfileError('native content verification needs an explicit boolean')
        if verify_content and self._access['read_scope'] != 'exact_owner_local':
            raise SourceProfileError('exact native content reading is outside the selected access scope')
        self._snapshot(include_content=verify_content)
        source, adapter = self._source(kind, payload)
        summary = None
        if adapter is not None:
            try:
                native = self._native_exact if verify_content else self._native_metadata
                if native is None:
                    native = NativeTextBindingResolver(self._context.public_root,
                        owner_context=self._context, read_bytes=self._reader)
                    if verify_content:
                        self._native_exact = native
                    else:
                        self._native_metadata = native
                summary = native.resolve(source['native_text_binding'],
                    verify_content=verify_content, allow_private_content=verify_content)
                for ref, digest in native.schema_digests.items():
                    previous = self._public.input_digests.get(ref)
                    if previous is not None and previous != digest:
                        raise SourceProfileError('native and semantic profile grammar snapshots differ')
                    self._public.input_digests[ref] = digest
            except NativeTextBindingError as error:
                raise SourceProfileError('owner-local native source binding is unresolved, unsafe or changed') from error
        self._snapshot(include_content=verify_content)
        return summary

    def load(self, kind, ref):
        self.validate_path(kind, ref)
        self._snapshot(include_content=False)
        raw = self._read(ref, MAX_RECORD_BYTES)
        digest = _hash(raw)
        if ref in self._source_inputs and self._source_inputs[ref] != digest:
            raise SourceProfileError('owner-local source record changed after its first read')
        self._source_inputs[ref] = digest
        source = _object(raw)
        self.validate(kind, source)
        return source

    def snapshot(self):
        """Recheck context, exact public grammar, records and native closure."""
        return self._snapshot(include_content=True)

    def _snapshot(self, *, include_content):
        # Even a previously verified reader's validate/load calls must not
        # reopen text bytes. The explicit full snapshot retains that closure
        # for a command's separately authorized final currentness check.
        if self._context_snapshot() != self._context_digest:
            raise SourceProfileError('owner-local source transport changed after selection')
        grammar = self._public.input_digests
        if len(grammar) + len(self._source_inputs) > MAX_SNAPSHOT_FILES:
            raise SourceProfileError('owner-local source snapshot exceeds its file-count budget')
        remaining = MAX_SNAPSHOT_BYTES
        for inputs in (grammar, self._source_inputs):
            for ref, digest in sorted(inputs.items()):
                raw = self._read(ref, min(MAX_RECORD_BYTES, remaining))
                remaining -= len(raw)
                if _hash(raw) != digest:
                    raise SourceProfileError('owner-local source record or grammar changed after resolution')
        try:
            native = {'metadata': (self._native_metadata.snapshot()
                                   if self._native_metadata is not None else None),
                      'exact': (self._native_exact.snapshot()
                                if include_content and self._native_exact is not None else None)}
        except NativeTextBindingError as error:
            raise SourceProfileError('owner-local native source closure changed after resolution') from error
        if self._context_snapshot() != self._context_digest:
            raise SourceProfileError('owner-local context changed during source snapshot')
        return 'sha256:' + _hash(_canonical({
            'context': self._context_digest, 'source_access': self._access,
            'source_binding': _hash(self._binding_raw) if self._binding_raw is not None else None,
            'contracts': grammar, 'sources': self._source_inputs, 'native': native,
        }))
