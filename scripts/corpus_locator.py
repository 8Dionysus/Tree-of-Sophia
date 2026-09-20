#!/usr/bin/env python3
"""Resolve and restore exact historical corpus or Git source files.

This module is deliberately a small byte locator.  A corpus descriptor names
one immutable store revision and member, while a Git descriptor names one
exact commit and tree path.  Restoration revalidates that immutable lookup
before streaming bytes and uses an exclusive hard link for the destination.
It does not consult a current pointer, resolve a ref name, or make any source
or repository changes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tempfile
from typing import Any, BinaryIO

try:  # The scripts directory is placed directly on sys.path by the tests.
    from corpus_store import CorpusStore, CorpusStoreError
except ImportError:  # pragma: no cover - useful when imported as a package.
    from scripts.corpus_store import CorpusStore, CorpusStoreError


CHUNK_SIZE = 1024 * 1024
MAX_GIT_BYTES = 256 * 1024 * 1024
_HEX40 = re.compile(r"[0-9a-f]{40}\Z")
_HEX64 = re.compile(r"[0-9a-f]{64}\Z")

_CORPUS_DESCRIPTOR_KEYS = frozenset(
    {"kind", "revision", "path", "sha256", "size_bytes", "mode"}
)
_GIT_DESCRIPTOR_KEYS = frozenset(
    {"kind", "commit", "path", "git_blob_oid", "size_bytes", "mode"}
)


class CorpusLocatorError(CorpusStoreError):
    """Raised when a historical source cannot be resolved or restored."""


def _error(message: str, cause: BaseException | None = None) -> CorpusLocatorError:
    error = CorpusLocatorError(message)
    if cause is not None:
        error.__cause__ = cause
    return error


def _exact_hex(value: Any, pattern: re.Pattern[str], *, label: str) -> str:
    if not isinstance(value, str) or pattern.fullmatch(value) is None:
        raise _error(f"{label} must be exact lowercase hexadecimal")
    return value


def _safe_relative_path(value: Any, *, label: str = "path") -> str:
    """Validate a canonical POSIX path without touching the source tree."""

    if not isinstance(value, str) or not value:
        raise _error(f"{label} must be a non-empty string")
    if "\x00" in value or "\\" in value or value.startswith("/"):
        raise _error(f"{label} must be a safe repository-relative path")
    if any(ord(char) < 0x20 for char in value):
        raise _error(f"{label} contains a control character")
    parts = value.split("/")
    if any(part in {"", ".", "..", ".git"} for part in parts):
        raise _error(f"{label} must be normalized and repository-relative")
    try:
        pure = PurePosixPath(value)
        value.encode("utf-8")
    except (TypeError, UnicodeError, ValueError) as exc:
        raise _error(f"{label} is not valid UTF-8") from exc
    if pure.is_absolute() or pure.as_posix() != value:
        raise _error(f"{label} must be normalized and repository-relative")
    return value


def _real_directory(path: Path, *, label: str) -> Path:
    """Return an absolute directory only when no path component is a link."""

    absolute = Path(path).absolute()
    try:
        resolved = absolute.resolve()
        metadata = absolute.lstat()
    except OSError as exc:
        raise _error(f"{label} is not accessible: {absolute}", exc) from exc
    if absolute != resolved or stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise _error(f"{label} must be a real directory: {absolute}")
    return absolute


def _real_file(path: Path, *, label: str) -> os.stat_result:
    """Check a file without accepting a symlink at the file or its ancestors."""

    absolute = Path(path).absolute()
    try:
        if absolute != absolute.resolve():
            raise _error(f"{label} contains a symlink: {absolute}")
        metadata = absolute.lstat()
    except CorpusLocatorError:
        raise
    except OSError as exc:
        raise _error(f"{label} is not accessible: {absolute}", exc) from exc
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise _error(f"{label} must be a regular file: {absolute}")
    return metadata


def _ensure_output_parent(output: Path) -> Path:
    absolute = Path(output).absolute()
    parent = absolute.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise _error(f"cannot create output parent: {parent}", exc) from exc
    return _real_directory(parent, label="output parent")


def _ensure_output_absent(output: Path) -> Path:
    absolute = Path(output).absolute()
    # lexists also catches a dangling symlink, which Path.exists() does not.
    if os.path.lexists(absolute):
        raise _error(f"refusing to overwrite output: {absolute}")
    if absolute != absolute.resolve():
        raise _error(f"output path contains a symlink: {absolute}")
    return absolute


def _exclusive_link(temporary: Path, output: Path, parent: Path) -> None:
    """Publish a completed temporary file without replacing a concurrent path."""

    _ensure_output_absent(output)
    _real_directory(parent, label="output parent")
    try:
        os.link(temporary, output, follow_symlinks=False)
    except FileExistsError as exc:
        raise _error(f"refusing to overwrite output: {output}", exc) from exc
    except OSError as exc:
        raise _error(f"cannot publish output: {output}", exc) from exc
    try:
        descriptor = os.open(parent, os.O_RDONLY | os.O_DIRECTORY)
    except OSError as exc:
        raise _error(f"cannot open output parent: {parent}", exc) from exc
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _new_temporary(parent: Path) -> tuple[BinaryIO, Path]:
    try:
        stream = tempfile.NamedTemporaryFile(
            mode="w+b", prefix=".corpus-locator-", dir=parent, delete=False
        )
    except OSError as exc:
        raise _error(f"cannot create temporary source in {parent}", exc) from exc
    return stream, Path(stream.name)


def _copy_verified(source: Path, target: BinaryIO, *, expected_sha256: str, expected_size: int) -> str:
    digest = hashlib.sha256()
    total = 0
    try:
        with source.open("rb") as stream:
            for block in iter(lambda: stream.read(CHUNK_SIZE), b""):
                total += len(block)
                if total > expected_size:
                    raise _error("source bytes exceed the declared size")
                digest.update(block)
                target.write(block)
    except CorpusLocatorError:
        raise
    except OSError as exc:
        raise _error(f"cannot read historical source object: {source}", exc) from exc
    actual = digest.hexdigest()
    if total != expected_size or actual != expected_sha256:
        raise _error("historical source bytes do not match the descriptor")
    target.flush()
    os.fsync(target.fileno())
    return actual


def _verify_selected_object(store: CorpusStore, entry: dict[str, Any]) -> None:
    """Verify only the object named by one already validated manifest entry."""

    try:
        # CorpusStore._verify_object checks both the exact byte count and the
        # SHA-256 of this one object.  The manifest/index validation above is
        # deliberately separate so an unrelated object is never read during a
        # single-member lookup.
        store._verify_object(entry)
    except CorpusLocatorError:
        raise
    except Exception as exc:
        raise _error(f"corpus object failed immutable verification: {exc}", exc) from exc


def _manifest_entry(manifest: dict[str, Any], path: str) -> dict[str, Any]:
    files = manifest.get("files")
    if not isinstance(files, list):
        raise _error("corpus revision has no file index")
    matches = [entry for entry in files if isinstance(entry, dict) and entry.get("path") == path]
    if len(matches) != 1:
        raise _error(f"historical corpus path is not present exactly once: {path}")
    return matches[0]


def _load_revision(store: CorpusStore, revision: str) -> dict[str, Any]:
    try:
        # The store validates the canonical snapshot, member index, and
        # identity/dependency coverage here.  Object bytes are checked only
        # after the caller has selected one exact member below.  A current
        # pointer is intentionally never consulted here.
        return store.load(revision, verify_objects=False)
    except CorpusLocatorError:
        raise
    except Exception as exc:  # malformed/missing store state is a locator error
        raise _error(f"cannot load exact corpus revision {revision}", exc) from exc


def _validate_corpus_descriptor(descriptor: Any) -> dict[str, Any]:
    if not isinstance(descriptor, dict) or set(descriptor) != _CORPUS_DESCRIPTOR_KEYS:
        raise _error("corpus descriptor has unexpected fields")
    if descriptor["kind"] != "corpus":
        raise _error("corpus descriptor has the wrong kind")
    _exact_hex(descriptor["revision"], _HEX64, label="revision")
    _safe_relative_path(descriptor["path"], label="corpus path")
    _exact_hex(descriptor["sha256"], _HEX64, label="source SHA-256")
    if type(descriptor["size_bytes"]) is not int or descriptor["size_bytes"] < 0:
        raise _error("corpus descriptor size_bytes is invalid")
    if type(descriptor["mode"]) is not int or descriptor["mode"] not in (0o644, 0o755):
        raise _error("corpus descriptor mode is invalid")
    return descriptor


def resolve_revision(
    store: CorpusStore,
    revision: str,
    *,
    source_id: str | None = None,
    path: str | None = None,
) -> dict[str, Any]:
    """Resolve one member of one exact immutable corpus revision."""

    if (source_id is None) == (path is None):
        raise _error("resolve_revision requires exactly one of source_id or path")
    revision = _exact_hex(revision, _HEX64, label="revision")
    manifest = _load_revision(store, revision)
    if source_id is not None:
        if not isinstance(source_id, str) or not source_id.strip():
            raise _error("source_id must be a non-empty string")
        identities = manifest.get("identities")
        if not isinstance(identities, dict) or source_id not in identities:
            raise _error(f"unknown immutable source_id: {source_id}")
        selected_path = _safe_relative_path(identities[source_id], label="indexed corpus path")
    else:
        selected_path = _safe_relative_path(path, label="corpus path")
    entry = _manifest_entry(manifest, selected_path)
    try:
        if set(entry) != {"path", "sha256", "size_bytes", "mode"}:
            raise _error("corpus revision member has unexpected fields")
        digest = _exact_hex(entry["sha256"], _HEX64, label="source SHA-256")
        size = entry["size_bytes"]
        mode = entry["mode"]
        if type(size) is not int or size < 0 or type(mode) is not int or mode not in (0o644, 0o755):
            raise _error("corpus revision member metadata is invalid")
    except KeyError as exc:
        raise _error("corpus revision member is incomplete", exc) from exc
    _verify_selected_object(
        store,
        {
            "path": selected_path,
            "sha256": digest,
            "size_bytes": size,
            "mode": mode,
        },
    )
    return {
        "kind": "corpus",
        "revision": revision,
        "path": selected_path,
        "sha256": digest,
        "size_bytes": size,
        "mode": mode,
    }


def restore_revision_source(
    store: CorpusStore, descriptor: dict[str, Any], output: Path
) -> dict[str, Any]:
    """Restore one corpus member after exact revision and object revalidation."""

    descriptor = _validate_corpus_descriptor(descriptor)
    output = _ensure_output_absent(output)
    manifest = _load_revision(store, descriptor["revision"])
    entry = _manifest_entry(manifest, descriptor["path"])
    try:
        exact = {
            "path": descriptor["path"],
            "sha256": descriptor["sha256"],
            "size_bytes": descriptor["size_bytes"],
            "mode": descriptor["mode"],
        }
        if entry != exact:
            raise _error("corpus descriptor does not match the immutable revision")
    except (KeyError, TypeError) as exc:
        raise _error("corpus revision member is malformed", exc) from exc

    try:
        source = store._object(descriptor["sha256"])
    except Exception as exc:
        raise _error("corpus store cannot locate the immutable object", exc) from exc
    _real_file(source, label="corpus object")
    # Revalidate the selected object before creating an output parent.  The
    # streaming copy below verifies the same object again while it is read.
    _verify_selected_object(store, entry)

    parent = _ensure_output_parent(output)

    stream, temporary = _new_temporary(parent)
    try:
        with stream:
            actual = _copy_verified(
                source,
                stream,
                expected_sha256=descriptor["sha256"],
                expected_size=descriptor["size_bytes"],
            )
            os.chmod(temporary, descriptor["mode"])
        _exclusive_link(temporary, output, parent)
    finally:
        temporary.unlink(missing_ok=True)
    return {**descriptor, "sha256": actual}


def _real_git_root(git_root: Path) -> Path:
    return _real_directory(Path(git_root), label="Git root")


def _git_run(git_root: Path, args: list[str], *, label: str) -> subprocess.CompletedProcess[bytes]:
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=git_root,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as exc:
        raise _error(f"{label} failed", exc) from exc
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise _error(f"{label} failed: {detail or f'exit {completed.returncode}'}")
    return completed


def _resolve_git_commit(git_root: Path, commit: str) -> None:
    resolved = _git_run(
        git_root,
        ["rev-parse", "--verify", f"{commit}^{{commit}}"],
        label="exact Git commit lookup",
    ).stdout.decode("ascii", errors="replace").strip()
    if resolved != commit:
        raise _error("supplied Git commit is not the exact resolved commit")


def _parse_git_tree_entry(raw: bytes, expected_path: str) -> dict[str, Any]:
    try:
        left, path_raw = raw.split(b"\t", 1)
        mode_raw, type_raw, oid_raw, size_raw = left.split()
    except ValueError as exc:
        raise _error("git ls-tree returned a malformed entry", exc) from exc
    if path_raw != expected_path.encode("utf-8"):
        raise _error("git ls-tree returned a non-literal path match")
    if mode_raw not in {b"100644", b"100755"} or type_raw != b"blob":
        raise _error(f"Git path is not a regular blob: {expected_path}")
    try:
        oid = oid_raw.decode("ascii")
        size = int(size_raw)
    except (UnicodeError, ValueError) as exc:
        raise _error("git ls-tree returned invalid blob metadata", exc) from exc
    if _HEX40.fullmatch(oid) is None or size < 0:
        raise _error("git ls-tree returned invalid blob metadata")
    return {
        "path": expected_path,
        "git_blob_oid": oid,
        "size_bytes": size,
        "mode": int(mode_raw, 8) & 0o777,
    }


def resolve_git(git_root: Path, commit: str, path: str) -> dict[str, Any]:
    """Resolve one regular blob at one exact commit and literal tree path."""

    git_root = _real_git_root(git_root)
    commit = _exact_hex(commit, _HEX40, label="Git commit")
    path = _safe_relative_path(path, label="Git path")
    _resolve_git_commit(git_root, commit)
    completed = _git_run(
        git_root,
        # --literal-pathspecs prevents wildcard, magic, and other pathspec
        # interpretation while the explicit -- ends option parsing.
        ["--literal-pathspecs", "ls-tree", "-l", "-z", commit, "--", path],
        label="exact Git tree lookup",
    )
    records = [record for record in completed.stdout.split(b"\0") if record]
    if len(records) != 1:
        raise _error(f"exact Git path is absent or non-unique: {path}")
    return {"kind": "git", "commit": commit, **_parse_git_tree_entry(records[0], path)}


def _validate_git_descriptor(descriptor: Any) -> dict[str, Any]:
    if not isinstance(descriptor, dict) or set(descriptor) != _GIT_DESCRIPTOR_KEYS:
        raise _error("Git descriptor has unexpected fields")
    if descriptor["kind"] != "git":
        raise _error("Git descriptor has the wrong kind")
    _exact_hex(descriptor["commit"], _HEX40, label="Git commit")
    _safe_relative_path(descriptor["path"], label="Git path")
    _exact_hex(descriptor["git_blob_oid"], _HEX40, label="Git blob OID")
    if type(descriptor["size_bytes"]) is not int or descriptor["size_bytes"] < 0:
        raise _error("Git descriptor size_bytes is invalid")
    if type(descriptor["mode"]) is not int or descriptor["mode"] not in (0o644, 0o755):
        raise _error("Git descriptor mode is invalid")
    return descriptor


def _reject_json_constant(value: str) -> None:
    raise ValueError(f"non-finite JSON number {value}")


def _reject_duplicate_json_keys(items: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in items:
        if key in result:
            raise ValueError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def _read_descriptor(path: Path, *, kind: str) -> dict[str, Any]:
    """Read one strict JSON descriptor file and validate its exact schema."""

    _real_file(Path(path), label="descriptor")
    descriptor_path = Path(path).absolute()
    try:
        raw = descriptor_path.read_bytes()
        descriptor = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except (OSError, UnicodeError, ValueError) as exc:
        raise _error(f"descriptor is not strict JSON: {descriptor_path}", exc) from exc
    if kind == "corpus":
        return _validate_corpus_descriptor(descriptor)
    if kind == "git":
        return _validate_git_descriptor(descriptor)
    raise _error(f"unsupported descriptor kind: {kind}")


def _open_existing_corpus_store(path: Path) -> CorpusStore:
    """Open an already-created corpus store without creating any directories."""

    root = _real_directory(Path(path), label="corpus store")
    for name in ("objects", "revisions", "staging"):
        _real_directory(root / name, label=f"corpus store {name}")

    # CorpusStore.__init__ is admission-oriented and mkdirs its root.  The
    # locator CLI is read-only, so construct the already validated object
    # without invoking that mutating initializer.
    store = CorpusStore.__new__(CorpusStore)
    store.root = root
    return store


def _emit_json(value: Any) -> None:
    try:
        encoded = json.dumps(
            value,
            sort_keys=True,
            ensure_ascii=False,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as exc:
        raise _error("command result is not strict JSON", exc) from exc
    sys.stdout.write(encoded + "\n")


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="corpus_locator",
        description="Resolve or restore exact historical Git and corpus bytes.",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    command = commands.add_parser("git-resolve")
    command.add_argument("--git-root", type=Path, required=True)
    command.add_argument("--commit", required=True)
    command.add_argument("--path", required=True)

    command = commands.add_parser("git-restore")
    command.add_argument("--git-root", type=Path, required=True)
    command.add_argument("--descriptor", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)

    command = commands.add_parser("corpus-resolve")
    command.add_argument("--store", type=Path, required=True)
    command.add_argument("--revision", required=True)
    selector = command.add_mutually_exclusive_group(required=True)
    selector.add_argument("--id", dest="source_id")
    selector.add_argument("--path")

    command = commands.add_parser("corpus-restore")
    command.add_argument("--store", type=Path, required=True)
    command.add_argument("--descriptor", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        if args.command == "git-resolve":
            result = resolve_git(args.git_root, args.commit, args.path)
        elif args.command == "git-restore":
            descriptor = _read_descriptor(args.descriptor, kind="git")
            result = restore_git_source(args.git_root, descriptor, args.output)
        elif args.command == "corpus-resolve":
            store = _open_existing_corpus_store(args.store)
            result = resolve_revision(
                store,
                args.revision,
                source_id=args.source_id,
                path=args.path,
            )
        elif args.command == "corpus-restore":
            store = _open_existing_corpus_store(args.store)
            descriptor = _read_descriptor(args.descriptor, kind="corpus")
            result = restore_revision_source(store, descriptor, args.output)
        else:  # pragma: no cover - argparse enforces the command choices.
            raise _error(f"unsupported command: {args.command}")
        _emit_json(result)
        return 0
    except (CorpusLocatorError, OSError, ValueError) as exc:
        sys.stderr.write(f"error: {exc}\n")
        return 2


def _terminate_process(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is None:
        try:
            process.kill()
        except OSError:
            pass
    try:
        process.wait()
    except OSError:
        pass
    for stream in (process.stdout, process.stderr):
        if stream is not None:
            try:
                stream.close()
            except OSError:
                pass


def _stream_git_blob(
    git_root: Path,
    oid: str,
    target: BinaryIO,
    *,
    expected_size: int,
    max_bytes: int,
) -> str:
    try:
        process = subprocess.Popen(
            ["git", "cat-file", "blob", oid],
            cwd=git_root,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as exc:
        raise _error("git cat-file failed", exc) from exc
    assert process.stdout is not None
    digest = hashlib.sha256()
    git_digest = hashlib.sha1(f"blob {expected_size}\0".encode("ascii"))
    total = 0
    try:
        while True:
            block = process.stdout.read(CHUNK_SIZE)
            if not block:
                break
            total += len(block)
            if total > max_bytes or total > expected_size:
                raise _error("Git blob exceeds the declared or permitted size")
            digest.update(block)
            git_digest.update(block)
            target.write(block)
        return_code = process.wait()
        stderr = process.stderr.read() if process.stderr is not None else b""
        if return_code != 0:
            detail = stderr.decode("utf-8", errors="replace").strip()
            raise _error(f"git cat-file failed: {detail or f'exit {return_code}'}")
    except CorpusLocatorError:
        _terminate_process(process)
        raise
    except OSError as exc:
        _terminate_process(process)
        raise _error("cannot read Git blob", exc) from exc
    finally:
        if process.stdout is not None:
            process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()
    if total != expected_size:
        raise _error("Git blob size differs from the historical tree entry")
    actual_oid = git_digest.hexdigest()
    if actual_oid != oid:
        raise _error("Git blob content does not match its object id")
    target.flush()
    os.fsync(target.fileno())
    return digest.hexdigest()


def restore_git_source(
    git_root: Path,
    descriptor: dict[str, Any],
    output: Path,
    *,
    max_bytes: int = MAX_GIT_BYTES,
) -> dict[str, Any]:
    """Restore an exact historical Git blob with bounded streaming."""

    descriptor = _validate_git_descriptor(descriptor)
    if type(max_bytes) is not int or max_bytes < 0 or max_bytes > MAX_GIT_BYTES:
        raise _error("max_bytes must be between zero and 256 MiB")
    if descriptor["size_bytes"] > max_bytes:
        raise _error("Git blob exceeds max_bytes")
    git_root = _real_git_root(git_root)
    output = _ensure_output_absent(output)
    parent = _ensure_output_parent(output)

    # Re-resolve the exact commit/path and compare every immutable tree fact.
    # This makes a changed/deleted path, a tampered OID, or a changed mode
    # fail closed instead of silently routing to another source.
    resolved = resolve_git(git_root, descriptor["commit"], descriptor["path"])
    if any(resolved[key] != descriptor[key] for key in _GIT_DESCRIPTOR_KEYS - {"kind"}):
        raise _error("Git descriptor does not match the exact historical tree entry")

    stream, temporary = _new_temporary(parent)
    try:
        with stream:
            actual_sha256 = _stream_git_blob(
                git_root,
                descriptor["git_blob_oid"],
                stream,
                expected_size=descriptor["size_bytes"],
                max_bytes=max_bytes,
            )
            os.chmod(temporary, descriptor["mode"])
        _exclusive_link(temporary, output, parent)
    finally:
        temporary.unlink(missing_ok=True)
    return {**descriptor, "sha256": actual_sha256}


__all__ = [
    "CorpusLocatorError",
    "MAX_GIT_BYTES",
    "resolve_revision",
    "restore_revision_source",
    "resolve_git",
    "restore_git_source",
    "main",
]


if __name__ == "__main__":  # pragma: no cover - exercised through subprocess tests.
    raise SystemExit(main())
