#!/usr/bin/env python3
"""Bounded, read-back verified transport for large corpus files.

This module deliberately knows only about byte transfer integrity.  It does
not decide rights, admission, publication, or object immutability.  The
caller supplies a transport with the small ``fetch``/``put`` interface used
by :class:`source_payload_r2.R2RestTransport`.
"""

from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tempfile
from typing import Any, Callable, Iterator, Protocol


SCHEMA_VERSION = "tos_chunked_file_v1"
MAX_CHUNK_BYTES = 64 * 1024 * 1024
READ_BLOCK_BYTES = 1024 * 1024
PART_INDEX_WIDTH = 8

_SHA256 = re.compile(r"^[0-9a-f]{64}$")
_WINDOWS_DRIVE = re.compile(r"^[A-Za-z]:")
_MANIFEST_FIELDS = {
    "schema_version",
    "file_sha256",
    "file_size_bytes",
    "chunk_bytes",
    "chunks",
}
_CHUNK_FIELDS = {"index", "offset", "size_bytes", "sha256", "key"}


class CorpusR2Error(RuntimeError):
    """A bounded transfer or manifest-integrity failure."""


class FileTransport(Protocol):
    """The existing transport surface required by this module."""

    def fetch(self, key: str, destination: Path) -> bool:
        """Fetch ``key`` into a new local file, returning False on absence."""

    def put(
        self,
        key: str,
        source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> Any:
        """Put one local file into ``key``."""


def _fail(message: str) -> None:
    raise CorpusR2Error(message)


def _validated_sha256(value: str, label: str) -> str:
    if not isinstance(value, str) or _SHA256.fullmatch(value) is None:
        _fail(f"{label} must be a lowercase SHA-256 digest")
    return value


def _validated_nonnegative_int(value: object, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        _fail(f"{label} must be a non-negative integer")
    return value


def _validated_chunk_bytes(value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        _fail("chunk_bytes must be an integer")
    if value <= 0 or value > MAX_CHUNK_BYTES:
        _fail(f"chunk_bytes must be between 1 and {MAX_CHUNK_BYTES} bytes")
    return value


def normalize_prefix(prefix: str) -> str:
    """Validate and return a canonical relative object-key prefix.

    Empty prefixes are allowed and produce keys beginning with
    ``files/sha256/``.  Dot segments, empty segments, backslashes, absolute
    paths, and Windows drive prefixes are rejected instead of being silently
    rewritten into a different object namespace.
    """

    if not isinstance(prefix, str):
        _fail("prefix must be a string")
    if prefix == "":
        return ""
    if (
        prefix.startswith("/")
        or "\\" in prefix
        or "\x00" in prefix
        or _WINDOWS_DRIVE.match(prefix) is not None
    ):
        _fail("prefix must be a relative slash-separated object-key prefix")
    pieces = prefix.split("/")
    if any(piece in {"", ".", ".."} for piece in pieces):
        _fail("prefix contains an unsafe path segment")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in prefix):
        _fail("prefix contains a control character")
    return "/".join(pieces)


def _validated_object_key(key: str) -> str:
    if not isinstance(key, str) or not key:
        _fail("object key must be a non-empty string")
    if (
        key.startswith("/")
        or "\\" in key
        or "\x00" in key
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in key)
    ):
        _fail("object key contains an unsafe character")
    pieces = key.split("/")
    if any(piece in {"", ".", ".."} for piece in pieces):
        _fail("object key contains an unsafe path segment")
    return key


def _key_prefix(prefix: str) -> str:
    return f"{prefix}/" if prefix else ""


def _part_key(prefix: str, file_sha256: str, index: int, part_sha256: str) -> str:
    return (
        f"{_key_prefix(prefix)}files/sha256/{file_sha256}/parts/"
        f"{index:0{PART_INDEX_WIDTH}d}-{part_sha256}"
    )


def _manifest_key(prefix: str, file_sha256: str) -> str:
    return f"{_key_prefix(prefix)}files/sha256/{file_sha256}/manifest.json"


def _stat_fingerprint(path: Path) -> tuple[int, ...]:
    try:
        info = path.lstat()
    except OSError as exc:
        raise CorpusR2Error(f"cannot stat local file: {path}") from exc
    # atime is deliberately omitted because reading a source can update it.
    return (
        info.st_dev,
        info.st_ino,
        stat.S_IFMT(info.st_mode),
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _require_regular_source(path: Path) -> tuple[int, ...]:
    try:
        info = path.lstat()
    except OSError as exc:
        raise CorpusR2Error(f"source file is unavailable: {path}") from exc
    if stat.S_ISLNK(info.st_mode):
        _fail("source must not be a symlink")
    if not stat.S_ISREG(info.st_mode):
        _fail("source must be a regular file")
    return _stat_fingerprint(path)


def _open_regular_read(path: Path, expected_stat: tuple[int, ...] | None = None):
    flags = os.O_RDONLY
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags | nofollow)
    except OSError as exc:
        raise CorpusR2Error(f"cannot open local source: {path}") from exc
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode):
            raise CorpusR2Error("source ceased to be a regular file")
        if expected_stat is not None:
            opened_fingerprint = (
                opened.st_dev,
                opened.st_ino,
                stat.S_IFMT(opened.st_mode),
                opened.st_nlink,
                opened.st_uid,
                opened.st_gid,
                opened.st_size,
                opened.st_mtime_ns,
                opened.st_ctime_ns,
            )
            if opened_fingerprint != expected_stat:
                raise CorpusR2Error("source changed before it could be read")
        return os.fdopen(descriptor, "rb")
    except Exception:
        os.close(descriptor)
        raise


