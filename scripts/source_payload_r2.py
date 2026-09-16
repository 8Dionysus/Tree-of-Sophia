#!/usr/bin/env python3
"""Persistent, read-back oriented Cloudflare R2 REST transport.

The importer owns the receipt-root lock and calls this adapter serially for
one planned object at a time.  The optional initial list is therefore only a
read optimisation: it is cached after pagination is proven complete, and a
successful PUT adds its key to that cache.  The cache does not provide a
remote compare-and-swap and must not be used as an authority for receipts.

Authentication deliberately reuses Wrangler's supported ``auth token
--json`` command.  Its stdout is parsed in memory and is never included in a
log, exception, or diagnostic.  No Wrangler private module is imported and no
credential file is read by this module.
"""

from __future__ import annotations

from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import subprocess
import time
from typing import Any, Callable, Iterator, Mapping
from urllib.parse import quote, urlencode, urlsplit
from http.client import HTTPException, HTTPSConnection


REST_HOST = "api.cloudflare.com"
REST_PREFIX = "/client/v4"
REST_ENDPOINT = f"https://{REST_HOST}{REST_PREFIX}"
MAX_UPLOAD_BYTES = 300 * 1024 * 1024
LIST_PAGE_SIZE = 1000
DEFAULT_REQUEST_SPACING = 0.30
DEFAULT_MAX_RETRIES = 3
DEFAULT_RETRY_BACKOFF = 0.50
DEFAULT_LIST_PREFIX = "blobs/sha256/"
READ_CHUNK_BYTES = 1024 * 1024

_ACCOUNT_ID = re.compile(r"^[a-fA-F0-9]{32}$")
_BUCKET_NAME = re.compile(r"^[a-z0-9](?:[a-z0-9-]{1,61}[a-z0-9])$")
_OBJECT_KEY = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/-]*$")


class R2TransportError(RuntimeError):
    """A safe, non-secret R2 transport failure."""


class _ListUnsupported(Exception):
    """The endpoint does not expose REST object listing for this account."""


class _RemoteStreamError(Exception):
    """A response stream failed and can be retried from byte zero."""


def _validated_account_id(account_id: str) -> str:
    if not isinstance(account_id, str) or not _ACCOUNT_ID.fullmatch(account_id):
        raise R2TransportError("Cloudflare account ID must be 32 hexadecimal characters")
    return account_id.lower()


def _validated_bucket(bucket: str) -> str:
    if not isinstance(bucket, str) or not _BUCKET_NAME.fullmatch(bucket):
        raise R2TransportError("invalid R2 bucket name")
    return bucket


def _validated_object_key(key: str) -> str:
    if (
        not isinstance(key, str)
        or not _OBJECT_KEY.fullmatch(key)
        or key.startswith("/")
        or ".." in key.split("/")
    ):
        raise R2TransportError("unsafe R2 object key")
    return key


def _validated_prefix(prefix: str | None) -> str | None:
    if prefix is None:
        return None
    if prefix == "":
        return ""
    return _validated_object_key(prefix)


def _validated_endpoint(endpoint: str) -> str:
    try:
        parsed = urlsplit(endpoint)
    except ValueError as exc:
        raise R2TransportError("invalid R2 REST endpoint") from exc
    if (
        parsed.scheme != "https"
        or parsed.hostname != REST_HOST
        or parsed.port is not None
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or parsed.path.rstrip("/") != REST_PREFIX
    ):
        raise R2TransportError("R2 REST endpoint must be https://api.cloudflare.com/client/v4")
    return REST_PREFIX


def capture_wrangler_auth_token(
    executable: Path,
    *,
    cwd: Path | None = None,
    environment: Mapping[str, str] | None = None,
) -> str:
    """Capture Wrangler's supported OAuth/API token without exposing it.

    The command's stdout is retained only long enough to parse JSON.  stderr
    is sent directly to ``DEVNULL`` so an authentication diagnostic cannot be
    accidentally copied into an importer error or receipt.
    """

    executable = Path(executable)
    if not executable.is_absolute():
        raise R2TransportError("Wrangler executable must be an absolute path")
    env = dict(os.environ if environment is None else environment)
    env.setdefault("WRANGLER_SEND_METRICS", "false")
    try:
        result = subprocess.run(
            [str(executable), "auth", "token", "--json"],
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            check=False,
        )
    except OSError as exc:
        raise R2TransportError("cannot execute Wrangler auth command") from exc
    if result.returncode != 0:
        raise R2TransportError("Wrangler auth token command failed")
    try:
        payload = json.loads(result.stdout)
    except (TypeError, ValueError):
        raise R2TransportError("Wrangler auth token output was invalid") from None
    if not isinstance(payload, dict):
        raise R2TransportError("Wrangler auth token output was invalid")
    token_type = payload.get("type")
    token = payload.get("token")
    if (
        token_type not in {"oauth", "api_token"}
        or not isinstance(token, str)
        or not token
        or "\r" in token
        or "\n" in token
    ):
        raise R2TransportError("Wrangler auth token output lacked a usable token")
    return token


