"""Fixed Document catalogue-field semantics, not historical event admission."""
import json

MODULE_REF = 'scripts/source_document_catalogue.py'

READER = 'document-catalogue-temporal-v1'
SCHEMA_REF = 'ToS/contracts/document-catalogue-claim.schema.json'
SCHEMA_VERSION = 'tos_document_catalogue_claim_v1'
ROLE = 'catalogue-assigned-document-date'
ADAPTER = 'document-catalogue-time-source-wording-v1'
PREDICATE = 'document_catalogue_date'
FIELDS = {PREDICATE: 'assigned-date', 'document_catalogue_origin': 'origin',
          'document_catalogue_destination': 'destination'}
CREATE_CONFIG = 'tos_local_document_catalogue_date_create_owner_v1'
REVISION_CONFIG = 'tos_local_document_catalogue_date_revision_owner_v1'
CONFIGS = {CREATE_CONFIG, REVISION_CONFIG}


def validate_attribution(claim):
    """Bind field meaning and exact quoted wording to declared evidence only.

    This checks a declaration, never fetches or authenticates a catalogue.
    """
    attribution = claim.get('qualifiers', {}).get('catalogue_attribution', {})
    if (attribution.get('field_role') != FIELDS[claim['predicate']]
            or attribution.get('evidence_ref') not in claim.get('evidence_refs', [])):
        raise ValueError('document catalogue attribution must bind its exact field role and cited evidence')
    if claim['predicate'] == PREDICATE and json.dumps(
            attribution.get('source_wording'), sort_keys=True, ensure_ascii=False) != json.dumps(
            claim['object'].get('source_wording'), sort_keys=True, ensure_ascii=False):
        raise ValueError('document date wording must equal the attributed catalogue field wording')
