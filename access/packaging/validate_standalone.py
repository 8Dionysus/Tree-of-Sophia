#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import os
import re
import sqlite3
import subprocess
import sys
import tempfile
import zipfile
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

BLOCKED_CODE_MARKERS = (b"/srv/" + b"AbyssOS", b"/srv/" + b"abyss-machine")
EXPECTED_QUERY_OPERATIONS = {
    "tos.status",
    "tos.snapshot",
    "tos.search",
    "tos.source-gaps.search",
    "tos.source.descend",
    "tos.dossier.inspect",
    "tos.view.open",
    "tos.node.inspect",
    "tos.neighborhood",
    "tos.epistemic.inspect",
    "tos.path.find",
    "tos.zarathustra.word-analysis.prepare",
}
EXPECTED_PAGE_COMMANDS = {
    "tos.page.context",
    "tos.page.open-view",
    "tos.page.search",
    "tos.page.find-source-gaps",
    "tos.page.prepare-word-analysis",
    "tos.page.select",
    "tos.page.inspect-selection",
    "tos.page.show-neighborhood",
    "tos.page.start-path",
    "tos.page.find-path",
    "tos.page.reroute-without-selection",
    "tos.page.inspect-epistemic",
    "tos.page.compare-readings",
    "tos.page.research-workspace",
    "tos.page.add-research-note",
    "tos.page.add-session-hypothesis",
    "tos.page.stage-proposal",
    "tos.page.exclude-selected-edge",
    "tos.page.save-route-comparison",
    "tos.page.workspace-undo",
    "tos.page.workspace-redo",
    "tos.page.workspace-export",
    "tos.page.workspace-import",
    "tos.page.clear-focus",
    "tos.page.cancel",
}
EXPECTED_KNOWLEDGE_OPERATIONS = {
    "tos.knowledge.catalog",
    "tos.knowledge.contracts",
    "tos.knowledge.search",
    "tos.knowledge.node.inspect",
    "tos.knowledge.relation.inspect",
    "tos.knowledge.focus",
    "tos.lens.open",
    "tos.lens.compile",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _allowlist_subject_paths(repo_root: Path, allowlist: dict[str, Any]) -> set[str]:
    """Validate exact allowlist closure and return all admitted repo paths."""
    # Keep the builder and validator on one manifest-reader implementation so
    # a bundle cannot be built with one closure interpretation and accepted by
    # another. The import is local because this script is also used standalone.
    packaging_root = Path(__file__).resolve().parent
    if packaging_root.as_posix() not in sys.path:
        sys.path.insert(0, packaging_root.as_posix())
    from build_standalone_bundle import _runtime_subject_paths

    return {
        path.relative_to(repo_root.resolve()).as_posix()
        for path in _runtime_subject_paths(repo_root.resolve(), allowlist)
    }


def _safe_extracted_path(root: Path, relative: str) -> Path:
    candidate = (root / relative).resolve()
    if not candidate.is_relative_to(root.resolve()):
        raise RuntimeError(f"bundle subject escapes archive root: {relative}")
    return candidate


def _compiled_subject_specs(allowlist: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Use the bundle builder's contract parser for the same field shape."""
    packaging_root = Path(__file__).resolve().parent
    if packaging_root.as_posix() not in sys.path:
        sys.path.insert(0, packaging_root.as_posix())
    from build_standalone_bundle import _validate_compiled_subjects

    return _validate_compiled_subjects(allowlist)


def _bundle_runtime_allowlist(extracted: Path) -> dict[str, Any]:
    path = extracted / "access/src/tos_access/runtime_data/access/contracts/runtime-data.v1.json"
    if not path.is_file():
        raise RuntimeError("standalone bundle has no runtime-data contract")
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise RuntimeError("standalone bundle runtime-data contract must be an object")
    return value


def _compiler_fingerprint(root: Path, paths: list[str]) -> str:
    digest = hashlib.sha256()
    for relative in paths:
        if (
            not isinstance(relative, str)
            or not relative
            or Path(relative).is_absolute()
            or ".." in Path(relative).parts
        ):
            raise RuntimeError("bundle query compiler paths must be safe repository-relative strings")
        path = _safe_extracted_path(root, relative)
        if not path.is_file():
            raise RuntimeError(f"bundle query compiler source is missing: {relative}")
        digest.update(relative.encode("utf-8") + b"\0")
        digest.update(sha256_file(path).encode("ascii") + b"\n")
    return digest.hexdigest()


def _validate_query_store_artifact(
    extracted: Path,
    subjects: list[dict[str, Any]],
    allowlist: Mapping[str, Any] | None = None,
) -> None:
    """Validate the generated immutable query snapshot declared by the contract."""
    if allowlist is None:
        allowlist = _bundle_runtime_allowlist(extracted)
    compiled_specs = _compiled_subject_specs(allowlist)
    partitioned = [
        subject
        for subject in subjects
        if isinstance(subject, dict)
        and subject.get("projection_schema") == "tos_partitioned_projection_v1"
    ]
    partitioned_ids = {
        subject.get("subject_id")
        for subject in partitioned
        if isinstance(subject.get("subject_id"), str)
    }
    if partitioned:
        packaging_root = Path(__file__).resolve().parent
        if packaging_root.as_posix() not in sys.path:
            sys.path.insert(0, packaging_root.as_posix())
        from build_standalone_bundle import _validate_partitioned_subject_policy

        _validate_partitioned_subject_policy(allowlist)
    active_specs = [
        spec
        for spec in compiled_specs
        if spec["required_when"] == "partitioned_projection_inputs"
        and partitioned_ids.intersection(spec["input_subject_ids"])
    ]
    generated_subjects = [
        subject
        for subject in subjects
        if isinstance(subject, dict) and subject.get("generated") is True
    ]
    if not partitioned:
        if generated_subjects:
            raise RuntimeError("bundle declares generated subjects without partitioned projection inputs")
        return
    if len(active_specs) != 1:
        raise RuntimeError("partitioned standalone bundle has no unique compiled subject contract")
    compiled_spec = active_specs[0]
    query_subjects = [
        subject
        for subject in generated_subjects
        if subject.get("subject_id") == compiled_spec["subject_id"]
    ]
    if len(generated_subjects) != 1 or len(query_subjects) != 1:
        raise RuntimeError("partitioned standalone bundle must declare exactly one compiled subject")
    subject = query_subjects[0]
    expected_source_path = compiled_spec["output_path"]
    expected_bundle_path = (
        Path("access/src/tos_access/runtime_data") / expected_source_path
    ).as_posix()
    if (
        subject.get("source_path") != expected_source_path
        or subject.get("bundle_path") != expected_bundle_path
        or subject.get("generated") is not True
    ):
        raise RuntimeError("compiled query-store subject has an unexpected path or provenance")
    query_path = _safe_extracted_path(extracted, str(subject["bundle_path"]))
    if not query_path.is_file():
        raise RuntimeError("compiled query-store artifact is missing")
    if any(Path(str(query_path) + suffix).exists() for suffix in ("-wal", "-journal")):
        raise RuntimeError("compiled query-store artifact has a mutable SQLite journal")
    compiler = subject.get("compiler")
    if not isinstance(compiler, dict):
        raise RuntimeError("compiled query-store subject has no compiler provenance")
    if compiler.get("builder_module") != compiled_spec["builder_module"]:
        raise RuntimeError("compiled query-store builder module does not match its contract")
    if not isinstance(compiler.get("schema"), str) or not isinstance(compiler.get("compiler_version"), str):
        raise RuntimeError("compiled query-store schema/compiler version is invalid")
    compiler_paths = compiler.get("compiler_paths")
    compiler_digest = compiler.get("compiler_sha256")
    if (
        not isinstance(compiler_paths, list)
        or not compiler_paths
        or not isinstance(compiler_digest, str)
        or not re.fullmatch(r"[0-9a-f]{64}", compiler_digest)
    ):
        raise RuntimeError("compiled query-store compiler provenance is invalid")
    declared_module_path = (
        Path("access/src")
        / Path(compiled_spec["builder_module"].replace(".", "/")).with_suffix(".py")
    ).as_posix()
    if declared_module_path not in compiler_paths:
        raise RuntimeError("compiled query-store compiler provenance omits its declared builder module")
    if _compiler_fingerprint(extracted, compiler_paths) != compiler_digest:
        raise RuntimeError("compiled query-store compiler bytes do not match its declared digest")
    bindings = compiler.get("input_bindings")
    if not isinstance(bindings, dict) or any(
        not isinstance(path, str)
        or not isinstance(value, str)
        or not re.fullmatch(r"[0-9a-f]{64}", value)
        for path, value in bindings.items()
    ):
        raise RuntimeError("compiled query-store input bindings are invalid")
    source_subjects = {
        item.get("subject_id"): item
        for item in subjects
        if isinstance(item, dict) and isinstance(item.get("subject_id"), str)
    }
    expected_bindings: dict[str, str] = {}
    for subject_id in compiled_spec["input_subject_ids"]:
        source = source_subjects.get(subject_id)
        if not isinstance(source, dict):
            raise RuntimeError(f"compiled query-store input subject is absent from bundle: {subject_id}")
        source_path = source.get("source_path")
        source_hash = source.get("sha256")
        if not isinstance(source_path, str) or not isinstance(source_hash, str):
            raise RuntimeError(f"compiled query-store input subject has no root identity: {subject_id}")
        if source_path in expected_bindings:
            raise RuntimeError(f"compiled query-store input subjects share a source path: {source_path}")
        expected_bindings[source_path] = source_hash
    if bindings != expected_bindings:
        raise RuntimeError("compiled query-store input bindings do not match bundled source subjects")

    try:
        with sqlite3.connect(query_path.as_uri() + "?mode=ro&immutable=1", uri=True) as db:
            metadata = {key: json.loads(value) for key, value in db.execute("SELECT key,value FROM metadata")}
            integrity = db.execute("PRAGMA integrity_check").fetchone()
    except (OSError, sqlite3.Error, ValueError, TypeError) as exc:
        raise RuntimeError(f"compiled query-store artifact is not a readable SQLite snapshot: {exc}") from exc
    if metadata.get("schema") != compiler["schema"] or metadata.get("complete") is not True:
        raise RuntimeError("compiled query-store artifact has an unsupported or incomplete schema")
    if metadata.get("compiler_version") != compiler.get("compiler_version"):
        raise RuntimeError("compiled query-store compiler version does not match its metadata")
    if metadata.get("snapshot_bindings") != bindings:
        raise RuntimeError("compiled query-store snapshot bindings do not match its declared inputs")
    if integrity != ("ok",):
        raise RuntimeError("compiled query-store SQLite integrity check failed")


class _QueryStoreRows(Sequence):
    """Read-only array view over one query-store table.

    JSON Schema normally recognizes only its ordinary array type.  The
    validator below adds this private view as one extra array implementation;
    every row is still decoded and checked by the declared item schema while
    the sequence itself never retains the table in memory.
    """

    _TABLES = {"knowledge_nodes", "knowledge_relations"}

    def __init__(self, store: Any, table: str):
        if table not in self._TABLES:
            raise ValueError(f"unsupported query-store row table: {table}")
        self._store = store
        self._table = table

    def __len__(self) -> int:
        return self._store.count(self._table)

    def __iter__(self):
        return self._store.rows(self._table)

    def __getitem__(self, index):
        raise TypeError("query-store row view does not support random access")


def _streaming_items_keyword(validator, items, instance, schema):
    """Validate ``items`` by one pass for the private query-store sequence."""
    from jsonschema import Draft202012Validator
    from jsonschema.exceptions import ValidationError

    if not isinstance(instance, _QueryStoreRows):
        yield from Draft202012Validator.VALIDATORS["items"](validator, items, instance, schema)
        return
    if not validator.is_type(instance, "array"):
        return

    prefix = len(schema.get("prefixItems", []))
    total = len(instance)
    extra = total - prefix
    if extra <= 0:
        return
    if items is False:
        # Do not slice the sequence to build an error representation: that
        # would either materialize the table or reintroduce random-access SQL.
        item = "items" if prefix != 1 else "item"
        yield ValidationError(
            f"Expected at most {prefix} {item} but found {extra} extra "
            "streamed query-store rows",
        )
        return

    for index, value in enumerate(itertools.islice(iter(instance), prefix, None), start=prefix):
        yield from validator.descend(instance=value, schema=items, path=index)


def _streaming_knowledge_graph_validator(schema: dict[str, Any], registry: Any):
    """Build a validator that admits only the private query-store array view."""
    from jsonschema import Draft202012Validator, validators

    base_type_checker = Draft202012Validator.TYPE_CHECKER

    def is_array(checker, instance):
        return base_type_checker.is_type(instance, "array") or isinstance(instance, _QueryStoreRows)

    type_checker = base_type_checker.redefine("array", is_array)
    validator_type = validators.extend(
        Draft202012Validator,
        validators={"items": _streaming_items_keyword},
        type_checker=type_checker,
    )
    return validator_type(schema, registry=registry)


def _iter_compiled_knowledge_rows(store: Any, table: str):
    """Yield canonical SQL identity fields and one decoded payload at a time."""
    columns = "id, payload" if table == "knowledge_nodes" else "id, from_id, to_id, payload"
    with store.connect() as db:
        for row in db.execute(f"SELECT {columns} FROM {table} ORDER BY id"):
            payload = json.loads(row[-1])
            if table == "knowledge_nodes":
                yield row[0], payload
            else:
                yield row[0], row[1], row[2], payload


def _validate_compiled_knowledge_graph(store: Any, graph: dict[str, Any], content_revision) -> None:
    """Validate a compiled graph without exporting its node/relation arrays."""
    counts = graph.get("counts", {})
    with store.connect() as db:
        node_count, node_distinct = db.execute(
            "SELECT count(*), count(DISTINCT id) FROM knowledge_nodes"
        ).fetchone()
        relation_count, relation_distinct = db.execute(
            "SELECT count(*), count(DISTINCT id) FROM knowledge_relations"
        ).fetchone()
        if node_count != node_distinct or relation_count != relation_distinct:
            raise RuntimeError("normalized knowledge graph IDs are not unique in the query store")
        if counts.get("nodes") != node_count or counts.get("relations") != relation_count:
            raise RuntimeError("compiled knowledge graph counts do not match query-store rows")
        dangling = [row[0] for row in db.execute(
            """
            SELECT relation.id
            FROM knowledge_relations AS relation
            LEFT JOIN knowledge_nodes AS source ON source.id = relation.from_id
            LEFT JOIN knowledge_nodes AS target ON target.id = relation.to_id
            WHERE source.id IS NULL OR target.id IS NULL
            ORDER BY relation.id
            LIMIT 5
            """
        )]
    if dangling:
        raise RuntimeError(f"normalized knowledge graph has dangling relations: {dangling}")

    for row_id, item in _iter_compiled_knowledge_rows(store, "knowledge_nodes"):
        if item.get("id") != row_id:
            raise RuntimeError(f"normalized knowledge node SQL/payload ID mismatch: {row_id}")
        if item.get("content_revision") != content_revision(item):
            raise RuntimeError("normalized knowledge item content revisions are stale")
    for row_id, from_id, to_id, item in _iter_compiled_knowledge_rows(store, "knowledge_relations"):
        if item.get("id") != row_id:
            raise RuntimeError(f"normalized knowledge relation SQL/payload ID mismatch: {row_id}")
        if item.get("from_id") != from_id or item.get("to_id") != to_id:
            raise RuntimeError(f"normalized knowledge relation SQL/payload endpoint mismatch: {row_id}")
        if item.get("content_revision") != content_revision(item):
            raise RuntimeError("normalized knowledge item content revisions are stale")


def _validate_knowledge_contracts(repo_root: Path) -> None:
    try:
        from jsonschema import Draft202012Validator
        from referencing import Registry, Resource
    except ImportError as exc:
        raise RuntimeError(
            "standalone source validation requires requirements-dev.txt"
        ) from exc

    contract_root = repo_root / "access/contracts"
    schema_names = (
        "knowledge-graph.v1.schema.json",
        "lens-spec.v1.schema.json",
        "lens-result.v1.schema.json",
        "exploration-request.v1.schema.json",
        "exploration-result.v1.schema.json",
    )
    schemas = {
        name: json.loads((contract_root / name).read_text(encoding="utf-8"))
        for name in schema_names
    }
    for name, schema in schemas.items():
        try:
            Draft202012Validator.check_schema(schema)
        except Exception as exc:
            raise RuntimeError(f"invalid knowledge contract schema {name}: {exc}") from exc
    registry = Registry().with_resources(
        (schema["$id"], Resource.from_contents(schema))
        for schema in schemas.values()
    )

    api = json.loads((contract_root / "knowledge-api.v1.json").read_text(encoding="utf-8"))
    operations = {item.get("operation_id"): item for item in api.get("operations", [])}
    if set(operations) != EXPECTED_KNOWLEDGE_OPERATIONS:
        raise RuntimeError(f"knowledge operation contract drift: {sorted(operations)}")
    compile_operation = operations["tos.lens.compile"]
    if compile_operation.get("http", {}).get("method") != "POST" or "creates no server state" not in str(
        compile_operation.get("post_semantics")
    ):
        raise RuntimeError("knowledge lens compile must remain a read-only structured query")

    access_src = (repo_root / "access/src").as_posix()
    if access_src not in sys.path:
        sys.path.insert(0, access_src)
    from tos_access.core import ToSAccessCore
    from tos_access.knowledge import _content_revision

    core = ToSAccessCore.discover(tos_root=repo_root)
    graph_validator = _streaming_knowledge_graph_validator(
        schemas["knowledge-graph.v1.schema.json"], registry
    )
    # A compiled snapshot is the normal path for partitioned projections.  Use
    # a private lazy array view so jsonschema validates every row and every
    # nested field without calling the explicit full-export route.
    store = core._query_store()
    if store is None:
        # Small legacy fixtures retain their existing full in-memory route.
        graph = core.knowledge_graph()
        graph_validator.validate(graph)
        nodes = graph.get("nodes", [])
        relations = graph.get("relations", [])
        node_ids = {item.get("id") for item in nodes}
        relation_ids = {item.get("id") for item in relations}
        if len(node_ids) != len(nodes) or len(relation_ids) != len(relations):
            raise RuntimeError("normalized knowledge graph IDs must be unique")
        if any(item.get("content_revision") != _content_revision(item) for item in [*nodes, *relations]):
            raise RuntimeError("normalized knowledge item content revisions are stale")
        dangling = [
            item.get("id")
            for item in relations
            if item.get("from_id") not in node_ids or item.get("to_id") not in node_ids
        ]
        if dangling:
            raise RuntimeError(f"normalized knowledge graph has dangling relations: {dangling[:5]}")
    else:
        graph = {
            **store.header,
            "nodes": _QueryStoreRows(store, "knowledge_nodes"),
            "relations": _QueryStoreRows(store, "knowledge_relations"),
        }
        graph_validator.validate(graph)
        _validate_compiled_knowledge_graph(store, graph, _content_revision)
        nodes = graph["nodes"]
        relations = graph["relations"]
    counts = graph.get("counts", {})
    semantic_mapping = counts.get("semantic_mapping", {})
    semantic_validation = counts.get("semantic_validation", {})
    if semantic_mapping.get("unmapped_nodes") != 0 or semantic_mapping.get("unmapped_relations") != 0:
        raise RuntimeError("normalized knowledge graph contains unmapped production vocabulary")
    if semantic_validation.get("valid") is not True or semantic_validation.get("violations"):
        raise RuntimeError("normalized knowledge graph semantic registry invariants failed")
    coverage = counts.get("display_coverage", {})
    expected_coverage = {
        "node_titles": counts.get("nodes"),
        "node_summaries": counts.get("nodes"),
        "relation_labels": counts.get("relations"),
        "relation_statements": counts.get("relations"),
        "relation_explanations": counts.get("relations"),
    }
    if any(coverage.get(key) != value for key, value in expected_coverage.items()):
        raise RuntimeError("normalized knowledge graph display coverage is incomplete")

    catalog = core.knowledge_catalog()
    contract_bundle = core.knowledge_contracts()
    if contract_bundle.get("schema") != "tos_knowledge_contract_bundle_v1":
        raise RuntimeError("knowledge contract bundle is not current")
    if set(contract_bundle.get("contracts", {})) != {
        "api",
        "knowledge_graph",
        "lens_spec",
        "lens_result",
        "entity_type_registry_schema",
        "relation_type_registry_schema",
        "entity_type_registry",
        "relation_type_registry",
    }:
        raise RuntimeError("knowledge contract bundle is incomplete")
    bundled_contracts = contract_bundle["contracts"]
    Draft202012Validator(
        bundled_contracts["entity_type_registry_schema"]
    ).validate(bundled_contracts["entity_type_registry"])
    Draft202012Validator(
        bundled_contracts["relation_type_registry_schema"]
    ).validate(bundled_contracts["relation_type_registry"])
    lens_validator = Draft202012Validator(schemas["lens-spec.v1.schema.json"])
    for lens in catalog.get("lenses", []):
        lens_validator.validate(lens)
    result = core.compile_knowledge_lens(
        {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "standalone-validation-smoke",
            "sources": ["philosophy"],
            "node_query": {
                "filters": [{"field": "source_graph", "op": "eq", "value": "philosophy"}]
            },
            "relation_query": {"enabled": False},
            "limits": {"nodes": 2, "relations": 0, "groups": 2},
        }
    )
    Draft202012Validator(
        schemas["lens-result.v1.schema.json"], registry=registry
    ).validate(result)
    exploration_contracts = core.knowledge_exploration_contracts()
    if exploration_contracts["capabilities"]["runtime"] != "local" or exploration_contracts["capabilities"]["restart_survival"] is not False:
        raise RuntimeError("local exploration must describe its own process-local storage")
    first_node = next(iter(graph["nodes"]), None)
    if first_node is None:
        raise RuntimeError("normalized knowledge graph has no nodes for exploration validation")
    page = core.knowledge_explore({"focus_node_id": first_node["id"], "page_nodes": 1})
    Draft202012Validator(schemas["exploration-result.v1.schema.json"], registry=registry).validate(page)
    if page["page"]["next_cursor"]:
        page = core.knowledge_explore({"cursor": page["page"]["next_cursor"]})
        Draft202012Validator(schemas["exploration-result.v1.schema.json"], registry=registry).validate(page)


def _validate_contracts(repo_root: Path) -> None:
    contract_root = repo_root / "access/contracts"
    runtime = json.loads((contract_root / "runtime-manifest.v1.json").read_text(encoding="utf-8"))
    if runtime.get("authority_owner") != "Tree-of-Sophia":
        raise RuntimeError("runtime authority owner must be Tree-of-Sophia")
    profiles = {item.get("profile_id"): item for item in runtime.get("runtime_profiles", [])}
    if profiles.get("standalone", {}).get("requires_abyssos") is not False:
        raise RuntimeError("standalone profile must not require AbyssOS")
    components = {item.get("component_id"): item for item in runtime.get("components", [])}
    lens_component = components.get("knowledge-lens-engine", {})
    if lens_component.get("required") is not True or lens_component.get("posture") != "read-only-derived-composition":
        raise RuntimeError("runtime manifest must require the read-only knowledge lens engine")
    posture = runtime.get("integration_posture")
    if posture != {
        "state": "paused",
        "scope": ["abyssos"],
        "default_profile": "standalone",
        "external_activation": "disabled",
        "unfreeze_requires": "explicit ToS operator command",
    }:
        raise RuntimeError("AbyssOS integration posture must remain explicitly paused")
    abyssos_profile = json.loads(
        (repo_root / "access/profiles/abyssos.v1.json").read_text(encoding="utf-8")
    )
    if abyssos_profile.get("availability") != "paused":
        raise RuntimeError("AbyssOS access profile must remain paused")
    migration = json.loads((contract_root / "web-actions.v1.json").read_text(encoding="utf-8"))
    expected_successors = {"query-operations.v1.json", "page-commands.v1.json"}
    if migration.get("status") != "superseded" or set(migration.get("superseded_by", [])) != expected_successors:
        raise RuntimeError("web action migration marker must route to the split contracts")
    query = json.loads((contract_root / "query-operations.v1.json").read_text(encoding="utf-8"))
    operation_ids = {item.get("operation_id") for item in query.get("operations", [])}
    if operation_ids != EXPECTED_QUERY_OPERATIONS:
        raise RuntimeError(f"query operation contract drift: {sorted(operation_ids)}")
    epistemic = json.loads((contract_root / "epistemic-packet.v1.schema.json").read_text(encoding="utf-8"))
    if epistemic.get("properties", {}).get("schema", {}).get("const") != "tos_philosophy_epistemic_packet_v1":
        raise RuntimeError("epistemic packet schema identity drift")
    authority = epistemic.get("properties", {}).get("authority_boundary", {}).get("properties", {})
    if {key: value.get("const") for key, value in authority.items()} != {
        "is_source": False,
        "is_canon": False,
        "is_semantic_truth": False,
        "is_rights_clearance": False,
    }:
        raise RuntimeError("epistemic packet authority boundary must fail closed")
    evidence_schema = json.loads(
        (repo_root / "ToS/contracts/epistemic-evidence-projection.schema.json").read_text(encoding="utf-8")
    )
    if evidence_schema.get("properties", {}).get("schema_version", {}).get("const") != "tos_epistemic_evidence_projection_v1":
        raise RuntimeError("Evidence Lens projection schema identity drift")
    evidence_packet = json.loads(
        (contract_root / "evidence-lens-packet.v1.schema.json").read_text(encoding="utf-8")
    )
    if evidence_packet.get("properties", {}).get("schema", {}).get("const") != "tos_evidence_lens_packet_v1":
        raise RuntimeError("Evidence Lens packet schema identity drift")
    page = json.loads((contract_root / "page-commands.v1.json").read_text(encoding="utf-8"))
    command_ids = {item.get("command_id") for item in page.get("commands", [])}
    if command_ids != EXPECTED_PAGE_COMMANDS:
        raise RuntimeError(f"page command contract drift: {sorted(command_ids)}")
    workspace = json.loads((contract_root / "research-workspace.v1.schema.json").read_text(encoding="utf-8"))
    if workspace.get("properties", {}).get("schema", {}).get("const") != "tos_research_workspace_session_v1":
        raise RuntimeError("research workspace packet schema identity drift")
    workspace_posture = workspace.get("$defs", {}).get("posture", {}).get("properties", {})
    if {key: value.get("const") for key, value in workspace_posture.items()} != {
        "session_hypothesis": True,
        "source": False,
        "reviewed": False,
        "canon": False,
    }:
        raise RuntimeError("research hypotheses must remain explicitly outside ToS authority")
    allowlist = json.loads((contract_root / "runtime-data.v1.json").read_text(encoding="utf-8"))
    paths = {item.get("source_path") for item in allowlist.get("subjects", [])}
    paths.update(_allowlist_subject_paths(repo_root, allowlist))
    required_projection_paths = {
        "ToS/derived-exports/epistemic_evidence_projection.min.json",
        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
    }
    missing_projection_paths = sorted(required_projection_paths - paths)
    if missing_projection_paths:
        raise RuntimeError(
            f"runtime allowlist is missing constructor inputs: {missing_projection_paths}"
        )
    if any("lexical-search" in str(path) or "/payload/" in str(path) for path in paths):
        raise RuntimeError("runtime allowlist admits an explicitly excluded subject")
    # Generated read models are admitted only through their explicit
    # compiled_subjects record. This also makes a partitioned root fail closed
    # when the contract forgot to declare the compiler output.
    packaging_root = Path(__file__).resolve().parent
    if packaging_root.as_posix() not in sys.path:
        sys.path.insert(0, packaging_root.as_posix())
    from build_standalone_bundle import _active_compiled_subject

    _active_compiled_subject(repo_root, allowlist)
    _validate_knowledge_contracts(repo_root)


def _scan_source(access_root: Path) -> None:
    suffixes = {".py", ".json", ".toml", ".ts", ".js", ".html"}
    for path in access_root.rglob("*"):
        if not path.is_file() or path.suffix not in suffixes or "runtime_data" in path.parts:
            continue
        payload = path.read_bytes()
        if any(marker in payload for marker in BLOCKED_CODE_MARKERS):
            raise RuntimeError(f"hard-coded host path: {path.relative_to(access_root)}")


def validate_source(repo_root: Path) -> dict[str, Any]:
    _validate_contracts(repo_root)
    _scan_source(repo_root / "access")
    env = os.environ.copy()
    env["PYTHONPATH"] = (repo_root / "access/src").as_posix()
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "tos_access",
            "--root",
            repo_root.as_posix(),
            "verify",
            "--profile",
            "standalone",
            "--json",
        ],
        cwd=repo_root,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"source doctor failed: {result.stdout}\n{result.stderr}")
    report = json.loads(result.stdout)
    return {
        "schema_version": "tos_standalone_validation_v1",
        "ok": True,
        "mode": "source",
        "doctor": report,
    }


