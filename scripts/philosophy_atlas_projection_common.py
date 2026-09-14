#!/usr/bin/env python3
"""Shared helpers for the ToS philosophy atlas projection export."""

from __future__ import annotations

import json
import hashlib
import copy
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

from philosophy_multilingual_common import content_language_contract, multilingual_label


REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
PROJECTION_PATH = TOS_ROOT / "derived-exports" / "philosophy_atlas_projection.min.json"
SCHEMA_REF = "ToS/contracts/philosophy-atlas-projection.schema.json"
SOURCE_ATLAS_REF = "ToS/philosophy/atlas/atlas.manifest.json"
DOSSIER_MANIFEST_REF = "ToS/philosophy/atlas/dossiers/branch.manifest.json"
# Existing manifest fields and dossier count fields, not new source identities.
BACKLOG_FIELDS = {
    "source_anchor_backlog": "source_anchor_count",
    "term_index": "term_count",
    "transmission_backlog": "transmission_count",
}
CANDIDATE_NODES_REF = "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl"
CANDIDATE_RELATIONS_REF = "ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl"
CANDIDATE_NODES_REFS = (
    CANDIDATE_NODES_REF,
    "ToS/philosophy/graph-workbench/proposed-nodes/table-ii-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-nodes/table-iii-prepared-dossiers.jsonl",
)
CANDIDATE_RELATIONS_REFS = (
    CANDIDATE_RELATIONS_REF,
    "ToS/philosophy/graph-workbench/proposed-relations/table-ii-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-relations/table-iii-prepared-dossiers.jsonl",
)
ENDPOINT_ALIASES_REF = (
    "ToS/philosophy/graph-workbench/proposed-relations/reviewed-endpoint-aliases.json"
)
VALIDATION_REFS = (
    "scripts/build_philosophy_atlas_projection.py",
    "scripts/validate_philosophy_atlas_projection.py",
    "tests/test_philosophy_atlas_projection.py",
)


def repo_ref(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path)} must contain a JSON object")
    return payload


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        payload = json.loads(line)
        if not isinstance(payload, dict):
            raise ValueError(f"{repo_ref(path)}:{line_number} must contain a JSON object")
        rows.append(payload)
    return rows


def load_optional_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    return load_jsonl(path)


class AuthoredAtlasSnapshot:
    """Exact public atlas/proposal inputs, not native corpus record admission.

    Row ordinals address parsed JSONL records; physical lines and exact byte
    digests are separate. Unknown nested source fields are never normalized.
    """

    def __init__(self, root: Path):
        self.root = root
        self.digests: dict[str, str] = {}

    def _bytes(self, ref: str) -> bytes:
        path = self.root / ref
        if (not ref.startswith('ToS/philosophy/') or path.is_symlink() or not path.is_file()
                or path.resolve() != path.absolute() or '..' in Path(ref).parts):
            raise ValueError('atlas source return requires an exact regular philosophy source file')
        with path.open('rb') as stream:
            raw = stream.read(32 * 1024 * 1024 + 1)
        if len(raw) > 32 * 1024 * 1024:
            raise ValueError('atlas source return exceeds its bounded source byte budget')
        return raw

    @staticmethod
    def _object(raw: bytes) -> dict[str, Any]:
        def unique(pairs):
            value = {}
            for key, entry in pairs:
                if key in value:
                    raise ValueError('duplicate JSON key in atlas source return')
                value[key] = entry
            return value

        def nonfinite(value):
            raise ValueError('nonfinite JSON value in atlas source return')

        record = json.loads(raw, object_pairs_hook=unique, parse_constant=nonfinite)
        if not isinstance(record, dict):
            raise ValueError('atlas source return requires a JSON object')
        return record

    def _context(self, ref, digest, record, **locator):
        if ref in self.digests and self.digests[ref] != digest:
            raise ValueError('atlas source changed during source return')
        self.digests[ref] = digest
        canonical = json.dumps(record, ensure_ascii=False, sort_keys=True,
                               separators=(',', ':'), allow_nan=False).encode('utf-8')
        return {'source_record': copy.deepcopy(record), 'source_record_ref': ref,
                'source_file_sha256': digest, 'source_record_sha256': hashlib.sha256(canonical).hexdigest(),
                **locator}

    def object(self, ref):
        raw = self._bytes(ref)
        record = self._object(raw)
        return record, self._context(ref, hashlib.sha256(raw).hexdigest(), record,
                                     source_format='json', source_pointer='')

    def rows(self, ref, identity_key):
        if not isinstance(identity_key, str) or not identity_key:
            raise ValueError('atlas keyed source return requires its declared identity field')
        return self._rows(ref, identity_key)

    def unkeyed_rows(self, ref):
        """Return every occurrence by exact file/row locator, never minted ID.

        Identical raw rows still occupy distinct source lines. A DOCX cell or
        optional dossier-local label is source context, not global identity.
        """
        return self._rows(ref, None)

    def _rows(self, ref, identity_key):
        raw = self._bytes(ref)
        rows, seen = [], set()
        # Bind an empty declared source too; absence of rows is not absence of input.
        digest = hashlib.sha256(raw).hexdigest()
        if ref in self.digests and self.digests[ref] != digest:
            raise ValueError('atlas source changed during source return')
        self.digests[ref] = digest
        for line, content in enumerate(raw.splitlines(), start=1):
            if not content.strip():
                continue
            record = self._object(content)
            if identity_key is not None:
                identity = record.get(identity_key)
                if not isinstance(identity, str) or not identity or identity in seen:
                    raise ValueError('atlas source row identity is missing or duplicated')
                seen.add(identity)
            if 'source_ref' in record and record['source_ref'] != ref:
                raise ValueError('atlas source row source_ref differs from its exact stream')
            rows.append((record, self._context(ref, digest, record, source_format='jsonl',
                                              source_row=len(rows) + 1, source_line=line)))
        return rows

    def verify_current(self):
        for ref, digest in self.digests.items():
            if hashlib.sha256(self._bytes(ref)).hexdigest() != digest:
                raise ValueError('atlas source changed during source return')


