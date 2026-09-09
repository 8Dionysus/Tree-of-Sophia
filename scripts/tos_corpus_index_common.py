#!/usr/bin/env python3
"""Shared builder helpers for the ToS whole-corpus index."""

from __future__ import annotations

import csv
import hashlib
import json
from pathlib import Path
import subprocess
from typing import Any

from jsonschema import Draft202012Validator
from source_metadata_snapshot import PublicationSnapshot, PublicationChanged
from build_source_witness_catalog import (artifact_catalog_entry, artifact_display_fields,
                                         load_artifact_record, canonical_json, RECORD_FILES, ADAPTED_RECORD_FILES,
                                         composite_catalog_entry, load_composite_record, composite_display_fields, COMPOSITE_SCHEMA,
                                         verify_catalog_publication)
from source_witness_human_forms import AssessedFormSnapshot, load_metadata_forms
from source_record_profiles import SourceRecordProfiles


REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
TOS_CORPUS_INDEX_PATH = TOS_ROOT / "derived-exports" / "tos_corpus_index.min.json"
SCHEMA_REF = "ToS/contracts/tos-corpus-index.schema.json"
SELF_REF = "ToS/derived-exports/tos_corpus_index.min.json"
VALIDATION_REFS = (
    "scripts/build_tos_corpus_index.py",
    "scripts/validate_tos_corpus_index.py",
    "tests/test_tos_corpus_index.py",
)

BRANCH_AUTHORITY = {
    "ToS": "source_home",
    "candidate-intake": "candidate_intake",
    "canon": "canon",
    "contracts": "contract",
    "derived-exports": "derived_export",
    "doctrine": "doctrine",
    "philosophy": "domain_topology",
    "public-compatibility": "public_compatibility",
    "research-packets": "research_packet",
    "review-ledger": "review_evidence",
    "source-witnesses": "source_witness",
    "zarathustra": "golden_route_orientation",
}

AUTHORITY_ORDER = (
    {
        "layer": "source_home",
        "owner_branch": "ToS",
        "meaning": "Tree of Sophia home surface, source-home manifest, and top-level home route cards",
    },
    {
        "layer": "source_witness",
        "owner_branch": "ToS/source-witnesses",
        "meaning": "source-facing witness and provenance surfaces",
    },
    {
        "layer": "golden_route_orientation",
        "owner_branch": "ToS/zarathustra",
        "meaning": "golden Zarathustra orientation route for the project's current living entry",
    },
    {
        "layer": "canon",
        "owner_branch": "ToS/canon",
        "meaning": "reviewed authored nodes, relation packs, and registries",
    },
    {
        "layer": "doctrine",
        "owner_branch": "ToS/doctrine",
        "meaning": "current ToS knowledge law, node contracts, templates, and interpretation discipline",
    },
    {
        "layer": "contract",
        "owner_branch": "ToS/contracts",
        "meaning": "public structural contracts for ToS-owned surfaces",
    },
    {
        "layer": "domain_topology",
        "owner_branch": "ToS/philosophy",
        "meaning": "branch-shaped philosophy topology and local graph workbench routes",
    },
    {
        "layer": "candidate_intake",
        "owner_branch": "ToS/candidate-intake",
        "meaning": "provisional extraction and promotion residue",
    },
    {
        "layer": "research_packet",
        "owner_branch": "ToS/research-packets",
        "meaning": "non-authoritative research scaffolds for later review",
    },
    {
        "layer": "review_evidence",
        "owner_branch": "ToS/review-ledger",
        "meaning": "dated inspection notes and review evidence for corpus growth",
    },
    {
        "layer": "public_compatibility",
        "owner_branch": "ToS/public-compatibility",
        "meaning": "public-safe mirrors and compatibility examples",
    },
    {
        "layer": "derived_export",
        "owner_branch": "ToS/derived-exports",
        "meaning": "generated downstream read models subordinate to ToS authority",
    },
    {
        "layer": "runtime_projection",
        "owner_branch": "abyss-stack",
        "meaning": "runtime access, visualization, MCP, UI, and projection stores only",
    },
)

GRAPH_VIEWS = (
    {
        "view_id": "corpus-topology",
        "purpose": "show the whole ToS home as a branch-shaped tree",
        "layout_hint": "elk-layered-or-graphviz-dot",
        "entry_surface": "ToS/source_home.manifest.json",
    },
    {
        "view_id": "authority-layers",
        "purpose": "switch corpus visibility by witness, research, candidate, canon, compatibility, and export layers",
        "layout_hint": "layered-filter",
        "entry_surface": "ToS/source_home.manifest.json",
    },
    {
        "view_id": "route-graph",
        "purpose": "inspect a concrete relation pack without losing its owner branch and provenance",
        "layout_hint": "directed-route-graph",
        "entry_surface": "ToS/canon/relations",
    },
    {
        "view_id": "node-neighborhood",
        "purpose": "expand around one node by bounded hops over the full corpus substrate",
        "layout_hint": "sigma-graphology-webgl",
        "entry_surface": "ToS/canon",
    },
    {
        "view_id": "provenance-dag",
        "purpose": "trace source witness or research packet pressure into candidate, canon, and export surfaces",
        "layout_hint": "dag",
        "entry_surface": "ToS/source-witnesses",
    },
    {
        "view_id": "promotion-flow",
        "purpose": "review candidate-intake material against canon promotion status",
        "layout_hint": "elk-layered-flow",
        "entry_surface": "ToS/candidate-intake",
    },
    {
        "view_id": "diff-snapshot",
        "purpose": "compare two corpus index snapshots for review",
        "layout_hint": "changed-subgraph",
        "entry_surface": "ToS/derived-exports/tos_corpus_index.min.json",
    },
)


def repo_ref(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def tracked_tos_paths() -> tuple[Path, ...]:
    """Return the Git-trackable ToS source view, excluding private ignored bytes."""

    completed = subprocess.run(
        (
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "ToS",
        ),
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
    )
    paths = []
    for raw_ref in completed.stdout.split(b"\0"):
        if not raw_ref:
            continue
        path = REPO_ROOT / raw_ref.decode("utf-8")
        # Physical payload bytes belong to the source-witness artifact/item
        # stores, not to this metadata/read-model resource index.
        if path.is_file() and "payload" not in path.relative_to(TOS_ROOT).parts:
            paths.append(path)
    return tuple(sorted(paths))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path)} must contain a JSON object")
    return payload