def _digest_source(path: Path, expected_stat: tuple[int, ...]) -> tuple[int, str]:
    digest = hashlib.sha256()
    total = 0
    with _open_regular_read(path, expected_stat) as stream:
        while True:
            block = stream.read(READ_BLOCK_BYTES)
            if not block:
                break
            total += len(block)
            digest.update(block)
    return total, digest.hexdigest()


def _ensure_stat_unchanged(path: Path, expected_stat: tuple[int, ...]) -> None:
    current = _require_regular_source(path)
    if current != expected_stat:
        _fail("source changed during transfer")


def _validated_scratch_root(path: Path) -> Path:
    path = Path(path)
    try:
        if path.exists():
            if path.is_symlink() or not path.is_dir():
                _fail("scratch_root must be a real directory")
        else:
            path.mkdir(parents=True, exist_ok=True)
        if path.is_symlink() or not path.is_dir():
            _fail("scratch_root must be a real directory")
    except OSError as exc:
        raise CorpusR2Error(f"cannot prepare scratch_root: {path}") from exc
    return path


@contextmanager
def _scratch_directory(scratch_root: Path) -> Iterator[Path]:
    root = _validated_scratch_root(scratch_root)
    try:
        with tempfile.TemporaryDirectory(prefix=".tos-chunked-", dir=str(root)) as name:
            yield Path(name)
    except OSError as exc:
        raise CorpusR2Error("cannot create or clean the bounded transfer scratch directory") from exc


def _require_new_destination(path: Path) -> None:
    if os.path.lexists(path):
        _fail("transport destination was not fresh")


