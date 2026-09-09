"""Exact native Artifact, retained Composite and Link metadata correction.

Native identity fields and schemas remain source-owned. This independently
delegated selected-file writer neither imports a payload nor accepts identity,
rights, provenance, membership or observation transitions.
"""
from datetime import datetime, timezone
import os
from pathlib import Path
import re

import source_commands as source
import source_command_contracts as contract
from build_source_witness_catalog import native_witness_contract

CONFIG = source.NATIVE_METADATA_REVISION_CONFIG
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_native_metadata_commands.py'
REVISION_FIELDS = {
    'artifact': {'path_identity', 'physical_description', 'find_context', 'bibliography'},
    'composite': {'preferred_label', 'editorial_object'},
    'link': {'preferred_label', 'variant_labels', 'notes', 'source_refs', 'provider_label'},
}
BASE_KEYS = {'schema_version', 'uid', 'principal_id', 'source_root', 'source_path',
             'authority_ref', 'expires_at', 'allowed_operations', 'allowed_form_ids',
             'record_id', 'record_type', 'record_schema_version', 'allowed_fields'}


def record_profile(config, record):
    relative = config['source_path']
    if config['record_type'] == 'link':
        ref = Path(relative)
        if (ref.name != 'link.json' or not ref.is_relative_to('ToS/source-witnesses/links')
                or record.get('schema_version') != 'tos_source_link_v1'):
            raise PermissionError('Link correction requires its exact native owner contract')
        schema_ref, identity, kind = 'ToS/contracts/source-link.schema.json', 'record_id', 'link'
    else:
        schema_ref, identity, kind = native_witness_contract(record, relative)
    if (kind != config['record_type'] or record.get(identity) != config['record_id']
            or record.get('schema_version') != config['record_schema_version']):
        raise PermissionError('native metadata differs from its exact delegated identity and schema')
    return {'record_type': kind, 'identity_field': identity, 'id_prefix': 'tos.' + kind + '.',
            'source_basename': Path(relative).name, 'schema_ref': schema_ref,
            'schema_version': record['schema_version'], 'source_scope': 'public_metadata_only'}


def validate_record(config, record):
    profile = record_profile(config, record)
    raw = source._read(Path(config['source_root']) / profile['schema_ref'], source.MAX_COMMAND_BYTES)
    schema = source._json_object(raw)
    if schema.get('$id') != 'https://tree-of-sophia.local/' + profile['schema_ref']:
        raise ValueError('native metadata contract identity differs')
    source.Draft202012Validator.check_schema(schema)
    source.Draft202012Validator(schema, format_checker=source.FormatChecker()).validate(record)
    return {profile['schema_ref']: source._digest(raw),
            MODULE_REF: source._digest(source._read(source.ROOT / MODULE_REF, source.MAX_SET_BYTES)),
            'scripts/build_source_witness_catalog.py': source._digest(source._read(
                source.ROOT / 'scripts/build_source_witness_catalog.py', source.MAX_SET_BYTES))}


def validate_descriptive_delta(previous, revised, kind):
    allowed = REVISION_FIELDS[kind] | {'record_version'}
    if any(previous.get(key) != revised.get(key) for key in set(previous) | set(revised) if key not in allowed):
        raise PermissionError('native metadata correction changed an identity, relation or authority field')
    if kind == 'artifact' and any(previous['path_identity'][key] != revised['path_identity'][key]
                                  for key in ('basis', 'provider_independent')):
        raise PermissionError('descriptive correction cannot change the physical identity path basis')


def configuration(config, *, owner_config=None):
    source._keys(config, BASE_KEYS)
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int or config['uid'] != os.getuid()
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref', 'record_schema_version'))
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or config['record_type'] not in REVISION_FIELDS
            or not isinstance(config['record_id'], str)
            or not re.fullmatch(r'tos\.' + config['record_type'] + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['record_id'])):
        raise PermissionError('native metadata correction delegation is invalid or expired')
    for key, allowed in (('allowed_operations', {'record.revise', 'record.recover'}),
                         ('allowed_fields', REVISION_FIELDS[config['record_type']])):
        values = config[key]
        if (not isinstance(values, list) or len(values) != len(set(values))
                or any(value not in allowed for value in values)):
            raise PermissionError('native metadata correction exceeds its descriptive scope')
    forms = config['allowed_form_ids']
    if (not isinstance(forms, list) or not 1 <= len(forms) <= 32 or len(forms) != len(set(forms))
            or any(not isinstance(value, str) or not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value)
                   for value in forms)):
        raise PermissionError('native metadata correction requires bounded exact forms')
    from source_metadata_transactions import _path
    relative = _path(config['source_path'])
    if any(part.startswith('.') for part in relative.parts):
        raise PermissionError('native metadata correction must address a public owner source')
    root = Path(config['source_root'])
    os.close(source._owned_path(root, directory=True))
    # Do not read a possibly intermediate source here: recovery is authorized
    # from the protected descriptor, then reconstructs before/after exactly.
    basename = {'artifact': 'artifact-witness.json', 'composite': 'composite-witness.json', 'link': 'link.json'}
    subtree = {'artifact': 'artifacts', 'composite': 'scholarly-composites', 'link': 'links'}
    if (relative.name != basename[config['record_type']]
            or relative.parts[:3] != ('ToS', 'source-witnesses', subtree[config['record_type']])):
        raise PermissionError('native metadata correction requires its exact owner path')
    return config, source._digest(source._canonical(config)), root / relative


def command_handlers():
    from source_selected_revisions import run_selected_revision
    proposal = {'fields', 'forms', 'reason'}
    return (contract.Handler('native-witness-link-selected-revision', (CONFIG,),
        (contract.describe(),
         contract.operation('prepare-revise', proposal, definition='Prepare exact descriptive native metadata and source-copy forms.', grants=('record.revise',)),
         contract.operation('record.revise', proposal | contract.COMMIT_KEYS | {'expected_publication'},
             definition='Publish only the exact native record, form set and retained revision history.',
             mutation='selected_record_successor', grants=('record.revise',)),
         contract.recovery('record.recover'), contract.inspect_version()),
        run_selected_revision, 'Correct native Artifact, retained Composite or Link descriptions without changing their structural identity.',
        typed_handles=('ToS/contracts/artifact-source-witness.schema.json',
                       'ToS/contracts/artifact-source-witness-v2.schema.json',
                       'ToS/contracts/scholarly-composite-witness.schema.json',
                       'ToS/contracts/source-link.schema.json', *contract.FORM_HANDLES),
        profile_selection='Exact native schema, actual identity field and owner basename; no Corpus recasting.',
        preconditions=('No payload, observation, URI, membership, rights, provenance or authority revision is delegated.',),
        configure=configuration, manages_publication=True),)
