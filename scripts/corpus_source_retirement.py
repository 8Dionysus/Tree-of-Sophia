"""Check a reviewed source retirement's exact provenance bindings.

This is an application of the existing provenance-event contract. The record
describes active-to-historical source membership; it grants neither a right to
erase the historical bytes nor semantic, rights, publication or canon approval.
The referenced owner review supplies the source-visible judgment; this check
verifies its recorded provenance bindings.
"""
from __future__ import annotations

from collections import defaultdict
from datetime import datetime
import json

from jsonschema import Draft202012Validator, FormatChecker

from corpus_store import CorpusCandidate, CorpusStoreError, ValidationIndex, hex_digest, relative_path

SCHEMA_REF = 'ToS/contracts/provenance-event.schema.json'
METHOD = 'corpus-source-retirement'
MAX_EVENT_BYTES = 1024 * 1024


def _json(raw: bytes) -> dict:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise CorpusStoreError('retirement JSON contains duplicate fields')
            result[key] = value
        return result

    try:
        result = json.loads(raw, object_pairs_hook=pairs)
        json.dumps(result, allow_nan=False)
    except (ValueError, UnicodeError) as error:
        raise CorpusStoreError('retirement input is not finite JSON') from error
    if not isinstance(result, dict):
        raise CorpusStoreError('retirement input must be a JSON object')
    return result


def validate_retirements(candidate: CorpusCandidate, base: dict | None) -> dict[str, str]:
    """Validate every new retirement, returning the event ID's owning path."""
    if not candidate.retirements:
        return {}
    if base is None:
        raise CorpusStoreError('source retirement requires an accepted base')
    schema = _json(candidate.read_bytes(SCHEMA_REF, max_bytes=MAX_EVENT_BYTES))
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    groups = defaultdict(list)
    for retirement in candidate.retirements:
        groups[retirement['event_ref']].append(retirement)
    identities = {}
    for event_ref, retirements in sorted(groups.items()):
        if not event_ref.startswith('ToS/source-witnesses/retirements/') or not event_ref.endswith('.json'):
            raise CorpusStoreError('retirement event must use the source retirement owner path')
        event = _json(candidate.read_bytes(event_ref, max_bytes=MAX_EVENT_BYTES))
        issues = list(validator.iter_errors(event))
        if issues:
            raise CorpusStoreError(f'{event_ref}: retirement event violates provenance schema: {issues[0].message}')
        method = event['method']
        if (event['schema_version'] != 'tos_provenance_event_v1'
                or event['event_type'] != 'migration'
                or not event['event_id'].startswith('tos.event.')
                or event['status'] not in {'completed', 'completed_with_warnings'}
                or method['name'] != METHOD or method['version'] != '1'):
            raise CorpusStoreError('retirement event does not declare the source retirement operation')
        if datetime.fromisoformat(event['ended_at'].replace('Z', '+00:00')) < datetime.fromisoformat(
                event['started_at'].replace('Z', '+00:00')):
            raise CorpusStoreError('retirement event ends before it starts')
        configuration = method['configuration']
        if set(configuration) != {'base_revision', 'retirements', 'reason', 'review_ref', 'review_sha256'}:
            raise CorpusStoreError('retirement configuration must bind base, exact targets and owner review')
        expected_targets = sorted(({'path': item['path'], 'sha256': item['sha256']} for item in retirements),
                                  key=lambda item: item['path'])
        if configuration['base_revision'] != base['revision'] or configuration['retirements'] != expected_targets:
            raise CorpusStoreError('retirement targets or accepted base differ from the source batch')
        if not isinstance(configuration['reason'], str) or not configuration['reason'].strip():
            raise CorpusStoreError('retirement needs a source-visible reason')
        review_ref = relative_path(configuration['review_ref'])
        review_sha256 = hex_digest(configuration['review_sha256'])
        if not review_ref.startswith('ToS/review-ledger/') or review_ref == event_ref:
            raise CorpusStoreError('retirement review must return to the source-owned review ledger')
        review = candidate.entry(review_ref)
        if review['sha256'] != review_sha256 or review['size_bytes'] == 0:
            raise CorpusStoreError('retirement review is empty or has a different digest')
        # The review may be larger than a small event. Stream it privately;
        # no content is interpreted here as a fabricated approval decision.
        candidate.materialize([review_ref])
        expected_inputs = [
            {'ref': item['path'], 'role': 'retired_source', 'sha256': item['sha256']}
            for item in expected_targets
        ] + [{'ref': review_ref, 'role': 'source_owner_review', 'sha256': review_sha256}]
        if event['inputs'] != expected_inputs:
            raise CorpusStoreError('retirement provenance inputs do not bind the exact sources and review')
        if event['outputs'] != [{'ref': event_ref, 'role': 'corpus_retirement_event'}]:
            raise CorpusStoreError('retirement provenance output must name this retained event')
        if event.get('receipt_refs') != [review_ref]:
            raise CorpusStoreError('retirement receipt must return to its exact owner review')
        identity = event['event_id']
        if identity in identities:
            raise CorpusStoreError('duplicate source retirement event ID')
        identities[identity] = event_ref
    return identities


def membership_transition(candidate: CorpusCandidate, base: dict | None,
                          event_identities: dict[str, str]) -> ValidationIndex | None:
    """Carry accepted facts through an exact retirement-only transaction.

    No surviving source record may change. New bytes are only the already
    validated retirement events and their new source-owner review documents.
    Unresolved incoming dependencies reject; a broader source edit returns to
    its source validator instead of receiving this narrower result.
    """
    if base is None or not candidate.retirements:
        return None
    retired = {event['path'] for event in candidate.retirements}
    if any(not path.startswith('ToS/source-witnesses/') for path in retired):
        return None
    event_paths = set(event_identities.values())
    review_paths = set()
    for path in event_paths:
        event = _json(candidate.read_bytes(path, max_bytes=MAX_EVENT_BYTES))
        review_paths.add(event['method']['configuration']['review_ref'])
    previous = {entry['path']: entry for entry in base['files']}
    # A different event version is a source correction, not a new tombstone
    # admitted by this narrow transition. Earlier event bytes remain recorded.
    if event_paths & previous.keys():
        return None
    added = candidate.paths - previous.keys()
    if added != event_paths | (review_paths - previous.keys()):
        return None
    if previous.keys() - candidate.paths != retired:
        return None
    if any(dict(candidate.entry(path)) != entry for path, entry in previous.items() if path not in retired):
        return None
    dependencies = {}
    for source, targets in base['dependencies'].items():
        if source in retired:
            continue
        if retired.intersection(targets):
            raise CorpusStoreError(f'retirement leaves an incoming source dependency unresolved: {source}')
        dependencies[source] = list(targets)
    identities = {identity: path for identity, path in base['identities'].items() if path not in retired}
    for identity, path in event_identities.items():
        if identity in base['identities']:
            raise CorpusStoreError('retirement event reuses an accepted source identity')
        identities[identity] = path
    for path in event_paths:
        event = _json(candidate.read_bytes(path, max_bytes=MAX_EVENT_BYTES))
        dependencies[path] = [event['method']['configuration']['review_ref']]
    return ValidationIndex(identities, dependencies)
