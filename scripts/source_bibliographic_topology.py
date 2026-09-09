"""Pure checks for declared bibliographic closure, not admission or write grants.

Callers own schema validation, exact byte/provenance resolution, authorization,
forms and atomic publication. Historical and native carriers can participate in
the same current closure only after their respective evidence is verified.
"""
from __future__ import annotations

from collections import Counter
import json
from pathlib import PurePosixPath
import re


TOPOLOGY_ROUTES = {
    'has_expression': ('work', 'expression', 'expression_claim_refs'),
    'embodied_by': ('expression', 'edition', 'embodiment_claim_refs'),
    'exemplified_by': ('edition', 'item', 'exemplar_claim_refs'),
}
MAX_ISSUES = 64


class BibliographicTopologyError(ValueError):
    """Bounded structural issues; never a textual/semantic assessment."""

    def __init__(self, issues):
        self.issues = tuple(issues[:MAX_ISSUES])
        super().__init__('; '.join(f'{where}: {message}' for where, message in self.issues))


def _refs(value):
    return (isinstance(value, list) and all(isinstance(ref, str) and ref for ref in value)
            and len(set(value)) == len(value))


def _fail(issues):
    if issues:
        raise BibliographicTopologyError(issues)


def validate_work_expression_delta(work_before, work_after, expression, claim, *,
                                   work_source_ref, expression_source_ref):
    """Check one append-only Work -> new Expression declaration.

    No existing descriptive field, authority posture, outgoing link or version
    history may be rewritten as a side effect. A full schema/profile check is
    still required by the caller; this function verifies the compound delta.
    """
    issues = []
    if not all(isinstance(value, dict) for value in (work_before, work_after, expression, claim)):
        raise BibliographicTopologyError([('delta', 'four source objects are required')])
    work_id = work_before.get('record_id')
    expression_id = expression.get('record_id')
    claim_id = claim.get('claim_id')
    if (work_before.get('record_type') != 'work' or work_after.get('record_type') != 'work'
            or not isinstance(work_id, str) or work_after.get('record_id') != work_id):
        issues.append(('work', 'the same existing Work identity must be preserved'))
    version = work_before.get('record_version')
    if (type(version) is not int or version < 1 or type(work_after.get('record_version')) is not int
            or work_after['record_version'] != version + 1):
        issues.append(('work', 'record_version must advance exactly once'))
    prior_refs = work_before.get('expression_claim_refs')
    if (not _refs(prior_refs) or not isinstance(claim_id, str) or not claim_id
            or claim_id in prior_refs
            or work_after.get('expression_claim_refs') != prior_refs + [claim_id]):
        issues.append(('work', 'expression_claim_refs must append exactly the new Claim'))
    excluded = {'record_version', 'expression_claim_refs'}
    if (json.dumps({key: value for key, value in work_before.items() if key not in excluded}, sort_keys=True)
            != json.dumps({key: value for key, value in work_after.items() if key not in excluded}, sort_keys=True)):
        issues.append(('work', 'all other Work fields must remain unchanged'))
    if (expression.get('record_type') != 'expression' or not isinstance(expression_id, str)
            or expression.get('work_ref') != work_id
            or type(expression.get('record_version')) is not int or expression['record_version'] != 1
            or expression.get('identity_status') != 'provisional'
            or expression.get('same_as_posture') != 'no_equivalence_claim'
            or expression.get('supersedes_ref') is not None):
        issues.append(('expression', 'one new provisional Expression must declare this Work without equivalence'))
    if (expression.get('responsibility_claim_refs') != []
            or expression.get('embodiment_claim_refs') != []
            or expression.get('derivation_claim_refs', []) != []):
        issues.append(('expression', 'initial creation cannot grant responsibility, embodiment or derivation'))
    for field in ('variant_labels', 'external_identifiers'):
        values = expression.get(field)
        if (not isinstance(values, list) or any(not isinstance(value, dict)
                or value.get('status') == 'verified' for value in values)):
            issues.append(('expression', f'{field} cannot introduce verified identity assertions'))
    if (claim.get('schema_version') != 'tos_source_relation_claim_v1'
            or claim.get('claim_type') != 'relation'
            or claim.get('assertion_layer') != 'bibliographic_assertion'
            or claim.get('predicate') != 'has_expression'
            or claim.get('subject_ref') != work_id or claim.get('object') != expression_id
            or type(claim.get('claim_version')) is not int or claim['claim_version'] != 1
            or claim.get('epistemic_status') != 'observed' or claim.get('polarity') != 'positive'
            or claim.get('confidence') is not None
            or claim.get('review_status') != 'unreviewed'
            or claim.get('assessment_refs', []) != [] or claim.get('supersedes_claim_ref') is not None
            or claim.get('visibility') != 'public_metadata_only'):
        issues.append(('claim', 'one new unreviewed public bibliographic relation must bind the exact endpoints'))
    evidence = claim.get('evidence_refs')
    if (not isinstance(work_source_ref, str) or not isinstance(expression_source_ref, str)
            or not _refs(evidence) or set(evidence) != {work_source_ref, expression_source_ref}):
        issues.append(('claim', 'evidence must be exactly the two linked metadata paths'))
    if not isinstance(work_source_ref, str) or not isinstance(expression_source_ref, str):
        issues.append(('delta', 'canonical source paths are required'))
    else:
        work_path, expression_path = PurePosixPath(work_source_ref), PurePosixPath(expression_source_ref)
        if (work_path.is_absolute() or '..' in work_path.parts
                or work_path.as_posix() != work_source_ref or work_path.name != 'work.json'
                or work_path.parts[:3] != ('ToS', 'source-witnesses', 'works')
                or expression_path.as_posix() != expression_source_ref
                or expression_path.parent.parent != work_path.parent / 'expressions'
                or expression_path.name != 'expression.json'
                or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', expression_path.parent.name)):
            issues.append(('delta', 'Expression must occupy one new canonical child of the exact Work home'))
    _fail(issues)


def validate_expression_edition_delta(expression_before, expression_after, edition, claim, *,
                                      expression_source_ref, edition_source_ref):
    """One existing Expression and a new provisional Edition, not all bibliography.

    This operation's single-Expression scope does not constrain other Edition
    homes, collections, documents or the global many-to-many embodiment model.
    No Item, File, responsibility, publication date or equivalence is inferred.
    """
    issues = []
    if not all(isinstance(value, dict) for value in (expression_before, expression_after, edition, claim)):
        raise BibliographicTopologyError([('delta', 'four source objects are required')])
    expression_id, edition_id, claim_id = (expression_before.get('record_id'),
                                          edition.get('record_id'), claim.get('claim_id'))
    if (expression_before.get('record_type') != 'expression'
            or expression_after.get('record_type') != 'expression'
            or not isinstance(expression_id, str) or expression_after.get('record_id') != expression_id):
        issues.append(('expression', 'the same existing Expression identity must be preserved'))
    version = expression_before.get('record_version')
    if (type(version) is not int or version < 1 or type(expression_after.get('record_version')) is not int
            or expression_after['record_version'] != version + 1):
        issues.append(('expression', 'record_version must advance exactly once'))
    prior_refs = expression_before.get('embodiment_claim_refs')
    if (not _refs(prior_refs) or not isinstance(claim_id, str) or not claim_id or claim_id in prior_refs
            or expression_after.get('embodiment_claim_refs') != prior_refs + [claim_id]):
        issues.append(('expression', 'embodiment_claim_refs must append exactly the new Claim'))
    excluded = {'record_version', 'embodiment_claim_refs'}
    if (json.dumps({key: value for key, value in expression_before.items() if key not in excluded}, sort_keys=True)
            != json.dumps({key: value for key, value in expression_after.items() if key not in excluded}, sort_keys=True)):
        issues.append(('expression', 'all other Expression fields must remain unchanged'))
    allowed = {'schema_version', 'record_type', 'record_id', 'record_version', 'preferred_label',
        'field_languages', 'variant_labels', 'identity_status', 'source_refs', 'external_identifiers',
        'same_as_posture', 'notes', 'supersedes_ref', 'embodies_expression_refs', 'edition_statement',
        'publication_claim_refs', 'provision_activity_claim_refs', 'exemplar_claim_refs',
        'responsibility_claim_refs'}
    if (set(edition) - allowed or edition.get('schema_version') != 'tos_corpus_record_v1'
            or edition.get('record_type') != 'edition' or not isinstance(edition_id, str)
            or edition.get('embodies_expression_refs') != [expression_id]
            or type(edition.get('record_version')) is not int or edition['record_version'] != 1
            or edition.get('identity_status') != 'provisional'
            or edition.get('same_as_posture') != 'no_equivalence_claim' or edition.get('supersedes_ref') is not None):
        issues.append(('edition', 'one new provisional single-Expression Edition must remain within this operation scope'))
    if (edition.get('publication_claim_refs') != [] or edition.get('exemplar_claim_refs') != []
            or edition.get('provision_activity_claim_refs', []) != []
            or edition.get('responsibility_claim_refs', []) != []):
        issues.append(('edition', 'initial creation cannot introduce publication, provision, Item or responsibility Claims'))
    for field in ('variant_labels', 'external_identifiers'):
        values = edition.get(field)
        if (not isinstance(values, list) or any(not isinstance(value, dict)
                or value.get('status') != 'unverified' for value in values)):
            issues.append(('edition', f'{field} require explicitly unverified identity assertions'))
    if (claim.get('schema_version') != 'tos_source_relation_claim_v1' or claim.get('claim_type') != 'relation'
            or claim.get('assertion_layer') != 'bibliographic_assertion' or claim.get('predicate') != 'embodied_by'
            or claim.get('subject_ref') != expression_id or claim.get('object') != edition_id
            or type(claim.get('claim_version')) is not int or claim['claim_version'] != 1
            or claim.get('epistemic_status') != 'observed' or claim.get('polarity') != 'positive'
            or claim.get('confidence') is not None or claim.get('review_status') != 'unreviewed'
            or claim.get('assessment_refs', []) != [] or claim.get('supersedes_claim_ref') is not None
            or claim.get('visibility') != 'public_metadata_only'):
        issues.append(('claim', 'one new unreviewed public bibliographic relation must bind the exact endpoints'))
    evidence = claim.get('evidence_refs')
    if (not isinstance(expression_source_ref, str) or not isinstance(edition_source_ref, str)
            or not _refs(evidence) or set(evidence) != {expression_source_ref, edition_source_ref}):
        issues.append(('claim', 'evidence must be exactly the two linked metadata paths'))
    if not isinstance(expression_source_ref, str) or not isinstance(edition_source_ref, str):
        issues.append(('delta', 'canonical source paths are required'))
    else:
        expression_path, edition_path = PurePosixPath(expression_source_ref), PurePosixPath(edition_source_ref)
        if (expression_path.is_absolute() or '..' in expression_path.parts
                or expression_path.as_posix() != expression_source_ref or expression_path.name != 'expression.json'
                or expression_path.parts[:3] != ('ToS', 'source-witnesses', 'works')
                or expression_path.parent.parent.name != 'expressions'
                or edition_path.as_posix() != edition_source_ref
                or edition_path.parent.parent != expression_path.parent / 'editions'
                or edition_path.name != 'edition.json'
                or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', edition_path.parent.name)):
            issues.append(('delta', 'Edition must occupy one new canonical child of the exact Expression home'))
    _fail(issues)


def validate_current_topology(records_by_id, claims, *, item_edition_by_id=None):
    """Check exact outgoing refs and inverse record links across all carriers.

    ``claims`` must include the complete verified current union, not just a new
    batch. Non-topology predicates are ignored. Item links come from separately
    verified item manifests; this helper never infers them from labels.
    """
    issues = []

    def issue(where, message):
        if len(issues) < MAX_ISSUES:
            issues.append((str(where)[:240], message))

    if not isinstance(records_by_id, dict) or not all(isinstance(value, dict) for value in records_by_id.values()):
        raise BibliographicTopologyError([('records', 'an identity-to-record mapping is required')])
    records = records_by_id
    for identity, record in records.items():
        if not isinstance(identity, str) or record.get('record_id') != identity:
            issue('records', 'record identity differs from its mapping key')
    expected = {predicate: {identity: set() for identity, record in records.items()
                           if record.get('record_type') == types[0]}
                for predicate, types in TOPOLOGY_ROUTES.items()}

    def link(predicate, subject, target):
        subject_type, object_type, _ = TOPOLOGY_ROUTES[predicate]
        if (not isinstance(subject, str) or subject not in records
                or records[subject].get('record_type') != subject_type
                or not isinstance(target, str) or target not in records
                or records[target].get('record_type') != object_type):
            issue(predicate, 'declared corpus-record link has an unresolved or mistyped endpoint')
            return
        expected[predicate][subject].add(target)

    for identity, record in records.items():
        if record.get('record_type') == 'expression':
            link('has_expression', record.get('work_ref'), identity)
        elif record.get('record_type') == 'edition':
            refs = record.get('embodies_expression_refs')
            if not _refs(refs):
                issue(identity, 'embodies_expression_refs must be distinct identities')
                continue
            for ref in refs:
                link('embodied_by', ref, identity)
    item_links = {} if item_edition_by_id is None else item_edition_by_id
    if not isinstance(item_links, dict):
        raise BibliographicTopologyError([('items', 'verified item-to-edition links must be a mapping')])
    for identity, record in records.items():
        if record.get('record_type') == 'item' and identity not in item_links:
            issue(identity, 'item has no separately verified embodiment link')
    for item, edition in item_links.items():
        link('exemplified_by', edition, item)

    actual = {predicate: {} for predicate in TOPOLOGY_ROUTES}
    seen_ids, seen_pairs, expression_works = set(), set(), {}
    counts = Counter({predicate: 0 for predicate in TOPOLOGY_ROUTES})
    for claim in claims:
        if not isinstance(claim, dict):
            issue('claims', 'each current Claim must be an object')
            continue
        predicate = claim.get('predicate')
        if not isinstance(predicate, str) or predicate not in TOPOLOGY_ROUTES:
            continue
        identity, subject, target = claim.get('claim_id'), claim.get('subject_ref'), claim.get('object')
        if not all(isinstance(value, str) and value for value in (identity, subject, target)):
            issue(predicate, 'topology Claims require identity endpoints')
            continue
        if identity in seen_ids:
            issue(identity, 'duplicate bibliographic topology Claim identity')
        seen_ids.add(identity)
        pair = (predicate, subject, target)
        if pair in seen_pairs:
            issue(identity, 'duplicate bibliographic topology pair')
        seen_pairs.add(pair)
        counts[predicate] += 1
        if target not in expected[predicate].get(subject, set()):
            issue(identity, 'claim is not backed by the declared corpus-record topology')
        if predicate == 'has_expression':
            if target in expression_works and expression_works[target] != subject:
                issue(identity, 'an Expression cannot have two Work owners')
            expression_works[target] = subject
        actual[predicate].setdefault(subject, {})[identity] = target
    for predicate, (subject_type, _, field) in TOPOLOGY_ROUTES.items():
        for identity, record in records.items():
            if record.get('record_type') != subject_type:
                continue
            refs = record.get(field)
            found = actual[predicate].get(identity, {})
            if not _refs(refs) or set(refs) != set(found):
                issue(identity, f'{field} does not close over exact {predicate} claims')
            if set(found.values()) != expected[predicate][identity]:
                issue(identity, f'{predicate} claims do not close over declared corpus-record links')
    _fail(issues)
    return dict(counts)