def owner_branch(path_ref: str) -> str:
    parts = Path(path_ref).parts
    if not parts or parts[0] != "ToS":
        return "repository"
    if len(parts) == 1:
        return "ToS"
    if len(parts) == 2 and (REPO_ROOT / path_ref).is_file():
        return "ToS"
    return f"ToS/{parts[1]}"


def authority_layer(path_ref: str) -> str:
    branch = owner_branch(path_ref)
    branch_name = branch.removeprefix("ToS/")
    return BRANCH_AUTHORITY.get(branch_name, "repository")


def resource_kind(path: Path) -> str:
    path_ref = repo_ref(path)
    name = path.name
    suffix = path.suffix.lower()
    if name == "AGENTS.md":
        return "route_card"
    if name == "source_home.manifest.json":
        return "source_home_manifest"
    if name == "philosophy.manifest.json":
        return "philosophy_manifest"
    if name == "branch.manifest.json":
        return "branch_manifest"
    if name == "node.json":
        return "node_payload"
    if name == "edges.csv":
        return "relation_pack"
    if path_ref.startswith("ToS/source-witnesses/"):
        return "source_witness"
    if path_ref.startswith("ToS/research-packets/"):
        return "research_packet"
    if path_ref.startswith("ToS/review-ledger/"):
        return "review_note"
    if path_ref.startswith("ToS/contracts/") and suffix == ".json":
        return "contract_schema"
    if path_ref.startswith("ToS/derived-exports/"):
        return "derived_export"
    if suffix == ".md":
        return "markdown"
    if suffix == ".csv":
        return "tabular"
    if suffix == ".json":
        return "json"
    return "binary" if suffix in {".xlsx", ".xls"} else suffix.lstrip(".") or "file"


def canonical_label(payload: dict[str, Any]) -> str:
    for key in ("canonical_label", "preferred_label", "label", "title", "name"):
        value = payload.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    node_id = payload.get("node_id")
    if isinstance(node_id, str) and node_id.strip():
        leaf = node_id.strip().rsplit(".", 1)[-1]
        return leaf.replace("-", " ").replace("_", " ")
    return "unnamed-node"


def route_hint_for_node(path_ref: str) -> str | None:
    marker = "/friedrich-nietzsche/"
    if marker not in path_ref:
        return None
    return path_ref.split(marker, 1)[1].rsplit("/node.json", 1)[0]


def route_hint_for_edges(path_ref: str) -> str:
    return path_ref.rsplit("/edges.csv", 1)[0].split("/", 2)[-1]


def build_branches(source_home: dict[str, Any], diagnostics: list[dict[str, str]]) -> list[dict[str, str]]:
    branches: list[dict[str, str]] = []
    for branch in source_home.get("branches", []):
        if not isinstance(branch, dict):
            diagnostics.append({"level": "error", "path": "ToS/source_home.manifest.json", "message": "branch entry is not an object"})
            continue
        path_ref = str(branch.get("path") or "")
        owner_surface = str(branch.get("owner_surface") or "")
        if path_ref and not (REPO_ROOT / path_ref).is_dir():
            diagnostics.append({"level": "error", "path": path_ref, "message": "branch path declared by source_home manifest is missing"})
        if owner_surface and not (REPO_ROOT / owner_surface).is_file():
            diagnostics.append({"level": "error", "path": owner_surface, "message": "branch owner surface declared by source_home manifest is missing"})
        branches.append(
            {
                "id": str(branch.get("id") or ""),
                "path": path_ref,
                "owner_surface": owner_surface,
                "authority_layer": authority_layer(path_ref),
                "role": str(branch.get("role") or ""),
            }
        )
    return branches


def build_manifests(
    diagnostics: list[dict[str, str]],
    tracked_paths: tuple[Path, ...],
) -> list[dict[str, Any]]:
    manifests: list[dict[str, Any]] = []
    source_home_seen = False
    for path in (candidate for candidate in tracked_paths if candidate.name.endswith(".manifest.json")):
        path_ref = repo_ref(path)
        if path_ref == "ToS/source_home.manifest.json":
            source_home_seen = True
        try:
            payload = load_json(path)
        except (json.JSONDecodeError, ValueError) as exc:
            diagnostics.append({"level": "error", "path": path_ref, "message": str(exc)})
            continue
        declared_path = payload.get("path") if isinstance(payload.get("path"), str) else None
        if path_ref == "ToS/source_home.manifest.json" and isinstance(payload.get("home"), str):
            declared_path = payload.get("home")
        manifests.append(
            {
                "path": path_ref,
                "manifest_kind": resource_kind(path),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "schema_version": str(payload.get("schema_version") or ""),
                "branch_id": payload.get("branch_id") if isinstance(payload.get("branch_id"), str) else None,
                "declared_path": declared_path,
                "sha256": sha256(path),
            }
        )
    source_home_path = TOS_ROOT / "source_home.manifest.json"
    source_home_ref = repo_ref(source_home_path)
    if not source_home_path.is_file():
        diagnostics.append({"level": "error", "path": source_home_ref, "message": "missing source-home manifest"})
    elif not source_home_seen:
        payload = load_json(source_home_path)
        manifests.insert(
            0,
            {
                "path": source_home_ref,
                "manifest_kind": "source_home_manifest",
                "owner_branch": "ToS",
                "authority_layer": "source_home",
                "schema_version": str(payload.get("schema_version") or ""),
                "branch_id": None,
                "declared_path": payload.get("home") if isinstance(payload.get("home"), str) else None,
                "sha256": sha256(source_home_path),
            },
        )
    return manifests


