"""One protected private Claim and its explicitly selected source closure.

This reader composes existing record, Claim and native-binding grammars. It
does not scan a private corpus, follow arbitrary citation-looking fields,
invent an endpoint type, authenticate a maker, assess a Claim or publish its
metadata. Metadata can contain source wording; these envelopes are private
assessment input, not a public-safe projection.
"""
from __future__ import annotations

import copy
from pathlib import Path

from native_text_binding import (
    BINDING_SCHEMA, NativeTextBindingError, NativeTextBindingResolver,
    check_local_research_rights,
)
from source_owner_context import (
    CONTEXT_SCHEMA_REF, OwnerLocalSourceContext, SourceOwnerContextError,
)
from source_owner_record_profiles import (
    MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_FILES, OwnerLocalSourceRecordProfiles,
    _canonical, _hash, _object,
)
from source_record_profiles import (
    MAX_CLAIM_FILE_BYTES, MAX_RECORD_BYTES, SOURCE_CLAIM_BASENAME,
    SourceClaimProfiles, SourceProfileError, SourceRecordProfiles,
)


MAX_SOURCE_RECORDS = 16
MAX_NATIVE_BINDINGS = 8
MAX_EVIDENCE_REFS = 64
MAX_CLAIM_ROWS = 1024
SOURCE_KEYS = {'path', 'record_id', 'profile_type_id', 'origin_id', 'source_access', 'source_binding'}
NATIVE_KEYS = {'binding', 'origin_id', 'source_access'}


def _text(value):
    return isinstance(value, str) and bool(value.strip())


def _access(value):
    access = _object(_canonical(value))
    if (set(access) != {'read_scope', 'access_allowed', 'authority_ref'}
            or not isinstance(access['read_scope'], str)
            or access['read_scope'] not in {'metadata_only', 'exact_owner_local'}
            or access['access_allowed'] is not True or not _text(access['authority_ref'])):
        raise SourceProfileError('private Claim reading needs an explicit bounded source access scope')
    return access


def _envelope(identity, version, payload, origin):
    return {'id': identity, 'version': version, 'payload': copy.deepcopy(payload), 'origin_id': origin}


def _ref(record):
    return {'id': record['id'], 'version': record['version'],
            'digest': 'sha256:' + _hash(_canonical(record['payload']))}


