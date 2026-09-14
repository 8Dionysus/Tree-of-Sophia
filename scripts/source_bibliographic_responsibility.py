"""Pure Expression translator-attachment checks, never source assessment.

The canonical registry owns the relation and typed schema. These checks own
only this append delta and exact current carrier closure. Competing attribution
Claims, including repeated endpoints, remain distinct qualified assertions.
"""
import json


def _refs(values):
    return (isinstance(values, list) and all(isinstance(value, str) and value for value in values)
            and len(values) == len(set(values)))


def validate_expression_responsibility_delta(before, after, agent, claim):
    if not all(isinstance(value, dict) for value in (before, after, agent, claim)):
        raise ValueError('responsibility attachment requires exact source objects')
    excluded = {'record_version', 'responsibility_claim_refs'}
    if (before.get('record_type') != 'expression' or after.get('record_type') != 'expression'
            or before.get('record_id') != after.get('record_id')
            or type(before.get('record_version')) is not int or before['record_version'] < 1
            or type(after.get('record_version')) is not int or after['record_version'] != before['record_version'] + 1
            or json.dumps({key: value for key, value in before.items() if key not in excluded}, sort_keys=True)
            != json.dumps({key: value for key, value in after.items() if key not in excluded}, sort_keys=True)):
        raise ValueError('attachment must preserve every Expression field except exact version and responsibility append')
    refs, identity = before.get('responsibility_claim_refs'), claim.get('claim_id')
    if (not _refs(refs) or not isinstance(identity, str) or not identity or identity in refs
            or after.get('responsibility_claim_refs') != [*refs, identity]):
        raise ValueError('responsibility_claim_refs must append exactly one new Claim identity')
    if (agent.get('record_type') != 'agent' or claim.get('subject_ref') != before.get('record_id')
            or claim.get('object') != agent.get('record_id') or not isinstance(agent.get('record_id'), str)
            or claim.get('schema_version') != 'tos_source_relation_claim_v1'
            or claim.get('claim_type') != 'relation' or claim.get('predicate') != 'translated_by'
            or claim.get('assertion_layer') not in {'bibliographic_assertion', 'scholarly_report'}
            or type(claim.get('claim_version')) is not int or claim['claim_version'] != 1
            or claim.get('review_status') != 'unreviewed' or claim.get('assessment_refs', []) != []
            or claim.get('supersedes_claim_ref') is not None or claim.get('visibility') != 'public_metadata_only'):
        raise ValueError('attachment requires an unreviewed qualified translator Claim to the existing Agent')
    validate_qualified_translator_claim(claim)
    if not _refs(claim.get('evidence_refs')) or not claim['evidence_refs']:
        raise ValueError('translator Claim requires explicit attribution evidence, distinct from endpoint bindings')


def validate_qualified_translator_claim(claim):
    qualifiers = claim.get('qualifiers')
    if (not isinstance(qualifiers, dict) or any(not isinstance(qualifiers.get(key), str) or not qualifiers[key].strip()
            for key in ('statement', 'statement_language', 'statement_script', 'attribution_scope'))):
        raise ValueError('translator Claim requires explicit statement language/script and attribution_scope qualification')


def validate_expression_responsibility_closure(expression, agents, claims):
    """Caller supplies the complete verified current responsibility union."""
    identities = set()
    for claim in claims:
        identity = claim.get('claim_id')
        agent = agents.get(claim.get('object')) if isinstance(claim.get('object'), str) else None
        if (not isinstance(identity, str) or identity in identities
                or claim.get('subject_ref') != expression.get('record_id')
                or claim.get('predicate') != 'translated_by' or agent is None or agent.get('record_type') != 'agent'):
            raise ValueError('Expression responsibility union has duplicate identities or mistyped endpoints')
        identities.add(identity)
    refs = expression.get('responsibility_claim_refs')
    if not _refs(refs) or set(refs) != identities:
        raise ValueError('Expression responsibility refs do not close over all verified current Claims')
