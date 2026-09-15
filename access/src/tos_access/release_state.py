"""Small, immutable local state for selecting a verified access release.

This module owns only the local release pointer and its immutable records.  It
does not discover code or data, calculate their digests, perform a release,
or decide source, semantic, or rights questions.  The caller supplies the
already selected code-owned verifier for each promotion and rollback.
"""

from __future__ import annotations

from contextlib import contextmanager
import copy
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tempfile
from typing import Any, Callable, Iterator

try:  # ``read_selection`` remains importable on platforms without POSIX flock.
    import fcntl
except ImportError:  # pragma: no cover - exercised only on non-POSIX hosts.
    fcntl = None  # type: ignore[assignment]


PAIR_SCHEMA = "tos_access_release_pair_v1"
POINTER_SCHEMA = "tos_access_release_pointer_v1"
REVOCATION_SCHEMA = "tos_access_release_revocation_v1"

PAIR_KEYS = frozenset(
    {
        "schema_version",
        "software_sha256",
        "data_revision",
        "data_manifest_sha256",
        "corpus_revision",
        "query_schema",
        "compiler_version",
    }
)
BINDING_KEYS = frozenset({"data_root", "software_archive"})
POINTER_KEYS = frozenset({"schema_version", "current", "previous"})
REVOCATION_KEYS = frozenset({"schema_version", "kind", "digest", "reason", "owner_ref"})
REVOCATION_KINDS = frozenset({"data", "corpus", "software"})
HEX64 = re.compile(r"[0-9a-f]{64}\Z")


class ReleaseStateError(ValueError):
    """Raised when local release state is malformed, stale, or unavailable."""


# A descriptive alias keeps integrations free to use either name without
# creating a second error hierarchy.
ReleaseStoreError = ReleaseStateError


def _error(message: str) -> ReleaseStateError:
    return ReleaseStateError(message)


def canonical(value: Any) -> bytes:
    """Render one finite, deterministic JSON value with a trailing newline."""

    try:
        rendered = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError, UnicodeError) as exc:
        raise _error(f"cannot render canonical JSON: {exc}") from exc
    try:
        return (rendered + "\n").encode("utf-8")
    except UnicodeError as exc:
        raise _error(f"canonical JSON is not valid UTF-8: {exc}") from exc


def _strict_object(raw: bytes, *, label: str) -> dict[str, Any]:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                raise _error(f"{label} contains duplicate JSON member {key!r}")
            result[key] = value
        return result

    def reject_constant(value: str) -> None:
        raise ValueError(f"non-finite JSON number {value}")

    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=reject_constant,
        )
    except ReleaseStateError:
        raise
    except (UnicodeError, TypeError, ValueError) as exc:
        raise _error(f"{label} is not valid finite UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise _error(f"{label} must contain a JSON object")
    return value


def _ensure_regular(path: Path, *, label: str) -> os.stat_result:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is unavailable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode):
        raise _error(f"{label} must not be a symlink: {path}")
    if not stat.S_ISREG(metadata.st_mode):
        raise _error(f"{label} must be a regular file: {path}")
    return metadata


def _ensure_directory(path: Path, *, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is unavailable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode):
        raise _error(f"{label} must not be a symlink: {path}")
    if not stat.S_ISDIR(metadata.st_mode):
        raise _error(f"{label} must be a directory: {path}")


def _sync_directory(path: Path) -> None:
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
    except OSError as exc:
        raise _error(f"cannot open state directory for fsync: {path}") from exc
    try:
        os.fsync(descriptor)
    except OSError as exc:
        raise _error(f"cannot fsync state directory: {path}") from exc
    finally:
        os.close(descriptor)


def _read_canonical(path: Path, *, label: str) -> dict[str, Any]:
    _ensure_regular(path, label=label)
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise _error(f"cannot read {label}: {path}") from exc
    value = _strict_object(raw, label=label)
    if canonical(value) != raw:
        raise _error(f"{label} is not canonical JSON: {path}")
    return value


