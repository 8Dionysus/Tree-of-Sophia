#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import pathlib
import sys
from typing import Any

from jsonschema import Draft202012Validator

PART_ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = pathlib.Path(__file__).resolve().parents[5]
SRC = PART_ROOT / 'config/tos_agon_threshold_intakes.config.json'
OUT = PART_ROOT / 'generated/tos_agon_threshold_intake_registry.min.json'
ENTRY_SCHEMA = PART_ROOT.parent / 'threshold-intake/schemas/tos-agon-threshold-intake.schema.json'
REGISTRY_SCHEMA = PART_ROOT / 'schemas/tos-agon-threshold-intake-registry.schema.json'
BUILDER = PART_ROOT / 'scripts/build_tos_agon_threshold_intake_registry.py'
ITEM_KEY = 'threshold_intakes'
REGISTRY_ID = 'tos.agon_threshold_intake.registry.v1'
REVIEW_PHASE_ORDER = 'XVIII'
REVIEW_PHASE_LABEL = 'Sophian Threshold'
RUNTIME_POSTURE = 'candidate_only'
UNIQUE_KEY_FIELD = 'intake_id'
REQUIRED_FORBIDDEN = ['live_verdict_authority', 'durable_scar_write', 'retention_execution', 'rank_mutation', 'trust_mutation', 'tree_of_sophia_canon_write', 'direct_tos_write', 'automatic_canonization', 'kag_as_canon', 'hidden_scheduler_action', 'assistant_contestant_drift', 'auto_doctrine_rewrite', 'center_takeover_of_owner_truth', 'eval_memo_stats_sovereignty', 'stats_as_canon_authority', 'eval_as_canon_authority', 'memo_as_canon_authority', 'sdk_hidden_write']
REQUIRED_STOP_LINES = []
ALLOWED_RUNTIME = ['none', 'candidate_only', 'local_dry_run_candidate_only', 'local_rehearsal_candidate_only']


def fail(message: str) -> int:
    print(message, file=sys.stderr)
    return 1


def load_json(path: pathlib.Path) -> Any:
    return json.loads(path.read_text(encoding='utf-8'))


def load_builder():
    spec = importlib.util.spec_from_file_location('_agon_threshold_registry_builder', BUILDER)
    if spec is None or spec.loader is None:
        raise RuntimeError(f'cannot load builder {BUILDER}')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def schema_error(schema_path: pathlib.Path, payload: Any, label: str) -> str | None:
    schema = load_json(schema_path)
    Draft202012Validator.check_schema(schema)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: [str(part) for part in error.path],
    )
    if not errors:
        return None
    error = errors[0]
    path = '.'.join(str(part) for part in error.path) or '<root>'
    return f'{label} schema violation at {path}: {error.message}'


def require_string_list(value: Any, field: str, key: str) -> str | None:
    if not isinstance(value, list) or not value:
        return f'{key} {field} must be a non-empty list'
    if any(not isinstance(item, str) or not item for item in value):
        return f'{key} {field} must contain non-empty strings only'
    return None


def validate_source_metadata(source: dict[str, Any]) -> str | None:
    expected = {
        'registry_id': REGISTRY_ID,
        'review_phase_order': REVIEW_PHASE_ORDER,
        'review_phase_label': REVIEW_PHASE_LABEL,
        'runtime_posture': RUNTIME_POSTURE,
    }
    for field, expected_value in expected.items():
        if source.get(field) != expected_value:
            return f'source {field} must be {expected_value!r}'
    return None


def validate_item(item: dict[str, Any]) -> str | None:
    key = item.get(UNIQUE_KEY_FIELD)
    if not isinstance(key, str) or not key:
        return f'missing {UNIQUE_KEY_FIELD}'
    if item.get('review_phase_order') != REVIEW_PHASE_ORDER:
        return f'{key} review_phase_order must be {REVIEW_PHASE_ORDER}'
    if item.get('live_protocol') is not False:
        return f'{key} live_protocol must be false'
    if item.get('assistant_contestant_allowed') is not False:
        return f'{key} assistant_contestant_allowed must be false'
    if item.get('runtime_effect') not in ALLOWED_RUNTIME:
        return f'{key} invalid runtime_effect: {item.get("runtime_effect")}'
    if item.get('review_status') != 'candidate_only':
        return f'{key} review_status must be candidate_only'
    if item.get('requires_review') is not True:
        return f'{key} requires_review must be true'
    for flag in ('tos_canonization_allowed', 'direct_tos_write_allowed', 'canon_write_allowed'):
        if item.get(flag) is not False:
            return f'{key} {flag} must be false'
    evidence_err = require_string_list(item.get('required_evidence'), 'required_evidence', key)
    if evidence_err:
        return evidence_err
    forbidden_err = require_string_list(item.get('forbidden_effects'), 'forbidden_effects', key)
    if forbidden_err:
        return forbidden_err
    forbidden = set(item['forbidden_effects'])
    for required in REQUIRED_FORBIDDEN:
        if required not in forbidden:
            return f'{key} missing forbidden effect {required}'
    return None


def validate() -> int:
    if not SRC.exists():
        return fail(f'missing source {SRC}')
    if not OUT.exists():
        return fail(f'missing generated registry {OUT}')
    for schema_path in (ENTRY_SCHEMA, REGISTRY_SCHEMA):
        if not schema_path.exists():
            return fail(f'missing schema {schema_path}')

    source = load_json(SRC)
    err = validate_source_metadata(source)
    if err:
        return fail(err)
    items = source.get(ITEM_KEY)
    if not isinstance(items, list):
        return fail(f'source {ITEM_KEY} must be a list')
    expected_count = len(items)
    if expected_count == 0:
        return fail(f'source {ITEM_KEY} must not be empty')

    seen: set[str] = set()
    for item in items:
        if not isinstance(item, dict):
            return fail(f'source {ITEM_KEY} entries must be objects')
        err = schema_error(ENTRY_SCHEMA, item, f'source {UNIQUE_KEY_FIELD}')
        if err:
            return fail(err)
        err = validate_item(item)
        if err:
            return fail(err)
        key = item[UNIQUE_KEY_FIELD]
        if key in seen:
            return fail(f'duplicate {UNIQUE_KEY_FIELD} {key}')
        seen.add(key)

    registry = load_json(OUT)
    try:
        expected_registry = load_builder().build()
    except Exception as exc:  # builder errors should become validation failures, not tracebacks.
        return fail(f'cannot rebuild expected registry: {exc}')
    if registry != expected_registry:
        return fail('generated registry is stale or does not match builder output')
    err = schema_error(REGISTRY_SCHEMA, registry, 'generated registry')
    if err:
        return fail(err)
    if registry.get('count') != expected_count:
        return fail(f'generated count must match source item length {expected_count}')
    if registry.get('count') != len(registry.get(ITEM_KEY, [])):
        return fail('generated count does not match generated item length')

    print(json.dumps({'ok': True, 'item_key': ITEM_KEY, 'count': expected_count}, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(validate())
