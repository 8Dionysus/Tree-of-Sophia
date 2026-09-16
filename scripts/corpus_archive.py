#!/usr/bin/env python3
"""Bounded, deterministic capture and restore of selected Git tree blobs.

This module handles byte transport only.  It does not decide corpus admission,
rights, publication, currentness, or any other source meaning.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tarfile
from typing import Any, BinaryIO, Callable, Iterator


CHUNK_SIZE = 1024 * 1024
_HEX40 = re.compile(r"[0-9a-f]{40}\Z")
_HEX64 = re.compile(r"[0-9a-f]{64}\Z")
_CAPTURE_FILES = frozenset({"source.tar.gz", "members.jsonl", "capture.json"})
_CAPTURE_V1_KEYS = frozenset(
    {
        "schema_version",
        "source_git_commit",
        "source_git_tree",
        "include_prefixes",
        "member_count",
        "source_bytes",
        "members_sha256",
        "archive_sha256",
        "archive_size_bytes",
    }
)
_CAPTURE_V2_KEYS = _CAPTURE_V1_KEYS | frozenset({"exclude_prefixes", "exclude_path_parts"})
# Kept as an internal compatibility alias for callers that imported the old
# constant while v1 was the only manifest format.
_CAPTURE_KEYS = _CAPTURE_V1_KEYS
_MEMBER_KEYS = frozenset({"path", "git_blob_oid", "size_bytes", "sha256", "mode"})


class CorpusArchiveError(ValueError):
    """Raised when a capture, archive, or restore violates this transport contract."""


def _error(message: str) -> CorpusArchiveError:
    return CorpusArchiveError(message)


def _canonical_bytes(value: Any) -> bytes:
    try:
        text = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError, UnicodeError) as exc:
        raise _error(f"cannot render canonical JSON: {exc}") from exc
    return (text + "\n").encode("utf-8")


def _strict_object(raw: bytes, *, label: str) -> dict[str, Any]:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                raise _error(f"{label} contains duplicate JSON member {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs)
    except CorpusArchiveError:
        raise
    except (UnicodeError, ValueError, TypeError) as exc:
        raise _error(f"{label} is not valid UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise _error(f"{label} must contain a JSON object")
    return value


def _is_hex(value: Any, pattern: re.Pattern[str], *, label: str) -> bool:
    return isinstance(value, str) and pattern.fullmatch(value) is not None


def _ensure_regular(path: Path, *, label: str) -> os.stat_result:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is not readable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise _error(f"{label} must be a regular file: {path}")
    return metadata


def _ensure_directory(path: Path, *, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is not readable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise _error(f"{label} must be a real directory: {path}")


def _mkdir_exclusive(path: Path, *, label: str) -> None:
    path = Path(path)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.mkdir()
    except FileExistsError as exc:
        raise _error(f"refusing to overwrite existing {label}: {path}") from exc
    except OSError as exc:
        raise _error(f"cannot create {label}: {path}: {exc}") from exc


def _write_exclusive(path: Path, raw: bytes, *, label: str) -> None:
    try:
        with path.open("xb") as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    except FileExistsError as exc:
        raise _error(f"refusing to overwrite existing {label}: {path}") from exc
    except OSError as exc:
        raise _error(f"cannot write {label}: {path}: {exc}") from exc


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(CHUNK_SIZE), b""):
                digest.update(chunk)
    except OSError as exc:
        raise _error(f"cannot read file for digest: {path}: {exc}") from exc
    return digest.hexdigest()


def _normalize_prefixes(prefixes: list[str], *, label: str = "include prefixes") -> list[str]:
    singular = "include prefix" if label == "include prefixes" else "exclude prefix"
    if not isinstance(prefixes, list):
        raise _error(f"{label} must be a list")
    normalized: set[str] = set()
    for raw in prefixes:
        if not isinstance(raw, str) or not raw:
            raise _error(f"{label} must contain non-empty strings")
        if "\x00" in raw or "\\" in raw:
            raise _error(f"{singular} is not a normalized POSIX path: {raw!r}")
        value = raw.rstrip("/")
        if not value or value == "." or value.startswith("/"):
            raise _error(f"{singular} must be repository-relative: {raw!r}")
        _validate_relative_path(value, label=f"{singular} {raw!r}")
        normalized.add(value)
    return sorted(normalized)


def _normalize_exclude_path_parts(parts: list[str]) -> list[str]:
    if not isinstance(parts, list):
        raise _error("exclude path parts must be a list")
    normalized: set[str] = set()
    for raw in parts:
        if not isinstance(raw, str) or not raw:
            raise _error("exclude path parts must contain non-empty strings")
        if raw in {".", ".."} or "/" in raw or "\\" in raw:
            raise _error(f"exclude path part must be one safe path component: {raw!r}")
        if any(ord(char) < 0x20 or ord(char) == 0x7F for char in raw):
            raise _error(f"exclude path part contains a control character: {raw!r}")
        try:
            raw.encode("utf-8")
        except UnicodeEncodeError as exc:
            raise _error(f"exclude path part is not valid UTF-8: {raw!r}") from exc
        normalized.add(raw)
    return sorted(normalized)


def _validate_relative_path(path: str, *, label: str = "path") -> str:
    if not isinstance(path, str) or not path:
        raise _error(f"{label} must be a non-empty string")
    if "\x00" in path or "\\" in path or path.startswith("/"):
        raise _error(f"{label} must be a safe repository-relative path: {path!r}")
    parts = path.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        raise _error(f"{label} must be a safe repository-relative path: {path!r}")
    if any(ord(char) < 0x20 for char in path):
        raise _error(f"{label} contains a control character: {path!r}")
    try:
        pure = PurePosixPath(path)
    except (TypeError, ValueError) as exc:
        raise _error(f"{label} is not a valid POSIX path: {path!r}") from exc
    if pure.is_absolute() or pure.as_posix() != path:
        raise _error(f"{label} is not normalized: {path!r}")
    if any("\udc80" <= char <= "\udcff" for char in path):
        raise _error(f"{label} is not valid UTF-8: {path!r}")
    return path


def _matches_prefix(path: str, prefixes: list[str]) -> bool:
    return any(path == prefix or path.startswith(prefix + "/") for prefix in prefixes)


def _matches_exclusions(path: str, prefixes: list[str], parts: list[str]) -> tuple[bool, str | None]:
    if _matches_prefix(path, prefixes):
        return True, "prefix"
    if any(component in parts for component in path.split("/")):
        return True, "path part"
    return False, None


def _git_output(repo_root: Path, args: list[str], *, label: str) -> str:
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=repo_root,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as exc:
        raise _error(f"{label} failed: {exc}") from exc
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise _error(f"{label} failed: {detail or f'exit {completed.returncode}'}")
    try:
        return completed.stdout.decode("ascii").strip()
    except UnicodeError as exc:
        raise _error(f"{label} returned non-ASCII output") from exc


def _source_tree(repo_root: Path, commit: str) -> str:
    if not isinstance(commit, str) or _HEX40.fullmatch(commit) is None:
        raise _error("commit must be exactly 40 lowercase hexadecimal characters")
    resolved = _git_output(
        repo_root,
        ["rev-parse", "--verify", f"{commit}^{{commit}}"],
        label="git commit resolution",
    )
    if resolved != commit:
        raise _error("supplied commit is not the exact resolved commit")
    tree = _git_output(
        repo_root,
        ["rev-parse", "--verify", f"{commit}^{{tree}}"],
        label="git tree resolution",
    )
    if _HEX40.fullmatch(tree) is None:
        raise _error("git returned an invalid tree object id")
    return tree


def _iter_nul_records(stream: BinaryIO) -> Iterator[bytes]:
    pending = b""
    while True:
        chunk = stream.read(CHUNK_SIZE)
        if not chunk:
            break
        pending += chunk
        records = pending.split(b"\0")
        pending = records.pop()
        yield from (record for record in records if record)
    if pending:
        raise _error("git ls-tree output was not NUL terminated")


def _selected_tree_entries(
    repo_root: Path,
    commit: str,
    prefixes: list[str],
    exclude_prefixes: list[str] | None = None,
    exclude_path_parts: list[str] | None = None,
) -> list[dict[str, Any]]:
    exclude_prefixes = [] if exclude_prefixes is None else exclude_prefixes
    exclude_path_parts = [] if exclude_path_parts is None else exclude_path_parts
    try:
        process = subprocess.Popen(
            ["git", "ls-tree", "-r", "-l", "-z", commit],
            cwd=repo_root,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as exc:
        raise _error(f"git ls-tree failed: {exc}") from exc
    assert process.stdout is not None
    selected: dict[str, dict[str, Any]] = {}
    try:
        for record in _iter_nul_records(process.stdout):
            try:
                left, path_raw = record.split(b"\t", 1)
                mode_raw, type_raw, oid_raw, size_raw = left.split()
                path = path_raw.decode("utf-8", errors="surrogateescape")
            except (ValueError, UnicodeError) as exc:
                raise _error("git ls-tree returned a malformed tree entry") from exc
            if not _matches_prefix(path, prefixes):
                continue
            excluded, _ = _matches_exclusions(path, exclude_prefixes, exclude_path_parts)
            if excluded:
                continue
            _validate_relative_path(path, label="Git tree path")
            if mode_raw not in {b"100644", b"100755"} or type_raw != b"blob":
                raise _error(f"selected Git path is not a regular blob: {path}")
            try:
                oid = oid_raw.decode("ascii")
            except UnicodeError as exc:
                raise _error(f"selected Git tree entry has an invalid object id: {path}") from exc
            try:
                size = int(size_raw)
            except ValueError as exc:
                raise _error(f"selected Git tree entry has an invalid size: {path}") from exc
            if _HEX40.fullmatch(oid) is None or size < 0:
                raise _error(f"selected Git tree entry is malformed: {path}")
            if path in selected:
                raise _error(f"duplicate selected Git path: {path}")
            selected[path] = {
                "path": path,
                "git_blob_oid": oid,
                "size_bytes": size,
                "mode": int(mode_raw, 8) & 0o777,
            }
    except BaseException:
        if process.poll() is None:
            process.kill()
        process.wait()
        process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()
        raise
    process.stdout.close()
    return_code = process.wait()
    stderr = process.stderr.read() if process.stderr is not None else b""
    if process.stderr is not None:
        process.stderr.close()
    if return_code != 0:
        detail = stderr.decode("utf-8", errors="replace").strip()
        raise _error(f"git ls-tree failed: {detail or f'exit {return_code}'}")
    return [selected[path] for path in sorted(selected)]


class _BlobReader:
    def __init__(self, batch: "_CatFileBatch", size: int):
        self._batch = batch
        self.remaining = size
        self.digest = hashlib.sha256()
        self._finished = False

    def read(self, size: int = -1) -> bytes:
        if size == 0 or self.remaining == 0:
            return b""
        amount = min(self.remaining, CHUNK_SIZE) if size < 0 else min(size, self.remaining)
        assert self._batch.stdout is not None
        data = self._batch.stdout.read(amount)
        if len(data) != amount:
            raise _error("git cat-file returned a truncated blob")
        self.remaining -= len(data)
        self.digest.update(data)
        if self.remaining == 0:
            self.finish()
        return data

    def finish(self) -> None:
        if self._finished:
            return
        if self.remaining:
            raise _error("git blob stream ended before its declared size")
        self._batch.finish_blob()
        self._finished = True


class _CatFileBatch:
    def __init__(self, repo_root: Path):
        self.repo_root = repo_root
        self.process: subprocess.Popen[bytes] | None = None
        self.stdin: BinaryIO | None = None
        self.stdout: BinaryIO | None = None
        self.stderr: BinaryIO | None = None
        self._blob_finished = True

    def __enter__(self) -> "_CatFileBatch":
        try:
            self.process = subprocess.Popen(
                ["git", "cat-file", "--batch"],
                cwd=self.repo_root,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        except OSError as exc:
            raise _error(f"git cat-file failed: {exc}") from exc
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        self.stdin, self.stdout, self.stderr = (
            self.process.stdin,
            self.process.stdout,
            self.process.stderr,
        )
        return self

    def request(self, oid: str, expected_size: int) -> _BlobReader:
        if not self._blob_finished:
            raise _error("git cat-file blob stream was not consumed")
        assert self.stdin is not None and self.stdout is not None
        try:
            self.stdin.write(oid.encode("ascii") + b"\n")
            self.stdin.flush()
            header = self.stdout.readline()
        except OSError as exc:
            raise _error(f"git cat-file could not request blob {oid}: {exc}") from exc
        fields = header.rstrip(b"\n").split()
        if len(fields) != 3:
            raise _error(f"git cat-file did not return blob {oid}")
        actual_oid, object_type, size_raw = fields
        if actual_oid.decode("ascii", errors="replace") != oid or object_type != b"blob":
            raise _error(f"git cat-file returned a non-blob for {oid}")
        try:
            size = int(size_raw)
        except ValueError as exc:
            raise _error(f"git cat-file returned an invalid size for {oid}") from exc
        if size != expected_size:
            raise _error(f"Git blob size differs from ls-tree for {oid}")
        self._blob_finished = False
        return _BlobReader(self, size)

    def finish_blob(self) -> None:
        assert self.stdout is not None
        delimiter = self.stdout.read(1)
        if delimiter != b"\n":
            raise _error("git cat-file blob response was not newline terminated")
        self._blob_finished = True

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        process = self.process
        if process is None:
            return
        try:
            if self.stdin is not None:
                self.stdin.close()
            if exc_type is not None and process.poll() is None:
                process.terminate()
            return_code = process.wait()
            stderr = self.stderr.read() if self.stderr is not None else b""
            if exc_type is None and return_code != 0:
                detail = stderr.decode("utf-8", errors="replace").strip()
                raise _error(f"git cat-file failed: {detail or f'exit {return_code}'}")
        except BrokenPipeError as exc:
            if exc_type is None:
                raise _error("git cat-file pipe closed unexpectedly") from exc
        finally:
            if self.stdout is not None:
                self.stdout.close()
            if self.stderr is not None:
                self.stderr.close()


def _tar_info(entry: dict[str, Any]) -> tarfile.TarInfo:
    info = tarfile.TarInfo(entry["path"])
    info.type = tarfile.REGTYPE
    info.mode = entry["mode"]
    info.size = entry["size_bytes"]
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    return info


def _member_bytes(entry: dict[str, Any]) -> bytes:
    return _canonical_bytes(entry)


def _write_source_archive(repo_root: Path, destination: Path, entries: list[dict[str, Any]]) -> None:
    archive_path = destination / "source.tar.gz"
    members_path = destination / "members.jsonl"
    try:
        archive_context = archive_path.open("xb")
    except FileExistsError as exc:
        raise _error(f"refusing to overwrite capture file: {exc.filename}") from exc
    except OSError as exc:
        raise _error(f"cannot create capture files: {exc}") from exc
    try:
        with archive_context:
            try:
                members_context = members_path.open("xb")
            except FileExistsError as exc:
                raise _error(f"refusing to overwrite capture file: {exc.filename}") from exc
            except OSError as exc:
                raise _error(f"cannot create capture files: {exc}") from exc
            with members_context:
                with gzip.GzipFile(
                    fileobj=archive_context,
                    mode="wb",
                    filename="",
                    mtime=0,
                    compresslevel=9,
                ) as compressed:
                    with tarfile.open(fileobj=compressed, mode="w|", format=tarfile.PAX_FORMAT) as archive:
                        with _CatFileBatch(repo_root) as blobs:
                            for entry in entries:
                                reader = blobs.request(entry["git_blob_oid"], entry["size_bytes"])
                                archive.addfile(_tar_info(entry), reader)
                                reader.finish()
                                entry["sha256"] = reader.digest.hexdigest()
                                members_context.write(_member_bytes(entry))
                members_context.flush()
                os.fsync(members_context.fileno())
            archive_context.flush()
            os.fsync(archive_context.fileno())
    except (OSError, tarfile.TarError) as exc:
        raise _error(f"cannot write source archive: {exc}") from exc


def _capture_manifest(
    repo_root: Path,
    commit: str,
    tree: str,
    prefixes: list[str],
    destination: Path,
    entries: list[dict[str, Any]],
    *,
    exclude_prefixes: list[str] | None = None,
    exclude_path_parts: list[str] | None = None,
) -> dict[str, Any]:
    members_path = destination / "members.jsonl"
    archive_path = destination / "source.tar.gz"
    manifest: dict[str, Any] = {
        "schema_version": "tos_corpus_capture_v1",
        "source_git_commit": commit,
        "source_git_tree": tree,
        "include_prefixes": prefixes,
        "member_count": len(entries),
        "source_bytes": sum(entry["size_bytes"] for entry in entries),
        "members_sha256": _sha256_file(members_path),
        "archive_sha256": _sha256_file(archive_path),
        "archive_size_bytes": archive_path.stat().st_size,
    }
    if exclude_prefixes or exclude_path_parts:
        manifest["schema_version"] = "tos_corpus_capture_v2"
        manifest["exclude_prefixes"] = [] if exclude_prefixes is None else exclude_prefixes
        manifest["exclude_path_parts"] = [] if exclude_path_parts is None else exclude_path_parts
    _write_exclusive(destination / "capture.json", _canonical_bytes(manifest), label="capture manifest")
    return manifest


def capture_git(
    repo_root: Path,
    commit: str,
    prefixes: list[str],
    destination: Path,
    *,
    exclude_prefixes: list[str] | None = None,
    exclude_path_parts: list[str] | None = None,
) -> dict[str, Any]:
    """Capture selected regular blobs from one exact Git commit."""
    repo_root = Path(repo_root)
    destination = Path(destination)
    _ensure_directory(repo_root, label="Git repository root")
    normalized = _normalize_prefixes(prefixes)
    normalized_exclude_prefixes = _normalize_prefixes(
        [] if exclude_prefixes is None else exclude_prefixes,
        label="exclude prefixes",
    )
    normalized_exclude_path_parts = _normalize_exclude_path_parts(
        [] if exclude_path_parts is None else exclude_path_parts
    )
    tree = _source_tree(repo_root, commit)
    entries = _selected_tree_entries(
        repo_root,
        commit,
        normalized,
        normalized_exclude_prefixes,
        normalized_exclude_path_parts,
    )
    _mkdir_exclusive(destination, label="capture output")
    _write_source_archive(repo_root, destination, entries)
    return _capture_manifest(
        repo_root,
        commit,
        tree,
        normalized,
        destination,
        entries,
        exclude_prefixes=normalized_exclude_prefixes or None,
        exclude_path_parts=normalized_exclude_path_parts or None,
    )


def _read_capture_manifest(capture_root: Path) -> dict[str, Any]:
    capture_root = Path(capture_root)
    _ensure_directory(capture_root, label="capture root")
    try:
        names = {item.name for item in capture_root.iterdir()}
    except OSError as exc:
        raise _error(f"cannot list capture root: {capture_root}: {exc}") from exc
    if names != _CAPTURE_FILES:
        raise _error("capture root must contain exactly source.tar.gz, members.jsonl and capture.json")
    manifest_path = capture_root / "capture.json"
    _ensure_regular(manifest_path, label="capture manifest")
    raw = manifest_path.read_bytes()
    if _canonical_bytes(_strict_object(raw, label="capture manifest")) != raw:
        raise _error("capture manifest is not canonical JSON")
    manifest = _strict_object(raw, label="capture manifest")
    schema_version = manifest.get("schema_version")
    if schema_version == "tos_corpus_capture_v1":
        expected_keys = _CAPTURE_V1_KEYS
    elif schema_version == "tos_corpus_capture_v2":
        expected_keys = _CAPTURE_V2_KEYS
    else:
        raise _error("capture manifest has an unsupported schema version")
    if set(manifest) != expected_keys:
        raise _error("capture manifest has an unexpected field set")
    if not _is_hex(manifest.get("source_git_commit"), _HEX40, label="source commit"):
        raise _error("capture manifest source_git_commit is invalid")
    if not _is_hex(manifest.get("source_git_tree"), _HEX40, label="source tree"):
        raise _error("capture manifest source_git_tree is invalid")
    prefixes = manifest.get("include_prefixes")
    if not isinstance(prefixes, list) or prefixes != _normalize_prefixes(prefixes):
        raise _error("capture manifest include_prefixes are not normalized and sorted")
    if schema_version == "tos_corpus_capture_v2":
        exclude_prefixes = manifest.get("exclude_prefixes")
        if not isinstance(exclude_prefixes, list) or exclude_prefixes != _normalize_prefixes(
            exclude_prefixes,
            label="exclude prefixes",
        ):
            raise _error("capture manifest exclude_prefixes are not normalized and sorted")
        exclude_path_parts = manifest.get("exclude_path_parts")
        if not isinstance(exclude_path_parts, list) or exclude_path_parts != _normalize_exclude_path_parts(
            exclude_path_parts
        ):
            raise _error("capture manifest exclude_path_parts are not normalized and sorted")
    for field in ("member_count", "source_bytes", "archive_size_bytes"):
        value = manifest.get(field)
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise _error(f"capture manifest {field} is invalid")
    for field in ("members_sha256", "archive_sha256"):
        if not _is_hex(manifest.get(field), _HEX64, label=field):
            raise _error(f"capture manifest {field} is invalid")
    return manifest


def _validate_member(
    member: dict[str, Any],
    prefixes: list[str],
    *,
    exclude_prefixes: list[str] | None = None,
    exclude_path_parts: list[str] | None = None,
    location: str,
) -> dict[str, Any]:
    if set(member) != _MEMBER_KEYS:
        raise _error(f"{location} has an unexpected field set")
    path = _validate_relative_path(member.get("path"), label=f"{location} path")
    if not _matches_prefix(path, prefixes):
        raise _error(f"{location} path is outside include_prefixes: {path}")
    excluded, reason = _matches_exclusions(
        path,
        [] if exclude_prefixes is None else exclude_prefixes,
        [] if exclude_path_parts is None else exclude_path_parts,
    )
    if excluded:
        raise _error(f"{location} path matches an excluded {reason}: {path}")
    if not _is_hex(member.get("git_blob_oid"), _HEX40, label="git blob oid"):
        raise _error(f"{location} git_blob_oid is invalid")
    size = member.get("size_bytes")
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        raise _error(f"{location} size_bytes is invalid")
    if not _is_hex(member.get("sha256"), _HEX64, label="member digest"):
        raise _error(f"{location} sha256 is invalid")
    mode = member.get("mode")
    if isinstance(mode, bool) or mode not in {0o644, 0o755}:
        raise _error(f"{location} mode is invalid")
    return {
        "path": path,
        "git_blob_oid": member["git_blob_oid"],
        "size_bytes": size,
        "sha256": member["sha256"],
        "mode": mode,
    }


def _iter_members(
    path: Path,
    prefixes: list[str],
    *,
    exclude_prefixes: list[str] | None = None,
    exclude_path_parts: list[str] | None = None,
) -> Iterator[dict[str, Any]]:
    _ensure_regular(path, label="members.jsonl")
    try:
        with path.open("rb") as stream:
            previous: str | None = None
            for line_number, raw in enumerate(stream, start=1):
                if not raw.endswith(b"\n"):
                    raise _error(f"members.jsonl line {line_number} is not newline terminated")
                member = _strict_object(raw[:-1], label=f"members.jsonl line {line_number}")
                if _canonical_bytes(member) != raw:
                    raise _error(f"members.jsonl line {line_number} is not canonical JSON")
                member = _validate_member(
                    member,
                    prefixes,
                    exclude_prefixes=exclude_prefixes,
                    exclude_path_parts=exclude_path_parts,
                    location=f"members.jsonl line {line_number}",
                )
                if previous is not None and member["path"] <= previous:
                    raise _error("members.jsonl paths are not strictly sorted and unique")
                previous = member["path"]
                yield member
    except OSError as exc:
        raise _error(f"cannot read members.jsonl: {exc}") from exc


def _validate_tar(
    capture_root: Path,
    manifest: dict[str, Any],
    *,
    on_open: Callable[[tarfile.TarInfo, dict[str, Any]], BinaryIO | None] | None = None,
) -> None:
    expected_members = _iter_members(
        capture_root / "members.jsonl",
        manifest["include_prefixes"],
        exclude_prefixes=manifest.get("exclude_prefixes"),
        exclude_path_parts=manifest.get("exclude_path_parts"),
    )
    archive_path = capture_root / "source.tar.gz"
    _ensure_regular(archive_path, label="source archive")
    try:
        with archive_path.open("rb") as raw_archive:
            with gzip.GzipFile(fileobj=raw_archive, mode="rb") as compressed:
                with tarfile.open(fileobj=compressed, mode="r|", bufsize=512) as archive:
                    for info in archive:
                        try:
                            expected = next(expected_members)
                        except StopIteration as exc:
                            raise _error("archive contains more members than members.jsonl") from exc
                        if not info.isreg() or info.type not in {tarfile.REGTYPE, tarfile.AREGTYPE}:
                            raise _error(f"archive member is not a regular file: {info.name!r}")
                        _validate_relative_path(info.name, label="archive member path")
                        excluded, reason = _matches_exclusions(
                            info.name,
                            manifest.get("exclude_prefixes", []),
                            manifest.get("exclude_path_parts", []),
                        )
                        if excluded:
                            raise _error(f"archive member path matches an excluded {reason}: {info.name}")
                        if info.name != expected["path"]:
                            raise _error("archive and members.jsonl have different sorted paths")
                        if info.size != expected["size_bytes"] or info.mode != expected["mode"]:
                            raise _error(f"archive metadata differs for {info.name}")
                        source = archive.extractfile(info)
                        if source is None:
                            raise _error(f"archive member cannot be read: {info.name}")
                        target = on_open(info, expected) if on_open is not None else None
                        digest = hashlib.sha256()
                        git_digest = hashlib.sha1()
                        git_digest.update(f"blob {expected['size_bytes']}\0".encode("ascii"))
                        read_bytes = 0
                        try:
                            for chunk in iter(lambda: source.read(CHUNK_SIZE), b""):
                                digest.update(chunk)
                                git_digest.update(chunk)
                                read_bytes += len(chunk)
                                if target is not None:
                                    target.write(chunk)
                        finally:
                            source.close()
                            if target is not None:
                                target.close()
                        if (
                            read_bytes != expected["size_bytes"]
                            or digest.hexdigest() != expected["sha256"]
                            or git_digest.hexdigest() != expected["git_blob_oid"]
                        ):
                            raise _error(f"archive bytes differ for {info.name}")
                    try:
                        next(expected_members)
                    except StopIteration:
                        pass
                    else:
                        raise _error("members.jsonl contains more members than archive")
                # Tar streams stop after the end-of-archive blocks and may not
                # consume the gzip trailer.  Drain the decoder so its CRC and
                # size checks reject an archive whose outer digest was forged.
                # Valid tar writers may leave zero record padding after the
                # two end blocks; any nonzero decoded tail is another member
                # or arbitrary appended data and is rejected.
                for chunk in iter(lambda: compressed.read(CHUNK_SIZE), b""):
                    if any(byte != 0 for byte in chunk):
                        raise _error("source archive has nonzero trailing tar bytes")
                if raw_archive.read(1):
                    raise _error("source archive has trailing bytes")
    except CorpusArchiveError:
        raise
    except (OSError, EOFError, gzip.BadGzipFile, tarfile.TarError) as exc:
        raise _error(f"cannot verify source archive: {exc}") from exc


def verify_capture(capture_root: Path) -> dict[str, Any]:
    """Verify a complete capture without extracting it."""
    capture_root = Path(capture_root)
    manifest = _read_capture_manifest(capture_root)
    members_path = capture_root / "members.jsonl"
    archive_path = capture_root / "source.tar.gz"
    if _sha256_file(members_path) != manifest["members_sha256"]:
        raise _error("members.jsonl digest differs from capture manifest")
    if _sha256_file(archive_path) != manifest["archive_sha256"]:
        raise _error("source archive digest differs from capture manifest")
    if archive_path.stat().st_size != manifest["archive_size_bytes"]:
        raise _error("source archive size differs from capture manifest")
    count = 0
    source_bytes = 0
    for member in _iter_members(
        members_path,
        manifest["include_prefixes"],
        exclude_prefixes=manifest.get("exclude_prefixes"),
        exclude_path_parts=manifest.get("exclude_path_parts"),
    ):
        count += 1
        source_bytes += member["size_bytes"]
    if count != manifest["member_count"] or source_bytes != manifest["source_bytes"]:
        raise _error("members.jsonl totals differ from capture manifest")
    _validate_tar(capture_root, manifest)
    return manifest


def _restore_open(destination: Path) -> Callable[[tarfile.TarInfo, dict[str, Any]], BinaryIO]:
    def open_member(info: tarfile.TarInfo, expected: dict[str, Any]) -> BinaryIO:
        relative = PurePosixPath(expected["path"])
        parent = destination
        for component in relative.parts[:-1]:
            parent = parent / component
            try:
                metadata = parent.lstat()
            except FileNotFoundError:
                try:
                    parent.mkdir()
                except FileExistsError:
                    pass
                metadata = parent.lstat()
            except OSError as exc:
                raise _error(f"cannot create restore directory {parent}: {exc}") from exc
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                raise _error(f"restore path has an unsafe ancestor: {parent}")
        target = parent / relative.parts[-1]
        try:
            stream = target.open("xb")
        except FileExistsError as exc:
            raise _error(f"refusing to overwrite restore member: {target}") from exc
        except OSError as exc:
            raise _error(f"cannot create restore member {target}: {exc}") from exc
        try:
            os.chmod(target, expected["mode"])
        except OSError:
            stream.close()
            raise
        return stream

    return open_member


def restore_capture(capture_root: Path, destination: Path) -> dict[str, Any]:
    """Verify a capture, stream its regular files into a new root, then receipt."""
    capture_root = Path(capture_root)
    destination = Path(destination)
    manifest = verify_capture(capture_root)
    _mkdir_exclusive(destination, label="restore output")
    _validate_tar(capture_root, manifest, on_open=_restore_open(destination))
    manifest_digest = _sha256_file(capture_root / "capture.json")
    receipt = {
        "schema_version": "tos_corpus_restore_receipt_v1",
        "source_git_commit": manifest["source_git_commit"],
        "member_count": manifest["member_count"],
        "source_bytes": manifest["source_bytes"],
        "manifest_sha256": manifest_digest,
    }
    _write_exclusive(destination / "restore-receipt.json", _canonical_bytes(receipt), label="restore receipt")
    return receipt


def _cli_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Capture and restore selected Git tree blobs")
    commands = parser.add_subparsers(dest="command", required=True)

    capture = commands.add_parser("capture")
    capture.add_argument("--repo-root", type=Path, required=True)
    capture.add_argument("--commit", required=True)
    capture.add_argument("--include-prefix", dest="prefixes", action="append", required=True)
    capture.add_argument("--exclude-prefix", dest="exclude_prefixes", action="append")
    capture.add_argument("--exclude-path-part", dest="exclude_path_parts", action="append")
    capture.add_argument("--output", type=Path, required=True)

    verify = commands.add_parser("verify")
    verify.add_argument("--capture", type=Path, required=True)

    restore = commands.add_parser("restore")
    restore.add_argument("--capture", type=Path, required=True)
    restore.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = _cli_parser().parse_args(argv)
    try:
        if arguments.command == "capture":
            result = capture_git(
                arguments.repo_root,
                arguments.commit,
                arguments.prefixes,
                arguments.output,
                exclude_prefixes=arguments.exclude_prefixes,
                exclude_path_parts=arguments.exclude_path_parts,
            )
        elif arguments.command == "verify":
            result = verify_capture(arguments.capture)
        else:
            result = restore_capture(arguments.capture, arguments.output)
    except (CorpusArchiveError, OSError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