def _digest_regular_file(path: Path, expected_size: int, expected_sha256: str) -> None:
    try:
        before = path.lstat()
    except OSError as exc:
        raise CorpusR2Error(f"transport did not produce a readable file: {path.name}") from exc
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        _fail("transport readback is not a regular file")
    if before.st_size != expected_size:
        _fail("transport readback size mismatch")
    digest = hashlib.sha256()
    total = 0
    try:
        with path.open("rb") as stream:
            while True:
                block = stream.read(READ_BLOCK_BYTES)
                if not block:
                    break
                total += len(block)
                digest.update(block)
    except OSError as exc:
        raise CorpusR2Error("cannot read transport readback") from exc
    try:
        after = path.lstat()
    except OSError as exc:
        raise CorpusR2Error("transport readback disappeared") from exc
    if (
        stat.S_ISLNK(after.st_mode)
        or not stat.S_ISREG(after.st_mode)
        or after.st_size != before.st_size
        or after.st_mtime_ns != before.st_mtime_ns
        or after.st_ctime_ns != before.st_ctime_ns
    ):
        _fail("transport readback changed while it was being verified")
    if total != expected_size or digest.hexdigest() != expected_sha256:
        _fail("transport readback digest mismatch")


def _call_fetch(transport: FileTransport, key: str, destination: Path) -> bool:
    _validated_object_key(key)
    _require_new_destination(destination)
    try:
        exists = transport.fetch(key, destination)
    except Exception as exc:
        raise CorpusR2Error(f"transport fetch failed for {key}") from exc
    if not isinstance(exists, bool):
        _fail("transport fetch must return a boolean")
    if not exists and os.path.lexists(destination):
        _fail("transport created a destination for an absent object")
    return exists


def _call_put(
    transport: FileTransport,
    key: str,
    source: Path,
    *,
    byte_size: int,
    media_type: str,
) -> None:
    _validated_object_key(key)
    try:
        info = source.lstat()
    except OSError as exc:
        raise CorpusR2Error("local upload part disappeared") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        _fail("local upload part is not a regular file")
    if info.st_size != byte_size:
        _fail("local upload part size changed")
    try:
        transport.put(
            key,
            source,
            byte_size=byte_size,
            media_type=media_type,
            storage_class="Standard",
        )
    except Exception as exc:
        raise CorpusR2Error(f"transport put failed for {key}") from exc


def _ensure_remote_object(
    transport: FileTransport,
    *,
    key: str,
    source: Path,
    byte_size: int,
    sha256: str,
    media_type: str,
    workdir: Path,
    label: str,
    before_put: Callable[[], None] | None = None,
) -> None:
    """Verify an object or put it once and verify a fresh readback."""

    existing_destination = workdir / f"{label}.existing"
    if _call_fetch(transport, key, existing_destination):
        try:
            _digest_regular_file(existing_destination, byte_size, sha256)
        finally:
            existing_destination.unlink(missing_ok=True)
        return

    if before_put is not None:
        before_put()
    _call_put(
        transport,
        key,
        source,
        byte_size=byte_size,
        media_type=media_type,
    )
    readback_destination = workdir / f"{label}.readback"
    if not _call_fetch(transport, key, readback_destination):
        _fail("transport object was absent immediately after put")
    try:
        _digest_regular_file(readback_destination, byte_size, sha256)
    finally:
        readback_destination.unlink(missing_ok=True)


def _write_part_from_source(
    stream: Any,
    destination: Path,
    *,
    index: int,
    remaining: int,
    full_digest: Any,
) -> tuple[int, str]:
    part_digest = hashlib.sha256()
    part_size = 0
    try:
        with destination.open("xb") as part:
            while remaining > 0:
                block = stream.read(min(READ_BLOCK_BYTES, remaining))
                if not block:
                    break
                part.write(block)
                part_digest.update(block)
                full_digest.update(block)
                part_size += len(block)
                remaining -= len(block)
    except OSError as exc:
        raise CorpusR2Error(f"cannot write local part {index}") from exc
    return part_size, part_digest.hexdigest()


def _canonical_json(payload: dict[str, Any]) -> bytes:
    try:
        text = json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as exc:
        raise CorpusR2Error("manifest cannot be encoded as canonical JSON") from exc
    return (text + "\n").encode("utf-8")


