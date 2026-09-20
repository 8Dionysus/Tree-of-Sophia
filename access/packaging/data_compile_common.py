from __future__ import annotations

import ast
import hashlib
import importlib
import json
import sqlite3
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

PARTITIONED_PROJECTION_ROOT_BYTES = 256 * 1024

QUERY_STORE_COMPILER_SEEDS = (
    "access/contracts/runtime-data.v1.json",
    "access/packaging/data_compile_common.py",
    "access/src/tos_access/__init__.py",
    "access/src/tos_access/knowledge_compile.py",
)


def query_store_compiler_paths(repo_root: Path) -> tuple[str, ...]:
    """Bind the selected compiler's local import closure without importing it.

    Adding a reader or codec to the compiler must invalidate compiled reuse;
    changing an unrelated HTTP/UI adapter must not. Both package-relative
    imports and the repository's pure helper modules have explicit roots.
    """
    repo_root = Path(repo_root).resolve()
    pending = list(QUERY_STORE_COMPILER_SEEDS)
    visited = set()

    def local_module(name):
        if not name or any(not part.isidentifier() for part in name.split('.')):
            raise RuntimeError('invalid local compiler import')
        roots = ('access/src',) if name.startswith('tos_access') else ('scripts', 'access/packaging')
        for parent in roots:
            stem = Path(parent).joinpath(*name.split('.'))
            for relative in (stem.with_suffix('.py'), stem / '__init__.py'):
                path = repo_root / relative
                if path.exists() or path.is_symlink():
                    return relative.as_posix()
        if name.startswith('tos_access'):
            raise RuntimeError(f'missing local compiler module: {name}')
        return None

    while pending:
        relative = pending.pop()
        if relative in visited:
            continue
        path = repo_root / relative
        if path.is_symlink() or path.resolve() != path.absolute() or not path.is_file():
            raise RuntimeError(f'missing or linked query-store compiler input: {relative}')
        visited.add(relative)
        if path.suffix != '.py':
            continue
        if relative.startswith('access/src/'):
            module = Path(relative).relative_to('access/src').with_suffix('').parts
            package = module[:-1]
        else:
            package = ()
        tree = ast.parse(path.read_bytes())
        # A package initializer exposes lazy public adapters through __getattr__.
        # Bind its bytes and eager imports; compiling a submodule does not call
        # those optional exports. Compiler modules retain their full closure.
        nodes = tree.body if path.name == '__init__.py' else ast.walk(tree)
        for node in nodes:
            names = []
            if isinstance(node, ast.Import):
                names = [alias.name for alias in node.names]
            elif isinstance(node, ast.ImportFrom):
                if node.level:
                    if not package or node.level > len(package):
                        raise RuntimeError(f'compiler import leaves its package: {relative}')
                    prefix = package[:len(package) - node.level + 1]
                    base = '.'.join((*prefix, *((node.module or '').split('.') if node.module else ())))
                else:
                    base = node.module or ''
                names = [base] if node.module else [base + '.' + alias.name for alias in node.names]
            for name in names:
                dependency = local_module(name)
                if dependency is not None:
                    pending.append(dependency)
    return tuple(sorted(visited))

COMPILED_SUBJECT_KEYS = frozenset(
    {
        "subject_id",
        "output_path",
        "builder_module",
        "required_when",
        "input_subject_ids",
        "consumer_roles",
        "identity_rule",
        "authority",
    }
)

SUPPORTED_COMPILED_SUBJECT_CONDITIONS = frozenset({"partitioned_projection_inputs"})

PARTITIONED_SUBJECT_POLICY_KEYS = frozenset(
    {"format", "inclusion", "discovery_glob_allowed", "source_authority"}
)

def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()