def _write_canonical(path: Path, value: dict[str, Any], *, label: str) -> None:
    _publish_hardlink(path, canonical(value), label=label)


def _publish_hardlink(path: Path, raw: bytes, *, label: str) -> None:
    """Publish immutable bytes without replacing an existing pathname."""

    parent = path.parent
    _ensure_directory(parent, label=f"{label} parent directory")
    if os.path.lexists(path):
        _ensure_regular(path, label=label)
        try:
            existing = path.read_bytes()
        except OSError as exc:
            raise _error(f"cannot read existing {label}: {path}") from exc
        if existing != raw:
            raise _error(f"existing {label} differs and cannot be overwritten: {path}")
        return

    temporary: Path | None = None
    linked = False
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            prefix=f".{path.name}.",
            dir=parent,
            delete=False,
        ) as stream:
            temporary = Path(stream.name)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        assert temporary is not None
        try:
            os.link(temporary, path)
            linked = True
        except FileExistsError:
            _ensure_regular(path, label=label)
            try:
                existing = path.read_bytes()
            except OSError as exc:
                raise _error(f"cannot read concurrently published {label}: {path}") from exc
            if existing != raw:
                raise _error(f"existing {label} differs and cannot be overwritten: {path}")
        if linked:
            _sync_directory(parent)
    except OSError as exc:
        raise _error(f"cannot publish {label}: {path}") from exc
    finally:
        if temporary is not None:
            try:
                temporary.unlink(missing_ok=True)
            except OSError as exc:
                if linked:
                    raise _error(f"cannot clean temporary {label}: {temporary}") from exc


def _atomic_pointer(path: Path, value: dict[str, Any], *, root: Path) -> None:
    """Atomically replace the regular current pointer and fsync its directory."""

    _ensure_directory(root, label="release store root")
    if os.path.lexists(path):
        _ensure_regular(path, label="current pointer")
    raw = canonical(value)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            prefix=".current.",
            dir=root,
            delete=False,
        ) as stream:
            temporary = Path(stream.name)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        assert temporary is not None
        os.replace(temporary, path)
        temporary = None
        _sync_directory(root)
    except OSError as exc:
        raise _error(f"cannot atomically publish current pointer: {path}") from exc
    finally:
        if temporary is not None:
            try:
                temporary.unlink(missing_ok=True)
            except OSError:
                pass