def _verify_external_manifest(bundle: Path, manifest_path: Path | None = None) -> tuple[Path, dict[str, Any]]:
    sidecar = manifest_path or bundle.with_suffix(bundle.suffix + ".manifest.json")
    if not sidecar.is_file():
        raise RuntimeError(f"missing external bundle manifest: {sidecar}")
    manifest = json.loads(sidecar.read_text(encoding="utf-8"))
    expected_digest = manifest.get("archive_sha256")
    expected_size = manifest.get("archive_size_bytes")
    if not isinstance(expected_digest, str) or not expected_digest:
        raise RuntimeError("external bundle manifest has no archive_sha256")
    if sha256_file(bundle) != expected_digest:
        raise RuntimeError("archive digest does not match external bundle manifest")
    if not isinstance(expected_size, int) or bundle.stat().st_size != expected_size:
        raise RuntimeError("archive size does not match external bundle manifest")
    return sidecar, manifest


def validate_bundle(
    bundle: Path,
    *,
    manifest_path: Path | None = None,
    with_mcp: bool = False,
) -> dict[str, Any]:
    sidecar, external_manifest = _verify_external_manifest(bundle, manifest_path)
    with tempfile.TemporaryDirectory(prefix="tos-standalone-validate-") as raw_temp:
        temp = Path(raw_temp)
        extracted = temp / "bundle"
        outside = temp / "outside"
        outside.mkdir()
        with zipfile.ZipFile(bundle) as archive:
            archive.extractall(extracted)
        if any(path.name == ".git" for path in extracted.rglob("*")):
            raise RuntimeError("standalone archive contains Git metadata")
        embedded_manifest_path = extracted / "bundle.manifest.json"
        manifest = json.loads(embedded_manifest_path.read_text(encoding="utf-8"))
        if manifest.get("schema_version") != "tos_standalone_bundle_manifest_v1":
            raise RuntimeError("unknown standalone bundle manifest schema")
        if any(external_manifest.get(key) != value for key, value in manifest.items()):
            raise RuntimeError("embedded bundle manifest does not match the external manifest")
        subjects = manifest.get("subjects")
        if not isinstance(subjects, list):
            raise RuntimeError("standalone bundle manifest subjects must be a list")
        for subject in subjects:
            if not isinstance(subject, dict) or not isinstance(subject.get("bundle_path"), str):
                raise RuntimeError("bundle subjects must contain exact bundle_path strings")
            path = _safe_extracted_path(extracted, subject["bundle_path"])
            if (
                not path.is_file()
                or sha256_file(path) != subject.get("sha256")
                or path.stat().st_size != subject.get("size_bytes")
            ):
                raise RuntimeError(f"bundle subject integrity failure: {subject.get('subject_id')}")
            closure = subject.get("closure")
            if closure is None:
                continue
            if subject.get("projection_schema") != "tos_partitioned_projection_v1" or not isinstance(closure, list):
                raise RuntimeError(f"bundle partition closure metadata is invalid: {subject.get('subject_id')}")
            closure_paths: set[str] = set()
            for member in closure:
                if not isinstance(member, dict) or not isinstance(member.get("bundle_path"), str):
                    raise RuntimeError(f"bundle partition closure member is invalid: {subject.get('subject_id')}")
                member_path = _safe_extracted_path(extracted, member["bundle_path"])
                member_relative = member_path.relative_to(extracted.resolve()).as_posix()
                if member_relative in closure_paths:
                    raise RuntimeError(f"bundle partition closure contains duplicate path: {member_relative}")
                closure_paths.add(member_relative)
                if (
                    not member_path.is_file()
                    or sha256_file(member_path) != member.get("sha256")
                    or member_path.stat().st_size != member.get("size_bytes")
                ):
                    raise RuntimeError(f"bundle partition member integrity failure: {member.get('source_path')}")
            if subject["bundle_path"] not in [item.get("bundle_path") for item in closure]:
                raise RuntimeError(f"bundle partition closure omitted its root: {subject.get('subject_id')}")
        _validate_query_store_artifact(extracted, subjects)
        package_src = extracted / "access/src"
        env = {
            key: value
            for key, value in os.environ.items()
            if key not in {"TOS_ROOT", "AOA_TOS_ROOT", "PYTHONPATH"}
        }
        env["PYTHONPATH"] = package_src.as_posix()
        probe = """
import json
import threading
import urllib.request
from tos_access.core import ToSAccessCore
from tos_access.doctor import doctor_report
from tos_access.http_server import make_server

core = ToSAccessCore.discover()
report = doctor_report(require_mcp=False)
assert report["ok"], report
view_id = core.philosophy_views()["views"][0]["view_id"]
packet = core.philosophy_view(view_id)
assert packet["node_count"] > 0 and packet["edge_count"] > 0
evidence = core.evidence_projection()
assert len(evidence["scenes"]) >= 2
catalog = core.knowledge_catalog()
contracts = core.knowledge_contracts()
assert contracts["schema"] == "tos_knowledge_contract_bundle_v1"
assert set(contracts["contracts"]) == {
    "api", "knowledge_graph", "lens_spec", "lens_result",
    "entity_type_registry_schema", "relation_type_registry_schema",
    "entity_type_registry", "relation_type_registry",
}
counts = catalog["counts"]
coverage = counts["display_coverage"]
assert coverage["node_titles"] == counts["nodes"]
assert coverage["node_summaries"] == counts["nodes"]
assert coverage["relation_labels"] == counts["relations"]
assert coverage["relation_statements"] == counts["relations"]
assert coverage["relation_explanations"] == counts["relations"]
lens = core.compile_knowledge_lens({
    "schema_version": "tos_lens_spec_v1",
    "lens_id": "archive-smoke",
    "sources": ["philosophy"],
    "node_query": {"filters": [{"field": "source_graph", "op": "eq", "value": "philosophy"}]},
    "relation_query": {"enabled": False},
    "limits": {"nodes": 2, "relations": 0, "groups": 2},
})
assert lens["schema"] == "tos_lens_result_v1" and lens["nodes"]
assert lens["source_revision"] == catalog["source_revision"]
focused = core.knowledge_focus(lens["nodes"][0]["id"], depth=0)
assert focused["focus"]["node_id"] == lens["nodes"][0]["id"]
assert focused["agent_summary"]["focus_node_id"] == lens["nodes"][0]["id"]
author_id = "source-navigation:tos.agent.friedrich-nietzsche"
author_focus = core.knowledge_focus(author_id, sources=["source-navigation"], depth=1)
assert author_focus["focus"]["node_id"] == author_id
assert author_focus["facets"]["predicates"].get("authored_by") == 7
server = make_server(core, port=0)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    base = f"http://127.0.0.1:{server.server_port}"
    health = json.load(urllib.request.urlopen(base + "/health"))
    status = json.load(urllib.request.urlopen(base + "/api/philosophy/status"))
    assert health["ok"] and status["projection_exists"]
finally:
    server.shutdown()
    server.server_close()
    thread.join(timeout=5)
print(json.dumps({"ok": True, "doctor": report, "view_id": view_id}))
"""
        result = subprocess.run(
            [sys.executable, "-c", probe],
            cwd=outside,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if result.returncode:
            raise RuntimeError(f"archive smoke failed: {result.stdout}\n{result.stderr}")
        smoke = json.loads(result.stdout)

        venv_root = temp / "venv"
        create_venv = subprocess.run(
            [sys.executable, "-m", "venv", venv_root.as_posix()],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if create_venv.returncode:
            raise RuntimeError(f"venv creation failed: {create_venv.stdout}\n{create_venv.stderr}")
        venv_python = venv_root / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        install_target = (extracted / "access").as_posix() + ("[mcp]" if with_mcp else "")
        install_command = [venv_python.as_posix(), "-m", "pip", "install"]
        if not with_mcp:
            install_command.append("--no-deps")
        install_command.append(install_target)
        install = subprocess.run(
            install_command,
            cwd=outside,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if install.returncode:
            raise RuntimeError(f"standalone package install failed: {install.stdout}\n{install.stderr}")
        installed_env = env.copy()
        installed_env.pop("PYTHONPATH", None)
        installed_command = [venv_python.as_posix(), "-m", "tos_access"]
        installed_command.extend(["verify", "--profile", "standalone", "--json"] if with_mcp else ["doctor", "--json"])
        installed_doctor = subprocess.run(
            installed_command,
            cwd=outside,
            env=installed_env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if installed_doctor.returncode:
            raise RuntimeError(f"installed doctor failed: {installed_doctor.stdout}\n{installed_doctor.stderr}")
        installed_report = json.loads(installed_doctor.stdout)
        mcp_smoke = None
        if with_mcp:
            mcp_probe = subprocess.run(
                [
                    venv_python.as_posix(),
                    "-c",
                    "from tos_access.core import ToSAccessCore; from tos_access.mcp_server import build_server; core=ToSAccessCore.discover(); assert build_server(tos_root=core.tos_root) is not None; print('ok')",
                ],
                cwd=outside,
                env=installed_env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            if mcp_probe.returncode:
                raise RuntimeError(f"installed MCP smoke failed: {mcp_probe.stdout}\n{mcp_probe.stderr}")
            mcp_smoke = "registered"
        return {
            "schema_version": "tos_standalone_validation_v1",
            "ok": True,
            "mode": "bundle",
            "archive_sha256": sha256_file(bundle),
            "external_manifest": sidecar.as_posix(),
            "source_ref": manifest.get("source_ref"),
            "smoke": smoke,
            "installed_doctor": installed_report,
            "installed_mcp_smoke": mcp_smoke,
            "claim_limit": "Archive integrity and standalone read paths only; no publication or consumer admission.",
        }


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate Tree of Sophia standalone access")
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--manifest", type=Path, help="external bundle manifest; defaults to <bundle>.manifest.json")
    parser.add_argument("--with-mcp", action="store_true", help="install the MCP extra and verify native MCP registration")
    args = parser.parse_args()
    result = (
        validate_bundle(
            args.bundle.resolve(),
            manifest_path=args.manifest.resolve() if args.manifest else None,
            with_mcp=args.with_mcp,
        )
        if args.bundle
        else validate_source(args.repo_root.resolve())
    )
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