def build_dossier_source_backlogs(snapshot, dossiers):
    """Full existing backlog records attached to their already-owned dossier.

    Only the aggregate manifest routes supply these rows. Branch mirrors and
    reviewed discovery leads remain separate routes and are not counted twice.
    """
    manifest, _ = snapshot.object(DOSSIER_MANIFEST_REF)
    refs = [manifest.get(field) for field in BACKLOG_FIELDS]
    if (manifest.get('branch_id') != 'philosophy.atlas.dossiers'
            or any(not isinstance(ref, str) or not ref.endswith('.jsonl') for ref in refs)
            or len(set(refs)) != len(refs)):
        raise ValueError('atlas backlog manifest has missing or ambiguous source family refs')
    by_id = {row['dossier_id']: row for row in dossiers}
    if len(by_id) != len(dossiers):
        raise ValueError('atlas backlog parent dossier identity is ambiguous')
    result = {identity: {} for identity in by_id}
    for family, count_field in BACKLOG_FIELDS.items():
        ref = manifest[family]
        rows = snapshot.unkeyed_rows(ref)
        digest = snapshot.digests[ref]
        for identity in by_id:
            result[identity][family] = {'source_ref': ref, 'source_file_sha256': digest,
                                      'record_count': 0, 'records': []}
        for record, context in rows:
            parent = by_id.get(record.get('dossier_id'))
            if (parent is None or record.get('atlas_row_id') != parent['dossier_id']
                    or record.get('source_ref') != ref
                    or any(record.get(key) != parent.get(key) for key in ('source_document', 'branch_path'))
                    or 'table_id' in record and record['table_id'] != parent.get('table_id')):
                raise ValueError('atlas backlog source row has no exact matching parent dossier')
            result[parent['dossier_id']][family]['records'].append(context)
        for identity, dossier in by_id.items():
            family_context = result[identity][family]
            count = len(family_context['records'])
            if type(dossier.get(count_field)) is not int or dossier[count_field] != count:
                raise ValueError('atlas backlog source rows differ from the declared dossier count')
            family_context['record_count'] = count
    return result


def validate_dossier_source_backlogs(item):
    """Portable locator/body consistency; source admission remains elsewhere."""
    properties = item.get('properties') or {}
    if 'source_backlogs' not in properties:
        return
    families = properties['source_backlogs']
    parent = properties.get('source_record')
    if (not isinstance(families, dict) or set(families) != set(BACKLOG_FIELDS)
            or not isinstance(parent, dict) or not isinstance(parent.get('dossier_id'), str)
            or item.get('node_id') != 'atlas-dossier:' + parent['dossier_id']):
        raise ValueError('atlas backlog context has no exact dossier owner')
    refs = set()
    for family, count_field in BACKLOG_FIELDS.items():
        context = families[family]
        if not isinstance(context, dict):
            raise ValueError('atlas backlog family context is not an object')
        ref, records = context.get('source_ref'), context.get('records')
        if (not isinstance(ref, str) or not ref.startswith('ToS/philosophy/') or not ref.endswith('.jsonl')
                or ref in refs or not isinstance(records, list)
                or type(context.get('record_count')) is not int
                or context['record_count'] != len(records)
                or type(parent.get(count_field)) is not int or parent[count_field] != len(records)):
            raise ValueError('atlas backlog family refs or declared counts are ambiguous')
        refs.add(ref)
        prior_row = prior_line = 0
        for entry in records:
            if not isinstance(entry, dict) or not isinstance(entry.get('source_record'), dict):
                raise ValueError('atlas backlog source envelope is incomplete')
            record = entry['source_record']
            if (entry.get('source_format') != 'jsonl' or record.get('source_ref') != ref
                    or entry.get('source_file_sha256') != context.get('source_file_sha256')
                    or record.get('dossier_id') != parent['dossier_id']
                    or record.get('atlas_row_id') != parent['dossier_id']
                    or any(record.get(key) != parent.get(key) for key in ('source_document', 'branch_path'))
                    or 'table_id' in record and record['table_id'] != parent.get('table_id')
                    or type(entry.get('source_row')) is not int or entry['source_row'] <= prior_row
                    or type(entry.get('source_line')) is not int or entry['source_line'] <= prior_line):
                raise ValueError('atlas backlog source locator or parent binding differs')
            validate_authored_source_context({'source_ref': ref, 'properties': entry})
            prior_row, prior_line = entry['source_row'], entry['source_line']


