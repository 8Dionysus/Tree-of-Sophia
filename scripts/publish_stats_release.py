#!/usr/bin/env python3
"""Build and publish one local Tree-of-Sophia stats-port integration.

The selected aoa-stats owner validates a private staged copy.  This module
transports the declared stats-port bytes and records the owner's result; it
does not define a measurement, validate semantic meaning, or replace the
owner's validator.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from typing import Any

from corpus_store import CorpusStoreError, canonical, digest_file, hex_digest, read_json, regular, _rename_new, _sync_dir
from downstream_status import DownstreamStatus, DownstreamStatusError


SOURCE_SCHEMA = "tos_stats_port_export_v1"
INTEGRATION_SCHEMA = "tos_stats_integration_v1"
SOURCE_KIND = "stats_port_export"
SOURCE_PATHS = (
    "stats/AGENTS.md",
    "stats/README.md",
    "stats/VALIDATION.md",
    "stats/port.manifest.json",
    "stats/packets/table-i-prepared-dossier-route-ratio.reference.json",
)
PORT_PATH = "stats/port.manifest.json"
PACKET_PATH = "stats/packets/table-i-prepared-dossier-route-ratio.reference.json"
MAX_SOURCE_BYTES = 1024 * 1024
INTEGRATION_KEYS = frozenset(
    {
        "schema_version",
        "source_kind",
        "source_revision",
        "evidence_posture",
        "observation",
        "files",
        "validator_sha256",
        "integration_revision",
    }
)
OBSERVATION_KEYS = frozenset(
    {"observation_id", "observed_at", "source_revision", "live_state"}
)


def _error(message: str) -> CorpusStoreError:
    return CorpusStoreError(message)


def _absolute_directory(raw: Path, *, label: str) -> Path:
    path = Path(raw).expanduser().absolute()
    if path.is_symlink() or path.resolve() != path:
        raise _error(f"{label} must not contain symlinks: {path}")
    if not path.exists():
        raise _error(f"{label} does not exist: {path}")
    if not path.is_dir():
        raise _error(f"{label} must be a directory: {path}")
    return path


def _regular_file(path: Path, *, label: str) -> os.stat_result:
    try:
        if path.is_symlink() or path.resolve() != path.absolute():
            raise _error(f"{label} must not be a symlink: {path}")
        return regular(path)
    except CorpusStoreError:
        raise
    except OSError as exc:
        raise _error(f"{label} is unavailable: {path}") from exc


def _finite_json(path: Path, *, label: str) -> dict[str, Any]:
    _regular_file(path, label=label)

    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                raise _error(f"{label} contains duplicate JSON field {key!r}")
            result[key] = value
        return result

    def reject_constant(value: str) -> None:
        raise ValueError(f"non-finite JSON value {value}")

    try:
        value = json.loads(
            path.read_bytes().decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=reject_constant,
        )
    except CorpusStoreError:
        raise
    except (TypeError, UnicodeError, ValueError) as exc:
        raise _error(f"{label} is not finite UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise _error(f"{label} must contain a JSON object")
    try:
        canonical(value)
    except ValueError as exc:
        raise _error(f"{label} contains a non-finite numeric value") from exc
    return value


def _source_records(source_root: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    total = 0
    for relative in SOURCE_PATHS:
        path = source_root / relative
        metadata = _regular_file(path, label=f"stats source {relative}")
        size = metadata.st_size
        total += size
        if total > MAX_SOURCE_BYTES:
            raise _error("stats port source exceeds the 1 MiB bound")
        records.append(
            {
                "path": relative,
                "sha256": digest_file(path),
                "size_bytes": size,
            }
        )
    return records


def _source_revision(records: list[dict[str, Any]]) -> str:
    body = {"schema_version": SOURCE_SCHEMA, "files": sorted(records, key=lambda entry: entry["path"])}
    return hashlib.sha256(canonical(body)).hexdigest()


def _records_equal(left: list[dict[str, Any]], right: list[dict[str, Any]]) -> bool:
    return left == right


def _ensure_directory(path: Path, *, label: str, create: bool = False) -> None:
    if os.path.lexists(path):
        if path.is_symlink() or path.resolve() != path.absolute():
            raise _error(f"{label} must not be a symlink: {path}")
        if not path.is_dir():
            raise _error(f"{label} must be a directory: {path}")
        return
    if not create:
        raise _error(f"{label} does not exist: {path}")
    try:
        path.mkdir()
    except OSError as exc:
        raise _error(f"cannot create {label}: {path}") from exc


def _copy_stage(source_root: Path, stage: Path, records: list[dict[str, Any]]) -> Path:
    tree_root = stage / "Tree-of-Sophia"
    tree_root.mkdir()
    for entry in records:
        relative = entry["path"]
        source = source_root / relative
        target = tree_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        _regular_file(target, label=f"staged stats source {relative}")
    staged_records = _source_records(tree_root)
    if not _records_equal(staged_records, records):
        raise _error("staged stats source differs from its input bytes")
    return tree_root


def _observed_stage_files(stage: Path) -> set[str]:
    observed: set[str] = set()
    for path in stage.rglob("*"):
        if path.is_symlink():
            raise _error(f"stats stage may not contain symlinks: {path}")
        if not path.is_dir():
            _regular_file(path, label="stats stage file")
            observed.add(path.relative_to(stage).as_posix())
    return observed


def _expected_stage_files() -> set[str]:
    return {f"Tree-of-Sophia/{relative}" for relative in SOURCE_PATHS}


def _read_observation(packet: dict[str, Any]) -> dict[str, Any]:
    provenance = packet.get("provenance")
    posture = packet.get("posture")
    if (
        not isinstance(packet.get("observation_id"), str)
        or not packet["observation_id"]
        or not isinstance(packet.get("observed_at"), str)
        or not packet["observed_at"]
        or not isinstance(provenance, dict)
        or not isinstance(provenance.get("source_revision"), str)
        or not provenance["source_revision"]
        or not isinstance(posture, dict)
        or "live_state" not in posture
    ):
        raise _error("stats packet lacks its explicit observation fields")
    return {
        "observation_id": packet["observation_id"],
        "observed_at": packet["observed_at"],
        "source_revision": provenance["source_revision"],
        "live_state": copy.deepcopy(posture["live_state"]),
    }


def _read_port_posture(port: dict[str, Any]) -> Any:
    if "evidence_posture" not in port:
        raise _error("stats port lacks evidence_posture")
    return copy.deepcopy(port["evidence_posture"])


def _validator_path(stats_root: Path) -> Path:
    validator = stats_root / "scripts" / "validate_stats_protocol.py"
    _regular_file(validator, label="selected stats validator")
    return validator


def _validator_identity(stats_root: Path) -> str:
    """Bind the selected owner package and grammar, not only its CLI wrapper.

    The stats owner imports its package initializer and loads shared schemas
    and owner inventory even for an explicit --port invocation. Package and
    grammar membership are part of this downstream consumer identity only.
    """
    paths = {_validator_path(stats_root)}
    for relative, suffix in (('src/aoa_stats_builder', '.py'), ('stats', '.schema.json')):
        base = stats_root / relative
        if base.is_symlink():
            raise _error('stats consumer input directory may not be a symlink')
        for current, directories, filenames in os.walk(base, followlinks=False):
            for name in directories:
                if (Path(current) / name).is_symlink():
                    raise _error('stats consumer input directory may not be a symlink')
            paths.update(Path(current) / name for name in filenames if name.endswith(suffix))
    inventory = stats_root / 'stats/federation/owner-inventory.json'
    if os.path.lexists(inventory):
        paths.add(inventory)
    bindings, total = {}, 0
    for path in sorted(paths):
        total += _regular_file(path, label='stats consumer dependency').st_size
        if total > 8 * 1024 * 1024 or len(paths) > 1024:
            raise _error('stats consumer identity exceeds its declared package limit')
        bindings[path.relative_to(stats_root).as_posix()] = digest_file(path)
    return hashlib.sha256(canonical({
        'files': bindings, 'python': list(sys.version_info[:3]),
        'packages': {name: importlib.metadata.version(name) for name in ('jsonschema', 'referencing')},
    })).hexdigest()


def _bounded_error(error: BaseException) -> str:
    message = str(error) or error.__class__.__name__
    return message[:4096]


def _run_consumer(validator: Path, tree_root: Path) -> None:
    port = tree_root / PORT_PATH
    try:
        completed = subprocess.run(
            (sys.executable, str(validator), "--port", str(port)),
            cwd=tree_root,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as exc:
        raise _error(f"could not execute selected stats validator: {exc}") from exc
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "owner validator rejected stats port").strip()
        raise _error(f"selected stats validator rejected staged port (exit {completed.returncode}): {detail[:4096]}")


def _integration_body(
    *,
    source_revision: str,
    evidence_posture: Any,
    observation: dict[str, Any],
    records: list[dict[str, Any]],
    validator_sha256: str,
) -> dict[str, Any]:
    return {
        "schema_version": INTEGRATION_SCHEMA,
        "source_kind": SOURCE_KIND,
        "source_revision": source_revision,
        "evidence_posture": copy.deepcopy(evidence_posture),
        "observation": copy.deepcopy(observation),
        "files": sorted(copy.deepcopy(records), key=lambda entry: entry["path"]),
        "validator_sha256": validator_sha256,
    }


def _manifest_for_stage(
    tree_root: Path,
    *,
    source_revision: str,
    evidence_posture: Any,
    observation: dict[str, Any],
    validator_sha256: str,
) -> dict[str, Any]:
    records = _source_records(tree_root)
    body = _integration_body(
        source_revision=source_revision,
        evidence_posture=evidence_posture,
        observation=observation,
        records=records,
        validator_sha256=validator_sha256,
    )
    return {
        **body,
        "integration_revision": hashlib.sha256(canonical(body)).hexdigest(),
    }


def _validate_manifest(root: Path, expected: dict[str, Any] | None = None) -> dict[str, Any]:
    root = Path(root).absolute()
    if root.is_symlink() or root.resolve() != root or not root.is_dir():
        raise _error("stats integration must be an explicit regular directory")
    try:
        manifest = read_json(root / "integration.json")
    except (CorpusStoreError, OSError, KeyError, TypeError, ValueError) as exc:
        raise _error(f"invalid stats integration manifest: {exc}") from exc
    if set(manifest) != INTEGRATION_KEYS or manifest.get("schema_version") != INTEGRATION_SCHEMA:
        raise _error("unsupported stats integration manifest")
    source_revision = hex_digest(manifest.get("source_revision"))
    hex_digest(manifest.get("validator_sha256"))
    integration_revision = hex_digest(manifest.get("integration_revision"))
    body = {key: value for key, value in manifest.items() if key != "integration_revision"}
    if hashlib.sha256(canonical(body)).hexdigest() != integration_revision:
        raise _error("stats integration identity mismatch")
    if root.name != integration_revision:
        raise _error("stats integration directory does not match its identity")
    if manifest.get("source_kind") != SOURCE_KIND:
        raise _error("stats integration has an unsupported source kind")
    evidence_posture = manifest.get("evidence_posture")
    if not isinstance(evidence_posture, dict):
        raise _error("stats integration evidence_posture must be an object")
    observation = manifest.get("observation")
    if not isinstance(observation, dict) or set(observation) != OBSERVATION_KEYS:
        raise _error("stats integration observation has an unexpected field set")
    if not isinstance(observation["observation_id"], str) or not observation["observation_id"]:
        raise _error("stats integration observation_id is invalid")
    if not isinstance(observation["observed_at"], str) or not observation["observed_at"]:
        raise _error("stats integration observed_at is invalid")
    if not isinstance(observation["source_revision"], str) or not observation["source_revision"]:
        raise _error("stats integration observation source_revision is invalid")
    files = manifest.get("files")
    if not isinstance(files, list):
        raise _error("stats integration files must be a list")
    expected_paths = sorted(SOURCE_PATHS)
    if [entry.get("path") for entry in files if isinstance(entry, dict)] != expected_paths:
        raise _error("stats integration source membership differs")
    total = 0
    for entry in files:
        if not isinstance(entry, dict) or set(entry) != {"path", "sha256", "size_bytes"}:
            raise _error("invalid stats integration source binding")
        relative = entry["path"]
        if relative not in SOURCE_PATHS:
            raise _error("stats integration contains an unexpected source path")
        sha256 = hex_digest(entry["sha256"])
        size = entry["size_bytes"]
        if type(size) is not int or size < 0:
            raise _error("invalid stats integration source byte size")
        total += size
        if total > MAX_SOURCE_BYTES:
            raise _error("stats integration exceeds the 1 MiB bound")
        path = root / "Tree-of-Sophia" / relative
        metadata = _regular_file(path, label=f"stats integration source {relative}")
        if metadata.st_size != size or digest_file(path) != sha256:
            raise _error("stats integration source digest mismatch")
    if _source_revision(files) != source_revision:
        raise _error("stats integration source revision does not match its files")
    observed = _observed_stage_files(root)
    expected_files = {"integration.json", *_expected_stage_files()}
    if observed != expected_files:
        raise _error("stats integration contains undeclared files")
    port = _finite_json(root / "Tree-of-Sophia" / PORT_PATH, label="published stats port")
    packet = _finite_json(root / "Tree-of-Sophia" / PACKET_PATH, label="published stats packet")
    if _read_port_posture(port) != evidence_posture or _read_observation(packet) != observation:
        raise _error("stats integration observation or posture differs from its source bytes")
    if expected is not None and manifest != expected:
        raise _error("existing stats integration differs and cannot be reused")
    return manifest


def _sync_tree(stage: Path) -> None:
    for path in stage.rglob("*"):
        if path.is_symlink():
            raise _error(f"stats release stage may not contain symlinks: {path}")
        if path.is_file():
            _regular_file(path, label="stats release stage file")
            with path.open("rb") as stream:
                os.fsync(stream.fileno())
    directories = sorted(
        (path for path in stage.rglob("*") if path.is_dir()),
        key=lambda path: len(path.parts),
        reverse=True,
    )
    for directory in directories:
        _sync_dir(directory)
    _sync_dir(stage)


def _publish_or_reuse(stage: Path, destination: Path, manifest: dict[str, Any]) -> Path:
    _ensure_directory(destination.parent, label="stats releases directory", create=True)
    if os.path.lexists(destination):
        _validate_manifest(destination, expected=manifest)
        return destination
    try:
        _rename_new(stage, destination)
    except (CorpusStoreError, OSError) as exc:
        if os.path.lexists(destination):
            _validate_manifest(destination, expected=manifest)
            return destination
        raise _error(f"could not publish stats integration: {exc}") from exc
    _sync_dir(destination.parent)
    _validate_manifest(destination, expected=manifest)
    return destination


def build_release(source_root: Path, stats_root: Path, release_root: Path) -> dict[str, Any]:
    source_root = _absolute_directory(source_root, label="stats source root")
    stats_root = _absolute_directory(stats_root, label="stats owner root")
    validator = _validator_path(stats_root)
    source_before = _source_records(source_root)
    source_revision = _source_revision(source_before)
    validator_before = _validator_identity(stats_root)
    downstream = DownstreamStatus(Path(release_root), "stats")
    attempt_id = downstream.begin(source_revision)
    try:
        _ensure_directory(downstream.root, label="stats release root")
        with tempfile.TemporaryDirectory(prefix=".stats-stage-", dir=downstream.root) as raw:
            stage = Path(raw) / "release"
            stage.mkdir()
            tree_root = _copy_stage(source_root, stage, source_before)
            port = _finite_json(tree_root / PORT_PATH, label="staged stats port")
            packet = _finite_json(tree_root / PACKET_PATH, label="staged stats packet")
            evidence_posture = _read_port_posture(port)
            observation = _read_observation(packet)
            if _observed_stage_files(stage) != _expected_stage_files():
                raise _error("stats stage contains undeclared files before consumer")
            _run_consumer(validator, tree_root)
            source_after = _source_records(source_root)
            staged_after = _source_records(tree_root)
            validator_after = _validator_identity(stats_root)
            if not _records_equal(source_before, source_after):
                raise _error("stats source bytes changed during owner validation")
            if not _records_equal(source_before, staged_after):
                raise _error("stats staged bytes changed during owner validation")
            if validator_after != validator_before:
                raise _error("selected stats validator changed during owner validation")
            if _observed_stage_files(stage) != _expected_stage_files():
                raise _error("stats stage contains undeclared files after consumer")
            manifest = _manifest_for_stage(
                tree_root,
                source_revision=source_revision,
                evidence_posture=evidence_posture,
                observation=observation,
                validator_sha256=validator_before,
            )
            (stage / "integration.json").write_bytes(canonical(manifest))
            _sync_tree(stage)
            destination = downstream.root / "releases" / manifest["integration_revision"]
            published = _publish_or_reuse(stage, destination, manifest)
        manifest_sha256 = hashlib.sha256((published / "integration.json").read_bytes()).hexdigest()
        downstream.succeed(
            attempt_id,
            artifact_revision=manifest["integration_revision"],
            artifact_manifest_sha256=manifest_sha256,
        )
        return manifest
    except Exception as exc:
        try:
            downstream.fail(attempt_id, _bounded_error(exc))
        except Exception as fail_error:
            raise _error(f"stats release failed: {exc}; status failure recording failed: {fail_error}") from exc
        if isinstance(exc, CorpusStoreError):
            raise
        if isinstance(exc, DownstreamStatusError):
            raise
        raise _error(f"stats release failed: {exc}") from exc


def status_release(release_root: Path, expected_revision: str) -> dict[str, Any]:
    expected_revision = hex_digest(expected_revision)
    downstream = DownstreamStatus(Path(release_root), "stats")
    accepted = downstream.status(expected_revision)
    result: dict[str, Any] = {
        "source_kind": SOURCE_KIND,
        "state": accepted["state"],
        "freshness": accepted["freshness"],
        "latest_attempt": accepted["latest_attempt"],
        "integration_revision": None,
        "observation": None,
    }
    state = accepted["state"]
    if state is None or state["last_success"] is None:
        return result
    success = state["last_success"]
    integration_revision = success["artifact_revision"]
    release = downstream.root / "releases" / integration_revision
    manifest = _validate_manifest(release)
    manifest_bytes = (release / "integration.json").read_bytes()
    if hashlib.sha256(manifest_bytes).hexdigest() != success["artifact_manifest_sha256"]:
        raise _error("last successful stats manifest digest differs")
    if manifest["integration_revision"] != integration_revision or manifest["source_revision"] != success["source_revision"]:
        raise _error("last successful stats manifest identity differs")
    result["integration_revision"] = integration_revision
    result["observation"] = copy.deepcopy(manifest["observation"])
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("build")
    build.add_argument("--source-root", type=Path, required=True)
    build.add_argument("--stats-root", type=Path, required=True)
    build.add_argument("--release-root", type=Path, required=True)
    status = commands.add_parser("status")
    status.add_argument("--release-root", type=Path, required=True)
    status.add_argument("--expected-revision", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "build":
            manifest = build_release(args.source_root, args.stats_root, args.release_root)
            result = {
                "source_kind": SOURCE_KIND,
                "source_revision": manifest["source_revision"],
                "integration_revision": manifest["integration_revision"],
            }
        else:
            result = status_release(args.release_root, args.expected_revision)
    except (CorpusStoreError, DownstreamStatusError, OSError, KeyError, TypeError, ValueError) as exc:
        parser.exit(1, f"stats release rejected: {exc}\n")
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