def _contract_relative_path(value: Any, *, field: str) -> str:
    """Validate one exact repository-relative path from a runtime contract."""
    if not isinstance(value, str) or not value:
        raise RuntimeError(f"compiled subject {field} must be a non-empty string")
    path = Path(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise RuntimeError(f"compiled subject {field} must be repository-relative: {value}")
    if any(marker in value for marker in ("*", "?", "[", "]")):
        raise RuntimeError(f"compiled subject {field} cannot contain a glob: {value}")
    return path.as_posix()

def _source_subject_index(allowlist: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    raw_subjects = allowlist.get("subjects", [])
    if not isinstance(raw_subjects, list):
        raise RuntimeError("runtime allowlist subjects must be a list")
    result: dict[str, Mapping[str, Any]] = {}
    for item in raw_subjects:
        if not isinstance(item, Mapping):
            raise RuntimeError("runtime allowlist subjects must be objects")
        subject_id = item.get("subject_id")
        if not isinstance(subject_id, str) or not subject_id:
            raise RuntimeError("runtime allowlist subjects must have non-empty subject_id strings")
        if subject_id in result:
            raise RuntimeError(f"runtime allowlist repeats subject_id: {subject_id}")
        _contract_relative_path(item.get("source_path"), field=f"{subject_id}.source_path")
        result[subject_id] = item
    return result

def _validate_compiled_subjects(allowlist: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Validate the generated-output declarations owned by runtime-data.v1."""
    raw_specs = allowlist.get("compiled_subjects", [])
    if not isinstance(raw_specs, list):
        raise RuntimeError("runtime allowlist compiled_subjects must be a list")
    source_subjects = _source_subject_index(allowlist)
    result: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    seen_outputs: set[str] = set()
    source_ids = set(source_subjects)
    for raw in raw_specs:
        if not isinstance(raw, Mapping):
            raise RuntimeError("runtime allowlist compiled_subjects must contain objects")
        if set(raw) != COMPILED_SUBJECT_KEYS:
            raise RuntimeError(
                "compiled subject field shape drift: "
                f"expected {sorted(COMPILED_SUBJECT_KEYS)}, got {sorted(raw)}"
            )
        spec = dict(raw)
        subject_id = spec["subject_id"]
        if not isinstance(subject_id, str) or not subject_id:
            raise RuntimeError("compiled subject subject_id must be a non-empty string")
        if subject_id in seen_ids or subject_id in source_ids:
            raise RuntimeError(f"compiled subject id is duplicated or collides with a source subject: {subject_id}")
        seen_ids.add(subject_id)

        output_path = _contract_relative_path(spec["output_path"], field=f"{subject_id}.output_path")
        if output_path in seen_outputs or output_path in {
            item["source_path"] for item in source_subjects.values()
        }:
            raise RuntimeError(f"compiled subject output path is duplicated or source-owned: {output_path}")
        seen_outputs.add(output_path)
        spec["output_path"] = output_path

        builder_module = spec["builder_module"]
        if (
            not isinstance(builder_module, str)
            or not builder_module
            or any(not part.isidentifier() for part in builder_module.split("."))
        ):
            raise RuntimeError(f"compiled subject builder_module is invalid: {builder_module!r}")
        condition = spec["required_when"]
        if condition not in SUPPORTED_COMPILED_SUBJECT_CONDITIONS:
            raise RuntimeError(f"unsupported compiled subject required_when: {condition!r}")

        input_ids = spec["input_subject_ids"]
        if (
            not isinstance(input_ids, list)
            or not input_ids
            or any(not isinstance(item, str) or not item for item in input_ids)
            or len(set(input_ids)) != len(input_ids)
        ):
            raise RuntimeError(f"compiled subject input_subject_ids are invalid: {subject_id}")
        missing = sorted(set(input_ids) - source_ids)
        if missing:
            raise RuntimeError(f"compiled subject references unknown input subjects: {missing}")
        spec["input_subject_ids"] = list(input_ids)

        for field in ("consumer_roles",):
            values = spec[field]
            if (
                not isinstance(values, list)
                or not values
                or any(not isinstance(item, str) or not item for item in values)
                or len(set(values)) != len(values)
            ):
                raise RuntimeError(f"compiled subject {field} is invalid: {subject_id}")
            spec[field] = list(values)
        for field in ("identity_rule", "authority"):
            if not isinstance(spec[field], str) or not spec[field].strip():
                raise RuntimeError(f"compiled subject {field} is invalid: {subject_id}")
        result.append(spec)
    return result

def _validate_partitioned_subject_policy(allowlist: Mapping[str, Any]) -> None:
    policy = allowlist.get("partitioned_subject_policy")
    if not isinstance(policy, Mapping) or set(policy) != PARTITIONED_SUBJECT_POLICY_KEYS:
        raise RuntimeError(
            "partitioned projection inputs require an exact partitioned_subject_policy record"
        )
    if policy.get("format") != "tos_partitioned_projection_v1":
        raise RuntimeError("partitioned subject policy format is unsupported")
    if policy.get("inclusion") != "exact-verified-manifest-closure":
        raise RuntimeError("partitioned subject policy must require exact verified manifest closure")
    if policy.get("discovery_glob_allowed") is not False:
        raise RuntimeError("partitioned subject policy must disable discovery globs")
    if not isinstance(policy.get("source_authority"), str) or not policy["source_authority"].strip():
        raise RuntimeError("partitioned subject policy source_authority is invalid")

def _partitioned_subject_ids(repo_root: Path, allowlist: Mapping[str, Any]) -> set[str]:
    source_subjects = _source_subject_index(allowlist)
    result: set[str] = set()
    for subject_id, item in source_subjects.items():
        relative = _contract_relative_path(item["source_path"], field=f"{subject_id}.source_path")
        source = repo_root / relative
        if source.is_file() and _is_partitioned_projection(source):
            result.add(subject_id)
    return result

def _active_compiled_subject(
    repo_root: Path,
    allowlist: Mapping[str, Any],
) -> tuple[dict[str, Any] | None, set[str]]:
    specs = _validate_compiled_subjects(allowlist)
    partitioned_ids = _partitioned_subject_ids(repo_root, allowlist)
    if partitioned_ids:
        _validate_partitioned_subject_policy(allowlist)
    active = [
        spec
        for spec in specs
        if spec["required_when"] == "partitioned_projection_inputs"
        and partitioned_ids.intersection(spec["input_subject_ids"])
    ]
    if partitioned_ids and len(active) != 1:
        raise RuntimeError(
            "partitioned projection inputs require exactly one active compiled subject; "
            f"found {len(active)}"
        )
    return (active[0] if active else None), partitioned_ids

def _relative_path(repo_root: Path, value: str | Path) -> str:
    """Return one normalized repository relative path or reject traversal."""
    candidate = Path(value)
    if candidate.is_absolute():
        try:
            candidate = candidate.resolve().relative_to(repo_root.resolve())
        except ValueError as exc:
            raise RuntimeError(f"runtime subject escapes repository root: {value}") from exc
    normalized = candidate.as_posix()
    if not normalized or normalized == "." or normalized.startswith("../") or "/../" in normalized:
        raise RuntimeError(f"runtime subject is not repository-relative: {value}")
    if any(part in {"", ".", ".."} for part in Path(normalized).parts):
        raise RuntimeError(f"runtime subject has unsafe path components: {value}")
    return normalized

def _is_partitioned_projection(path: Path) -> bool:
    """Identify the versioned manifest without treating arbitrary JSON as one."""
    try:
        if path.is_symlink() or not path.is_file() or path.stat().st_size > PARTITIONED_PROJECTION_ROOT_BYTES:
            return False
        with path.open("rb") as stream:
            raw = stream.read(PARTITIONED_PROJECTION_ROOT_BYTES + 1)
        if len(raw) > PARTITIONED_PROJECTION_ROOT_BYTES:
            return False
        value = json.loads(raw.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return False
    return isinstance(value, Mapping) and value.get("schema_version") == "tos_partitioned_projection_v1"

def _projection_closure(repo_root: Path, root: Path) -> tuple[list[Path], dict[str, Any]]:
    """Resolve an exact partition manifest closure through the owner reader.

    The projection reader is the source owner for manifest structure. Packaging
    only consumes its exact closure; it never expands a wildcard or walks a
    sibling directory. Hash and byte-size verification is performed by the
    reader and again here before copy/archive admission.
    """
    access_src = repo_root / "access" / "src"
    if access_src.as_posix() not in sys.path:
        sys.path.insert(0, access_src.as_posix())
    try:
        from tos_access.projection_store import ProjectionReader
    except ImportError as exc:
        raise RuntimeError("partitioned projection support is unavailable in this access source") from exc
    # No cache is retained here: closure traversal is the integrity pass and
    # every referenced data/index object must be checked against its declared
    # content address on disk.
    reader = ProjectionReader(root, cache_bytes=0)
    manifest = getattr(reader, "manifest", None)
    if not isinstance(manifest, Mapping):
        raise RuntimeError("partitioned projection reader did not expose its validated manifest")
    raw_paths = list(reader.closure_paths())
    reader.require_current()
    if not raw_paths:
        raise RuntimeError(f"partitioned projection closure is empty: {root}")
    root_resolved = root.resolve()
    closure: dict[str, Path] = {_relative_path(repo_root, root_resolved): root_resolved}
    for raw in raw_paths:
        path = Path(raw)
        if not path.is_absolute():
            path = root.parent / path
        path = path.resolve()
        relative = _relative_path(repo_root, path)
        if not path.is_file():
            raise RuntimeError(f"partitioned projection closure member is missing: {relative}")
        closure[relative] = path
    if _relative_path(repo_root, root_resolved) not in closure:
        raise RuntimeError("partitioned projection reader omitted its root manifest from closure")
    # The root is self-authenticated by the reader's exact bytes. Part
    # descriptors are carried by the validated manifest document, while the
    # public metadata method intentionally omits the growing collection tree.
    metadata = dict(manifest)
    return [closure[key] for key in sorted(closure)], metadata

def _file_identities(repo_root: Path, paths: list[Path]) -> dict[str, tuple[str, int]]:
    """Capture exact source bytes after the owner reader verifies a closure."""
    return {
        _relative_path(repo_root, path): (sha256_file(path), path.stat().st_size)
        for path in paths
    }

def _verify_projection_closure(
    repo_root: Path,
    root: Path,
) -> tuple[list[Path], dict[str, Any], dict[str, tuple[str, int]]]:
    closure, metadata = _projection_closure(repo_root, root)
    identities = _file_identities(repo_root, closure)
    return closure, metadata, identities

def _runtime_subject_paths(repo_root: Path, allowlist: Mapping[str, Any]) -> list[Path]:
    """Return the exact allowlisted files, expanding only owner manifests."""
    result: dict[str, Path] = {}
    for item in allowlist.get("subjects", []):
        if not isinstance(item, Mapping) or not isinstance(item.get("source_path"), str):
            raise RuntimeError("runtime allowlist subjects must have exact source_path strings")
        source_path = item["source_path"]
        if any(marker in source_path for marker in ("*", "?", "[", "]")):
            raise RuntimeError(f"runtime allowlist cannot contain glob patterns: {source_path}")
        relative = _relative_path(repo_root, source_path)
        source = repo_root / relative
        if not source.is_file():
            if item.get("required"):
                raise RuntimeError(f"missing required runtime subject: {relative}")
            continue
        if _is_partitioned_projection(source):
            closure, _, _ = _verify_projection_closure(repo_root, source)
            for member in closure:
                result[_relative_path(repo_root, member)] = member
        else:
            result[relative] = source
    return [result[key] for key in sorted(result)]

def _query_store_compiler_fingerprint(repo_root: Path) -> str:
    """Fingerprint compiler code and its transitive local query helpers.

    The generated SQLite file is deliberately absent from this digest. It is
    a checked output whose own hash is recorded in the bundle manifest; source
    identity remains bound to the exact inputs and compiler implementation.
    """
    digest = hashlib.sha256()
    for relative in query_store_compiler_paths(repo_root):
        path = repo_root / relative
        if not path.is_file():
            raise RuntimeError(f"missing query-store compiler input: {relative}")
        digest.update(relative.encode("utf-8") + b"\0")
        digest.update(sha256_file(path).encode("ascii") + b"\n")
    return digest.hexdigest()

def _query_store_input_bindings(
    repo_root: Path,
    allowlist: Mapping[str, Any],
    compiled_subject: Mapping[str, Any],
    *,
    data_root: Path | None = None,
) -> dict[str, str]:
    """Bind the compiler to source roots selected by subject IDs."""
    selected_root = repo_root if data_root is None else Path(data_root)
    source_subjects = _source_subject_index(allowlist)
    bindings: dict[str, str] = {}
    for subject_id in compiled_subject["input_subject_ids"]:
        item = source_subjects[subject_id]
        relative = _contract_relative_path(item["source_path"], field=f"{subject_id}.source_path")
        path = selected_root / relative
        if not path.is_file():
            raise RuntimeError(f"missing compiled query-store input: {subject_id} ({relative})")
        if relative in bindings:
            raise RuntimeError(f"compiled query-store input subjects share a source path: {relative}")
        bindings[relative] = sha256_file(path)
    return bindings

def _query_store_compiler_contract(
    repo_root: Path,
    allowlist: Mapping[str, Any],
    compiled_subject: Mapping[str, Any],
    *,
    data_root: Path | None = None,
) -> tuple[str, str, dict[str, str], str]:
    """Resolve the compiler ABI and all source bindings for one compiled subject."""
    input_bindings = _query_store_input_bindings(
        repo_root, allowlist, compiled_subject, data_root=data_root
    )
    probe = """
import importlib
import json
import sys
from pathlib import Path

request = json.load(sys.stdin)
root = Path(request['root'])
sys.path.insert(0, str(root / 'access' / 'src'))
module = importlib.import_module(request['module'])
print(json.dumps({
    'inputs': getattr(module, 'INPUTS', None),
    'default_relative_path': str(getattr(module, 'DEFAULT_RELATIVE_PATH', '')),
    'schema': getattr(module, 'SCHEMA', None),
    'compiler_version': getattr(module, 'COMPILER_VERSION', None),
}))
"""
    result = subprocess.run(
        [sys.executable, "-I", "-c", probe],
        input=json.dumps(
            {"root": str(repo_root.resolve()), "module": str(compiled_subject["builder_module"])}
        ),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"standalone partitioned bundle requires the query-store compiler: {result.stderr}")
    try:
        compiler_contract = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError("query-store compiler contract probe returned invalid JSON") from exc
    compiler_inputs = compiler_contract.get("inputs")
    if not isinstance(compiler_inputs, Mapping) or not compiler_inputs:
        raise RuntimeError("compiled query-store builder module has no INPUTS mapping")
    if set(compiler_inputs.values()) != set(input_bindings):
        raise RuntimeError(
            "compiled query-store input subjects do not match the builder module INPUTS mapping"
        )
    output_path = _contract_relative_path(
        compiled_subject["output_path"], field=f"{compiled_subject['subject_id']}.output_path"
    )
    compiler_default = compiler_contract.get("default_relative_path")
    if not isinstance(compiler_default, str) or Path(compiler_default).as_posix() != output_path:
        raise RuntimeError(
            "compiled query-store output_path does not match the builder module default: "
            f"{output_path}"
        )
    schema = compiler_contract.get("schema")
    compiler_version = compiler_contract.get("compiler_version")
    if not isinstance(schema, str) or not isinstance(compiler_version, str):
        raise RuntimeError("compiled query-store builder module has no schema/compiler version")
    return schema, compiler_version, input_bindings, _query_store_compiler_fingerprint(repo_root)

def _compile_query_store(
    repo_root: Path,
    output: Path,
    allowlist: Mapping[str, Any],
    compiled_subject: Mapping[str, Any],
    *,
    data_root: Path | None = None,
    _isolated: bool = False,
) -> tuple[Path, dict[str, Any]]:
    """Compile one immutable standalone query snapshot from its contract record."""
    if not _isolated:
        # A caller may already have another checkout's tos_access in sys.modules.
        # A fresh interpreter binds execution to the requested compiler source.
        probe = """
import json, runpy, sys
from pathlib import Path
request = json.load(sys.stdin)
root = Path(request['root'])
sys.path.insert(0, str(root / 'access' / 'src'))
builder = runpy.run_path(request['builder'])
path, metadata = builder['_compile_query_store'](
    root, Path(request['output']), request['allowlist'], request['subject'],
    data_root=Path(request['data_root']) if request.get('data_root') else None,
    _isolated=True)
print(json.dumps({'path': str(path), 'metadata': metadata}))
"""
        result = subprocess.run(
            [sys.executable, "-I", "-c", probe],
            input=json.dumps({"root": str(repo_root.resolve()), "output": str(output.resolve()),
                              "builder": str(Path(__file__).resolve()),
                              "allowlist": dict(allowlist), "subject": dict(compiled_subject),
                              "data_root": str(Path(data_root).resolve()) if data_root is not None else None}),
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        if result.returncode:
            raise RuntimeError(f"isolated query-store compilation failed: {result.stderr}")
        payload = json.loads(result.stdout)
        return Path(payload["path"]), payload["metadata"]
    compiler_digest_before = _query_store_compiler_fingerprint(repo_root)
    access_src = repo_root / "access" / "src"
    if access_src.as_posix() not in sys.path:
        sys.path.insert(0, access_src.as_posix())
    try:
        compiler_module = importlib.import_module(str(compiled_subject["builder_module"]))
    except ImportError as exc:
        raise RuntimeError("standalone partitioned bundle requires the query-store compiler") from exc
    compile_knowledge_store = getattr(compiler_module, "compile_knowledge_store", None)
    if not callable(compile_knowledge_store):
        raise RuntimeError(
            "compiled query-store builder module has no compile_knowledge_store callable: "
            f"{compiled_subject['builder_module']}"
        )
    compiler_inputs = getattr(compiler_module, "INPUTS", None)
    if not isinstance(compiler_inputs, Mapping) or not compiler_inputs:
        raise RuntimeError("compiled query-store builder module has no INPUTS mapping")
    selected_data_root = repo_root if data_root is None else Path(data_root).resolve()
    input_bindings = _query_store_input_bindings(
        repo_root, allowlist, compiled_subject, data_root=selected_data_root
    )
    if set(compiler_inputs.values()) != set(input_bindings):
        raise RuntimeError(
            "compiled query-store input subjects do not match the builder module INPUTS mapping"
        )
    compiler_default = getattr(compiler_module, "DEFAULT_RELATIVE_PATH", None)
    output_path = _contract_relative_path(
        compiled_subject["output_path"], field=f"{compiled_subject['subject_id']}.output_path"
    )
    if compiler_default is None or Path(compiler_default).as_posix() != output_path:
        raise RuntimeError(
            "compiled query-store output_path does not match the builder module default: "
            f"{output_path}"
        )
    schema = getattr(compiler_module, "SCHEMA", None)
    compiler_version = getattr(compiler_module, "COMPILER_VERSION", None)
    if not isinstance(schema, str) or not isinstance(compiler_version, str):
        raise RuntimeError("compiled query-store builder module has no schema/compiler version")

    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    result = compile_knowledge_store(selected_data_root, output, allow_legacy=True)
    candidate = output
    if isinstance(result, Mapping) and isinstance(result.get("output"), str):
        candidate = Path(result["output"]).resolve()
    elif isinstance(result, (str, Path)):
        candidate = Path(result).resolve()
    if candidate != output or not candidate.is_file():
        raise RuntimeError(f"query-store compiler did not publish the requested output: {output}")
    if any(Path(str(candidate) + suffix).exists() for suffix in ("-wal", "-journal")):
        raise RuntimeError("query-store compiler left a mutable SQLite journal beside the snapshot")
    try:
        with sqlite3.connect(candidate.as_uri() + "?mode=ro&immutable=1", uri=True) as db:
            metadata = {
                key: json.loads(value)
                for key, value in db.execute("SELECT key,value FROM metadata")
            }
    except (OSError, sqlite3.Error, ValueError, TypeError) as exc:
        raise RuntimeError(f"compiled query-store output is not readable: {exc}") from exc
    if metadata.get("schema") != schema or metadata.get("compiler_version") != compiler_version:
        raise RuntimeError("compiled query-store output has an unexpected schema/compiler version")
    if metadata.get("snapshot_bindings") != input_bindings or metadata.get("complete") is not True:
        raise RuntimeError("compiled query-store output is not bound to the declared source subjects")
    compiler_digest = _query_store_compiler_fingerprint(repo_root)
    if compiler_digest != compiler_digest_before:
        raise RuntimeError("query-store compiler changed during compilation")
    return candidate, {
        "schema": schema,
        "compiler_version": compiler_version,
        "builder_module": compiled_subject["builder_module"],
        "compiler_sha256": compiler_digest,
        "compiler_paths": list(query_store_compiler_paths(repo_root)),
        "input_bindings": input_bindings,
    }

__all__ = [
    'COMPILED_SUBJECT_KEYS',
    'PARTITIONED_PROJECTION_ROOT_BYTES',
    'PARTITIONED_SUBJECT_POLICY_KEYS',
    'query_store_compiler_paths',
    'SUPPORTED_COMPILED_SUBJECT_CONDITIONS',
    '_active_compiled_subject',
    '_compile_query_store',
    '_contract_relative_path',
    '_file_identities',
    '_is_partitioned_projection',
    '_partitioned_subject_ids',
    '_projection_closure',
    '_query_store_compiler_contract',
    '_query_store_compiler_fingerprint',
    '_query_store_input_bindings',
    '_relative_path',
    '_runtime_subject_paths',
    '_source_subject_index',
    '_validate_compiled_subjects',
    '_validate_partitioned_subject_policy',
    '_verify_projection_closure',
    'sha256_file',
]