def validate_authored_source_context(item):
    """Portable body/locator consistency, never authentication of an export."""
    properties = item.get('properties') or {}
    if 'source_record' not in properties:
        return
    record = properties['source_record']
    if not isinstance(record, dict) or properties.get('source_record_ref') != item.get('source_ref'):
        raise ValueError('atlas source context has no exact owner path')
    canonical = json.dumps(record, ensure_ascii=False, sort_keys=True,
                           separators=(',', ':'), allow_nan=False).encode('utf-8')
    if properties.get('source_record_sha256') != hashlib.sha256(canonical).hexdigest():
        raise ValueError('atlas source context body differs from its canonical record digest')
    for field in ('source_row', 'source_line'):
        if field in properties and (type(properties[field]) is not int or properties[field] < 1):
            raise ValueError('atlas source row and physical line must be positive integer locators')
    alias_source = properties.get('endpoint_alias_source')
    if alias_source is not None:
        if not isinstance(alias_source, dict) or properties.get('endpoint_alias_ref') != ENDPOINT_ALIASES_REF:
            raise ValueError('atlas endpoint alias context has no exact owner route')
        validate_authored_source_context({'source_ref': ENDPOINT_ALIASES_REF, 'properties': alias_source})
        aliases = alias_source['source_record'].get('aliases')
        pointers = properties.get('endpoint_alias_pointers')
        if (not isinstance(aliases, list) or not isinstance(pointers, list) or not pointers
                or any(not isinstance(pointer, str) or not pointer.startswith('/aliases/')
                       or not pointer.removeprefix('/aliases/').isdigit()
                       or int(pointer.removeprefix('/aliases/')) >= len(aliases) for pointer in pointers)):
            raise ValueError('atlas endpoint alias pointer does not select its exact source context')
    validate_dossier_source_backlogs(item)


def add_node(
    nodes: list[dict[str, Any]],
    node_id: str,
    node_type: str,
    label: str,
    source_ref: str,
    **properties: Any,
) -> None:
    clean_properties = {key: value for key, value in properties.items() if value is not None}
    nodes.append(
        {
            "node_id": node_id,
            "node_type": node_type,
            "label": label,
            "multilingual": multilingual_label(label, source_ref, {"node_type": node_type, **clean_properties}),
            "source_ref": source_ref,
            "properties": clean_properties,
        }
    )


def add_edge(
    edges: list[dict[str, Any]],
    edge_id: str,
    from_id: str,
    predicate_id: str,
    to_id: str,
    source_ref: str,
    **properties: Any,
) -> None:
    edges.append(
        {
            "edge_id": edge_id,
            "from_id": from_id,
            "predicate_id": predicate_id,
            "to_id": to_id,
            "source_ref": source_ref,
            "properties": {key: value for key, value in properties.items() if value is not None},
        }
    )


def row_projection_fields(row: dict[str, Any]) -> dict[str, Any]:
    normalized = row.get("normalized")
    if not isinstance(normalized, dict):
        normalized = {}
    research_node = (
        normalized.get("macroregion_research_node")
        or normalized.get("research_node")
        or normalized.get("node_and_task")
        or row.get("row_id")
    )
    return {
        "table_id": row.get("table_id"),
        "table_label": row.get("table_label"),
        "row_order": row.get("row_order"),
        "source_document": row.get("source_document"),
        "source_section": row.get("source_section"),
        "launch_order": normalized.get("launch_order"),
        "status": normalized.get("status"),
        "confidence": normalized.get("confidence"),
        "formation": normalized.get("formation"),
        "fixation": (
            normalized.get("written_fixation")
            or normalized.get("fixation_translation")
            or normalized.get("fixation_print_institutional_entry")
        ),
        "canonization": (
            normalized.get("canonization_redaction_commentary")
            or normalized.get("canonization_commentary")
            or normalized.get("canonization_academization")
        ),
        "research_node": research_node,
        "dossier_id": row.get("dossier_id"),
        "dossier_available": row.get("dossier_available"),
        "dossier_intake_status": normalized.get("dossier_intake_status"),
    }


def candidate_node_ref(candidate_id: str) -> str:
    return f"candidate-node:{candidate_id}"


def endpoint_ref(dossier_id: str, label: str) -> str:
    digest = hashlib.sha1(f"{dossier_id}|{label}".encode("utf-8")).hexdigest()[:12]
    return f"candidate-endpoint:{dossier_id}:{digest}"


def endpoint_role_properties(roles: set[str]) -> dict[str, Any]:
    ordered_roles = sorted(roles)
    if not ordered_roles:
        raise ValueError("candidate endpoint must have at least one observed role")
    return {
        "endpoint_role": ordered_roles[0] if len(ordered_roles) == 1 else "source_and_target",
        "endpoint_roles": ordered_roles,
    }


def qualified_endpoint_dossier_id(label: str) -> str | None:
    dossier_id, separator, qualifier = label.partition("/")
    if not separator or not dossier_id.strip() or not qualifier.strip():
        return None
    return dossier_id.strip()