def _manifest_location(manifest_key: str) -> tuple[str, str]:
    _validated_object_key(manifest_key)
    suffix = "/files/sha256/"
    if manifest_key.startswith("files/sha256/"):
        prefix = ""
        remainder = manifest_key[len("files/sha256/") :]
    else:
        marker = manifest_key.rfind(suffix)
        if marker <= 0:
            _fail("manifest key does not contain the required files/sha256 path")
        prefix = normalize_prefix(manifest_key[:marker])
        remainder = manifest_key[marker + len(suffix) :]
    match = re.fullmatch(r"([0-9a-f]{64})/manifest\.json", remainder)
    if match is None:
        _fail("manifest key has an invalid file digest suffix")
    file_sha256 = match.group(1)
    expected_key = _manifest_key(prefix, file_sha256)
    if expected_key != manifest_key:
        _fail("manifest key is not canonical")
    return prefix, file_sha256


def _reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail("manifest contains duplicate JSON keys")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> Any:
    _fail(f"manifest contains invalid JSON number {value}")


def _load_manifest(path: Path, expected_sha256: str) -> dict[str, Any]:
    try:
        manifest_size = path.lstat().st_size
    except OSError as exc:
        raise CorpusR2Error("manifest readback is unavailable") from exc
    _digest_regular_file(path, manifest_size, expected_sha256)
    try:
        manifest_bytes = path.read_bytes()
        manifest_text = manifest_bytes.decode("utf-8")
        payload = json.loads(
            manifest_text,
            object_pairs_hook=_reject_duplicate_pairs,
            parse_constant=_reject_json_constant,
        )
    except CorpusR2Error:
        raise
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CorpusR2Error("manifest is not valid UTF-8 JSON") from exc
    if not isinstance(payload, dict):
        _fail("manifest top level must be an object")
    if _canonical_json(payload) != manifest_bytes:
        _fail("manifest JSON is not canonical")
    return payload


def _validate_manifest_payload(
    payload: dict[str, Any],
    *,
    prefix: str,
    path_file_sha256: str,
) -> tuple[str, int, int, list[dict[str, Any]]]:
    if set(payload) != _MANIFEST_FIELDS:
        _fail("manifest fields do not match the chunked-file schema")
    if payload.get("schema_version") != SCHEMA_VERSION:
        _fail("manifest schema_version is unsupported")
    file_sha256 = _validated_sha256(payload.get("file_sha256"), "manifest file_sha256")
    if file_sha256 != path_file_sha256:
        _fail("manifest file_sha256 is not bound to its manifest key")
    file_size = _validated_nonnegative_int(payload.get("file_size_bytes"), "manifest file_size_bytes")
    chunk_bytes = _validated_chunk_bytes(payload.get("chunk_bytes"))
    chunks = payload.get("chunks")
    if not isinstance(chunks, list):
        _fail("manifest chunks must be a list")
    expected_count = (file_size + chunk_bytes - 1) // chunk_bytes
    if len(chunks) != expected_count:
        _fail("manifest chunk count does not match file size")
    offset = 0
    validated: list[dict[str, Any]] = []
    for expected_index, item in enumerate(chunks):
        if not isinstance(item, dict) or set(item) != _CHUNK_FIELDS:
            _fail("manifest chunk fields do not match the chunked-file schema")
        index = item.get("index")
        if isinstance(index, bool) or not isinstance(index, int) or index != expected_index:
            _fail("manifest chunk indexes are not contiguous and ordered")
        item_offset = item.get("offset")
        if (
            isinstance(item_offset, bool)
            or not isinstance(item_offset, int)
            or item_offset != offset
        ):
            _fail("manifest chunk offsets are not contiguous")
        size_bytes = item.get("size_bytes")
        if isinstance(size_bytes, bool) or not isinstance(size_bytes, int):
            _fail("manifest chunk size_bytes must be an integer")
        expected_size = min(chunk_bytes, file_size - offset)
        if size_bytes != expected_size or size_bytes <= 0:
            _fail("manifest chunk size is inconsistent with chunk_bytes")
        part_sha256 = _validated_sha256(item.get("sha256"), "manifest chunk sha256")
        key = item.get("key")
        if not isinstance(key, str) or key != _part_key(prefix, file_sha256, index, part_sha256):
            _fail("manifest chunk key is not bound to its index and digest")
        validated.append(
            {
                "index": index,
                "offset": item_offset,
                "size_bytes": size_bytes,
                "sha256": part_sha256,
                "key": key,
            }
        )
        offset += size_bytes
    if offset != file_size:
        _fail("manifest chunk sizes do not sum to file size")
    return file_sha256, file_size, chunk_bytes, validated


