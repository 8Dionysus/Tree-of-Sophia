"""Bounded v4 source selection through the existing confidential transport.

This adapter neither authors a second corpus nor gives private material a
public form. Records and adjacent current forms retain their shared grammar;
the issuer chooses exact sources and read scope independently of commands.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

from assessment_journal import JournalConflict, _keys, _json_object
from knowledge_assessment import MAX_ASSESSMENTS, MAX_RECORD_BYTES, Record, _canonical
from source_owner_context import _open, _read
from source_owner_record_profiles import OwnerLocalSourceRecordProfiles


def source_access(value):
    _keys(value, {'read_scope', 'access_allowed', 'authority_ref'})
    if (not isinstance(value['read_scope'], str)
            or value['read_scope'] not in {'metadata_only', 'exact_owner_local'}
            or value['access_allowed'] is not True
            or not isinstance(value['authority_ref'], str) or not value['authority_ref'].strip()):
        raise PermissionError('private source reading needs an explicit protected access scope')
    return value['read_scope']


def confidential_journal(context, directory):
    path = Path(directory)
    if (not isinstance(directory, str) or not path.is_absolute() or path.as_posix() != directory
            or '..' in path.parts or path == context.private_root
            or not path.is_relative_to(context.private_root)):
        raise PermissionError('v4 journal requires its own directory in the selected private root')
    os.close(_open(path, directory=True, private_root=context.private_root))
    return path


class OwnerLocalAssessmentSources:
    """One bounded selection, retaining opaque byte/currentness dependencies."""

    def __init__(self, context, selections, claim_selections=()):
        self.context, self.records, self.paths, self.form_sets = context, [], {}, {}
        self.readers, self.observed, self.contracts = [], {}, {}
        self._indexed, self.claim_dependencies, self.claim_languages = {}, {}, {}
        self.native_records, self.native_summaries = [], []
        self.form_validator = None
        self.total = 0
        if not isinstance(selections, list) or len(selections) > 64:
            raise ValueError('owner-local assessment sources need at most 64 explicit selections')
        if not isinstance(claim_selections, (list, tuple)) or len(claim_selections) > 32:
            raise ValueError('owner-local assessment needs at most 32 explicit Claim selections')
        identifiers = set()
        # Reject a later unauthorized selection before reading an earlier one.
        for selection in selections:
            _keys(selection, {'path', 'record_id', 'profile_type_id', 'origin_id',
                              'source_access', 'source_binding', 'form_ids'})
            source_access(selection['source_access'])
            ids = selection['form_ids']
            if (not isinstance(selection['record_id'], str) or not selection['record_id']
                    or not isinstance(selection['profile_type_id'], str)
                    or not isinstance(ids, list) or len(ids) > 32
                    or any(not isinstance(value, str) or not value for value in ids)
                    or len(set(ids)) != len(ids)):
                raise ValueError('private assessment requires explicit bounded record and form identities')
            selected = [selection['record_id'], *ids]
            if len(set(selected)) != len(selected) or identifiers.intersection(selected):
                raise ValueError('private source selection repeats a record or form identity')
            identifiers.update(selected)
        for selection in claim_selections:
            _keys(selection, {'path', 'claim_id', 'relation_type_id', 'origin_id', 'source_access',
                              'source_records', 'native_bindings', 'verify_content', 'form_ids'})
            read_scope = source_access(selection['source_access'])
            ids = selection['form_ids']
            if (not isinstance(selection['claim_id'], str) or not selection['claim_id']
                    or not isinstance(selection['relation_type_id'], str) or not selection['relation_type_id']
                    or type(selection['verify_content']) is not bool
                    or not isinstance(ids, list) or len(ids) > 32
                    or any(not isinstance(value, str) or not value for value in ids)
                    or len(set(ids)) != len(ids)):
                raise ValueError('private Claim assessment requires bounded declared identities and read mode')
            if selection['verify_content'] and read_scope != 'exact_owner_local':
                raise PermissionError('exact Claim evidence is outside the selected source access scope')
            selected = [selection['claim_id'], *ids]
            if len(set(selected)) != len(selected) or identifiers.intersection(selected):
                raise ValueError('private Claim selection repeats a source or form identity')
            identifiers.update(selected)
            # Preflight every nested access grant before reading an earlier
            # selection. Source bodies can never supply another selection.
            for name, keys in (
                ('source_records', {'path', 'record_id', 'profile_type_id', 'origin_id', 'source_access', 'source_binding'}),
                ('native_bindings', {'binding', 'origin_id', 'source_access'}),
            ):
                entries = selection[name]
                if not isinstance(entries, list) or len(entries) > 64:
                    raise ValueError('private Claim source selection exceeds its bounded contract')
                for entry in entries:
                    _keys(entry, keys)
                    read_scope = source_access(entry['source_access'])
                    if name == 'source_records' and entry['source_binding'] is None:
                        if read_scope != 'metadata_only':
                            raise PermissionError('non-native Claim source selection is metadata-only')
                    elif selection['verify_content'] and read_scope != 'exact_owner_local':
                        raise PermissionError('exact Claim evidence is outside a selected source access scope')
        if len(identifiers) > MAX_ASSESSMENTS:
            raise ValueError('private source selection exceeds the assessment record budget')
        self.explicit_ids = set(identifiers)
        self.context_digest = context.snapshot()
        self._load_grammar()
        for selection in selections:
            reader = OwnerLocalSourceRecordProfiles(context, selection['source_access'],
                selection['source_binding'], source_reader=self._read)
            entry = next((row for row in reader.registry['types']
                          if row['type_id'] == selection['profile_type_id']), None)
            profile = entry.get('source_record_profile') if entry else None
            if profile is None or profile['reader'] != 'semantic-metadata-v1':
                raise PermissionError('private assessment requires a declared semantic metadata profile')
            body = reader.load(profile['record_type'], selection['path'])
            if body['record_id'] != selection['record_id']:
                raise ValueError('private source does not carry the selected record identity')
            subject = self._add(body, 'record_id', 'record_version', selection['origin_id'], selection['path'])
            self.readers.append(reader)
            self.contracts.update(reader.input_digests)
            self._load_forms(selection, subject)
        for selection in claim_selections:
            from source_owner_claim_profiles import OwnerLocalSourceClaimProfiles
            reader = OwnerLocalSourceClaimProfiles(context, selection['source_access'],
                source_records=selection['source_records'], native_bindings=selection['native_bindings'],
                verify_content=selection['verify_content'], source_reader=self._read)
            claim = reader.load(selection['path'], selection['claim_id'], origin_id=selection['origin_id'],
                                relation_type_id=selection['relation_type_id'])
            subject = self._add_record(claim, selection['path'])
            source_paths = {row['record_id']: row['path'] for row in selection['source_records']}
            for row in reader.records:
                self._add_record(row, source_paths.get(row['id']))
                if (row['payload'].get('schema_version') == 'tos_native_text_unit_assessment_subject_v1'
                        or any(row['id'] == ref['id'] for summary in reader.native_summaries
                               for ref in summary['record_refs'])):
                    self.native_records.append(row)
            self.native_summaries.extend(reader.native_summaries)
            self.claim_dependencies[subject.id] = reader.dependency_refs(subject.id)
            self.claim_languages[subject.id] = set(reader.required_languages(subject.id))
            self.readers.append(reader)
            self.contracts.update(reader.contract_digests)
            self._load_forms(selection, subject, claim=True)
        self.snapshot()

    def _load_forms(self, selection, subject, *, claim=False):
        if not selection['form_ids']:
            return
        from source_commands import _validate_history
        from source_witness_human_forms import claim_forms_path
        path = Path(selection['path'])
        form_path = (claim_forms_path(path, subject.id) if claim
                     else path.with_name(path.stem + '.human-forms.json')).as_posix()
        raw = self.context.read_bytes(self.context.path(form_path), 2 * MAX_RECORD_BYTES, read_bytes=self._read)
        package = _json_object(raw)
        _validate_history(package, validator=self.form_validator)
        if package['subject'] != subject.ref:
            raise JournalConflict('private form set binds a different source snapshot')
        forms = {row['form_id']: row for row in package['forms']}
        for identifier in selection['form_ids']:
            if identifier not in forms:
                raise ValueError('private form is absent or is not a current adjacent form')
            form = forms[identifier]
            if form['subject'] != subject.ref:
                raise JournalConflict('private form binds a different source snapshot')
            self._add(form, 'form_id', 'form_version', selection['origin_id'], form_path)
            if claim:
                self.claim_dependencies[identifier] = [subject.ref, *self.claim_dependencies[subject.id]]
                self.claim_languages[identifier] = set(self.claim_languages[subject.id])
        self.form_sets[form_path] = package

    def _load_grammar(self):
        """One protected grammar serves history, policy and rendering alike."""
        schemas = {}
        for name in ('knowledge-assessment', 'knowledge-assessment-policy',
                     'knowledge-assessment-authority', 'knowledge-assessment-competence',
                     'knowledge-assessment-batch', 'human-form', 'human-form-set', 'human-form-template'):
            ref = 'ToS/contracts/' + name + '.schema.json'
            raw = self.context.read_bytes(self.context.path(ref), MAX_RECORD_BYTES, read_bytes=self._read)
            schema = _json_object(raw)
            if schema.get('$id') != 'https://treeofsophia.local/' + ref:
                raise ValueError('private assessment/form grammar differs from its declared identity')
            Draft202012Validator.check_schema(schema)
            schemas[name] = schema
            self.contracts[ref] = hashlib.sha256(raw).hexdigest()
        registry = Registry().with_resources((schema['$id'], Resource.from_contents(schema))
                                             for schema in schemas.values())
        self.assessment_validators = {
            suffix or '-assessment': Draft202012Validator(schemas['knowledge-assessment' + suffix],
                registry=registry, format_checker=FormatChecker())
            for suffix in ('', '-policy', '-authority', '-competence', '-batch')}
        from human_forms import compile_source_form_validators
        self.form_validator, self.materializer_validators = compile_source_form_validators(schemas)

    def required_languages(self, identifier, sourced):
        """Languages of the selected private subject and its explicit sources.

        A description maker stays distinct from a segmentation maker. Actual
        language fields constrain competence scope, never infer a translation.
        """
        bodies = {row['id']: row['payload'] for row in sourced}
        body = bodies.get(identifier)
        if body is None:
            return set()
        selected = {identifier}
        if body.get('schema_version') == 'tos_human_form_v1':
            selected.add(body['subject']['id'])
            selected.update(binding['record']['id'] for binding in body['bindings'].values())
        result = {language.casefold() for language in self.claim_languages.get(identifier, ())}
        for identity in selected:
            body = bodies.get(identity, {})
            fields = body.get('field_languages', {})
            scopes = [body, body.get('semantic_content', {}), body.get('semantic_scope', {}),
                      *(fields.values() if isinstance(fields, dict) else ())]
            if body.get('record_type') == 'lexical-form':
                scopes.append(body.get('form_identity', {}))
            result.update(value['language'].casefold() for value in scopes if isinstance(value, dict)
                          and isinstance(value.get('language'), str) and value['language'])
        return result

    def required_sources(self, identifier):
        """Current exact Claim grounding; never inferred from review prose."""
        result = []
        for ref in self.claim_dependencies.get(identifier, ()):
            row = self._indexed.get(ref['id'])
            record = Record.from_payload(**row) if row is not None else None
            if record is None or record.ref != ref:
                raise JournalConflict('private Claim grounding differs from the selected source snapshot')
            if record.id == identifier:
                raise ValueError('private Claim grounding cannot depend on its own assessment target')
            result.append(record)
        return tuple(result)

    def load_quality_grammar(self):
        """V5-only grammar: private v4 keeps its exact original read closure."""
        ref = 'ToS/contracts/native-text-layer-quality-basis.schema.json'
        raw = self.context.read_bytes(self.context.path(ref), MAX_RECORD_BYTES, read_bytes=self._read)
        schema = _json_object(raw)
        if schema.get('$id') != 'https://tree-of-sophia.local/' + ref:
            raise ValueError('quality basis grammar has another source identity')
        Draft202012Validator.check_schema(schema)
        self.contracts[ref] = hashlib.sha256(raw).hexdigest()
        return Draft202012Validator(schema, format_checker=FormatChecker())

    def _add(self, body, identity, version, origin, path):
        record = Record.from_payload(body[identity], body[version], body, origin_id=origin)
        return self._add_record({'id': record.id, 'version': record.version,
                                'payload': body, 'origin_id': origin}, path)

    def _add_record(self, row, path=None):
        record = Record.from_payload(**row)
        value = {'id': record.id, 'version': record.version, 'payload': record.payload,
                 'origin_id': record.origin_id}
        old = self._indexed.get(record.id)
        if old is not None and _canonical(old) != _canonical(value):
            raise ValueError('private source dependency shadows another body, version or origin')
        if path is not None:
            if record.id in self.paths and self.paths[record.id] != path:
                raise ValueError('private source dependency has more than one selected source path')
            self.paths[record.id] = path
        if old is None:
            if len(self.records) >= MAX_ASSESSMENTS:
                raise ValueError('private source and grounding records exceed the assessment budget')
            self._indexed[record.id] = value
            self.records.append(value)
        return record

    def _read(self, path, limit):
        if path not in self.observed and len(self.observed) >= 128:
            raise ValueError('private assessment source file-count budget exceeded')
        budget = min(limit, 8 * MAX_RECORD_BYTES - self.total) if path not in self.observed else limit
        raw = _read(path, budget, private_root=self.context.private_root
                    if path.is_relative_to(self.context.private_root) else None)
        digest = hashlib.sha256(raw).hexdigest()
        if path in self.observed and self.observed[path][0] != digest:
            raise JournalConflict('private assessment source changed during selection')
        if path not in self.observed:
            self.total += len(raw)
            self.observed[path] = (digest, len(raw))
        return raw

    def snapshot(self):
        if self.context.snapshot() != self.context_digest:
            raise JournalConflict('private assessment transport changed')
        readers = [reader.snapshot() for reader in self.readers]
        for path, (_digest, size) in list(self.observed.items()):
            self._read(path, size)
        return 'sha256:' + hashlib.sha256(_canonical({
            'context': self.context_digest, 'readers': readers,
            'sources': {str(path): digest for path, (digest, _size) in self.observed.items()},
        })).hexdigest()