class R2RestTransport:
    """Persistent Cloudflare REST R2 transport for one private bucket.

    ``auth_token`` is intended for tests or a caller that already captured a
    token in memory.  Production callers should omit it and let this class
    invoke Wrangler's official auth command.  ``connection_factory``,
    ``clock`` and ``sleep`` are dependency-injection seams for deterministic
    tests; the default connection is a reusable :class:`HTTPSConnection`.
    """

    def __init__(
        self,
        *,
        account_id: str,
        bucket: str,
        executable: Path,
        cwd: Path | None = None,
        auth_token: str | None = None,
        endpoint: str = REST_ENDPOINT,
        request_spacing: float = DEFAULT_REQUEST_SPACING,
        max_retries: int = DEFAULT_MAX_RETRIES,
        retry_backoff: float = DEFAULT_RETRY_BACKOFF,
        timeout: float = 120.0,
        list_prefix: str | None = DEFAULT_LIST_PREFIX,
        list_objects: bool = True,
        connection_factory: Callable[..., Any] | None = None,
        clock: Callable[[], float] | None = None,
        sleep: Callable[[float], None] | None = None,
    ) -> None:
        self.account_id = _validated_account_id(account_id)
        self.bucket = _validated_bucket(bucket)
        self.executable = Path(executable)
        if not self.executable.is_absolute():
            raise R2TransportError("Wrangler executable must be an absolute path")
        if auth_token is not None and (
            not isinstance(auth_token, str)
            or not auth_token
            or "\r" in auth_token
            or "\n" in auth_token
        ):
            raise R2TransportError("R2 auth token must be a non-empty string")
        _validated_endpoint(endpoint)
        if request_spacing < 0:
            raise R2TransportError("request spacing cannot be negative")
        if max_retries < 0:
            raise R2TransportError("max retries cannot be negative")
        if retry_backoff < 0 or timeout <= 0:
            raise R2TransportError("retry backoff and timeout must be positive")
        self.cwd = cwd
        self._token = auth_token
        self._request_spacing = float(request_spacing)
        self._max_retries = int(max_retries)
        self._retry_backoff = float(retry_backoff)
        self._timeout = float(timeout)
        self._list_prefix = _validated_prefix(list_prefix)
        self._list_objects = bool(list_objects)
        self._connection_factory = connection_factory or HTTPSConnection
        self._clock = clock or time.monotonic
        self._sleep = sleep or time.sleep
        self._connection: Any | None = None
        self._last_request_at: float | None = None
        self._list_state = "unknown" if self._list_objects else "disabled"
        self._known_keys: set[str] = set()

    def __enter__(self) -> "R2RestTransport":
        return self

    def __exit__(self, _exc_type: Any, _exc: Any, _tb: Any) -> None:
        self.close()

    def close(self) -> None:
        connection, self._connection = self._connection, None
        if connection is not None:
            try:
                connection.close()
            except (OSError, HTTPException):
                pass

    def _reset_connection(self) -> None:
        self.close()

    def _get_connection(self) -> Any:
        if self._connection is None:
            self._connection = self._connection_factory(REST_HOST, timeout=self._timeout)
        return self._connection

    def _ensure_token(self) -> str:
        if self._token is None:
            self._token = capture_wrangler_auth_token(self.executable, cwd=self.cwd)
        return self._token

    def _refresh_token(self) -> None:
        # The request loop permits one refresh and fails closed on a
        # consecutive 401. A later request starts a fresh budget, so a token
        # that expires again during a long batch can be refreshed once more.
        self._token = capture_wrangler_auth_token(self.executable, cwd=self.cwd)

    def _throttle(self) -> None:
        now = self._clock()
        if self._last_request_at is not None:
            wait_for = self._request_spacing - (now - self._last_request_at)
            if wait_for > 0:
                self._sleep(wait_for)
        self._last_request_at = self._clock()

    @staticmethod
    def _response_header(response: Any, name: str) -> str | None:
        getter = getattr(response, "getheader", None)
        if getter is None:
            return None
        try:
            return getter(name)
        except (KeyError, TypeError):
            return None

    def _retry_delay(self, response: Any, retry_number: int) -> float:
        retry_after = self._response_header(response, "Retry-After")
        if retry_after:
            try:
                return min(30.0, max(0.0, float(retry_after)))
            except ValueError:
                pass
        return min(30.0, self._retry_backoff * (2 ** max(0, retry_number - 1)))

    def _discard_response(self, response: Any) -> None:
        try:
            response.read()
            response.close()
        except (OSError, HTTPException, ValueError):
            self._reset_connection()

    def _request_response(
        self,
        method: str,
        path: str,
        headers: Mapping[str, str],
        *,
        body_factory: Callable[[], Any] | None = None,
        retry_state: list[int] | None = None,
    ) -> Any:
        retry_state = retry_state if retry_state is not None else [0]
        auth_refresh_attempted = False
        while True:
            self._throttle()
            try:
                connection = self._get_connection()
            except (OSError, HTTPException):
                self._reset_connection()
                if retry_state[0] < self._max_retries:
                    retry_state[0] += 1
                    self._sleep(self._retry_delay(None, retry_state[0]))
                    continue
                raise R2TransportError("Cloudflare REST request exceeded retry budget") from None
            try:
                if body_factory is None:
                    connection.request(method, path, headers=dict(headers))
                    response = connection.getresponse()
                else:
                    with body_factory() as body:
                        connection.request(method, path, body=body, headers=dict(headers))
                        response = connection.getresponse()
            except (OSError, HTTPException) as exc:
                self._reset_connection()
                if retry_state[0] < self._max_retries:
                    retry_state[0] += 1
                    self._sleep(self._retry_delay(None, retry_state[0]))
                    continue
                raise R2TransportError("Cloudflare REST request exceeded retry budget") from None

            status = int(getattr(response, "status", 0))
            if status == 401:
                self._discard_response(response)
                if auth_refresh_attempted:
                    raise R2TransportError("Cloudflare REST authentication failed")
                auth_refresh_attempted = True
                self._refresh_token()
                if isinstance(headers, dict):
                    headers["Authorization"] = f"Bearer {self._token}"
                continue
            if status == 429 or status >= 500:
                if retry_state[0] < self._max_retries:
                    retry_state[0] += 1
                    delay = self._retry_delay(response, retry_state[0])
                    self._discard_response(response)
                    self._sleep(delay)
                    continue
                self._discard_response(response)
                raise R2TransportError("Cloudflare REST request exceeded retry budget")
            return response

    def _auth_headers(self, *, accept_json: bool = False) -> dict[str, str]:
        headers = {"Authorization": f"Bearer {self._ensure_token()}"}
        if accept_json:
            headers["Accept"] = "application/json"
        return headers

    def _object_path(self, object_key: str) -> str:
        key = _validated_object_key(object_key)
        encoded = quote(key, safe="/-._~")
        return f"{REST_PREFIX}/accounts/{self.account_id}/r2/buckets/{self.bucket}/objects/{encoded}"

    def _list_path(
        self,
        cursor: str | None,
        *,
        start_after: str | None = None,
    ) -> str:
        params: dict[str, str] = {"per_page": str(LIST_PAGE_SIZE)}
        if self._list_prefix is not None:
            params["prefix"] = self._list_prefix
        if cursor is not None:
            params["cursor"] = cursor
        if start_after is not None:
            params["start_after"] = start_after
        return (
            f"{REST_PREFIX}/accounts/{self.account_id}/r2/buckets/{self.bucket}/objects?"
            f"{urlencode(params)}"
        )

    def _list_page(
        self,
        cursor: str | None,
        *,
        start_after: str | None = None,
    ) -> tuple[list[str], bool, str | None, bool]:
        response = self._request_response(
            "GET",
            self._list_path(cursor, start_after=start_after),
            self._auth_headers(accept_json=True),
        )
        status = int(getattr(response, "status", 0))
        if status in {400, 403, 404, 405, 501}:
            self._discard_response(response)
            raise _ListUnsupported
        if status < 200 or status >= 300:
            self._discard_response(response)
            raise R2TransportError(f"Cloudflare REST list failed with HTTP {status}")
        try:
            raw = response.read()
            response.close()
        except (OSError, HTTPException, ValueError) as exc:
            self._reset_connection()
            raise R2TransportError("Cloudflare REST list response could not be read") from exc
        try:
            payload = json.loads(raw)
        except (TypeError, ValueError):
            raise _ListUnsupported from None
        if (
            not isinstance(payload, dict)
            or payload.get("success") is not True
            or not isinstance(payload.get("errors"), list)
            or payload["errors"]
            or not isinstance(payload.get("result"), list)
        ):
            raise _ListUnsupported
        keys: list[str] = []
        for entry in payload["result"]:
            if not isinstance(entry, dict) or not isinstance(entry.get("key"), str):
                raise _ListUnsupported
            try:
                key = _validated_object_key(entry["key"])
            except R2TransportError:
                raise _ListUnsupported from None
            if self._list_prefix is not None and not key.startswith(self._list_prefix):
                # A response outside the requested prefix cannot safely seed
                # the cache; a normal object GET remains the safe fallback.
                raise _ListUnsupported
            if keys and key <= keys[-1]:
                # ListObjects is ordered lexicographically.  Duplicate or
                # backward keys would make start_after unable to prove EOF.
                raise _ListUnsupported
            if start_after is not None and key <= start_after:
                # The endpoint ignored or repeated the marker.  Never cache a
                # partial listing in that case.
                raise _ListUnsupported
            keys.append(key)
        result_info = payload.get("result_info")
        if not isinstance(result_info, dict):
            # Some R2 accounts return no result_info/cursor metadata.  The
            # documented start_after parameter still proves EOF when the
            # next ordered page is explicitly empty.  The fourth return
            # value tells _ensure_listing to use that marker protocol.
            return keys, len(keys) == 0, None, True
        is_truncated = result_info.get("is_truncated", result_info.get("isTruncated"))
        next_cursor = result_info.get("cursor") or result_info.get("next_cursor") or result_info.get("nextCursor")
        if next_cursor is not None and not isinstance(next_cursor, str):
            raise R2TransportError("Cloudflare REST list cursor was invalid")
        if is_truncated is True and not next_cursor:
            raise _ListUnsupported
        if is_truncated is False:
            if next_cursor:
                raise _ListUnsupported
            return keys, True, None, False
        if next_cursor:
            return keys, False, next_cursor, False
        # Do not infer exhaustion from a short page. Without explicit
        # is_truncated=false, the start_after fallback is the only safe
        # completion proof, and a malformed result_info cannot provide it.
        raise _ListUnsupported

    def _ensure_listing(self) -> None:
        if self._list_state != "unknown":
            return
        self._list_state = "unsupported"
        try:
            all_keys: set[str] = set()
            cursor: str | None = None
            start_after: str | None = None
            seen_cursors: set[str] = set()
            last_key: str | None = None
            for _ in range(10000):
                page_keys, complete, next_cursor, marker_mode = self._list_page(
                    cursor,
                    start_after=start_after,
                )
                if last_key is not None and page_keys and page_keys[0] <= last_key:
                    # This also protects the cursor protocol from a repeated
                    # or backward page, which cannot support an exact cache.
                    raise _ListUnsupported
                all_keys.update(page_keys)
                if complete:
                    self._known_keys = all_keys
                    self._list_state = "complete"
                    return
                if marker_mode:
                    if not page_keys:
                        self._known_keys = all_keys
                        self._list_state = "complete"
                        return
                    next_marker = page_keys[-1]
                    if start_after is not None and next_marker <= start_after:
                        raise _ListUnsupported
                    last_key = next_marker
                    start_after = next_marker
                    cursor = None
                    continue
                if not next_cursor or next_cursor in seen_cursors:
                    raise _ListUnsupported
                seen_cursors.add(next_cursor)
                cursor = next_cursor
                start_after = None
                if page_keys:
                    last_key = page_keys[-1]
            raise _ListUnsupported
        except _ListUnsupported:
            # A missing list capability is safe to handle with ordinary GET.
            self._list_state = "unsupported"

    def fetch(self, object_key: str, destination: Path) -> bool:
        _validated_object_key(object_key)
        destination = Path(destination)
        if destination.exists():
            raise R2TransportError("transport destination already exists")
        if self._list_state == "unknown" and (
            self._list_prefix is None or object_key.startswith(self._list_prefix)
        ):
            self._ensure_listing()
        if self._list_state == "complete" and (
            self._list_prefix is None or object_key.startswith(self._list_prefix)
        ) and object_key not in self._known_keys:
            return False
        # Keep request and response-stream failures under one retry budget.
        # A retry always starts a fresh GET and a fresh destination, so a
        # partial read can never be appended to a later attempt.
        retry_state = [0]
        while True:
            response = self._request_response(
                "GET",
                self._object_path(object_key),
                self._auth_headers(),
                retry_state=retry_state,
            )
            status = int(getattr(response, "status", 0))
            if status == 404:
                self._discard_response(response)
                return False
            if status < 200 or status >= 300:
                self._discard_response(response)
                raise R2TransportError(f"Cloudflare REST read failed with HTTP {status}")
            created = False
            try:
                try:
                    target = destination.open("xb")
                except OSError as exc:
                    raise R2TransportError("cannot create remote readback destination") from exc
                created = True
                with target:
                    received = 0
                    while True:
                        try:
                            block = response.read(READ_CHUNK_BYTES)
                        except (OSError, HTTPException) as exc:
                            raise _RemoteStreamError from exc
                        if not block:
                            break
                        received += len(block)
                        if received > MAX_UPLOAD_BYTES:
                            raise R2TransportError(
                                f"R2 readback exceeds the {MAX_UPLOAD_BYTES}-byte object limit"
                            )
                        try:
                            target.write(block)
                        except OSError as exc:
                            raise R2TransportError(
                                "cannot write remote readback destination"
                            ) from exc
                try:
                    response.close()
                except (OSError, HTTPException) as exc:
                    raise _RemoteStreamError from exc
            except _RemoteStreamError:
                if created:
                    destination.unlink(missing_ok=True)
                try:
                    response.close()
                except (OSError, HTTPException):
                    pass
                self._reset_connection()
                if retry_state[0] < self._max_retries:
                    retry_state[0] += 1
                    self._sleep(self._retry_delay(None, retry_state[0]))
                    continue
                raise R2TransportError("Cloudflare REST read stream exceeded retry budget") from None
            except R2TransportError:
                if created:
                    destination.unlink(missing_ok=True)
                try:
                    response.close()
                except (OSError, HTTPException):
                    pass
                self._reset_connection()
                raise
            return True

    @contextmanager
    def _source_body(self, source: Path) -> Iterator[Any]:
        try:
            stream = Path(source).open("rb")
        except OSError as exc:
            raise R2TransportError("cannot read local upload source") from exc
        try:
            yield stream
        finally:
            stream.close()

    def put(
        self,
        object_key: str,
        source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> None:
        _validated_object_key(object_key)
        if storage_class != "Standard":
            raise R2TransportError("only R2 Standard storage is enabled by this adapter")
        if not isinstance(byte_size, int) or byte_size < 0 or byte_size > MAX_UPLOAD_BYTES:
            raise R2TransportError(
                f"R2 REST single-object upload limit is {MAX_UPLOAD_BYTES} bytes"
            )
        try:
            if Path(source).stat().st_size != byte_size:
                raise R2TransportError("upload source byte-size does not match the plan")
        except OSError as exc:
            raise R2TransportError("cannot stat local upload source") from exc
        if (
            not isinstance(media_type, str)
            or not media_type
            or "\r" in media_type
            or "\n" in media_type
        ):
            raise R2TransportError("upload media type is required")
        headers = self._auth_headers()
        headers.update(
            {
                "Content-Length": str(byte_size),
                "Content-Type": media_type,
                "Cache-Control": "private,no-store",
                "cf-r2-storage-class": storage_class,
            }
        )
        # Reuse the same bounded retry state for request failures and a
        # broken response body. Each retry invokes body_factory again, which
        # rewinds the upload by reopening the local snapshot.
        retry_state = [0]
        while True:
            response = self._request_response(
                "PUT",
                self._object_path(object_key),
                headers,
                body_factory=lambda: self._source_body(Path(source)),
                retry_state=retry_state,
            )
            status = int(getattr(response, "status", 0))
            if status < 200 or status >= 300:
                self._discard_response(response)
                raise R2TransportError(f"Cloudflare REST upload failed with HTTP {status}")
            try:
                response.read()
                response.close()
            except (OSError, HTTPException, ValueError) as exc:
                self._reset_connection()
                if retry_state[0] < self._max_retries:
                    retry_state[0] += 1
                    self._sleep(self._retry_delay(None, retry_state[0]))
                    continue
                raise R2TransportError(
                    "Cloudflare REST upload response could not be read"
                ) from exc
            break
        if self._list_state == "complete":
            self._known_keys.add(object_key)
