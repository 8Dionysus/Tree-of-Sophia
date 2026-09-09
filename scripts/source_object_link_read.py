"""Exact retained object-Link v1 source return; never a write profile.

This adapter reads the legacy stream only. Its five bibliographic subject
kinds and source-declared uncertainty remain unchanged by newer Link routes.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource
from referencing.exceptions import Unresolvable
from source_record_profiles import (
    MAX_CLAIM_FILE_BYTES, MAX_RECORD_BYTES, _read_json, _unique_object, _nonfinite,
)

SOURCE_REF = 'ToS/source-witnesses/relations/object-link/object-link-claims.jsonl'
SCHEMA_REF = 'ToS/contracts/object-link-claim.schema.json'
SCHEMA_VERSION = 'tos_object_link_claim_v1'
SUBJECT_KINDS = frozenset({'work', 'expression', 'edition', 'collection', 'item'})
PREDICATES = frozenset({'described_by', 'metadata_at', 'downloadable_at', 'rights_statement_at'})


class LegacyObjectLinkError(ValueError):
    """The retained source cannot be returned without changing its meaning."""


def canonical_digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
        separators=(',', ':'), allow_nan=False).encode('utf-8')).hexdigest()


class LegacyObjectLinkReader:
    """One strict source/schema snapshot, shared by both derived carriers."""

    def __init__(self, root: Path):
        self.root = Path(root)
        self.input_digests = {}
        schema = _read_json(self.root, SCHEMA_REF, self.input_digests)
        if schema.get('$id') != 'https://tree-of-sophia.local/' + SCHEMA_REF:
            raise LegacyObjectLinkError('legacy object-Link schema identity differs from its owner path')
        Draft202012Validator.check_schema(schema)
        registry = Registry().with_resource(schema['$id'], Resource.from_contents(schema))
        self.validator = Draft202012Validator(schema, registry=registry)
        target = self.root / SOURCE_REF
        if target.is_symlink() or not target.is_file() or target.resolve() != target.absolute():
            raise LegacyObjectLinkError('legacy object-Link stream must be its exact regular source file')
        with target.open('rb') as stream:
            raw = stream.read(MAX_CLAIM_FILE_BYTES + 1)
        if len(raw) > MAX_CLAIM_FILE_BYTES:
            raise LegacyObjectLinkError('legacy object-Link stream exceeds its source byte budget')
        self.input_digests[SOURCE_REF] = hashlib.sha256(raw).hexdigest()
        self._rows, seen = {}, set()
        for number, line in enumerate(raw.splitlines(), start=1):
            if not line.strip():
                continue
            if len(line) > MAX_RECORD_BYTES:
                raise LegacyObjectLinkError('legacy object-Link Claim exceeds its source byte budget')
            try:
                claim = json.loads(line, object_pairs_hook=_unique_object, parse_constant=_nonfinite)
            except (ValueError, UnicodeError) as error:
                raise LegacyObjectLinkError('legacy object-Link Claim is not strict JSON') from error
            self.validate(claim)
            if claim['claim_id'] in seen:
                raise LegacyObjectLinkError('duplicate retained object-Link Claim identity')
            seen.add(claim['claim_id'])
            self._rows[number] = claim

    def validate(self, claim, objects=None):
        try:
            valid = self.validator.is_valid(claim)
        except Unresolvable as error:
            raise LegacyObjectLinkError('legacy object-Link schema has an undeclared dependency') from error
        if (not isinstance(claim, dict) or claim.get('schema_version') != SCHEMA_VERSION
                or not valid or claim.get('predicate') not in PREDICATES):
            raise LegacyObjectLinkError('retained object-Link Claim violates its exact legacy contract')
        if claim['subject_ref'].split('.')[1] not in SUBJECT_KINDS:
            raise LegacyObjectLinkError('legacy object-Link subject is outside its five-kind domain')
        if objects is not None:
            subject, target = objects.get(claim['subject_ref']), objects.get(claim['object'])
            if (not isinstance(subject, dict) or subject.get('record_type') not in SUBJECT_KINDS
                    or not isinstance(target, dict) or target.get('record_type') != 'link'):
                raise LegacyObjectLinkError('legacy object-Link endpoints do not resolve in their exact domains')

    def rows(self):
        return [(line, copy.deepcopy(claim)) for line, claim in self._rows.items()]

    def from_catalog(self, entry):
        line = entry.get('source_claim_line')
        if entry.get('source_claim_file_ref') != SOURCE_REF or type(line) is not int or line not in self._rows:
            raise LegacyObjectLinkError('legacy object-Link catalog must name its exact source file and line')
        claim = self._rows[line]
        if (entry.get('claim_id') != claim['claim_id']
                or entry.get('claim_sha256') != canonical_digest(claim)
                or entry.get('source_schema_ref', SCHEMA_REF) != SCHEMA_REF):
            raise LegacyObjectLinkError('legacy object-Link catalog source identity or digest drifted')
        return copy.deepcopy(claim)

    def context(self, line, claim):
        if type(line) is not int or self._rows.get(line) != claim:
            raise LegacyObjectLinkError('legacy object-Link context is not the selected retained row')
        return {'source_claim': copy.deepcopy(claim), 'source_claim_file_ref': SOURCE_REF,
                'source_claim_line': line, 'source_sha256': canonical_digest(claim),
                'source_schema_ref': SCHEMA_REF, 'source_adapter': 'retained-object-link-v1'}

    def verify_current(self):
        for ref, digest in self.input_digests.items():
            target = self.root / ref
            if target.is_symlink() or not target.is_file() or target.resolve() != target.absolute():
                raise LegacyObjectLinkError('legacy object-Link source/schema changed during source return')
            limit = MAX_CLAIM_FILE_BYTES if ref == SOURCE_REF else MAX_RECORD_BYTES
            with target.open('rb') as stream:
                raw = stream.read(limit + 1)
            if len(raw) > limit or hashlib.sha256(raw).hexdigest() != digest:
                raise LegacyObjectLinkError('legacy object-Link source/schema changed during source return')
