#!/usr/bin/env python3
"""Build and validate an explicit Tree of Sophia data snapshot.

This module is deliberately a transport/compiler boundary.  It selects data
only through the versioned access runtime contract, invokes the software
checkout's query compiler, and leaves corpus admission and currentness to the
owner of those concerns.
"""

from __future__ import annotations

import ctypes
import errno
import hashlib
import importlib.util
import json
import os
import shutil
import stat
import sys
import tempfile
from collections.abc import Mapping
from pathlib import Path
from typing import Any


_ACCESS_SRC = Path(__file__).resolve().parents[1] / "src"
if str(_ACCESS_SRC) not in sys.path:
    sys.path.insert(0, str(_ACCESS_SRC))

from tos_access.data_snapshot import (  # noqa: E402
    CHUNK_SIZE,
    DATA_SNAPSHOT_SCHEMA,
    QUERY_STORE_RELATIVE_PATH,  # noqa: F401
    _canonical_bytes,
    _ensure_directory,
    _ensure_regular,
    _hex64,
    _relative_under,
    _safe_relative,
    _sha256_with_size,
    _strict_object,
    _walk_tree,
    verify_data_snapshot,
)


RUNTIME_ALLOWLIST_SCHEMA = "tos_access_runtime_data_allowlist_v1"


def _load_data_compile_common(software_root: Path):
    """Load data compiler helpers from the requested software checkout."""
    path = Path(software_root) / "access/packaging/data_compile_common.py"
    _ensure_regular(path, label="software data compiler helpers")
    name = f"_tos_data_snapshot_compile_common_{os.getpid()}_{id(path)}"
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load software data compiler helpers: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(name, None)
        raise
    return module, name


def _load_allowlist(software_root: Path) -> dict[str, Any]:
    path = software_root / "access/contracts/runtime-data.v1.json"
    _ensure_regular(path, label="software runtime-data contract")
    try:
        value = _strict_object(path.read_bytes(), label="software runtime-data contract")
    except OSError as exc:
        raise RuntimeError(f"software runtime-data contract is not readable: {path}") from exc
    if value.get("schema_version") != RUNTIME_ALLOWLIST_SCHEMA:
        raise RuntimeError("software runtime-data contract has an unsupported schema version")
    return value


def _data_allowlist(allowlist: Mapping[str, Any], data_root: Path) -> dict[str, Any]:
    """Drop software-owned API schemas before asking the owner reader for paths."""
    raw_subjects = allowlist.get("subjects")
    if not isinstance(raw_subjects, list):
        raise RuntimeError("software runtime-data contract subjects must be a list")
    subjects: list[Mapping[str, Any]] = []
    for item in raw_subjects:
        if not isinstance(item, Mapping):
            raise RuntimeError("software runtime-data contract subjects must be objects")
        raw_path = item.get("source_path")
        relative = _relative_under(data_root, raw_path, label="runtime subject path")
        if relative == "access/contracts" or relative.startswith("access/contracts/"):
            continue
        subjects.append(item)
    result = dict(allowlist)
    result["subjects"] = subjects
    return result


def _source_tree_regular(root: Path, path: Path) -> str:
    """Reject symlinked source paths and symlinked ancestors."""
    root = Path(root).resolve()
    # Keep the lexical path for lstat: resolving it first would turn a linked
    # source that points elsewhere inside ``root`` into an apparently regular
    # file and silently lose the ownership boundary.
    candidate = Path(path)
    if candidate.is_absolute():
        try:
            relative = candidate.absolute().relative_to(root.absolute()).as_posix()
        except ValueError as exc:
            raise RuntimeError(f"runtime subject path escapes its selected root: {path}") from exc
    else:
        relative = candidate.as_posix()
    relative = _safe_relative(relative, label="runtime subject path")
    current = root
    for part in relative.split("/")[:-1]:
        current = current / part
        try:
            metadata = current.lstat()
        except OSError as exc:
            raise RuntimeError(f"runtime subject parent is not readable: {current}") from exc
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise RuntimeError(f"runtime subject path has an unsafe parent: {relative}")
    source = root / relative
    _ensure_regular(source, label=f"runtime subject {relative}")
    return relative




