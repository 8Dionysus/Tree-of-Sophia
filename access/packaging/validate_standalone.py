#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
import os
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any

BLOCKED_CODE_MARKERS = (b"/srv/" + b"AbyssOS", b"/srv/" + b"abyss-machine")
EXPECTED_QUERY_OPERATIONS = {
    "tos.status",
    "tos.snapshot",
    "tos.search",
    "tos.knowledge.search",
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
    "tos.page.knowledge-search",
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
    "tos.knowledge.search.capabilities",
    "tos.knowledge.node.inspect",
    "tos.knowledge.relation.inspect",
    "tos.knowledge.temporal.compare",
    "tos.knowledge.focus",
    "tos.lens.open",
    "tos.lens.compile",
    "tos.source.read.contracts",
    "tos.source.read.capabilities",
    "tos.source.handle.discover",
    "tos.source.record.read",
}


_DATA_COMPILE_COMMON_PATH = Path(__file__).resolve().with_name("data_compile_common.py")
if not _DATA_COMPILE_COMMON_PATH.is_file():
    raise ImportError(f"missing data compiler helpers: {_DATA_COMPILE_COMMON_PATH}")
_DATA_COMPILE_COMMON_SPEC = importlib.util.spec_from_file_location(
    "_tos_validate_data_compile_common", _DATA_COMPILE_COMMON_PATH
)
if _DATA_COMPILE_COMMON_SPEC is None or _DATA_COMPILE_COMMON_SPEC.loader is None:
    raise ImportError(f"unable to load data compiler helpers: {_DATA_COMPILE_COMMON_PATH}")
_data_compile_common = importlib.util.module_from_spec(_DATA_COMPILE_COMMON_SPEC)
_DATA_COMPILE_COMMON_SPEC.loader.exec_module(_data_compile_common)
_runtime_subject_paths = _data_compile_common._runtime_subject_paths
_active_compiled_subject = _data_compile_common._active_compiled_subject


def _allowlist_subject_paths(repo_root: Path, allowlist: dict[str, Any]) -> set[str]:
    """Validate exact allowlist closure and return all admitted repo paths."""
    return {
        path.relative_to(repo_root.resolve()).as_posix()
        for path in _runtime_subject_paths(repo_root.resolve(), allowlist)
    }


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


def _validate_knowledge_contracts(repo_root: Path, *, data_root: Path | None = None) -> None:
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
        "temporal-comparison-request.v1.schema.json",
        "temporal-comparison-result.v1.schema.json",
        "exploration-request.v1.schema.json",
        "exploration-result.v1.schema.json",
        "exploration-request.v2.schema.json",
        "exploration-result.v2.schema.json",
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

    if data_root is None:
        return

    access_src = (repo_root / "access/src").as_posix()
    if access_src not in sys.path:
        sys.path.insert(0, access_src)
    from tos_access.core import ToSAccessCore
    from tos_access.knowledge import _content_revision

    core = ToSAccessCore.discover(tos_root=data_root)
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
        "temporal_comparison_request",
        "temporal_comparison_result",
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
    for kind, carriers in (("node", graph["nodes"]), ("relation", graph["relations"])):
        if not carriers:
            continue
        origin = carriers[0]
        request = {
            "schema_version": "tos_exploration_request_v2", "source_revision": graph["source_revision"],
            "origin": {"kind": kind, "id": origin["id"], "content_revision": origin["content_revision"]},
            "max_depth": 0,
        }
        Draft202012Validator(schemas["exploration-request.v2.schema.json"], registry=registry).validate(request)
        page = core.knowledge_explore(request)
        Draft202012Validator(schemas["exploration-result.v2.schema.json"], registry=registry).validate(page)
        if page["status"] != "complete" or page["page"]["primary_node_ids"] or page["page"]["primary_relation_ids"]:
            raise RuntimeError("zero-depth typed exploration must return origin context only")


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
    _validate_knowledge_contracts(repo_root)


def _validate_runtime_data(repo_root: Path, data_root: Path) -> None:
    """Validate one selected data release; never part of API-schema checking."""
    allowlist = json.loads(
        (repo_root / "access/contracts/runtime-data.v1.json").read_text(encoding="utf-8")
    )
    # Resolve and verify all partition members only at the data boundary.
    _allowlist_subject_paths(data_root, allowlist)
    _active_compiled_subject(data_root, allowlist)
    _validate_knowledge_contracts(repo_root, data_root=data_root)


def _scan_source(access_root: Path) -> None:
    suffixes = {".py", ".json", ".toml", ".ts", ".js", ".mjs", ".html"}
    owned_roots = ("src/tos_access", "contracts", "profiles", "packaging", "web/src")
    paths = [access_root / "pyproject.toml", access_root / "web/index.html"]
    for relative in owned_roots:
        paths.extend((access_root / relative).rglob("*"))
    for path in paths:
        if not path.is_file() or path.suffix not in suffixes or "runtime_data" in path.parts:
            continue
        payload = path.read_bytes()
        if any(marker in payload for marker in BLOCKED_CODE_MARKERS):
            raise RuntimeError(f"hard-coded host path: {path.relative_to(access_root)}")


def validate_software(repo_root: Path) -> dict[str, Any]:
    """Check owned code/contracts without selecting or scanning any corpus."""
    _validate_contracts(repo_root)
    _scan_source(repo_root / "access")
    return {
        "schema_version": "tos_standalone_validation_v1",
        "ok": True,
        "mode": "software",
        "data_validated": False,
    }


def validate_source(repo_root: Path) -> dict[str, Any]:
    _validate_contracts(repo_root)
    _validate_runtime_data(repo_root, repo_root)
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



def main() -> None:
    parser = argparse.ArgumentParser(description="Validate Tree of Sophia standalone access")
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument(
        "--software",
        action="store_true",
        help="validate software contracts without loading a data snapshot",
    )
    args = parser.parse_args()
    result = (
        validate_software(args.repo_root.resolve())
        if args.software
        else validate_source(args.repo_root.resolve())
    )
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