def load_reviewed_endpoint_aliases(
    candidate_nodes: list[dict[str, Any]],
    candidate_relations: list[dict[str, Any]],
    admitted_dossier_ids: set[str],
    *, payload=None,
) -> dict[tuple[str, str, str], str]:
    if payload is None:
        payload = load_json(REPO_ROOT / ENDPOINT_ALIASES_REF)
    if payload.get("schema_version") != "tos_reviewed_endpoint_aliases_v2":
        raise ValueError(f"{ENDPOINT_ALIASES_REF} has unsupported schema_version")
    rows = payload.get("aliases")
    if not isinstance(rows, list):
        raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases must be an array")

    candidates_by_id = {
        str(candidate.get("candidate_id")): candidate
        for candidate in candidate_nodes
        if isinstance(candidate.get("candidate_id"), str) and candidate.get("candidate_id")
    }
    observed_endpoints = {
        (str(relation.get("dossier_id") or ""), role, str(relation.get(label_field) or ""))
        for relation in candidate_relations
        for role, label_field in (
            ("source", "source_endpoint_label"),
            ("target", "target_endpoint_label"),
        )
    }
    aliases: dict[tuple[str, str, str], str] = {}
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] must be an object")
        endpoint_label = row.get("endpoint_label")
        origin_dossier_id = row.get("origin_dossier_id")
        endpoint_role = row.get("endpoint_role")
        target_dossier_id = row.get("target_dossier_id")
        target_candidate_id = row.get("target_candidate_id")
        target_label = row.get("target_label")
        identity_fields = (
            endpoint_label,
            origin_dossier_id,
            endpoint_role,
            target_dossier_id,
            target_candidate_id,
            target_label,
        )
        if not all(isinstance(value, str) and value for value in identity_fields):
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] has incomplete identity")
        if row.get("projection_review_status") != "reviewed_for_pre_canon_routing":
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] is not reviewed for routing")
        if endpoint_role not in {"source", "target"}:
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] has invalid endpoint role")
        if origin_dossier_id not in admitted_dossier_ids:
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] has an unadmitted origin dossier")
        alias_key = (origin_dossier_id, endpoint_role, endpoint_label)
        if alias_key not in observed_endpoints:
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] is not an observed endpoint")
        qualified_dossier_id = qualified_endpoint_dossier_id(endpoint_label)
        if qualified_dossier_id is not None and qualified_dossier_id != target_dossier_id:
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] dossier qualifier mismatch")
        if target_dossier_id not in admitted_dossier_ids:
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] targets an unadmitted dossier")
        candidate = candidates_by_id.get(target_candidate_id)
        if candidate is None or candidate.get("dossier_id") != target_dossier_id:
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] target candidate mismatch")
        if candidate.get("label") != target_label:
            raise ValueError(f"{ENDPOINT_ALIASES_REF}.aliases[{index}] target label drift")
        if alias_key in aliases:
            raise ValueError(f"{ENDPOINT_ALIASES_REF} repeats endpoint key {alias_key!r}")
        aliases[alias_key] = target_candidate_id
    return aliases


def reviewed_endpoint_alias_candidate_id(
    label: str,
    origin_dossier_id: str,
    endpoint_role: str,
    admitted_dossier_ids: set[str],
    aliases: dict[tuple[str, str, str], str],
) -> str | None:
    if origin_dossier_id not in admitted_dossier_ids or endpoint_role not in {"source", "target"}:
        return None
    target_dossier_id = qualified_endpoint_dossier_id(label)
    if target_dossier_id is not None and target_dossier_id not in admitted_dossier_ids:
        return None
    return aliases.get((origin_dossier_id, endpoint_role, label))


def unavailable_master_row_endpoint_id(
    label: str,
    unavailable_master_row_ids: set[str],
) -> str | None:
    for row_id in sorted(unavailable_master_row_ids, key=len, reverse=True):
        if label == row_id:
            return row_id
        if label.startswith(row_id) and label[len(row_id) : len(row_id) + 1] in {
            " ",
            "/",
            ":",
            "—",
            "–",
            "-",
            "(",
        }:
            return row_id
    return None


def load_schema() -> dict[str, Any]:
    return load_json(REPO_ROOT / SCHEMA_REF)


def validate_payload_schema(payload: dict[str, Any]) -> None:
    validator = Draft202012Validator(load_schema())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.absolute_path))
    if errors:
        error = errors[0]
        path = "".join(f"[{item}]" if isinstance(item, int) else f".{item}" for item in error.absolute_path)
        raise ValueError(f"schema violation at {path.lstrip('.') or '<root>'}: {error.message}")
    for item in [*payload.get('nodes', []), *payload.get('edges', [])]:
        validate_authored_source_context(item)