def upload_file(
    transport: FileTransport,
    source: Path,
    *,
    prefix: str,
    expected_sha256: str,
    expected_size: int,
    scratch_root: Path,
    chunk_bytes: int = MAX_CHUNK_BYTES,
) -> dict[str, Any]:
    """Upload a regular file in verified chunks and then its manifest."""

    source = Path(source)
    prefix = normalize_prefix(prefix)
    expected_sha256 = _validated_sha256(expected_sha256, "expected_sha256")
    expected_size = _validated_nonnegative_int(expected_size, "expected_size")
    chunk_bytes = _validated_chunk_bytes(chunk_bytes)
    baseline_stat = _require_regular_source(source)

    actual_size, actual_sha256 = _digest_source(source, baseline_stat)
    if actual_size != expected_size or actual_sha256 != expected_sha256:
        _fail("source does not match expected SHA-256 and size")
    _ensure_stat_unchanged(source, baseline_stat)

    manifest_key = _manifest_key(prefix, expected_sha256)
    chunks: list[dict[str, Any]] = []
    with _scratch_directory(Path(scratch_root)) as workdir:
        full_digest = hashlib.sha256()
        second_pass_size = 0
        index = 0
        with _open_regular_read(source, baseline_stat) as stream:
            while True:
                part_path = workdir / f"part-{index:0{PART_INDEX_WIDTH}d}.bin"
                part_size, part_sha256 = _write_part_from_source(
                    stream,
                    part_path,
                    index=index,
                    remaining=chunk_bytes,
                    full_digest=full_digest,
                )
                if part_size == 0:
                    part_path.unlink(missing_ok=True)
                    break
                offset = second_pass_size
                second_pass_size += part_size
                key = _part_key(prefix, expected_sha256, index, part_sha256)
                _ensure_remote_object(
                    transport,
                    key=key,
                    source=part_path,
                    byte_size=part_size,
                    sha256=part_sha256,
                    media_type="application/octet-stream",
                    workdir=workdir,
                    label=f"part-{index:0{PART_INDEX_WIDTH}d}",
                )
                chunks.append(
                    {
                        "index": index,
                        "offset": offset,
                        "size_bytes": part_size,
                        "sha256": part_sha256,
                        "key": key,
                    }
                )
                part_path.unlink(missing_ok=True)
                index += 1

        second_pass_sha256 = full_digest.hexdigest()
        if second_pass_size != expected_size or second_pass_sha256 != expected_sha256:
            _fail("source changed while chunks were being prepared")
        _ensure_stat_unchanged(source, baseline_stat)

        manifest_payload: dict[str, Any] = {
            "schema_version": SCHEMA_VERSION,
            "file_sha256": expected_sha256,
            "file_size_bytes": expected_size,
            "chunk_bytes": chunk_bytes,
            "chunks": chunks,
        }
        manifest_bytes = _canonical_json(manifest_payload)
        manifest_sha256 = hashlib.sha256(manifest_bytes).hexdigest()
        manifest_path = workdir / "manifest.json"
        try:
            manifest_path.write_bytes(manifest_bytes)
        except OSError as exc:
            raise CorpusR2Error("cannot write local manifest") from exc
        _ensure_remote_object(
            transport,
            key=manifest_key,
            source=manifest_path,
            byte_size=len(manifest_bytes),
            sha256=manifest_sha256,
            media_type="application/json",
            workdir=workdir,
            label="manifest",
            before_put=lambda: _ensure_stat_unchanged(source, baseline_stat),
        )
        _ensure_stat_unchanged(source, baseline_stat)

    return {
        "manifest_key": manifest_key,
        "manifest_sha256": manifest_sha256,
        "file_sha256": expected_sha256,
        "file_size_bytes": expected_size,
        "chunk_count": len(chunks),
        "readback_verified": True,
    }