def _selected_sources(
    helper: Any,
    software_root: Path,
    data_root: Path,
    allowlist: Mapping[str, Any],
) -> dict[str, Path]:
    """Resolve the exact source closure using the existing owner helper."""
    previous_bytecode_setting = os.environ.get("PYTHONDONTWRITEBYTECODE")
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    try:
        partitioned = False
        for item in allowlist.get("subjects", []):
            if not isinstance(item, Mapping):
                continue
            relative = _relative_under(
                data_root, item.get("source_path"), label="runtime subject path"
            )
            if helper._is_partitioned_projection(data_root / relative):
                partitioned = True
                break
        if partitioned:
            # The owner helper uses ``repo_root/access/src`` to load its
            # projection reader.  A data release has no reason to carry
            # software code, so run that exact helper in a clean interpreter
            # with the selected software package preloaded.  This also
            # prevents a code-shaped distractor below data_root from becoming
            # the projection reader.
            probe = """
import importlib
import json
import runpy
import sys
from pathlib import Path

request = json.load(sys.stdin)
software = Path(request['software'])
sys.path.insert(0, str(software / 'access' / 'src'))
importlib.import_module('tos_access.projection_store')
builder = runpy.run_path(request['builder'])
paths = builder['_runtime_subject_paths'](Path(request['data']), request['allowlist'])
print(json.dumps([str(path) for path in paths]))
"""
            import subprocess

            result = subprocess.run(
                [sys.executable, "-I", "-c", probe],
                input=json.dumps(
                    {
                        "software": str(software_root.resolve()),
                        "data": str(data_root.resolve()),
                        "builder": str(
                            software_root / "access/packaging/data_compile_common.py"
                        ),
                        "allowlist": dict(allowlist),
                    }
                ),
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            if result.returncode:
                raise RuntimeError(
                    f"partitioned runtime subject closure failed: {result.stderr.strip()}"
                )
            try:
                paths = json.loads(result.stdout)
            except json.JSONDecodeError as exc:
                raise RuntimeError(
                    "partitioned runtime subject closure returned invalid JSON"
                ) from exc
            if not isinstance(paths, list) or any(
                not isinstance(path, str) for path in paths
            ):
                raise RuntimeError("partitioned runtime subject closure returned invalid paths")
        else:
            paths = helper._runtime_subject_paths(data_root, allowlist)
    finally:
        if previous_bytecode_setting is None:
            os.environ.pop("PYTHONDONTWRITEBYTECODE", None)
        else:
            os.environ["PYTHONDONTWRITEBYTECODE"] = previous_bytecode_setting
    result: dict[str, Path] = {}
    for path in paths:
        relative = _source_tree_regular(data_root, Path(path))
        if relative in result:
            raise RuntimeError(f"runtime subject closure contains duplicate path: {relative}")
        result[relative] = data_root / relative
    return {relative: result[relative] for relative in sorted(result)}


def _source_records(data_root: Path, sources: Mapping[str, Path]) -> dict[str, tuple[str, int]]:
    records: dict[str, tuple[str, int]] = {}
    for relative in sorted(sources):
        records[relative] = _sha256_with_size(
            sources[relative], label=f"runtime subject {relative}"
        )
    return records


def _copy_source(source: Path, target: Path, expected: tuple[str, int], *, relative: str) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    try:
        with source.open("rb") as source_stream, target.open("xb") as target_stream:
            shutil.copyfileobj(source_stream, target_stream, length=CHUNK_SIZE)
            target_stream.flush()
            os.fsync(target_stream.fileno())
    except FileExistsError as exc:
        raise RuntimeError(f"staged runtime subject already exists: {relative}") from exc
    except OSError as exc:
        raise RuntimeError(f"unable to stage runtime subject {relative}: {exc}") from exc
    actual = _sha256_with_size(target, label=f"staged runtime subject {relative}")
    if actual != expected:
        raise RuntimeError(f"runtime subject changed while copying: {relative}")




def _source_bindings(records: Mapping[str, tuple[str, int]]) -> dict[str, str]:
    return {relative: records[relative][0] for relative in sorted(records)}


def _compiler_manifest(metadata: Mapping[str, Any], *, compiler_sha256: str) -> dict[str, Any]:
    """Keep only the data snapshot compiler ABI fields."""
    required = ("schema", "compiler_version", "compiler_paths", "input_bindings")
    if any(key not in metadata for key in required):
        raise RuntimeError("query-store compiler returned incomplete metadata")
    schema = metadata.get("schema")
    compiler_version = metadata.get("compiler_version")
    paths = metadata.get("compiler_paths")
    bindings = metadata.get("input_bindings")
    if not isinstance(schema, str) or not schema:
        raise RuntimeError("query-store compiler schema is invalid")
    if not isinstance(compiler_version, str) or not compiler_version:
        raise RuntimeError("query-store compiler version is invalid")
    if not isinstance(paths, list) or not paths or any(not isinstance(path, str) for path in paths):
        raise RuntimeError("query-store compiler paths are invalid")
    normalized_paths = [_safe_relative(path, label="query-store compiler path") for path in paths]
    if len(set(normalized_paths)) != len(normalized_paths):
        raise RuntimeError("query-store compiler paths are duplicated")
    if not isinstance(bindings, Mapping) or not bindings:
        raise RuntimeError("query-store compiler input bindings are invalid")
    normalized_bindings: dict[str, str] = {}
    for relative, digest in bindings.items():
        normalized = _safe_relative(relative, label="query-store compiler input path")
        normalized_bindings[normalized] = _hex64(
            digest, label=f"query-store compiler input {normalized} digest"
        )
    return {
        "schema": schema,
        "compiler_version": compiler_version,
        "compiler_sha256": _hex64(compiler_sha256, label="query-store compiler digest"),
        "compiler_paths": normalized_paths,
        "input_bindings": {key: normalized_bindings[key] for key in sorted(normalized_bindings)},
    }


def _validate_compiler_sources(software_root: Path, helper: Any) -> None:
    """Reject links in the software files that define the compiler ABI."""
    discover = getattr(helper, "query_store_compiler_paths", None)
    paths = discover(software_root) if callable(discover) else None
    if not isinstance(paths, (tuple, list)) or not paths:
        raise RuntimeError("query-store compiler dependency set is unavailable")
    for relative in paths:
        relative = _safe_relative(relative, label="query-store compiler path")
        _source_tree_regular(software_root, software_root / relative)


def _temporary_no_bytecode() -> tuple[str | None, str | None]:
    previous = os.environ.get("PYTHONDONTWRITEBYTECODE")
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    return "PYTHONDONTWRITEBYTECODE", previous


def _restore_environment(name: str, previous: str | None) -> None:
    if previous is None:
        os.environ.pop(name, None)
    else:
        os.environ[name] = previous


def _rename_noreplace(source: Path, destination: Path) -> None:
    """Publish one completed stage without ever replacing a destination."""
    try:
        libc = ctypes.CDLL(None, use_errno=True)
        renameat2 = libc.renameat2
    except (AttributeError, OSError) as exc:
        raise RuntimeError("atomic no-replace directory publication is unavailable") from exc
    renameat2.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    renameat2.restype = ctypes.c_int
    result = renameat2(
        -100,
        os.fsencode(Path(source)),
        -100,
        os.fsencode(Path(destination)),
        1,  # renameat2 RENAME_NOREPLACE
    )
    if result != 0:
        error_number = ctypes.get_errno()
        if error_number == errno.EEXIST:
            raise RuntimeError(
                f"refusing to overwrite existing data snapshot: {destination}"
            )
        raise RuntimeError(
            f"atomic no-replace directory publication failed: {os.strerror(error_number)}"
        )


def build_data_snapshot(
    software_root: Path,
    data_root: Path,
    output: Path,
    *,
    corpus_revision: str,
    reuse_snapshot: Path | None = None,
) -> dict[str, Any]:
    """Build one immutable source-selected data directory atomically."""
    software_input = Path(software_root)
    data_input = Path(data_root)
    _ensure_directory(software_input, label="software root")
    _ensure_directory(data_input, label="data root")
    software_root = software_input.resolve()
    data_root = data_input.resolve()
    output = Path(output).absolute()
    _hex64(corpus_revision, label="corpus_revision")
    if os.path.lexists(output):
        raise RuntimeError(f"refusing to overwrite existing data snapshot: {output}")

    reuse_root: Path | None = None
    reuse_manifest: dict[str, Any] | None = None
    if reuse_snapshot is not None:
        reuse_root = Path(reuse_snapshot).absolute()
        if reuse_root != reuse_root.resolve():
            raise RuntimeError('reuse snapshot root may not contain symlinks')
        reuse_manifest = verify_data_snapshot(reuse_root, require_compatible=False)

    allowlist = _load_allowlist(software_root)
    helper, helper_name = _load_data_compile_common(software_root)
    try:
        _validate_compiler_sources(software_root, helper)
        compiler_build_start = helper._query_store_compiler_fingerprint(software_root)
        compiled_specs = helper._validate_compiled_subjects(allowlist)
        if len(compiled_specs) != 1:
            raise RuntimeError(
                "data snapshot requires exactly one declared compiled subject"
            )
        compiled_subject = compiled_specs[0]
        output_relative = _safe_relative(
            compiled_subject["output_path"], label="compiled subject output path"
        )
        if output_relative == "" or output_relative.startswith("access/contracts/"):
            raise RuntimeError("compiled subject output path is not a data path")

        data_allowlist = _data_allowlist(allowlist, data_root)
        sources = _selected_sources(helper, software_root, data_root, data_allowlist)
        records_before = _source_records(data_root, sources)

        output.parent.mkdir(parents=True, exist_ok=True)
        environment_name, previous_environment = _temporary_no_bytecode()
        try:
            with tempfile.TemporaryDirectory(
                prefix="tos-data-snapshot-", dir=output.parent
            ) as raw_temp:
                temporary_root = Path(raw_temp)
                stage = temporary_root / "snapshot"
                stage_data = stage / "data"
                stage_data.mkdir(parents=True)

                for relative in sorted(records_before):
                    _copy_source(
                        sources[relative],
                        stage_data / relative,
                        records_before[relative],
                        relative=relative,
                    )
                    # A second source hash immediately after each copy catches
                    # replacement while the bounded stream was open.
                    after_copy = _sha256_with_size(
                        sources[relative], label=f"runtime subject {relative}"
                    )
                    if after_copy != records_before[relative]:
                        raise RuntimeError(f"runtime subject changed while copying: {relative}")

                compiler_before = helper._query_store_compiler_fingerprint(software_root)
                if compiler_before != compiler_build_start:
                    raise RuntimeError("query-store compiler changed before compilation")
                schema, compiler_version, compiler_bindings, contract_fingerprint = (
                    helper._query_store_compiler_contract(
                        software_root,
                        allowlist,
                        compiled_subject,
                        data_root=stage_data,
                    )
                )
                if contract_fingerprint != compiler_before:
                    raise RuntimeError("query-store compiler changed before compilation")

                query_output = stage_data / output_relative
                expected_compiler = _compiler_manifest(
                    {
                        "schema": schema,
                        "compiler_version": compiler_version,
                        "compiler_paths": list(helper.query_store_compiler_paths(software_root)),
                        "input_bindings": compiler_bindings,
                    },
                    compiler_sha256=compiler_before,
                )
                if reuse_manifest is not None:
                    cached_compiler = reuse_manifest["compiler"]
                else:
                    cached_compiler = None
                if reuse_root is not None and cached_compiler == expected_compiler:
                    cached_member = next(
                        (
                            member
                            for member in reuse_manifest["members"]
                            if member["path"] == f"data/{QUERY_STORE_RELATIVE_PATH}"
                        ),
                        None,
                    )
                    if cached_member is None:
                        raise RuntimeError("verified reuse snapshot has no query-store member")
                    _copy_source(
                        reuse_root / "data" / QUERY_STORE_RELATIVE_PATH,
                        query_output,
                        (cached_member["sha256"], cached_member["size_bytes"]),
                        relative=output_relative,
                    )
                    compiled_output = query_output
                    compiler_metadata = dict(cached_compiler)
                else:
                    compiled_output, compiler_metadata = helper._compile_query_store(
                        software_root,
                        query_output,
                        allowlist,
                        compiled_subject,
                        data_root=stage_data,
                    )
                if Path(compiled_output).resolve() != query_output.resolve():
                    raise RuntimeError("query-store compiler published an unexpected output")
                compiler_after = helper._query_store_compiler_fingerprint(software_root)
                if compiler_after != compiler_before:
                    raise RuntimeError("query-store compiler changed during compilation")
                if compiler_metadata.get("schema") != schema:
                    raise RuntimeError("compiled query-store schema differs from its contract")
                if compiler_metadata.get("compiler_version") != compiler_version:
                    raise RuntimeError("compiled query-store version differs from its contract")
                if compiler_metadata.get("input_bindings") != compiler_bindings:
                    raise RuntimeError("compiled query-store inputs differ from its contract")
                if compiler_metadata.get("compiler_sha256") != compiler_after:
                    raise RuntimeError("compiled query-store compiler digest is stale")
                if compiler_metadata.get("compiler_paths") != list(
                    helper.query_store_compiler_paths(software_root)
                ):
                    raise RuntimeError("compiled query-store compiler dependency set is stale")

                # Re-read the owner closure and all source bytes after compile.
                # This catches changes in non-compiler subjects as well as a
                # changed partition manifest/part set.
                sources_after = _selected_sources(
                    helper, software_root, data_root, data_allowlist
                )
                records_after = _source_records(data_root, sources_after)
                if records_after != records_before:
                    raise RuntimeError("runtime data source closure changed during compilation")
                compiler_final = helper._query_store_compiler_fingerprint(software_root)
                if compiler_final != compiler_build_start:
                    raise RuntimeError("query-store compiler changed before publication")

                compiler = _compiler_manifest(
                    compiler_metadata, compiler_sha256=compiler_after
                )
                # The SQLite compiler binds exactly its five source inputs. All
                # of those inputs must also be present in the full snapshot
                # source binding map, with the same bytes.
                full_bindings = _source_bindings(records_before)
                for relative, digest in compiler["input_bindings"].items():
                    if full_bindings.get(relative) != digest:
                        raise RuntimeError(
                            f"compiled query-store input is absent or stale in data snapshot: {relative}"
                        )

                staged_files, _ = _walk_tree(stage_data, label="staged data")
                expected_files = set(records_before) | {output_relative}
                if set(staged_files) != expected_files:
                    extra = sorted(set(staged_files) - expected_files)
                    missing = sorted(expected_files - set(staged_files))
                    raise RuntimeError(
                        "staged data file set differs from the selected closure: "
                        f"extra={extra}, missing={missing}"
                    )
                if output_relative in records_before:
                    raise RuntimeError("compiled query-store output collides with a source subject")

                members: list[dict[str, Any]] = []
                for relative in sorted(staged_files):
                    digest, size = _sha256_with_size(
                        staged_files[relative], label=f"staged data member {relative}"
                    )
                    members.append(
                        {
                            "path": f"data/{relative}",
                            "size_bytes": size,
                            "sha256": digest,
                        }
                    )
                members.sort(key=lambda member: member["path"])
                body = {
                    "schema_version": DATA_SNAPSHOT_SCHEMA,
                    "corpus_revision": corpus_revision,
                    "input_bindings": full_bindings,
                    "compiler": compiler,
                    "members": members,
                }
                manifest = {
                    **body,
                    "data_revision": hashlib.sha256(_canonical_bytes(body)).hexdigest(),
                }
                manifest_path = stage / "manifest.json"
                with manifest_path.open("xb") as stream:
                    stream.write(_canonical_bytes(manifest))
                    stream.flush()
                    os.fsync(stream.fileno())

                # The manifest is the final stage write.  The destination was
                # checked before all work and is linked by one directory rename.
                if os.path.lexists(output):
                    raise RuntimeError(f"refusing to overwrite existing data snapshot: {output}")
                _rename_noreplace(stage, output)
                try:
                    directory_fd = os.open(output.parent, os.O_RDONLY | os.O_DIRECTORY)
                except OSError:
                    directory_fd = None
                if directory_fd is not None:
                    try:
                        os.fsync(directory_fd)
                    finally:
                        os.close(directory_fd)
                return manifest
        finally:
            _restore_environment(environment_name, previous_environment)
    finally:
        sys.modules.pop(helper_name, None)










__all__ = ["build_data_snapshot", "verify_data_snapshot"]


def main(argv=None) -> int:
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    operations = parser.add_subparsers(dest='operation', required=True)
    build = operations.add_parser('build')
    build.add_argument('--software-root', type=Path, required=True)
    build.add_argument('--data-root', type=Path, required=True)
    build.add_argument('--corpus-revision', required=True)
    build.add_argument('--output', type=Path, required=True)
    build.add_argument('--reuse-snapshot', type=Path)
    verify = operations.add_parser('verify')
    verify.add_argument('snapshot', type=Path)
    args = parser.parse_args(argv)
    if args.operation == 'build':
        result = build_data_snapshot(args.software_root, args.data_root,
                                    args.output, corpus_revision=args.corpus_revision,
                                    reuse_snapshot=args.reuse_snapshot)
    else:
        result = verify_data_snapshot(args.snapshot)
    print(json.dumps(result, sort_keys=True, ensure_ascii=False, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