class OwnerLocalSourceClaimProfiles:
    """A frozen one-Claim reader; exact mode is not an upgrade of metadata.

    ``source_records`` selects existing profile type IDs and exact paths, not
    source bodies. ``native_bindings`` selects only additional native evidence;
    a selected Occurrence contributes its independently supplied binding.
    Native envelopes are supporting evidence, never independently authorized
    native assessment targets. All selection access is checked before source
    metadata is opened. A failed load cannot be reused with another selection.
    ``prepare_candidate`` grounds caller-authored Claim data for a writer;
    it never invents a stored source or changes the stored-only ``load`` route.
    """

    def __init__(self, context, source_access, *, source_records=(), native_bindings=(),
                 verify_content=False, source_reader=None):
        if not isinstance(context, OwnerLocalSourceContext):
            raise SourceProfileError('private Claim reading needs the explicit protected source context')
        if source_reader is not None and not callable(source_reader):
            raise SourceProfileError('private Claim source reader must be an internal callable')
        if type(verify_content) is not bool:
            raise SourceProfileError('private Claim content mode needs an explicit boolean')
        access = _access(source_access)
        if verify_content and access['read_scope'] != 'exact_owner_local':
            raise SourceProfileError('exact Claim evidence is outside the selected source access scope')
        if (not isinstance(source_records, (list, tuple)) or len(source_records) > MAX_SOURCE_RECORDS
                or not isinstance(native_bindings, (list, tuple)) or len(native_bindings) > MAX_NATIVE_BINDINGS):
            raise SourceProfileError('private Claim selection exceeds its finite record or binding budget')
        sources, natives = [], []
        # Preflight *every* grant before constructing a source reader. Reject
        # inline endpoint payloads, extra grant fields and caller type aliases.
        for raw in source_records:
            row = _object(_canonical(raw))
            if (set(row) != SOURCE_KEYS or any(not _text(row[key]) for key in
                    ('path', 'record_id', 'profile_type_id', 'origin_id'))
                    or row['source_binding'] is not None and not isinstance(row['source_binding'], dict)):
                raise SourceProfileError('private Claim source selection requires exact profile/path keys')
            row['source_access'] = _access(row['source_access'])
            if row['source_binding'] is None:
                if row['source_access']['read_scope'] != 'metadata_only':
                    raise SourceProfileError('non-native Claim source selection is metadata-only')
            elif verify_content and row['source_access']['read_scope'] != 'exact_owner_local':
                raise SourceProfileError('exact bound Claim evidence is outside a source selection access scope')
            sources.append(row)
        for raw in native_bindings:
            row = _object(_canonical(raw))
            if set(row) != NATIVE_KEYS or not isinstance(row['binding'], dict) or not _text(row['origin_id']):
                raise SourceProfileError('private Claim native selection requires exact binding keys')
            row['source_access'] = _access(row['source_access'])
            if verify_content and row['source_access']['read_scope'] != 'exact_owner_local':
                raise SourceProfileError('exact native Claim evidence is outside a binding access scope')
            natives.append(row)
        self._context, self._reader, self._access = context, source_reader, access
        self._verify_content = verify_content
        self._sources, self._native_selections = sources, natives
        self._context_digest = self._context_snapshot()
        self._public = SourceRecordProfiles(context.public_root)
        self._claims = SourceClaimProfiles(context.public_root)
        self._private_readers, self._native = {}, {}
        self._source_inputs, self._claim_input = {}, None
        self._selection, self._claim, self._failed = None, None, False
        self._mode, self._candidate_digest = None, None
        self._records, self._summaries, self._languages = {}, [], ()
        self._source_kinds = {}
        for row in sources:
            entity = self._claims.entities.get(row['profile_type_id'], {})
            profile = entity.get('source_record_profile')
            if profile is None or profile['record_type'] not in self._public.profiles:
                raise SourceProfileError('Claim endpoint type has no understood source record profile')
            kind = profile['record_type']
            self._source_kinds[row['profile_type_id']] = kind
            try:
                private = context.role(row['path']) == 'owner-local-root'
            except SourceOwnerContextError as error:
                raise SourceProfileError('Claim source selection leaves its exact owner transport') from error
            if private:
                reader = OwnerLocalSourceRecordProfiles(context, row['source_access'],
                    row['source_binding'], source_reader)
                reader.validate_path(kind, row['path'])
                self._private_readers[row['path']] = reader
            else:
                self._public.validate_path(kind, row['path'])
            if row['source_binding'] is not None:
                self._add_native(row['source_binding'], row['origin_id'], row['source_access'],
                                 source_id=row['record_id'])
        for row in natives:
            self._add_native(row['binding'], row['origin_id'], row['source_access'])
        self.snapshot()

    def _context_snapshot(self):
        try:
            return self._context.snapshot()
        except (SourceOwnerContextError, OSError) as error:
            raise SourceProfileError('private Claim source context changed or became unsafe') from error

    def _read(self, ref, limit):
        try:
            return self._context.read_bytes(self._context.path(ref), limit, read_bytes=self._reader)
        except (SourceOwnerContextError, OSError, ValueError) as error:
            raise SourceProfileError('private Claim input is absent, unsafe or changed') from error

    def _protected_read(self, path, limit):
        try:
            return self._context.read_bytes(path, limit, read_bytes=self._reader)
        except (SourceOwnerContextError, OSError, ValueError) as error:
            raise SourceProfileError('Claim public source dependency is absent, unsafe or changed') from error

    def _add_native(self, binding, origin, access, *, source_id=None):
        key = _canonical(binding)
        if key in self._native:
            row = self._native[key]
            if row['origin_id'] != origin or row['source_access'] != access:
                raise SourceProfileError('one native Claim source has conflicting origins or access scopes')
        else:
            if len(self._native) >= MAX_NATIVE_BINDINGS:
                raise SourceProfileError('private Claim native closure exceeds its binding-count budget')
            resolver = NativeTextBindingResolver(self._context.public_root,
                owner_context=self._context, read_bytes=self._reader)
            try:
                # This validates public binding grammar only, not source bytes.
                resolver._validate(binding, BINDING_SCHEMA)
            except NativeTextBindingError as error:
                raise SourceProfileError('private Claim native selection has an invalid binding') from error
            if any(row['binding']['unit_id'] == binding['unit_id'] for row in self._native.values()):
                raise SourceProfileError('one native Claim identity has conflicting selected bindings')
            row = {'binding': copy.deepcopy(binding), 'origin_id': origin,
                   'source_access': copy.deepcopy(access), 'source_ids': set(), 'resolver': resolver}
            self._native[key] = row
        if source_id is not None:
            row['source_ids'].add(source_id)

    def _grammar(self):
        result = {CONTEXT_SCHEMA_REF: self._context.contract_digest}
        inputs = [self._claims.input_digests, self._public.input_digests,
                  *(reader.input_digests for reader in self._private_readers.values()),
                  *(row['resolver'].schema_digests for row in self._native.values())]
        for mapping in inputs:
            for ref, digest in mapping.items():
                if ref in result and result[ref] != digest:
                    raise SourceProfileError('private Claim readers disagree on their public grammar snapshot')
                result[ref] = digest
        return result

    def snapshot(self):
        """Recheck consumed closure; private paths remain inside this digest."""
        if self._failed:
            raise SourceProfileError('failed private Claim selection cannot be reused')
        if self._context_snapshot() != self._context_digest:
            raise SourceProfileError('private Claim transport changed after selection')
        grammar = self._grammar()
        if len(grammar) + len(self._source_inputs) > MAX_SNAPSHOT_FILES:
            raise SourceProfileError('private Claim metadata exceeds its snapshot file-count budget')
        remaining = MAX_SNAPSHOT_BYTES
        for inputs in (grammar, self._source_inputs):
            for ref, digest in sorted(inputs.items()):
                raw = self._read(ref, min(MAX_RECORD_BYTES, remaining))
                remaining -= len(raw)
                if _hash(raw) != digest:
                    raise SourceProfileError('private Claim source or grammar changed after first read')
        if self._claim_input is not None:
            ref, digest = self._claim_input
            if _hash(self._read(ref, MAX_CLAIM_FILE_BYTES)) != digest:
                raise SourceProfileError('private Claim stream changed after first read')
        try:
            natives = sorted(row['resolver'].snapshot() for row in self._native.values())
            private = sorted(reader.snapshot() for reader in self._private_readers.values())
            identity = self._public.native_identity_snapshot(read_bytes=self._protected_read, only_if_used=True)
            public_native = self._public.native_text_snapshot(read_bytes=self._protected_read)
        except NativeTextBindingError as error:
            raise SourceProfileError('private Claim native source closure changed after resolution') from error
        if self._context_snapshot() != self._context_digest:
            raise SourceProfileError('private Claim source context changed during closure recheck')
        return 'sha256:' + _hash(_canonical({'context': self._context_digest,
            'selection': self._selection, 'source_access': self._access,
            'source_selections': self._sources, 'native_selections': self._native_selections,
            'verify_content': self._verify_content, 'contracts': grammar,
            'sources': self._source_inputs, 'claim_stream': self._claim_input,
            **({'candidate': {'mode': 'candidate', 'digest': self._candidate_digest}}
               if self._mode == 'candidate' else {}),
            'native': natives, 'private_records': private,
            'public_native_identity': identity, 'public_native': public_native}))

    @property
    def contract_digests(self):
        self.snapshot()
        return self._grammar()

    def _loaded(self, claim_id=None):
        if self._claim is None or claim_id is not None and claim_id != self._claim['id']:
            raise SourceProfileError('private Claim reader has no such selected Claim')
        self.snapshot()

    @property
    def records(self):
        self._loaded()
        return tuple(copy.deepcopy(row) for _, row in sorted(self._records.items()))

    @property
    def native_summaries(self):
        self._loaded()
        return tuple(copy.deepcopy(self._summaries))

    def dependency_refs(self, claim_id):
        self._loaded(claim_id)
        return tuple(_ref(row) for _, row in sorted(self._records.items()))

    def required_languages(self, claim_id):
        self._loaded(claim_id)
        return self._languages

    def _insert(self, row):
        identity = row['id']
        if identity == self._selection[1]:
            raise SourceProfileError('Claim and source evidence cannot shadow one identity')
        if identity in self._records and _canonical(self._records[identity]) != _canonical(row):
            raise SourceProfileError('Claim source identity has conflicting bodies, versions or origins')
        self._records[identity] = row

    def _claim_path(self, ref):
        try:
            path = Path(ref)
            if (self._context.role(ref) != 'owner-local-root'
                    or path.name != SOURCE_CLAIM_BASENAME
                    or len(path.parts) < len(Path(self._context.private_prefix).parts) + 2
                    or any(part.startswith('.') or part in {'catalog', 'payload', 'local-content'}
                           for part in path.parts)):
                raise SourceProfileError('private Claim path leaves its exact metadata package')
        except (SourceOwnerContextError, TypeError, ValueError) as error:
            raise SourceProfileError('private Claim needs its exact selected private source path') from error

    def load(self, path, claim_id, *, origin_id, relation_type_id=None):
        if self._mode == 'candidate':
            raise SourceProfileError('a private Claim candidate cannot become a stored-source selection')
        if (not _text(claim_id) or not _text(origin_id)
                or relation_type_id is not None and not _text(relation_type_id)):
            raise SourceProfileError('private Claim requires explicit identity, origin and optional relation type')
        selection = (path, claim_id, origin_id, relation_type_id)
        if self._selection is not None:
            if selection != self._selection:
                raise SourceProfileError('one private Claim reader cannot change its frozen selection')
            self._loaded(claim_id)
            return copy.deepcopy(self._claim)
        self._claim_path(path)
        self._mode = 'stored'
        self._selection = selection
        try:
            return self._load(path, claim_id, origin_id, relation_type_id)
        except (ValueError, OSError, RecursionError) as error:
            self._failed = True
            raise SourceProfileError('private Claim selection is unsupported, unresolved, unsafe or changed') from error

    def prepare_candidate(self, claim, *, origin_id, relation_type_id=None):
        """Ground one frozen candidate against independently selected sources.

        There is no candidate source path, file read, publication or admission.
        Source and native evidence retain exactly the same protected readers,
        grants and freshness checks as the stored-source route.
        """
        if self._mode == 'stored':
            raise SourceProfileError('a stored private Claim reader cannot become a candidate selection')
        try:
            raw = _canonical(claim)
            if len(raw) > MAX_RECORD_BYTES:
                raise SourceProfileError('private Claim candidate exceeds its record byte budget')
            selected = _object(raw)
        except (ValueError, TypeError, RecursionError) as error:
            raise SourceProfileError('private Claim candidate must be one bounded strict JSON object') from error
        claim_id = selected.get('claim_id')
        if (not _text(claim_id) or not _text(origin_id)
                or relation_type_id is not None and not _text(relation_type_id)):
            raise SourceProfileError('private Claim candidate requires explicit identity and origin')
        selection = (None, claim_id, origin_id, relation_type_id)
        digest = 'sha256:' + _hash(raw)
        if self._selection is not None:
            if selection != self._selection or digest != self._candidate_digest:
                raise SourceProfileError('one private Claim reader cannot change its frozen candidate')
            self._loaded(claim_id)
            return copy.deepcopy(self._claim)
        self._mode, self._selection, self._candidate_digest = 'candidate', selection, digest
        try:
            self.snapshot()
            return self._ground_selected(selected, claim_id, origin_id, relation_type_id)
        except (ValueError, OSError, RecursionError) as error:
            self._failed = True
            raise SourceProfileError('private Claim candidate is unsupported, unresolved, unsafe or changed') from error

    def _load(self, path, claim_id, origin_id, relation_type_id):
        self.snapshot()
        raw = self._read(path, MAX_CLAIM_FILE_BYTES)
        self._claim_input = (path, _hash(raw))
        selected, seen, count = None, set(), 0
        for line in raw.splitlines():
            if not line.strip():
                continue
            count += 1
            if count > MAX_CLAIM_ROWS:
                raise SourceProfileError('private Claim stream exceeds its row-count budget')
            if len(line) > MAX_RECORD_BYTES:
                raise SourceProfileError('private Claim row exceeds its raw record byte budget')
            row = _object(line)
            identity = row.get('claim_id')
            if not _text(identity) or identity in seen:
                raise SourceProfileError('private Claim stream has missing or duplicate Claim identity')
            seen.add(identity)
            if identity == claim_id:
                selected = row
        if selected is None:
            raise SourceProfileError('selected Claim is absent from its stored source')
        return self._ground_selected(selected, claim_id, origin_id, relation_type_id)

    def _ground_selected(self, selected, claim_id, origin_id, relation_type_id):
        if selected.get('visibility') != 'local_only':
            raise SourceProfileError('selected Claim is absent or not local_only')
        self._claims._validate_shape(selected)
        predicate = selected['predicate']
        if (self._claims.profiles[predicate]['reader'] not in {'semantic-relation-v1', 'identity-relation-v1'}
                or relation_type_id is not None
                and self._claims.relations[predicate]['relation_type_id'] != relation_type_id):
            raise SourceProfileError('selected Claim has an unsupported or unexpected relation profile')
        identities = self._claims.identity_refs(selected)
        evidence = (*selected['evidence_refs'], *selected.get('counterevidence_refs', ()))
        quote_anchors = tuple(row['anchor_ref'] for row in selected.get('supporting_quotes', ()))
        if len(evidence) + len(quote_anchors) > MAX_EVIDENCE_REFS:
            raise SourceProfileError('selected Claim exceeds its evidence-ref budget')
        objects, aliases = {}, {}
        for selection in self._sources:
            ref, identity = selection['path'], selection['record_id']
            if identity not in identities and identity not in evidence and ref not in evidence:
                raise SourceProfileError('source selection is outside the selected Claim endpoint/evidence closure')
            kind = self._source_kinds[selection['profile_type_id']]
            source_raw = self._read(ref, MAX_RECORD_BYTES)
            digest = _hash(source_raw)
            if ref in self._source_inputs and self._source_inputs[ref] != digest:
                raise SourceProfileError('Claim source changed between duplicate selections')
            self._source_inputs[ref] = digest
            reader = self._private_readers.get(ref, self._public)
            source = reader.load(kind, ref)
            if _canonical(source) != _canonical(_object(source_raw)) or _hash(self._read(ref, MAX_RECORD_BYTES)) != digest:
                raise SourceProfileError('Claim source changed while its real profile reader resolved it')
            if source['record_id'] != identity or source.get('native_text_binding') != selection['source_binding']:
                raise SourceProfileError('Claim source differs from its selected identity or full native binding')
            record = _envelope(identity, source['record_version'], source, selection['origin_id'])
            self._insert(record)
            objects[identity] = source
            for alias in (ref, identity):
                if alias in aliases and aliases[alias] != identity:
                    raise SourceProfileError('Claim source evidence alias is ambiguous')
                aliases[alias] = identity
        self._claims._validate_shape(selected, objects)
        native_aliases, native_anchors = {}, {}
        for key, row in self._native.items():
            binding = row['binding']
            for alias in (binding['packet_ref'], binding['packet_id'], binding['unit_id'],
                          binding['text_layer']['record_ref'], binding['text_layer']['layer_id'],
                          *binding['ordered_anchor_refs']):
                native_aliases.setdefault(alias, set()).add(key)
            for anchor in binding['ordered_anchor_refs']:
                native_anchors.setdefault(anchor, set()).add(key)
        used_native = {key for key, row in self._native.items() if row['source_ids']}
        for ref in evidence:
            native = native_aliases.get(ref, set())
            if ref in aliases:
                if native:
                    raise SourceProfileError('Claim evidence is ambiguous between source-record and native adapters')
            elif len(native) == 1:
                used_native.update(native)
            else:
                raise SourceProfileError('Claim evidence needs one explicitly selected source or native adapter')
        for anchor in quote_anchors:
            native = native_anchors.get(anchor, set())
            if len(native) != 1:
                raise SourceProfileError('Claim quote anchor leaves the exact selected native unit')
            used_native.update(native)
        if used_native != self._native.keys():
            raise SourceProfileError('native selection is outside the selected Claim evidence closure')
        # Resolve *all* metadata and rights before any exact representation.
        for row in self._native.values():
            resolver, binding = row['resolver'], row['binding']
            resolver.resolve(binding)
            if self._verify_content:
                layer = resolver._record(binding['text_layer']['record_ref'],
                                         expected=binding['text_layer']['record_sha256'])
                check_local_research_rights(resolver, layer)
        self.snapshot()
        for row in self._native.values():
            view = row['resolver'].assessment_records(row['binding'], origin_id=row['origin_id'],
                verify_content=self._verify_content, allow_private_content=self._verify_content)
            for record in view['records']:
                self._insert(record)
            self._summaries.append({**view['summary'], 'origin_id': row['origin_id'],
                'record_refs': [_ref(record) for record in view['records']], 'supporting_only': True})
        languages = set()
        def add(value):
            if _text(value):
                languages.add(value)
        add(selected.get('qualifiers', {}).get('statement_language'))
        for source in objects.values():
            fields = source.get('field_languages')
            for value in fields.values() if isinstance(fields, dict) else ():
                if isinstance(value, dict):
                    add(value.get('language'))
            for field in ('semantic_scope', 'semantic_content', 'form_identity'):
                if isinstance(source.get(field), dict):
                    add(source[field].get('language'))
        for summary in self._summaries:
            add(summary['language'])
        self._languages = tuple(sorted(languages))
        self._claim = _envelope(claim_id, selected['claim_version'], selected, origin_id)
        self.snapshot()
        return copy.deepcopy(self._claim)
