"""Small local status state for independently published downstream consumers.

This module records attempt and publication bookkeeping for the ``kag`` and
``stats`` owners.  It does not validate artifacts, copy payloads, discover
owners, or decide whether an owner may publish an artifact.
"""

from __future__ import annotations

from contextlib import contextmanager
import copy
from datetime import datetime, timezone
import fcntl
import json
import os
from pathlib import Path
import re
import stat
import tempfile
from typing import Any, Iterator
import secrets


SCHEMA_VERSION = "downstream_status_v1"
CONSUMERS = frozenset({"kag", "stats"})
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
ATTEMPT_ID = re.compile(r"[0-9a-f]{32}\Z")
TIMESTAMP = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\Z")
MAX_ERROR_CHARS = 4096

STATE_KEYS = frozenset(
    {"schema_version", "consumer", "latest", "last_success", "previous_success"}
)
RUNNING_KEYS = frozenset({"attempt_id", "source_revision", "started_at", "state"})
SUCCESS_KEYS = frozenset(
    {
        "attempt_id",
        "source_revision",
        "started_at",
        "completed_at",
        "artifact_revision",
        "artifact_manifest_sha256",
    }
)
FAILED_KEYS = frozenset(
    {"attempt_id", "source_revision", "started_at", "completed_at", "state", "error"}
)


class DownstreamStatusError(ValueError):
    """Raised when local downstream status is unsafe or has an invalid transition."""


def _error(message: str) -> DownstreamStatusError:
    return DownstreamStatusError(message)


def _canonical(value: dict[str, Any]) -> bytes:
    try:
        rendered = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        return (rendered + "\n").encode("utf-8")
    except (TypeError, UnicodeError, ValueError) as exc:
        raise _error(f"cannot render canonical downstream status: {exc}") from exc