def restore_file(
    transport: FileTransport,
    *,
    manifest_key: str,
    expected_manifest_sha256: str,
    output: Path,
    scratch_root: Path,
) -> dict[str, Any]:
    """Restore a verified manifest and its parts with an exclusive hardlink."""

    manifest_key = _validated_object_key(manifest_key)
    expected_manifest_sha256 = _validated_sha256(
        expected_manifest_sha256,
        "expected_manifest_sha256",
    )
    output = Path(output)
    if os.path.lexists(output):
        _fail("restore output already exists")
    prefix, path_file_sha256 = _manifest_location(manifest_key)

    with _scratch_directory(Path(scratch_root)) as workdir:
        manifest_path = workdir / "manifest.remote"
        if not _call_fetch(transport, manifest_key, manifest_path):
            _fail("manifest object is absent")
        try:
            payload = _load_manifest(manifest_path, expected_manifest_sha256)
        finally:
            manifest_path.unlink(missing_ok=True)
        file_sha256, file_size, _chunk_bytes, chunks = _validate_manifest_payload(
            payload,
            prefix=prefix,
            path_file_sha256=path_file_sha256,
        )

        assembled = workdir / "assembled.bin"
        digest = hashlib.sha256()
        total = 0
        try:
            with assembled.open("xb") as destination:
                for chunk in chunks:
                    part_path = workdir / f"part-{chunk['index']:0{PART_INDEX_WIDTH}d}.remote"
                    if not _call_fetch(transport, chunk["key"], part_path):
                        _fail("manifest part object is absent")
                    try:
                        _digest_regular_file(part_path, chunk["size_bytes"], chunk["sha256"])
                        with part_path.open("rb") as part:
                            while True:
                                block = part.read(READ_BLOCK_BYTES)
                                if not block:
                                    break
                                destination.write(block)
                                digest.update(block)
                                total += len(block)
                    finally:
                        part_path.unlink(missing_ok=True)
                destination.flush()
                os.fsync(destination.fileno())
        except OSError as exc:
            raise CorpusR2Error("cannot assemble restored file") from exc

        if total != file_size or digest.hexdigest() != file_sha256:
            _fail("restored file digest or size mismatch")
        try:
            output.parent.mkdir(parents=True, exist_ok=True)
        except OSError as exc:
            raise CorpusR2Error("cannot prepare restore output directory") from exc
        if os.path.lexists(output):
            _fail("restore output appeared during transfer")
        try:
            os.link(assembled, output, follow_symlinks=False)
        except FileExistsError as exc:
            raise CorpusR2Error("restore output appeared during transfer") from exc
        except OSError as exc:
            raise CorpusR2Error("cannot publish restored file with an exclusive hardlink") from exc

    return {
        "manifest_key": manifest_key,
        "manifest_sha256": expected_manifest_sha256,
        "file_sha256": file_sha256,
        "file_size_bytes": file_size,
        "chunk_count": len(chunks),
        "readback_verified": True,
        "output": str(output),
    }


__all__ = [
    "CorpusR2Error",
    "FileTransport",
    "MAX_CHUNK_BYTES",
    "SCHEMA_VERSION",
    "normalize_prefix",
    "restore_file",
    "upload_file",
]