def _validate_digest(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or HEX64.fullmatch(value) is None:
        raise _error(f"{label} must be a lowercase SHA-256 digest")
    return value


def _validate_text(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise _error(f"{label} must be a non-empty string")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise _error(f"{label} contains a control character")
    try:
        value.encode("utf-8")
    except UnicodeError as exc:
        raise _error(f"{label} is not valid UTF-8") from exc
    return value


def _validate_absolute_path(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise _error(f"{label} must be an absolute path string")
    if "\\" in value or "\x00" in value:
        raise _error(f"{label} contains an unsafe path character")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise _error(f"{label} contains a control character")
    try:
        encoded = value.encode("utf-8")
        path = Path(value)
        pure = PurePosixPath(value)
        normalized = os.path.normpath(value)
        resolved = path.resolve(strict=False)
    except (OSError, UnicodeError, ValueError) as exc:
        raise _error(f"{label} is not a safe absolute path") from exc
    del encoded
    if not os.path.isabs(value) or pure.as_posix() != value:
        raise _error(f"{label} must be a normalized absolute path")
    if any(part in {".", ".."} for part in pure.parts):
        raise _error(f"{label} contains traversal segments")
    if normalized != value or resolved != path:
        raise _error(f"{label} must not contain traversal or symlinked components")
    return value


def _validate_pair(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != PAIR_KEYS:
        raise _error("release pair has an unexpected field set")
    if value.get("schema_version") != PAIR_SCHEMA:
        raise _error("release pair has an unsupported schema version")
    for field in ("software_sha256", "data_revision", "data_manifest_sha256", "corpus_revision"):
        _validate_digest(value.get(field), label=f"pair {field}")
    _validate_text(value.get("query_schema"), label="pair query_schema")
    _validate_text(value.get("compiler_version"), label="pair compiler_version")
    return dict(value)


def _validate_bindings(value: Any) -> dict[str, str]:
    if not isinstance(value, dict) or set(value) != BINDING_KEYS:
        raise _error("release bindings have an unexpected field set")
    return {
        "data_root": _validate_absolute_path(value.get("data_root"), label="binding data_root"),
        "software_archive": _validate_absolute_path(
            value.get("software_archive"),
            label="binding software_archive",
        ),
    }


def _validate_pointer(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != POINTER_KEYS:
        raise _error("current pointer has an unexpected field set")
    if value.get("schema_version") != POINTER_SCHEMA:
        raise _error("current pointer has an unsupported schema version")
    current = _validate_digest(value.get("current"), label="current pointer current")
    previous = value.get("previous")
    if previous is not None:
        previous = _validate_digest(previous, label="current pointer previous")
        if previous == current:
            raise _error("current pointer current and previous must differ")
    return {"schema_version": POINTER_SCHEMA, "current": current, "previous": previous}


def _validate_kind(value: Any) -> str:
    if not isinstance(value, str) or value not in REVOCATION_KINDS:
        raise _error("revocation kind must be data, corpus, or software")
    return value


def _validate_revocation(value: Any, *, expected_kind: str | None = None, expected_digest: str | None = None) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != REVOCATION_KEYS:
        raise _error("revocation record has an unexpected field set")
    if value.get("schema_version") != REVOCATION_SCHEMA:
        raise _error("revocation record has an unsupported schema version")
    kind = _validate_kind(value.get("kind"))
    digest = _validate_digest(value.get("digest"), label="revocation digest")
    if expected_kind is not None and kind != expected_kind:
        raise _error("revocation record kind does not match its directory")
    if expected_digest is not None and digest != expected_digest:
        raise _error("revocation record digest does not match its filename")
    reason = _validate_text(value.get("reason"), label="revocation reason")
    owner_ref = _validate_text(value.get("owner_ref"), label="revocation owner_ref")
    return {
        "schema_version": REVOCATION_SCHEMA,
        "kind": kind,
        "digest": digest,
        "reason": reason,
        "owner_ref": owner_ref,
    }


def pair_id_for(pair: dict[str, Any]) -> str:
    """Return the stable ID of one validated release pair."""

    validated = _validate_pair(pair)
    return hashlib.sha256(canonical(validated)).hexdigest()


def _optional_digest(value: str | None, *, label: str) -> str | None:
    if value is None:
        return None
    return _validate_digest(value, label=label)


class ReleaseStore:
    """Manage immutable pair/binding records and one compare-and-swap pointer."""

    def __init__(self, root: Path, *, create: bool = True) -> None:
        if not isinstance(create, bool):
            raise _error("create must be a boolean")
        raw_root = Path(root)
        if any(part in {".", ".."} for part in raw_root.parts):
            raise _error("release store root contains traversal segments")
        try:
            absolute_root = raw_root.absolute()
            resolved_root = absolute_root.resolve(strict=False)
        except (OSError, ValueError) as exc:
            raise _error("release store root is not a safe path") from exc
        if absolute_root != resolved_root:
            raise _error("release store root must not contain symlinks")
        try:
            if os.path.lexists(absolute_root):
                _ensure_directory(absolute_root, label="release store root")
            elif not create:
                raise _error(f"release store root does not exist: {absolute_root}")
            else:
                absolute_root.mkdir(parents=True)
            _ensure_directory(absolute_root, label="release store root")
        except OSError as exc:
            raise _error(f"cannot create release store root: {absolute_root}") from exc

        self.root = absolute_root
        self.pairs = self.root / "pairs"
        self.bindings = self.root / "bindings"
        self.revocations = self.root / "revocations"
        self.current_path = self.root / "current.json"
        self._lock_path = self.root / ".release.lock"
        self._create = create
        self._prepare_directory(self.pairs, "pairs directory", create=create)
        self._prepare_directory(self.bindings, "bindings directory", create=create)
        self._prepare_directory(self.revocations, "revocations directory", create=create)
        for kind in sorted(REVOCATION_KINDS):
            self._prepare_directory(
                self.revocations / kind,
                f"{kind} revocations directory",
                create=create,
            )

    @staticmethod
    def _prepare_directory(path: Path, label: str, *, create: bool) -> None:
        try:
            if os.path.lexists(path):
                _ensure_directory(path, label=label)
            elif not create:
                raise _error(f"{label} does not exist: {path}")
            else:
                path.mkdir()
            _ensure_directory(path, label=label)
        except OSError as exc:
            raise _error(f"cannot create {label}: {path}") from exc

    @contextmanager
    def _lock(self, *, write: bool) -> Iterator[None]:
        if fcntl is None:
            if os.path.lexists(self._lock_path):
                _ensure_regular(self._lock_path, label="release state lock")
            if write:
                raise _error("release state writes require POSIX flock support")
            # Immutable reads do not create a lock file on platforms without
            # flock.  The caller still receives all structural validation.
            yield
            return
        lock_exists = os.path.lexists(self._lock_path)
        if not lock_exists and not write:
            # Reads never create private state, including for a writable store
            # that has not yet needed its writer lock.
            yield
            return
        try:
            nofollow = getattr(os, "O_NOFOLLOW", 0)
            descriptor = os.open(
                self._lock_path,
                (os.O_RDWR | os.O_CREAT if write else os.O_RDONLY) | nofollow,
                0o600,
            )
        except OSError as exc:
            raise _error(f"cannot open release state lock: {self._lock_path}") from exc
        try:
            metadata = os.fstat(descriptor)
            if not stat.S_ISREG(metadata.st_mode):
                raise _error("release state lock must be a regular file")
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX if write else fcntl.LOCK_SH)
            except OSError as exc:
                raise _error("cannot lock release state") from exc
            yield
        finally:
            try:
                os.close(descriptor)
            except OSError:
                pass

    def _read_pointer(self) -> dict[str, Any] | None:
        if not os.path.lexists(self.current_path):
            return None
        return _validate_pointer(
            _read_canonical(self.current_path, label="current pointer")
        )

    def _validate_layout(self) -> None:
        """Recheck managed directories so post-init symlink swaps fail closed."""

        _ensure_directory(self.root, label="release store root")
        _ensure_directory(self.pairs, label="pairs directory")
        _ensure_directory(self.bindings, label="bindings directory")
        _ensure_directory(self.revocations, label="revocations directory")
        for kind in REVOCATION_KINDS:
            _ensure_directory(self.revocations / kind, label=f"{kind} revocations directory")

    def _pair_path(self, pair_id: str) -> Path:
        return self.pairs / f"{_validate_digest(pair_id, label='pair id')}.json"

    def _binding_path(self, pair_id: str) -> Path:
        return self.bindings / f"{_validate_digest(pair_id, label='pair id')}.json"

    def _revocation_path(self, kind: str, digest: str) -> Path:
        return self.revocations / _validate_kind(kind) / f"{_validate_digest(digest, label='revocation digest')}.json"

    def _load_pair(self, pair_id: str) -> dict[str, Any]:
        pair_id = _validate_digest(pair_id, label="pair id")
        pair = _validate_pair(_read_canonical(self._pair_path(pair_id), label="release pair"))
        if pair_id_for(pair) != pair_id:
            raise _error("release pair digest does not match its filename")
        return pair

    def _load_bindings(self, pair_id: str) -> dict[str, str]:
        return _validate_bindings(
            _read_canonical(self._binding_path(pair_id), label="release bindings")
        )

    def _load_existing_or_none(self, path: Path, *, label: str) -> bytes | None:
        if not os.path.lexists(path):
            return None
        _ensure_regular(path, label=label)
        try:
            return path.read_bytes()
        except OSError as exc:
            raise _error(f"cannot read {label}: {path}") from exc

    def _check_immutable_pair_files(
        self,
        pair_id: str,
        pair: dict[str, Any],
        bindings: dict[str, str],
    ) -> None:
        expected_pair = canonical(pair)
        expected_bindings = canonical(bindings)
        existing_pair = self._load_existing_or_none(self._pair_path(pair_id), label="release pair")
        if existing_pair is not None and existing_pair != expected_pair:
            raise _error("existing release pair differs and cannot be overwritten")
        existing_bindings = self._load_existing_or_none(
            self._binding_path(pair_id),
            label="release bindings",
        )
        if existing_bindings is not None and existing_bindings != expected_bindings:
            raise _error("existing release bindings differ and cannot be overwritten")

    @staticmethod
    def _run_verifier(
        pair: dict[str, Any],
        bindings: dict[str, str],
        verify_pair: Callable[[dict[str, Any], dict[str, str]], Any],
    ) -> None:
        if not callable(verify_pair):
            raise _error("verify_pair callback is required")
        try:
            result = verify_pair(copy.deepcopy(pair), copy.deepcopy(bindings))
        except Exception as exc:
            raise _error(f"release pair verification failed: {exc}") from exc
        if result is not None:
            raise _error("verify_pair callback must return None")

    def _read_revocation(self, kind: str, digest: str) -> dict[str, Any] | None:
        path = self._revocation_path(kind, digest)
        if not os.path.lexists(path):
            return None
        record = _validate_revocation(
            _read_canonical(path, label="revocation record"),
            expected_kind=kind,
            expected_digest=digest,
        )
        return record

    def _assert_available_unlocked(self, pair: dict[str, Any]) -> None:
        pair = _validate_pair(pair)
        digests = {
            "data": pair["data_revision"],
            "corpus": pair["corpus_revision"],
            "software": pair["software_sha256"],
        }
        for kind, digest in digests.items():
            record = self._read_revocation(kind, digest)
            if record is not None:
                raise _error(f"release {kind} digest {digest} is revoked")

    def assert_available(self, pair: dict[str, Any]) -> None:
        """Fail closed when any selected data/corpus/software digest is revoked."""

        validated = _validate_pair(pair)
        with self._lock(write=False):
            self._validate_layout()
            self._assert_available_unlocked(validated)

    def promote(
        self,
        pair: dict[str, Any],
        bindings: dict[str, str],
        *,
        expected_current: str | None,
        verify_pair: Callable[[dict[str, Any], dict[str, str]], Any],
    ) -> str:
        """Validate and atomically promote one pair against an expected pointer."""

        validated_pair = _validate_pair(pair)
        validated_bindings = _validate_bindings(bindings)
        pair_id = pair_id_for(validated_pair)
        expected = _optional_digest(expected_current, label="expected_current")
        # Verification can hash a large artifact. Do not block urgent revocation
        # or existing readers while it runs; CAS and revocation recheck happen below.
        self._run_verifier(validated_pair, validated_bindings, verify_pair)
        with self._lock(write=True):
            self._validate_layout()
            pointer = self._read_pointer()
            current = None if pointer is None else pointer["current"]
            if current != expected:
                raise _error("expected_current does not match current release")
            if current is not None:
                self._load_pair(current)
                self._load_bindings(current)

            self._check_immutable_pair_files(pair_id, validated_pair, validated_bindings)
            if current == pair_id:
                stored_pair = self._load_pair(pair_id)
                stored_bindings = self._load_bindings(pair_id)
                if stored_pair != validated_pair or stored_bindings != validated_bindings:
                    raise _error("already-current pair is not identical to requested bindings")
                self._assert_available_unlocked(validated_pair)
                return pair_id

            self._assert_available_unlocked(validated_pair)
            self._publish_pair_files(pair_id, validated_pair, validated_bindings)
            latest = self._read_pointer()
            latest_current = None if latest is None else latest["current"]
            if latest_current != expected:
                raise _error("expected_current changed before promotion")
            _atomic_pointer(
                self.current_path,
                {
                    "schema_version": POINTER_SCHEMA,
                    "current": pair_id,
                    "previous": latest_current,
                },
                root=self.root,
            )
            return pair_id

    def _publish_pair_files(
        self,
        pair_id: str,
        pair: dict[str, Any],
        bindings: dict[str, str],
    ) -> None:
        _write_canonical(self._pair_path(pair_id), pair, label="release pair")
        _write_canonical(self._binding_path(pair_id), bindings, label="release bindings")

    def read_selection(self) -> dict[str, Any]:
        """Read and validate the current selection without hashing bound data."""

        with self._lock(write=False):
            self._validate_layout()
            pointer = self._read_pointer()
            if pointer is None:
                raise _error("no current release pointer exists")
            pair_id = pointer["current"]
            pair = self._load_pair(pair_id)
            bindings = self._load_bindings(pair_id)
            self._assert_available_unlocked(pair)
            return {
                "pair_id": pair_id,
                "pair": pair,
                "bindings": bindings,
                "previous": pointer["previous"],
            }

    def revoke(
        self,
        kind: str,
        digest: str,
        *,
        reason: str,
        owner_ref: str,
    ) -> dict[str, Any]:
        """Record one immutable revocation outside the rollback pointer."""

        validated_kind = _validate_kind(kind)
        validated_digest = _validate_digest(digest, label="revocation digest")
        record = _validate_revocation(
            {
                "schema_version": REVOCATION_SCHEMA,
                "kind": validated_kind,
                "digest": validated_digest,
                "reason": reason,
                "owner_ref": owner_ref,
            }
        )
        with self._lock(write=True):
            self._validate_layout()
            path = self._revocation_path(validated_kind, validated_digest)
            existing = self._read_revocation(validated_kind, validated_digest)
            if existing is not None:
                if existing != record:
                    raise _error("existing revocation differs and cannot be overwritten")
                return existing
            _write_canonical(path, record, label="revocation record")
            return record

    def rollback(
        self,
        *,
        expected_current: str | None,
        verify_pair: Callable[[dict[str, Any], dict[str, str]], Any],
    ) -> str:
        """Atomically switch to the stored previous pair after re-verification."""

        expected = _optional_digest(expected_current, label="expected_current")
        with self._lock(write=False):
            self._validate_layout()
            pointer = self._read_pointer()
            if pointer is None:
                raise _error("cannot rollback without a current release pointer")
            current_id = pointer["current"]
            if current_id != expected:
                raise _error("expected_current does not match current release")
            previous_id = pointer["previous"]
            if previous_id is None:
                raise _error("current release has no previous pair to roll back to")

            # Validate the current record too: switching a pointer over a
            # corrupt current state must fail closed and preserve the pointer.
            self._load_pair(current_id)
            self._load_bindings(current_id)
            previous_pair = self._load_pair(previous_id)
            previous_bindings = self._load_bindings(previous_id)
        self._run_verifier(previous_pair, previous_bindings, verify_pair)
        with self._lock(write=True):
            self._validate_layout()
            if self._read_pointer() != pointer:
                raise _error("expected_current changed during rollback verification")
            if (self._load_pair(previous_id) != previous_pair
                    or self._load_bindings(previous_id) != previous_bindings):
                raise _error("previous pair changed during rollback verification")
            self._assert_available_unlocked(previous_pair)
            _atomic_pointer(
                self.current_path,
                {
                    "schema_version": POINTER_SCHEMA,
                    "current": previous_id,
                    "previous": current_id,
                },
                root=self.root,
            )
            return previous_id


__all__ = [
    "BINDING_KEYS",
    "PAIR_KEYS",
    "PAIR_SCHEMA",
    "POINTER_KEYS",
    "POINTER_SCHEMA",
    "REVOCATION_KEYS",
    "REVOCATION_KINDS",
    "REVOCATION_SCHEMA",
    "ReleaseStateError",
    "ReleaseStore",
    "ReleaseStoreError",
    "canonical",
    "pair_id_for",
]