def build_nodes(
    diagnostics: list[dict[str, str]],
    tracked_paths: tuple[Path, ...],
) -> list[dict[str, Any]]:
    nodes: list[dict[str, Any]] = []
    for path in (candidate for candidate in tracked_paths if candidate.name == "node.json"):
        path_ref = repo_ref(path)
        try:
            payload = load_json(path)
        except (json.JSONDecodeError, ValueError) as exc:
            diagnostics.append({"level": "error", "path": path_ref, "message": str(exc)})
            continue
        node_id = payload.get("node_id")
        node_type = payload.get("node_type")
        if not isinstance(node_id, str) or not node_id:
            diagnostics.append({"level": "error", "path": path_ref, "message": "node payload is missing node_id"})
            continue
        if not isinstance(node_type, str) or not node_type:
            diagnostics.append({"level": "error", "path": path_ref, "message": "node payload is missing node_type"})
            continue
        nodes.append(
            {
                "node_id": node_id,
                "node_type": node_type,
                "label": canonical_label(payload),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "source_path": path_ref,
                "source_sha256": sha256(path),
                "route_hint": route_hint_for_node(path_ref),
                "properties": dict(payload),
            }
        )
    return nodes


def read_edge_rows(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        return list(reader.fieldnames or []), list(reader)


def build_relations(
    diagnostics: list[dict[str, str]],
    tracked_paths: tuple[Path, ...],
) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    relation_packs: list[dict[str, Any]] = []
    relation_edges: list[dict[str, str]] = []
    for path in (candidate for candidate in tracked_paths if candidate.name == "edges.csv"):
        path_ref = repo_ref(path)
        try:
            columns, rows = read_edge_rows(path)
        except csv.Error as exc:
            diagnostics.append({"level": "error", "path": path_ref, "message": f"invalid CSV: {exc}"})
            continue
        pack_id = path_ref.rsplit("/edges.csv", 1)[0].removeprefix("ToS/")
        relation_packs.append(
            {
                "pack_id": pack_id,
                "path": path_ref,
                "route_hint": route_hint_for_edges(path_ref),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "edge_count": len(rows),
                "columns": columns,
                "sha256": sha256(path),
            }
        )
        for row in rows:
            edge_id = str(row.get("edge_id") or f"{pack_id}:{len(relation_edges) + 1}")
            relation_edges.append(
                {
                    "edge_id": edge_id,
                    "pack_id": pack_id,
                    "from_id": str(row.get("from_id") or ""),
                    "predicate_id": str(row.get("predicate_id") or ""),
                    "to_id": str(row.get("to_id") or ""),
                    "owner_branch": owner_branch(path_ref),
                    "authority_layer": authority_layer(path_ref),
                    "layer": str(row.get("layer") or ""),
                    "status": str(row.get("status") or ("canon" if path_ref.startswith("ToS/canon/") else "unmarked")),
                }
            )
    return relation_packs, relation_edges


def build_resources(tracked_paths: tuple[Path, ...]) -> list[dict[str, Any]]:
    resources: list[dict[str, Any]] = []
    for path in tracked_paths:
        path_ref = repo_ref(path)
        if path_ref == SELF_REF:
            continue
        resources.append(
            {
                "path": path_ref,
                "resource_kind": resource_kind(path),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "sha256": sha256(path),
                "size_bytes": path.stat().st_size,
            }
        )
    return resources


def _jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    return [
        payload
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
        for payload in (json.loads(line),)
        if isinstance(payload, dict)
    ]


def _source_navigation_branch_kind(path_ref: str) -> str:
    parts = Path(path_ref).parts
    if "traditions" in parts and parts.index("traditions") == len(parts) - 2:
        return "tradition"
    if "regions" in parts and parts.index("regions") == len(parts) - 2:
        return "region"
    if "eras" in parts and parts.index("eras") == len(parts) - 2:
        return "era"
    return "branch"


def project_text_packet(packet: dict[str, Any], source_ref: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Project declared stand-off structure, never read the underlying text.

    Restricted packets are not exported. Public metadata of a private text uses
    an explicit field allowlist, excluding exact-form hashes and display text.
    IDs of representations include packet/version, while record_id keeps the
    source-owned entity identity. Competing schemes therefore never overwrite.
    """
    schema = packet.get('schema_version')
    if schema not in {'tos_source_text_unit_packet_v1', 'tos_semantic_annotation_packet_v2'}:
        return [], []
    rights = packet.get('rights_and_visibility', {})
    visibility = rights.get('packet_visibility', rights.get('record_visibility'))
    if visibility not in {'public', 'public_metadata_only'}:
        return [], []
    content = (visibility == 'public' and rights.get('publication_authorized') is True
               and not rights.get('private_source_used')
               and rights.get('effective_visibility', rights.get('source_content_visibility')) in {'public', 'public_synthetic'})
    # Semantic bodies must not be reconstructed from restricted lexical hashes.
    if schema == 'tos_semantic_annotation_packet_v2' and not content:
        return [], []
    schema_name = 'source-text-unit-packet-v1.schema.json' if schema == 'tos_source_text_unit_packet_v1' else 'semantic-annotation-packet-v2.schema.json'
    Draft202012Validator(load_json(REPO_ROOT / 'ToS/contracts' / schema_name)).validate(packet)
    scope = packet['source_scope']
    packet_id = packet.get('packet_id', packet.get('annotation_id'))
    version = packet.get('packet_version', packet.get('annotation_version'))
    namespace = hashlib.sha256(f'{source_ref}:{packet_id}:{version}'.encode()).hexdigest()[:20]
    nodes: list[dict[str, Any]] = []
    edges: list[dict[str, Any]] = []
    identities: dict[str, str] = {}

    def add(identity: str, kind: str, record: dict[str, Any], title: str | None = None) -> str:
        identifier = f'{identity}@{namespace}'
        identities[identity] = identifier
        nodes.append({'node_id': identifier, 'node_kind': kind, 'label': title or identity,
                      'source_ref': source_ref, 'identity_status': 'source-declared-versioned-record',
                      'properties': {**record, 'record_id': identity, 'packet_id': packet_id,
                         'packet_version': version, 'content_available': content,
                         'publication_posture': 'public' if content else 'public_metadata_only',
                         'content_posture': packet.get('content_posture'),
                         'review_status': record.get('admission_status', record.get('boundary_posture', 'not-recorded'))}})
        return identifier

    def edge(left: str, predicate: str, right: str, claim_ref: str | None = None):
        key = hashlib.sha256(f'{namespace}:{left}:{predicate}:{right}'.encode()).hexdigest()
        value = {'edge_id': f'text-spine:{key}', 'from_id': left, 'to_id': right,
                 'predicate_id': predicate, 'edge_kind': 'stand-off-source-structure',
                 'source_refs': [source_ref], 'review_status': 'source-declared-not-semantic-acceptance'}
        if claim_ref:
            value['claim_ref'] = claim_ref
        edges.append(value)

    layer = packet.get('source_layer', {})
    layer_ref = layer.get('text_layer_ref', scope.get('source_text_layer_ref'))
    layer_digest = layer.get('text_layer_sha256')
    layer_identity = 'text-layer:' + hashlib.sha256(f'{layer_ref}:{layer_digest}'.encode()).hexdigest()
    layer_id = add(layer_identity, 'text-layer', dict(layer) if content else
                   {k: layer[k] for k in ('text_layer_ref', 'language', 'immutable', 'position_unit', 'interval', 'visibility') if k in layer},
                   f"Text layer · {layer.get('language', 'source')} · {source_ref.rsplit('/', 1)[-1]}")
    edge(scope['work_ref'], 'has_text_layer', layer_id)
    annotation = add(packet_id, 'annotation', {'rights_and_visibility': rights, 'source_scope': scope},
                     f"Stand-off packet · version {version}")
    edge(layer_id, 'has_annotation', annotation)
    anchors = packet.get('anchors', scope.get('source_anchors', []))
    for anchor in anchors:
        if anchor['selector']['start'] > anchor['selector']['end']:
            raise ValueError(f'{source_ref}: reversed anchor selector')
        record = dict(anchor) if content else {key: anchor[key] for key in ('anchor_ref', 'ordinal', 'selector', 'anchor_role', 'text_layer_ref') if key in anchor}
        identifier = add(anchor['anchor_ref'], 'anchor', record, f"Anchor · {anchor.get('ordinal', anchor['anchor_ref'])}")
        edge(identifier, 'anchored_in', layer_id)
    for unit in packet.get('units', []):
        record = dict(unit) if content else {key: unit[key] for key in ('unit_id', 'unit_version', 'unit_kind', 'surface_posture', 'continuity', 'ordered_anchor_refs', 'parent_unit_refs', 'ordered_child_unit_refs', 'boundary_posture', 'semantic_promotion') if key in unit}
        identifier = add(unit['unit_id'], 'text-unit', record, f"{unit['unit_kind']} · {unit['unit_id']}")
        edge(layer_id, 'has_text_unit', identifier)
        for anchor in unit['ordered_anchor_refs']:
            if anchor not in identities:
                raise ValueError(f'{source_ref}: unresolved unit anchor {anchor}')
            edge(identifier, 'has_anchor', identities[anchor])
    for entity in packet.get('entities', []):
        labels = entity.get('display_labels', [])
        record = {**entity, 'variant_labels': labels}
        # Native stand-off entities are not authored description profiles.
        # Only the adapter kind changes; source kind, identity and body remain
        # exact, including lexical_sense's native spelling and admission state.
        kind = entity['entity_kind'].replace('_', '-')
        if entity['entity_kind'] in {'occurrence', 'lexeme', 'lexical_sense', 'sign'}:
            kind = 'annotation-' + kind
        identifier = add(entity['entity_id'], kind, record,
                         labels[0]['value'] if labels else None)
        edge(annotation, 'annotation_member', identifier)
        for anchor in entity['identity_basis']['anchor_refs']:
            if anchor not in identities:
                raise ValueError(f'{source_ref}: unresolved occurrence anchor {anchor}')
            edge(identifier, 'has_anchor', identities[anchor])
    # Keep assertions, their evidence, and reviews addressable; no materialized
    # semantic edge is emitted merely because an annotation mentions two things.
    for claim in packet.get('claims', []):
        identifier = add(claim['claim_id'], 'annotation-claim', dict(claim), claim['proposition']['predicate'])
        edge(annotation, 'annotation_member', identifier)
        subject = identities.get(claim['proposition']['subject_ref'])
        if subject is None:
            raise ValueError(f'{source_ref}: unresolved annotation claim subject')
        edge(identifier, 'assertion_subject', subject)
        obj = claim['proposition']['object']
        if obj.get('kind') == 'entity_ref':
            if obj['entity_ref'] not in identities:
                raise ValueError(f'{source_ref}: unresolved annotation claim object')
            edge(identifier, 'assertion_object', identities[obj['entity_ref']])
        else:
            value_id = add(f"{claim['claim_id']}:object", 'literal', dict(obj), f"Claim object · {obj['kind']}")
            edge(identifier, 'assertion_object', value_id)
        for anchor in claim['target_anchor_refs']:
            edge(identifier, 'has_anchor', identities[anchor])
        for index, evidence in enumerate(claim['evidence']):
            evidence_id = add(f"{claim['claim_id']}:evidence:{index}", 'annotation-evidence', dict(evidence), evidence['description'])
            edge(identifier, 'assertion_evidence', evidence_id)
            for anchor in evidence['anchor_refs']:
                edge(evidence_id, 'has_anchor', identities[anchor])
    for review in packet.get('reviews', []):
        identifier = add(review['review_id'], 'annotation-review', dict(review), f"Review · {review.get('decision', review.get('outcome'))}")
        edge(annotation, 'annotation_member', identifier)
        for claim in packet.get('claims', []):
            if review['review_id'] in claim['review_refs']:
                edge(identities[claim['claim_id']], 'assertion_review', identifier)
    for relation in packet.get('relations', []):
        if relation['claim_ref'] not in identities:
            raise ValueError(f'{source_ref}: unresolved semantic relation claim')
        identifier = add(relation['relation_id'], 'annotation-relation', dict(relation), relation['relation_type'])
        edge(identifier, 'assertion_subject', identities[relation['subject_ref']])
        edge(identifier, 'assertion_object', identities[relation['object_ref']])
        edge(identifier, 'asserted_by', identities[relation['claim_ref']])
    return nodes, edges


def _nearest_branch_parents(branch_paths: list[str]) -> dict[str, str]:
    ordered = sorted(branch_paths, key=lambda item: (len(Path(item).parts), item))
    # Preserve the previous lexical tie-break for equivalent path spellings.
    refs_by_path: dict[Path, str] = {}
    for path_ref in ordered:
        refs_by_path.setdefault(Path(path_ref), path_ref)
    parents = {}
    for child_ref in ordered:
        # Path.parents is nearest-first. Only authored branches may be parents;
        # unrepresented intermediate directories do not create graph nodes.
        for parent in Path(child_ref).parents:
            if parent in refs_by_path:
                parents[child_ref] = refs_by_path[parent]
                break
    return parents


def build_source_navigation(diagnostics: list[dict[str, str]], *,
                            assessed_forms: AssessedFormSnapshot | None = None) -> dict[str, Any]:
    snapshot = PublicationSnapshot(REPO_ROOT)
    result = _build_source_navigation(diagnostics, assessed_forms=assessed_forms, publication=snapshot)
    snapshot.verify_current()
    return result


def _build_source_navigation(diagnostics, *, assessed_forms, publication):
    """Join authored topology and source records into a read-only descent graph."""

    nodes: dict[str, dict[str, Any]] = {}
    edges: dict[str, dict[str, Any]] = {}
    rights: list[dict[str, Any]] = []
    version_reader = None
    metadata_reader = None
    catalog_digests = None

    def add_node(
        node_id: str,
        node_kind: str,
        label: str,
        source_ref: str,
        identity_status: str = "not_applicable",
        properties: dict[str, Any] | None = None,
    ) -> None:
        candidate = {
            "node_id": node_id,
            "node_kind": node_kind,
            "label": label,
            "source_ref": source_ref,
            "identity_status": identity_status,
            "properties": properties or {},
        }
        existing = nodes.get(node_id)
        if existing is not None and existing != candidate:
            diagnostics.append(
                {
                    "level": "error",
                    "path": source_ref,
                    "message": f"source-navigation node {node_id} has conflicting projections",
                }
            )
            return
        nodes[node_id] = candidate

    def add_edge(
        edge_id: str,
        from_id: str,
        predicate_id: str,
        to_id: str,
        edge_kind: str,
        source_refs: list[str],
        review_status: str = "not_applicable",
        claim_ref: str | None = None,
    ) -> None:
        candidate: dict[str, Any] = {
            "edge_id": edge_id,
            "from_id": from_id,
            "predicate_id": predicate_id,
            "to_id": to_id,
            "edge_kind": edge_kind,
            "review_status": review_status,
            "source_refs": sorted(dict.fromkeys(source_refs)),
        }
        if claim_ref:
            candidate["claim_ref"] = claim_ref
        existing = edges.get(edge_id)
        if existing is not None and existing != candidate:
            diagnostics.append(
                {
                    "level": "error",
                    "path": source_refs[0] if source_refs else "ToS",
                    "message": f"source-navigation edge {edge_id} has conflicting projections",
                }
            )
            return
        edges[edge_id] = candidate

    def add_version_node(reference: dict[str, Any], resolved: dict[str, Any], record_kind: str) -> tuple[str, str]:
        available = resolved['status'] == 'available'
        view = {'schema_version': 'tos_record_version_view_v1',
            'record_ref': dict(reference), 'record_kind': record_kind,
            'status': resolved['status'], 'reason': resolved['reason'],
            'version_status': resolved['version_status'], 'record': resolved['record'],
            'provenance': resolved['provenance'] if available else {},
            'grants_current_use': False, 'performs_assessment': False}
        version_id = 'record-version:' + hashlib.sha256(canonical_json(reference).encode('utf-8')).hexdigest()
        version_ref = 'ToS/contracts/record-version-view.schema.json#/properties/record_ref'
        if available:
            locator = resolved['provenance']['source']
            version_ref = locator['archive_blob_ref'] or locator['source_ref']
            if record_kind == 'claim':
                version_ref += '#L' + str(locator['line'])
        add_node(version_id, 'record-version', 'Exact record version', version_ref,
                 'not_applicable', {'record_version_view': view})
        return version_id, version_ref

    branch_by_path: dict[str, dict[str, Any]] = {}
    for manifest_path in sorted((TOS_ROOT / "philosophy" / "eras").rglob("branch.manifest.json")):
        try:
            manifest = load_json(manifest_path)
        except (json.JSONDecodeError, ValueError) as exc:
            diagnostics.append({"level": "error", "path": repo_ref(manifest_path), "message": str(exc)})
            continue
        branch_id = manifest.get("branch_id")
        path_ref = manifest.get("path")
        if not isinstance(branch_id, str) or not isinstance(path_ref, str):
            continue
        branch_by_path[path_ref] = manifest
        add_node(
            branch_id,
            _source_navigation_branch_kind(path_ref),
            str(manifest.get("role") or Path(path_ref).name.replace("-", " ").title()),
            repo_ref(manifest_path),
            properties={"branch_path": path_ref, "role": str(manifest.get("role") or "")},
        )

    for child_path, parent_path in _nearest_branch_parents(list(branch_by_path)).items():
        parent_id = str(branch_by_path[parent_path]["branch_id"])
        child_id = str(branch_by_path[child_path]["branch_id"])
        add_edge(
            f"source-navigation:branch:{parent_id}:{child_id}",
            parent_id,
            "contains",
            child_id,
            "authored_branch_hierarchy",
            [repo_ref(TOS_ROOT.parent / child_path / "branch.manifest.json")],
        )

    catalog_root = TOS_ROOT / "source-witnesses" / "catalog"
    catalog_manifest_path = catalog_root / "catalog.manifest.json"
    if publication.token is not None and not catalog_manifest_path.is_file():
        raise PublicationChanged('initialized source publication requires its catalog manifest')
    if catalog_manifest_path.is_file():
        from metadata_version_reader import MetadataVersionReader
        metadata_reader = MetadataVersionReader(REPO_ROOT)
        catalog_manifest_raw = catalog_manifest_path.read_bytes()
        catalog_manifest = json.loads(catalog_manifest_raw)
        artifact_validators = {}
        profiles = SourceRecordProfiles(REPO_ROOT)
        corpus_validator = Draft202012Validator(load_json(REPO_ROOT / 'ToS/contracts/corpus-record.schema.json'))
        allowed_files = {**RECORD_FILES, **profiles.catalog_files, **ADAPTED_RECORD_FILES}
        # Validate the closure before opening any path supplied by a manifest.
        for record_type, file_ref in catalog_manifest.get('record_files', {}).items():
            if (record_type not in allowed_files
                    or file_ref != 'ToS/source-witnesses/catalog/' + allowed_files[record_type]):
                raise ValueError('catalog family has no exact understood source-profile path')
        if publication.token is not None or 'selected_metadata_publication' in catalog_manifest:
            if catalog_manifest.get('claim_file') != 'ToS/source-witnesses/catalog/claims.jsonl':
                raise ValueError('catalog has no exact understood claim path')
            catalog_digests = {ref: sha256(REPO_ROOT / ref) for ref in
                              [*catalog_manifest.get('record_files', {}).values(), catalog_manifest['claim_file']]}
            verify_catalog_publication(catalog_manifest, publication.token, catalog_digests)
        for record_type, file_ref in sorted(catalog_manifest.get("record_files", {}).items()):
            if (record_type not in allowed_files
                    or file_ref != 'ToS/source-witnesses/catalog/' + allowed_files[record_type]):
                raise ValueError('catalog family has no exact understood source-profile path')
            for entry in _jsonl(REPO_ROOT / str(file_ref)):
                record_id = str(entry.get("record_id") or "")
                source_ref = str(entry.get("source_record_ref") or "")
                if not record_id or not source_ref:
                    continue
                native_composite = record_type == 'composite' and (
                    entry.get('source_schema_ref') == COMPOSITE_SCHEMA or record_type not in profiles.profiles)
                source_record = (load_composite_record(REPO_ROOT, source_ref) if native_composite
                                 else profiles.verify_entry(record_type, entry) if record_type in profiles.profiles
                                 else load_artifact_record(REPO_ROOT, source_ref) if record_type == 'artifact'
                                 else load_json(REPO_ROOT / source_ref))
                if record_type == 'artifact' and entry != artifact_catalog_entry(
                        REPO_ROOT, source_record, source_ref, artifact_validators):
                    raise ValueError(f'{source_ref}: physical artifact catalog/source mapping drifted')
                if native_composite and entry != composite_catalog_entry(
                        REPO_ROOT, source_record, source_ref, artifact_validators):
                    raise ValueError(f'{source_ref}: scholarly composite catalog/source mapping drifted')
                native_metadata = record_type in RECORD_FILES and record_type != 'link'
                if native_metadata:
                    if (not corpus_validator.is_valid(source_record)
                            or source_record.get('record_id') != record_id
                            or source_record.get('record_type') != record_type):
                        raise ValueError(f'{source_ref}: native metadata catalog/source identity drifted')
                    if hashlib.sha256(canonical_json(source_record).encode('utf-8')).hexdigest() != entry.get('record_sha256'):
                        raise ValueError(f'{source_ref}: native metadata catalog/source digest drifted')
                properties = dict(source_record)
                properties["source_record"] = dict(source_record)
                if record_type == 'artifact':
                    properties.update(artifact_display_fields(source_record))
                if native_composite:
                    properties.update(composite_display_fields(source_record))
                if record_type in profiles.profiles or native_metadata or record_type == 'artifact' or native_composite:
                    forms = load_metadata_forms(REPO_ROOT, source_ref, source_record, access_allowed=True)
                    if forms is not None:
                        forms_ref, _forms_raw, materialized = forms
                        properties.update(human_forms=materialized, human_forms_source_ref=forms_ref,
                                          source_sha256=entry['record_sha256'])
                properties.update(dict(entry.get("links") or {}))
                variant_labels = source_record.get("variant_labels")
                if isinstance(variant_labels, list):
                    properties["variant_labels"] = variant_labels
                notes = source_record.get("notes")
                if isinstance(notes, str) and notes.strip():
                    properties["description"] = notes.strip()
                external_identifiers = source_record.get("external_identifiers")
                if isinstance(external_identifiers, list):
                    properties["external_identifiers"] = external_identifiers
                if record_type == "link":
                    properties.update(
                        {
                            key: source_record.get(key)
                            for key in (
                                "uri",
                                "link_kind",
                                "provider_label",
                                "interface_type",
                                "access_status",
                                "observed_at",
                                "observation_ref",
                                "mutable",
                                "provenance_event_ref",
                            )
                        }
                    )
                history = None
                if metadata_reader.supports(record_type, source_ref=source_ref):
                    history = metadata_reader.exact_refs(record_id)
                    properties['record_history'] = {'schema_version': 'tos_metadata_record_history_v1', **history}
                add_node(
                    record_id,
                    str(record_type),
                    str(entry.get("preferred_label") or record_id),
                    source_ref,
                    str(entry.get("identity_status") or "unknown"),
                    properties,
                )
                if history is not None:
                    for reference in history['refs']:
                        resolved = metadata_reader.resolve(reference)
                        version_id, version_ref = add_version_node(reference, resolved, 'metadata')
                        add_edge('source-navigation:record-history:' + version_id, record_id,
                                 'has_record_version', version_id, 'exact_historical_record_reference',
                                 [source_ref, version_ref])
                    if history['status'] != 'available':
                        diagnostics.append({'level': 'warning', 'path': source_ref,
                            'message': 'exact metadata history unavailable: ' + history['status'] + '/' + history['reason']})
                if record_type == 'sign':
                    # The immutable issuance reference points to a version,
                    # never to a convenient current Claim with the same ID.
                    # Read only source metadata here; access consumes this view
                    # without scanning archives or consulting live authority.
                    if version_reader is None:
                        from claim_version_reader import ClaimVersionReader
                        version_reader = ClaimVersionReader(REPO_ROOT)
                    reference = source_record['promotion_basis']['candidate']
                    resolved = version_reader.resolve(reference)
                    available = resolved['status'] == 'available'
                    version_id, version_ref = add_version_node(reference, resolved, 'claim')
                    basis_ref = source_ref + '#/promotion_basis/candidate'
                    add_edge('source-navigation:promotion-basis:' + record_id, record_id,
                             'promotion_basis_version', version_id, 'exact_historical_record_reference',
                             [basis_ref, version_ref])
                    if not available:
                        diagnostics.append({'level': 'warning', 'path': source_ref,
                            'message': 'exact Sign promotion basis unavailable: ' + resolved['status'] + '/' + resolved['reason']})

    planting_root = TOS_ROOT / "philosophy" / "eras"
    for planting_path in sorted(planting_root.rglob("source-planting.json")):
        planting = load_json(planting_path)
        planting_id = planting.get("planting_id")
        branch_path = planting.get("branch_path")
        witness = planting.get("source_witness")
        if not isinstance(planting_id, str) or not isinstance(branch_path, str) or not isinstance(witness, dict):
            continue
        planting_ref = repo_ref(planting_path)
        add_node(
            planting_id,
            "source_planting",
            str(planting.get("source_backlog_anchor", {}).get("source_label") or planting_id),
            planting_ref,
            str(planting.get("authority", {}).get("source_status") or "unknown"),
            {
                "status": planting.get("status"),
                "discovery_ref": planting.get("discovery_ref"),
                "research_ref": planting.get("research_ref"),
            },
        )
        branch = branch_by_path.get(branch_path)
        if isinstance(branch, dict):
            branch_id = str(branch.get("branch_id") or "")
            add_edge(
                f"source-navigation:planting:{branch_id}:{planting_id}",
                branch_id,
                "has_source_planting",
                planting_id,
                "authored_source_planting",
                [planting_ref],
            )
        witness_id = next(
            (
                value
                for key in ("work_id", "artifact_id", "composite_id", "item_id")
                for value in (witness.get(key),)
                if isinstance(value, str) and value
            ),
            None,
        )
        if witness_id:
            witness_ref = str(witness.get("record_ref") or planting_ref)
            if witness_id not in nodes:
                add_node(witness_id, "source_witness", witness_id, witness_ref, "provisional")
            add_edge(
                f"source-navigation:witness:{planting_id}:{witness_id}",
                planting_id,
                str(witness.get("relationship") or "references_source_witness"),
                witness_id,
                "authored_source_planting",
                [planting_ref, witness_ref],
            )

    claim_files = (
        "work-expression-claims.jsonl",
        "expression-edition-claims.jsonl",
        "edition-item-claims.jsonl",
        "object-link-claims.jsonl",
        "responsibility-claims.jsonl",
    )
    for basename in claim_files:
        for claim_path in sorted((TOS_ROOT / "source-witnesses").rglob(basename)):
            claim_ref = repo_ref(claim_path)
            for claim in _jsonl(claim_path):
                if claim.get("visibility") not in {"public", "public_payload", "public_metadata_only"}:
                    continue
                subject_ref = claim.get("subject_ref")
                object_ref = claim.get("object")
                claim_id = claim.get("claim_id")
                if not all(isinstance(value, str) and value for value in (subject_ref, object_ref, claim_id)):
                    continue
                if subject_ref not in nodes or object_ref not in nodes:
                    diagnostics.append(
                        {
                            "level": "error",
                            "path": claim_ref,
                            "message": f"source-navigation claim {claim_id} has an unresolved endpoint",
                        }
                    )
                    continue
                add_edge(
                    f"source-navigation:claim:{claim_id}",
                    subject_ref,
                    str(claim.get("predicate") or "related_to"),
                    object_ref,
                    "evidence_claim",
                    [claim_ref, *[str(ref) for ref in claim.get("evidence_refs", [])]],
                    str(claim.get("review_status") or "unknown"),
                    claim_id,
                )

    for manifest_path in sorted((TOS_ROOT / "source-witnesses").rglob("item.manifest.json")):
        manifest = load_json(manifest_path)
        item_id = manifest.get("item_id")
        if not isinstance(item_id, str) or item_id not in nodes:
            continue
        manifest_ref = repo_ref(manifest_path)
        for file_entry in manifest.get("payload_files", []):
            if not isinstance(file_entry, dict) or not isinstance(file_entry.get("file_id"), str):
                continue
            file_id = str(file_entry["file_id"])
            add_node(
                file_id,
                "file",
                str(file_entry.get("original_basename") or file_id),
                manifest_ref,
                "content_addressed",
                {
                    key: file_entry.get(key)
                    for key in ("media_type", "byte_size", "sha256", "fixity_verified_at")
                },
            )
            add_edge(
                f"source-navigation:file:{item_id}:{file_id}",
                item_id,
                "has_file",
                file_id,
                "authored_item_manifest",
                [manifest_ref],
            )

    # Source packet metadata only; no local-content or payload reads.
    packet_paths = sorted({* (TOS_ROOT / 'source-witnesses').rglob('source-text-unit*.json'),
                           * (TOS_ROOT / 'source-witnesses').rglob('*.source-text-unit.v1.json'),
                           * (TOS_ROOT / 'source-witnesses').rglob('semantic-annotation*.json')})
    for packet_path in packet_paths:
        packet_ref = repo_ref(packet_path)
        if any(part in {'payload', 'local-content'} for part in packet_path.parts):
            continue
        packet = load_json(packet_path)
        projected_nodes, projected_edges = project_text_packet(packet, packet_ref)
        for node in projected_nodes:
            add_node(node['node_id'], node['node_kind'], node['label'], node['source_ref'], node['identity_status'], node['properties'])
        for edge in projected_edges:
            if edge['from_id'] not in nodes or edge['to_id'] not in nodes:
                diagnostics.append({'level': 'warning', 'path': packet_ref, 'message': 'text spine endpoint not projected: ' + edge['edge_id']})
                continue
            add_edge(edge['edge_id'], edge['from_id'], edge['predicate_id'], edge['to_id'], edge['edge_kind'], edge['source_refs'], edge['review_status'], edge.get('claim_ref'))

    for rights_path in sorted((TOS_ROOT / "source-witnesses").rglob("rights.json")):
        record = load_json(rights_path)
        if record.get("visibility") not in {"public", "public_payload", "public_metadata_only"}:
            continue
        rights_ref = repo_ref(rights_path)
        assessments = [record, *[item for item in record.get("layer_assessments", []) if isinstance(item, dict)]]
        for index, assessment in enumerate(assessments):
            rights_id = str(assessment.get("layer_id") or record.get("rights_id") or f"{rights_ref}#{index}")
            rights.append(
                {
                    "rights_id": rights_id,
                    "scope_refs": sorted(str(ref) for ref in assessment.get("scope_refs", []) if isinstance(ref, str)),
                    "assessment_status": str(assessment.get("assessment_status") or "unknown"),
                    "review_status": str(assessment.get("review_status") or record.get("review_status") or "unknown"),
                    "redistribution_posture": str(assessment.get("redistribution_posture") or record.get("redistribution_posture") or "unknown"),
                    "derivative_posture": str(assessment.get("derivative_posture") or record.get("derivative_posture") or "unknown"),
                    "server_processing_posture": str(assessment.get("server_processing_posture") or record.get("server_processing_posture") or "unknown"),
                    "visibility": str(record.get("visibility") or "unknown"),
                    "license_uri": assessment.get("license_uri") or record.get("license_uri"),
                    "rights_statement_uri": assessment.get("rights_statement_uri") or record.get("rights_statement_uri"),
                    "restrictions": [str(item) for item in assessment.get("restrictions", record.get("restrictions", []))],
                    "source_ref": rights_ref,
                }
            )

    projected_nodes = [nodes[key] for key in sorted(nodes)]
    if assessed_forms is not None:
        if not isinstance(assessed_forms, AssessedFormSnapshot):
            raise TypeError('assessed forms require an explicit protected owner snapshot')
        projected_nodes = assessed_forms.materialize(projected_nodes)
    if version_reader is not None:
        version_reader.verify_current()
    if metadata_reader is not None:
        metadata_reader.verify_current()
    if catalog_digests is not None and (
            catalog_manifest_path.read_bytes() != catalog_manifest_raw
            or any(sha256(REPO_ROOT / ref) != digest for ref, digest in catalog_digests.items())):
        raise PublicationChanged('catalog bytes changed during corpus source navigation')
    return {
        "schema_version": "tos_source_navigation_v1",
        "authority_boundary": (
            "generated read-only navigation; authored branch manifests, source records, claims, "
            "item manifests, and rights records retain authority"
            + ('; local assessed research candidate, not public clearance or a current runtime grant'
               if assessed_forms is not None else '')
        ),
        "counts": {"nodes": len(nodes), "edges": len(edges), "rights": len(rights)},
        "nodes": projected_nodes,
        "edges": [edges[key] for key in sorted(edges)],
        "rights": sorted(rights, key=lambda item: item["rights_id"]),
    }


def load_schema() -> dict[str, Any]:
    return load_json(REPO_ROOT / SCHEMA_REF)


def validate_payload_schema(payload: dict[str, Any]) -> None:
    validator = Draft202012Validator(load_schema())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.absolute_path))
    if errors:
        error = errors[0]
        path = "".join(f"[{item}]" if isinstance(item, int) else f".{item}" for item in error.absolute_path)
        raise ValueError(f"schema violation at {path.lstrip('.') or '<root>'}: {error.message}")


def build_payload(*, assessed_forms: AssessedFormSnapshot | None = None) -> dict[str, Any]:
    snapshot = PublicationSnapshot(REPO_ROOT)
    payload = _build_payload(assessed_forms=assessed_forms)
    snapshot.verify_current()
    return payload


def _build_payload(*, assessed_forms) -> dict[str, Any]:
    diagnostics: list[dict[str, str]] = []
    tracked_paths = tracked_tos_paths()
    source_home = load_json(TOS_ROOT / "source_home.manifest.json")
    branches = build_branches(source_home, diagnostics)
    manifests = build_manifests(diagnostics, tracked_paths)
    nodes = build_nodes(diagnostics, tracked_paths)
    relation_packs, relation_edges = build_relations(diagnostics, tracked_paths)
    resources = build_resources(tracked_paths)
    source_navigation = build_source_navigation(diagnostics, assessed_forms=assessed_forms)
    payload: dict[str, Any] = {
        "schema_version": "tos_corpus_index_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_corpus_index",
        "authority_order": list(AUTHORITY_ORDER),
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "allowed": [
                "read ToS-owned corpus surfaces",
                "serve MCP resources and tools that point back to ToS",
                "build runtime graph projections, UI views, and Neo4j caches",
                "emit review diagnostics without changing ToS authority",
            ],
            "not_allowed": [
                "move canonical ToS meaning into abyss-stack",
                "treat Neo4j, MCP, UI, or runtime cache as source truth",
                "write ToS canon without ToS validators and explicit operator route",
            ],
        },
        "validation_refs": list(VALIDATION_REFS),
        "counts": {
            "branches": len(branches),
            "manifests": len(manifests),
            "nodes": len(nodes),
            "relation_packs": len(relation_packs),
            "relation_edges": len(relation_edges),
            "resources": len(resources),
            "source_navigation_nodes": source_navigation["counts"]["nodes"],
            "source_navigation_edges": source_navigation["counts"]["edges"],
            "diagnostics": len(diagnostics),
        },
        "graph_views": list(GRAPH_VIEWS),
        "branches": branches,
        "manifests": manifests,
        "nodes": nodes,
        "relation_packs": relation_packs,
        "relation_edges": relation_edges,
        "resources": resources,
        "source_navigation": source_navigation,
        "diagnostics": diagnostics,
    }
    validate_payload_schema(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"