def build_payload() -> dict[str, Any]:
    diagnostics: list[dict[str, str]] = []
    source_snapshot = AuthoredAtlasSnapshot(REPO_ROOT)
    atlas, atlas_context = source_snapshot.object(SOURCE_ATLAS_REF)
    dossier_index_path = REPO_ROOT / "ToS/philosophy/atlas/dossiers/index.jsonl"
    graph_shape_path = REPO_ROOT / "ToS/philosophy/atlas/dossiers/graph-shape-summary.json"
    dossier_sources = source_snapshot.rows(repo_ref(dossier_index_path), 'dossier_id')
    dossier_rows = [row for row, context in dossier_sources]
    dossier_backlogs = build_dossier_source_backlogs(source_snapshot, dossier_rows)
    graph_shape = load_json(graph_shape_path)
    candidate_node_sources = [
        pair
        for source_ref in CANDIDATE_NODES_REFS
        if (REPO_ROOT / source_ref).exists()
        for pair in source_snapshot.rows(source_ref, 'candidate_id')
    ]
    candidate_relation_sources = [
        pair
        for source_ref in CANDIDATE_RELATIONS_REFS
        if (REPO_ROOT / source_ref).exists()
        for pair in source_snapshot.rows(source_ref, 'candidate_id')
    ]
    candidate_nodes = [row for row, context in candidate_node_sources]
    candidate_relations = [row for row, context in candidate_relation_sources]
    for records in (candidate_nodes, candidate_relations):
        if len({row['candidate_id'] for row in records}) != len(records):
            raise ValueError('atlas candidate identity is duplicated across source streams')

    nodes: list[dict[str, Any]] = []
    edges: list[dict[str, Any]] = []
    row_count = 0
    master_row_ids: set[str] = set()

    add_node(nodes, "philosophy", "domain-root", "Philosophy", "ToS/philosophy/philosophy.manifest.json")
    add_node(nodes, "philosophy.atlas", "atlas", "Philosophy Atlas", SOURCE_ATLAS_REF, **atlas_context)
    add_node(
        nodes,
        "philosophy.atlas.master-tables",
        "atlas-section",
        "Master Tables",
        "ToS/philosophy/atlas/master-tables/branch.manifest.json",
    )
    add_node(
        nodes,
        "philosophy.atlas.dossiers",
        "atlas-section",
        "Dossiers",
        "ToS/philosophy/atlas/dossiers/branch.manifest.json",
    )
    add_node(
        nodes,
        "philosophy.graph-views",
        "view-section",
        "Graph Views",
        "ToS/philosophy/graph-workbench/views/README.md",
    )

    add_edge(edges, "edge:philosophy:has-atlas", "philosophy", "has_atlas", "philosophy.atlas", SOURCE_ATLAS_REF)
    add_edge(
        edges,
        "edge:atlas:has-master-tables",
        "philosophy.atlas",
        "has_section",
        "philosophy.atlas.master-tables",
        SOURCE_ATLAS_REF,
    )
    add_edge(
        edges,
        "edge:atlas:has-dossiers",
        "philosophy.atlas",
        "has_section",
        "philosophy.atlas.dossiers",
        SOURCE_ATLAS_REF,
    )
    add_edge(
        edges,
        "edge:atlas:has-graph-views",
        "philosophy.atlas",
        "has_view_section",
        "philosophy.graph-views",
        SOURCE_ATLAS_REF,
    )

    for table in atlas.get("master_tables", []):
        if not isinstance(table, dict):
            diagnostics.append(
                {
                    "level": "error",
                    "path": SOURCE_ATLAS_REF,
                    "message": "master_tables entry is not an object",
                }
            )
            continue
        table_id = str(table.get("table_id") or "")
        table_node_id = f"atlas-table:{table_id}"
        table_manifest_ref = str(table.get("manifest") or "")
        rows_ref = str(table.get("rows") or "")
        row_sources = source_snapshot.rows(rows_ref, 'row_id')
        rows = [row for row, context in row_sources]
        _, table_context = source_snapshot.object(table_manifest_ref)
        row_count += len(rows)
        add_node(
            nodes,
            table_node_id,
            "master-table",
            str(table.get("table_label") or table_id),
            table_manifest_ref,
            table_id=table_id,
            row_count=len(rows),
            source_document=table.get("source_document"),
            rows_ref=rows_ref,
            **table_context,
        )
        add_edge(
            edges,
            f"edge:master-tables:contains:{table_id}",
            "philosophy.atlas.master-tables",
            "contains_table",
            table_node_id,
            table_manifest_ref,
            row_count=len(rows),
        )
        for row, source_context in row_sources:
            row_id = str(row.get("row_id") or "")
            if row_id:
                if row_id in master_row_ids:
                    raise ValueError('atlas master row identity is duplicated across tables')
                master_row_ids.add(row_id)
            node_id = f"atlas-row:{row_id}"
            fields = row_projection_fields(row)
            add_node(nodes, node_id, "master-table-row", row_id, rows_ref, **fields, **source_context)
            add_edge(
                edges,
                f"edge:{table_id}:contains-row:{row_id}",
                table_node_id,
                "contains_row",
                node_id,
                rows_ref,
                row_order=row.get("row_order"),
            )
            dossier_id = row.get("dossier_id")
            if isinstance(dossier_id, str) and dossier_id:
                add_edge(
                    edges,
                    f"edge:row:{row_id}:has-dossier:{dossier_id}",
                    node_id,
                    "has_prepared_dossier",
                    f"atlas-dossier:{dossier_id}",
                    rows_ref,
                )

    for dossier, source_context in dossier_sources:
        dossier_id = str(dossier.get("dossier_id") or "")
        node_id = f"atlas-dossier:{dossier_id}"
        add_node(
            nodes,
            node_id,
            "prepared-dossier",
            str(dossier.get("title") or dossier_id),
            repo_ref(dossier_index_path),
            dossier_id=dossier_id,
            source_document=dossier.get("source_document"),
            node_row_count=dossier.get("node_row_count"),
            relation_row_count=dossier.get("relation_row_count"),
            table_count=dossier.get("table_count"),
            table_id=dossier.get("table_id"),
            route_kind=dossier.get("route_kind"),
            review_posture=dossier.get("review_posture"),
            review_reason=dossier.get("review_reason"),
            master_status=dossier.get("master_status"),
            master_confidence=dossier.get("master_confidence"),
            source_backlogs=dossier_backlogs[dossier_id],
            **source_context,
        )
        add_edge(
            edges,
            f"edge:dossiers:contains:{dossier_id}",
            "philosophy.atlas.dossiers",
            "contains_dossier",
            node_id,
            repo_ref(dossier_index_path),
        )
        node_type_counts = dossier.get("node_type_counts")
        if isinstance(node_type_counts, dict):
            for node_type, count in sorted(node_type_counts.items()):
                type_node_id = f"atlas-node-type:{node_type}"
                add_edge(
                    edges,
                    f"edge:dossier:{dossier_id}:node-type:{node_type}",
                    node_id,
                    "has_node_type_pressure",
                    type_node_id,
                    repo_ref(dossier_index_path),
                    count=count,
                )
        relation_counts = dossier.get("relation_counts")
        if isinstance(relation_counts, dict):
            for relation, count in sorted(relation_counts.items()):
                relation_node_id = f"atlas-relation-kind:{relation}"
                add_edge(
                    edges,
                    f"edge:dossier:{dossier_id}:relation-kind:{relation}",
                    node_id,
                    "has_relation_pressure",
                    relation_node_id,
                    repo_ref(dossier_index_path),
                    count=count,
                )

    for node_type, count in sorted((graph_shape.get("node_type_counts") or {}).items()):
        add_node(
            nodes,
            f"atlas-node-type:{node_type}",
            "atlas-node-type",
            str(node_type),
            repo_ref(graph_shape_path),
            count=count,
        )
        add_edge(
            edges,
            f"edge:atlas:node-type:{node_type}",
            "philosophy.atlas",
            "has_node_type_pressure",
            f"atlas-node-type:{node_type}",
            repo_ref(graph_shape_path),
            count=count,
        )

    for relation, count in sorted((graph_shape.get("relation_counts") or {}).items()):
        add_node(
            nodes,
            f"atlas-relation-kind:{relation}",
            "atlas-relation-kind",
            str(relation),
            repo_ref(graph_shape_path),
            count=count,
        )
        add_edge(
            edges,
            f"edge:atlas:relation-kind:{relation}",
            "philosophy.atlas",
            "has_relation_pressure",
            f"atlas-relation-kind:{relation}",
            repo_ref(graph_shape_path),
            count=count,
        )

    for candidate, source_context in candidate_node_sources:
        candidate_id = str(candidate.get("candidate_id") or "")
        dossier_id = str(candidate.get("dossier_id") or "")
        candidate_source_ref = str(candidate.get("source_ref") or CANDIDATE_NODES_REF)
        if not candidate_id:
            diagnostics.append(
                {
                    "level": "warning",
                    "path": candidate_source_ref,
                    "message": "candidate node row without candidate_id was skipped",
                }
            )
            continue
        node_id = candidate_node_ref(candidate_id)
        add_node(
            nodes,
            node_id,
            "candidate-node",
            str(candidate.get("label") or candidate_id),
            candidate_source_ref,
            candidate_id=candidate_id,
            dossier_id=dossier_id,
            atlas_row_id=candidate.get("atlas_row_id"),
            branch_path=candidate.get("branch_path"),
            original_node_id=candidate.get("original_node_id"),
            original_node_type=candidate.get("node_kind"),
            period=candidate.get("period"),
            priority=candidate.get("priority"),
            canon_status=candidate.get("canon_status"),
            authority_posture=candidate.get("authority_posture"),
            source_document=candidate.get("source_document"),
            table_id=candidate.get("table_id"),
            route_kind=candidate.get("route_kind"),
            review_posture=candidate.get("review_posture"),
            review_reason=candidate.get("review_reason"),
            master_status=candidate.get("master_status"),
            master_confidence=candidate.get("master_confidence"),
            **source_context,
        )
        if dossier_id:
            add_edge(
                edges,
                f"edge:dossier:{dossier_id}:candidate-node:{candidate_id}",
                f"atlas-dossier:{dossier_id}",
                "has_candidate_node",
                node_id,
                candidate_source_ref,
            )

    admitted_dossier_ids = {
        str(dossier.get("dossier_id"))
        for dossier in dossier_rows
        if isinstance(dossier.get("dossier_id"), str) and dossier.get("dossier_id")
    }
    unavailable_master_row_ids = master_row_ids - admitted_dossier_ids
    alias_payload, alias_context = source_snapshot.object(ENDPOINT_ALIASES_REF)
    endpoint_aliases = load_reviewed_endpoint_aliases(
        candidate_nodes,
        candidate_relations,
        admitted_dossier_ids,
        payload=alias_payload,
    )
    alias_pointers = {(row['origin_dossier_id'], row['endpoint_role'], row['endpoint_label']):
                      f'/aliases/{index}' for index, row in enumerate(alias_payload['aliases'])}
    endpoint_roles_by_id: dict[str, set[str]] = {}
    for relation in candidate_relations:
        dossier_id = str(relation.get("dossier_id") or "")
        source_candidate_id = relation.get("source_candidate_id")
        target_candidate_id = relation.get("target_candidate_id")
        source_label = str(relation.get("source_endpoint_label") or "source endpoint")
        target_label = str(relation.get("target_endpoint_label") or "target endpoint")
        source_alias_candidate_id = reviewed_endpoint_alias_candidate_id(
            source_label,
            dossier_id,
            "source",
            admitted_dossier_ids,
            endpoint_aliases,
        )
        target_alias_candidate_id = reviewed_endpoint_alias_candidate_id(
            target_label,
            dossier_id,
            "target",
            admitted_dossier_ids,
            endpoint_aliases,
        )
        source_master_row_id = unavailable_master_row_endpoint_id(
            source_label,
            unavailable_master_row_ids,
        )
        target_master_row_id = unavailable_master_row_endpoint_id(
            target_label,
            unavailable_master_row_ids,
        )
        if (
            not (isinstance(source_candidate_id, str) and source_candidate_id)
            and source_label not in admitted_dossier_ids
            and source_alias_candidate_id is None
            and source_master_row_id is None
        ):
            endpoint_roles_by_id.setdefault(endpoint_ref(dossier_id, source_label), set()).add("source")
        if (
            not (isinstance(target_candidate_id, str) and target_candidate_id)
            and target_label not in admitted_dossier_ids
            and target_alias_candidate_id is None
            and target_master_row_id is None
        ):
            endpoint_roles_by_id.setdefault(endpoint_ref(dossier_id, target_label), set()).add("target")

    endpoint_nodes: set[str] = set()
    for relation, source_context in candidate_relation_sources:
        candidate_id = str(relation.get("candidate_id") or "")
        dossier_id = str(relation.get("dossier_id") or "")
        relation_kind = str(relation.get("relation_kind") or "related_to")
        source_candidate_id = relation.get("source_candidate_id")
        target_candidate_id = relation.get("target_candidate_id")
        source_label = str(relation.get("source_endpoint_label") or "source endpoint")
        target_label = str(relation.get("target_endpoint_label") or "target endpoint")
        relation_source_ref = str(relation.get("source_ref") or CANDIDATE_RELATIONS_REF)
        source_alias_candidate_id = reviewed_endpoint_alias_candidate_id(
            source_label,
            dossier_id,
            "source",
            admitted_dossier_ids,
            endpoint_aliases,
        )
        target_alias_candidate_id = reviewed_endpoint_alias_candidate_id(
            target_label,
            dossier_id,
            "target",
            admitted_dossier_ids,
            endpoint_aliases,
        )
        source_master_row_id = unavailable_master_row_endpoint_id(
            source_label,
            unavailable_master_row_ids,
        )
        target_master_row_id = unavailable_master_row_endpoint_id(
            target_label,
            unavailable_master_row_ids,
        )
        if not candidate_id:
            diagnostics.append(
                {
                    "level": "warning",
                    "path": relation_source_ref,
                    "message": "candidate relation row without candidate_id was skipped",
                }
            )
            continue
        if isinstance(source_candidate_id, str) and source_candidate_id:
            from_id = candidate_node_ref(source_candidate_id)
        elif source_label in admitted_dossier_ids:
            from_id = f"atlas-dossier:{source_label}"
        elif source_alias_candidate_id is not None:
            from_id = candidate_node_ref(source_alias_candidate_id)
        elif source_master_row_id is not None:
            from_id = f"atlas-row:{source_master_row_id}"
        else:
            from_id = endpoint_ref(dossier_id, source_label)
            if from_id not in endpoint_nodes:
                endpoint_nodes.add(from_id)
                add_node(
                    nodes,
                    from_id,
                    "candidate-endpoint",
                    source_label,
                    relation_source_ref,
                    dossier_id=dossier_id,
                    branch_path=relation.get("branch_path"),
                    canon_status="pre-canon",
                    table_id=relation.get("table_id"),
                    route_kind=relation.get("route_kind"),
                    review_posture=relation.get("review_posture"),
                    review_reason=relation.get("review_reason"),
                    master_status=relation.get("master_status"),
                    master_confidence=relation.get("master_confidence"),
                    **endpoint_role_properties(endpoint_roles_by_id[from_id]),
                )
        if isinstance(target_candidate_id, str) and target_candidate_id:
            to_id = candidate_node_ref(target_candidate_id)
        elif target_label in admitted_dossier_ids:
            to_id = f"atlas-dossier:{target_label}"
        elif target_alias_candidate_id is not None:
            to_id = candidate_node_ref(target_alias_candidate_id)
        elif target_master_row_id is not None:
            to_id = f"atlas-row:{target_master_row_id}"
        else:
            to_id = endpoint_ref(dossier_id, target_label)
            if to_id not in endpoint_nodes:
                endpoint_nodes.add(to_id)
                add_node(
                    nodes,
                    to_id,
                    "candidate-endpoint",
                    target_label,
                    relation_source_ref,
                    dossier_id=dossier_id,
                    branch_path=relation.get("branch_path"),
                    canon_status="pre-canon",
                    table_id=relation.get("table_id"),
                    route_kind=relation.get("route_kind"),
                    review_posture=relation.get("review_posture"),
                    review_reason=relation.get("review_reason"),
                    master_status=relation.get("master_status"),
                    master_confidence=relation.get("master_confidence"),
                    **endpoint_role_properties(endpoint_roles_by_id[to_id]),
                )
        add_edge(
            edges,
            f"edge:candidate-relation:{candidate_id}",
            from_id,
            relation_kind,
            to_id,
            relation_source_ref,
            candidate_id=candidate_id,
            dossier_id=dossier_id,
            atlas_row_id=relation.get("atlas_row_id"),
            branch_path=relation.get("branch_path"),
            relation_label=relation.get("relation_label"),
            confidence=relation.get("confidence"),
            canon_status=relation.get("canon_status"),
            authority_posture=relation.get("authority_posture"),
            endpoint_resolution=relation.get("endpoint_resolution"),
            comment=relation.get("comment"),
            table_id=relation.get("table_id"),
            route_kind=relation.get("route_kind"),
            review_posture=relation.get("review_posture"),
            review_reason=relation.get("review_reason"),
            master_status=relation.get("master_status"),
            master_confidence=relation.get("master_confidence"),
            projection_endpoint_resolution=(
                (
                    "reviewed_origin_role_alias"
                    if (
                        source_alias_candidate_id is not None
                        and qualified_endpoint_dossier_id(source_label) is None
                    )
                    or (
                        target_alias_candidate_id is not None
                        and qualified_endpoint_dossier_id(target_label) is None
                    )
                    else "reviewed_qualified_alias"
                )
                if source_alias_candidate_id is not None or target_alias_candidate_id is not None
                else (
                    "known_unavailable_master_row"
                    if source_master_row_id is not None or target_master_row_id is not None
                    else (
                        "exact_admitted_dossier"
                        if source_label in admitted_dossier_ids or target_label in admitted_dossier_ids
                        else None
                    )
                )
            ),
            endpoint_alias_ref=(
                ENDPOINT_ALIASES_REF
                if source_alias_candidate_id is not None or target_alias_candidate_id is not None
                else None
            ),
            endpoint_alias_source=(
                copy.deepcopy(alias_context)
                if source_alias_candidate_id is not None or target_alias_candidate_id is not None else None
            ),
            endpoint_alias_pointers=(
                [alias_pointers[key] for key, target in
                 (((dossier_id, 'source', source_label), source_alias_candidate_id),
                  ((dossier_id, 'target', target_label), target_alias_candidate_id)) if target is not None]
                if source_alias_candidate_id is not None or target_alias_candidate_id is not None else None
            ),
            **source_context,
        )

    views_root = REPO_ROOT / "ToS/philosophy/graph-workbench/views"
    for path in sorted(views_root.glob("*.graph.md")):
        view_id = path.stem.removesuffix(".graph")
        node_id = f"graph-view:{view_id}"
        add_node(nodes, node_id, "graph-view", view_id, repo_ref(path), view_file=repo_ref(path))
        add_edge(
            edges,
            f"edge:graph-views:contains:{view_id}",
            "philosophy.graph-views",
            "contains_view",
            node_id,
            repo_ref(path),
        )

    payload: dict[str, Any] = {
        "schema_version": "tos_philosophy_atlas_projection_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_philosophy_atlas_projection",
        "source_atlas_ref": SOURCE_ATLAS_REF,
        "content_language_contract": content_language_contract(),
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "runtime_scope": [
                "read this projection as a ToS-owned graph input",
                "serve MCP/API resources that point back to ToS surfaces",
                "render UI, graph layout, and local caches downstream",
            ],
            "tos_authority_scope": [
                "canon remains in ToS canon and source-owned atlas surfaces",
                "source witnesses and atlas rows remain the authored evidence route",
                "runtime graph state returns through an explicit ToS change route",
            ],
        },
        "validation_refs": list(VALIDATION_REFS),
        "counts": {
            "master_tables": len(atlas.get("master_tables", [])),
            "master_rows": row_count,
            "dossiers": len(dossier_rows),
            "dossier_node_rows": int(graph_shape.get("node_row_count") or 0),
            "dossier_relation_rows": int(graph_shape.get("relation_row_count") or 0),
            "candidate_nodes": len(candidate_nodes),
            "candidate_relations": len(candidate_relations),
            "candidate_endpoint_placeholders": len(endpoint_nodes),
            "graph_views": len(list(views_root.glob("*.graph.md"))),
            "nodes": len(nodes),
            "edges": len(edges),
            "diagnostics": len(diagnostics),
        },
        "nodes": nodes,
        "edges": edges,
        "diagnostics": diagnostics,
    }
    validate_payload_schema(payload)
    source_snapshot.verify_current()
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"