def _strict_object(raw: bytes, *, label: str) -> dict[str, Any]:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                raise _error(f"{label} contains duplicate field {key!r}")
            result[key] = value
        return result

    def reject_constant(value: str) -> None:
        raise ValueError(f"non-finite JSON value {value}")

    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=reject_constant,
        )
    except DownstreamStatusError:
        raise
    except (TypeError, UnicodeError, ValueError) as exc:
        raise _error(f"{label} is not valid finite UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise _error(f"{label} must contain an object")
    return value


def _digest(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or HEX64.fullmatch(value) is None:
        raise _error(f"{label} must be a lowercase SHA-256 digest")
    return value


def _attempt(value: Any, *, label: str = "attempt_id") -> str:
    if not isinstance(value, str) or ATTEMPT_ID.fullmatch(value) is None:
        raise _error(f"{label} must be a lowercase random attempt id")
    return value


def _timestamp(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or TIMESTAMP.fullmatch(value) is None:
        raise _error(f"{label} must be a UTC timestamp")
    try:
        parsed = datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise _error(f"{label} is not a valid UTC timestamp") from exc
    if parsed.tzinfo != timezone.utc:
        raise _error(f"{label} must be UTC")
    return value


def _success_record(value: Any, *, label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != SUCCESS_KEYS:
        raise _error(f"{label} has an unexpected field set")
    record = {
        "attempt_id": _attempt(value.get("attempt_id"), label=f"{label}.attempt_id"),
        "source_revision": _digest(
            value.get("source_revision"), label=f"{label}.source_revision"
        ),
        "started_at": _timestamp(value.get("started_at"), label=f"{label}.started_at"),
        "completed_at": _timestamp(
            value.get("completed_at"), label=f"{label}.completed_at"
        ),
        "artifact_revision": _digest(
            value.get("artifact_revision"), label=f"{label}.artifact_revision"
        ),
        "artifact_manifest_sha256": _digest(
            value.get("artifact_manifest_sha256"),
            label=f"{label}.artifact_manifest_sha256",
        ),
    }
    if record["completed_at"] < record["started_at"]:
        raise _error(f"{label}.completed_at precedes started_at")
    return record


def _latest(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise _error("latest attempt must be an object")
    state = value.get("state")
    if state == "running":
        if set(value) != RUNNING_KEYS:
            raise _error("running latest attempt has an unexpected field set")
        return {
            "attempt_id": _attempt(value.get("attempt_id"), label="latest.attempt_id"),
            "source_revision": _digest(
                value.get("source_revision"), label="latest.source_revision"
            ),
            "started_at": _timestamp(value.get("started_at"), label="latest.started_at"),
            "state": "running",
        }
    if state == "succeeded":
        if set(value) != SUCCESS_KEYS | {"state"}:
            raise _error("succeeded latest attempt has an unexpected field set")
        record = _success_record(
            {key: value[key] for key in SUCCESS_KEYS},
            label="latest",
        )
        return {**record, "state": "succeeded"}
    if state == "failed":
        if set(value) != FAILED_KEYS:
            raise _error("failed latest attempt has an unexpected field set")
        error = value.get("error")
        if not isinstance(error, str) or not error or len(error) > MAX_ERROR_CHARS:
            raise _error("latest.error must be a bounded non-empty string")
        if any(ord(character) == 0 or ord(character) == 0x7F for character in error):
            raise _error("latest.error contains an unsafe control character")
        return {
            "attempt_id": _attempt(value.get("attempt_id"), label="latest.attempt_id"),
            "source_revision": _digest(
                value.get("source_revision"), label="latest.source_revision"
            ),
            "started_at": _timestamp(value.get("started_at"), label="latest.started_at"),
            "completed_at": _timestamp(
                value.get("completed_at"), label="latest.completed_at"
            ),
            "state": "failed",
            "error": error,
        }
    raise _error("latest attempt has an unsupported state")


def _validate_state(value: Any, *, consumer: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != STATE_KEYS:
        raise _error("downstream state has an unexpected field set")
    if value.get("schema_version") != SCHEMA_VERSION:
        raise _error("downstream state has an unsupported schema version")
    if value.get("consumer") != consumer:
        raise _error("downstream state consumer does not match")
    latest = _latest(value.get("latest"))
    last_success_value = value.get("last_success")
    previous_success_value = value.get("previous_success")
    last_success = (
        None
        if last_success_value is None
        else _success_record(last_success_value, label="last_success")
    )
    previous_success = (
        None
        if previous_success_value is None
        else _success_record(previous_success_value, label="previous_success")
    )
    if latest["state"] == "succeeded":
        expected = {key: latest[key] for key in SUCCESS_KEYS}
        if last_success != expected:
            raise _error("succeeded latest attempt must be the last_success")
    if last_success is None and previous_success is not None:
        raise _error("previous_success requires last_success")
    if last_success is not None and previous_success is not None:
        if last_success["attempt_id"] == previous_success["attempt_id"]:
            raise _error("last_success and previous_success must differ")
        if previous_success["completed_at"] > last_success["completed_at"]:
            raise _error("previous_success is newer than last_success")
    return {
        "schema_version": SCHEMA_VERSION,
        "consumer": consumer,
        "latest": latest,
        "last_success": last_success,
        "previous_success": previous_success,
    }


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _new_attempt_id() -> str:
    return secrets.token_hex(16)


def _existing_components_have_no_symlink(path: Path) -> None:
    current = Path(path.anchor)
    for component in path.parts[1:]:
        current /= component
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            return
        except OSError as exc:
            raise _error(f"cannot inspect downstream status path: {current}") from exc
        if stat.S_ISLNK(metadata.st_mode):
            raise _error(f"downstream status path must not contain symlinks: {current}")


def _ensure_directory(path: Path, *, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is unavailable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode):
        raise _error(f"{label} must not be a symlink: {path}")
    if not stat.S_ISDIR(metadata.st_mode):
        raise _error(f"{label} must be a directory: {path}")


def _ensure_regular(path: Path, *, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is unavailable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode):
        raise _error(f"{label} must not be a symlink: {path}")
    if not stat.S_ISREG(metadata.st_mode):
        raise _error(f"{label} must be a regular file: {path}")


def _sync_directory(path: Path) -> None:
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
    except OSError as exc:
        raise _error(f"cannot open downstream status directory: {path}") from exc
    try:
        os.fsync(descriptor)
    except OSError as exc:
        raise _error(f"cannot fsync downstream status directory: {path}") from exc
    finally:
        os.close(descriptor)


class DownstreamStatus:
    """Own one small status state for one independent downstream consumer."""

    def __init__(self, root: Path, consumer: str):
        if consumer not in CONSUMERS:
            raise _error("consumer must be exactly 'kag' or 'stats'")
        try:
            raw_root = Path(root).expanduser()
            if any(part in {".", ".."} for part in raw_root.parts):
                raise _error("downstream status root contains traversal segments")
            absolute_root = raw_root.absolute()
        except DownstreamStatusError:
            raise
        except (OSError, TypeError, ValueError) as exc:
            raise _error("downstream status root is not a safe path") from exc
        _existing_components_have_no_symlink(absolute_root)
        if os.path.lexists(absolute_root):
            _ensure_directory(absolute_root, label="downstream status root")
        self.root = absolute_root
        self.consumer = consumer
        self.state_path = self.root / "state.json"
        self.lock_path = self.root / ".lock"

    def _validate_existing_layout(self) -> bool:
        _existing_components_have_no_symlink(self.root)
        if not os.path.lexists(self.root):
            return False
        _ensure_directory(self.root, label="downstream status root")
        for path, label in (
            (self.state_path, "downstream state"),
            (self.lock_path, "downstream status lock"),
        ):
            if os.path.lexists(path):
                _ensure_regular(path, label=label)
        return True

    def _ensure_writer_root(self) -> None:
        _existing_components_have_no_symlink(self.root)
        try:
            self.root.mkdir(parents=True, exist_ok=True)
        except OSError as exc:
            raise _error(f"cannot create downstream status root: {self.root}") from exc
        _existing_components_have_no_symlink(self.root)
        _ensure_directory(self.root, label="downstream status root")

    @contextmanager
    def _writer_lock(self) -> Iterator[None]:
        self._ensure_writer_root()
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(
                self.lock_path,
                os.O_RDWR | os.O_CREAT | nofollow,
                0o600,
            )
        except OSError as exc:
            raise _error(f"cannot open downstream status lock: {self.lock_path}") from exc
        try:
            metadata = os.fstat(descriptor)
            if not stat.S_ISREG(metadata.st_mode):
                raise _error("downstream status lock must be a regular file")
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX)
            except OSError as exc:
                raise _error("cannot lock downstream status") from exc
            yield
        finally:
            os.close(descriptor)

    def _read_state_unlocked(self) -> dict[str, Any] | None:
        if not os.path.lexists(self.state_path):
            return None
        _ensure_regular(self.state_path, label="downstream state")
        try:
            raw = self.state_path.read_bytes()
        except OSError as exc:
            raise _error(f"cannot read downstream state: {self.state_path}") from exc
        value = _strict_object(raw, label="downstream state")
        if _canonical(value) != raw:
            raise _error(f"downstream state is not canonical JSON: {self.state_path}")
        return _validate_state(value, consumer=self.consumer)

    def _read_state(self) -> dict[str, Any] | None:
        if not self._validate_existing_layout():
            return None
        if not os.path.lexists(self.lock_path):
            return self._read_state_unlocked()
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(self.lock_path, os.O_RDONLY | nofollow)
        except OSError as exc:
            raise _error(f"cannot open downstream status lock: {self.lock_path}") from exc
        try:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_SH)
            except OSError as exc:
                raise _error("cannot read locked downstream status") from exc
            return self._read_state_unlocked()
        finally:
            os.close(descriptor)

    def _write_state(self, value: dict[str, Any]) -> None:
        _ensure_directory(self.root, label="downstream status root")
        if os.path.lexists(self.state_path):
            _ensure_regular(self.state_path, label="downstream state")
        raw = _canonical(_validate_state(value, consumer=self.consumer))
        temporary: Path | None = None
        try:
            descriptor, name = tempfile.mkstemp(prefix=".state.", dir=self.root)
            temporary = Path(name)
            try:
                os.fchmod(descriptor, 0o600)
                with os.fdopen(descriptor, "wb") as stream:
                    descriptor = -1
                    stream.write(raw)
                    stream.flush()
                    os.fsync(stream.fileno())
            finally:
                if descriptor != -1:
                    os.close(descriptor)
            os.replace(temporary, self.state_path)
            temporary = None
            _sync_directory(self.root)
        except OSError as exc:
            raise _error(f"cannot atomically publish downstream state: {self.state_path}") from exc
        finally:
            if temporary is not None:
                try:
                    temporary.unlink(missing_ok=True)
                except OSError:
                    pass

    @staticmethod
    def _empty_state(consumer: str, latest: dict[str, Any]) -> dict[str, Any]:
        return {
            "schema_version": SCHEMA_VERSION,
            "consumer": consumer,
            "latest": latest,
            "last_success": None,
            "previous_success": None,
        }

    def begin(self, source_revision: str) -> str:
        source_revision = _digest(source_revision, label="source_revision")
        attempt_id = _new_attempt_id()
        started_at = _now()
        latest = {
            "attempt_id": attempt_id,
            "source_revision": source_revision,
            "started_at": started_at,
            "state": "running",
        }
        with self._writer_lock():
            existing = self._read_state_unlocked()
            if existing is None:
                state = self._empty_state(self.consumer, latest)
            else:
                state = {
                    **existing,
                    "latest": latest,
                }
            self._write_state(state)
        return attempt_id

    def succeed(
        self,
        attempt_id: str,
        *,
        artifact_revision: str,
        artifact_manifest_sha256: str,
    ) -> None:
        attempt_id = _attempt(attempt_id)
        artifact_revision = _digest(artifact_revision, label="artifact_revision")
        artifact_manifest_sha256 = _digest(
            artifact_manifest_sha256,
            label="artifact_manifest_sha256",
        )
        with self._writer_lock():
            state = self._read_state_unlocked()
            if state is None:
                raise _error("cannot succeed without a begun attempt")
            latest = state["latest"]
            if latest["attempt_id"] != attempt_id:
                raise _error("attempt is stale and cannot advance downstream status")
            if latest["state"] != "running":
                raise _error("only a running latest attempt may succeed")
            completed_at = _now()
            success = {
                "attempt_id": attempt_id,
                "source_revision": latest["source_revision"],
                "started_at": latest["started_at"],
                "completed_at": completed_at,
                "artifact_revision": artifact_revision,
                "artifact_manifest_sha256": artifact_manifest_sha256,
            }
            next_state = {
                **state,
                "latest": {**success, "state": "succeeded"},
                "last_success": success,
                "previous_success": state["last_success"],
            }
            self._write_state(next_state)

    def fail(self, attempt_id: str, error: str) -> None:
        attempt_id = _attempt(attempt_id)
        if not isinstance(error, str) or not error:
            raise _error("error must be a non-empty string")
        if any(ord(character) == 0 or ord(character) == 0x7F for character in error):
            raise _error("error contains an unsafe control character")
        error = error[:MAX_ERROR_CHARS]
        with self._writer_lock():
            state = self._read_state_unlocked()
            if state is None:
                raise _error("cannot fail without a begun attempt")
            latest = state["latest"]
            if latest["attempt_id"] != attempt_id:
                raise _error("attempt is stale and cannot advance downstream status")
            if latest["state"] != "running":
                raise _error("only a running latest attempt may fail")
            next_state = {
                **state,
                "latest": {
                    "attempt_id": attempt_id,
                    "source_revision": latest["source_revision"],
                    "started_at": latest["started_at"],
                    "completed_at": _now(),
                    "state": "failed",
                    "error": error,
                },
            }
            self._write_state(next_state)

    def status(self, expected_source_revision: str) -> dict[str, Any]:
        expected_source_revision = _digest(
            expected_source_revision,
            label="expected_source_revision",
        )
        state = self._read_state()
        if state is None:
            return {"state": None, "freshness": "missing", "latest_attempt": None}
        last_success = state["last_success"]
        freshness = (
            "missing"
            if last_success is None
            else "current"
            if last_success["source_revision"] == expected_source_revision
            else "stale"
        )
        return {
            "state": copy.deepcopy(state),
            "freshness": freshness,
            "latest_attempt": copy.deepcopy(state["latest"]),
        }


__all__ = ["DownstreamStatus", "DownstreamStatusError"]
