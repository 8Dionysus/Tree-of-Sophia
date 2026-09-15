from __future__ import annotations

import json
from http.client import IncompleteRead, RemoteDisconnected
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import source_payload_r2 as r2


ACCOUNT_ID = "a" * 32
BUCKET = "tos-source-payloads"
KEY = "blobs/sha256/aa/" + "a" * 64


def list_payload(result: list[dict[str, str]], **extra: object) -> dict[str, object]:
    payload: dict[str, object] = {
        "success": True,
        "errors": [],
        "messages": [],
        "result": result,
    }
    payload.update(extra)
    return payload


class FakeResponse:
    def __init__(
        self,
        status: int,
        body: bytes = b"",
        headers: dict[str, str] | None = None,
        *,
        read_error: bool | BaseException = False,
    ):
        self.status = status
        self._body = body
        self._offset = 0
        self.headers = headers or {}
        self.read_error = read_error
        self.closed = False

    def read(self, amount: int = -1) -> bytes:
        if self.read_error:
            if isinstance(self.read_error, BaseException):
                raise self.read_error
            raise OSError("simulated response read failure")
        if amount < 0:
            amount = len(self._body) - self._offset
        result = self._body[self._offset : self._offset + amount]
        self._offset += len(result)
        return result

    def getheader(self, name: str) -> str | None:
        return self.headers.get(name)

    def close(self) -> None:
        self.closed = True


class FakeConnection:
    def __init__(self, responses: list[FakeResponse], request_errors: list[BaseException] | None = None):
        self.responses = responses
        self.request_errors = list(request_errors or [])
        self.requests: list[tuple[str, str, dict[str, str], bytes]] = []
        self.attempted_bodies: list[bytes] = []
        self.closed = False

    def request(self, method: str, path: str, body=None, headers=None) -> None:
        body_bytes = b""
        if body is not None:
            chunks: list[bytes] = []
            while True:
                chunk = body.read(7)
                if not chunk:
                    break
                chunks.append(chunk)
            body_bytes = b"".join(chunks)
            self.attempted_bodies.append(body_bytes)
        if self.request_errors:
            raise self.request_errors.pop(0)
        self.requests.append((method, path, dict(headers or {}), body_bytes))

    def getresponse(self) -> FakeResponse:
        if not self.responses:
            raise AssertionError("fake response queue exhausted")
        return self.responses.pop(0)

    def close(self) -> None:
        self.closed = True


class PartialThenIncompleteResponse(FakeResponse):
    def __init__(self, status: int, partial: bytes):
        super().__init__(status)
        self.partial = partial
        self.sent_partial = False

    def read(self, amount: int = -1) -> bytes:
        if not self.sent_partial:
            self.sent_partial = True
            return self.partial
        raise IncompleteRead(self.partial, len(self.partial) + 1)


class FakeClock:
    def __init__(self) -> None:
        self.value = 0.0
        self.sleeps: list[float] = []

    def now(self) -> float:
        return self.value

    def sleep(self, duration: float) -> None:
        self.sleeps.append(duration)
        self.value += duration


def make_transport(
    connection: FakeConnection,
    *,
    clock: FakeClock | None = None,
    account_id: str = ACCOUNT_ID,
    bucket: str = BUCKET,
    **kwargs,
) -> r2.R2RestTransport:
    clock = clock or FakeClock()
    return r2.R2RestTransport(
        account_id=account_id,
        bucket=bucket,
        executable=Path("/usr/bin/wrangler"),
        auth_token="memory-token",
        connection_factory=lambda _host, **_options: connection,
        clock=clock.now,
        sleep=clock.sleep,
        **kwargs,
    )


class SourcePayloadR2Tests(unittest.TestCase):
    def test_put_and_fetch_stream_with_private_headers_and_safe_path(self) -> None:
        body = b"source bytes streamed in chunks\n"
        remote = FakeResponse(200, body)
        connection = FakeConnection([FakeResponse(200), remote])
        transport = make_transport(connection, list_objects=False)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.bin"
            output = root / "output.bin"
            source.write_bytes(body)
            transport.put(
                KEY,
                source,
                byte_size=len(body),
                media_type="text/plain",
                storage_class="Standard",
            )
            self.assertTrue(transport.fetch(KEY, output))
            self.assertEqual(body, output.read_bytes())
        put_method, put_path, put_headers, put_body = connection.requests[0]
        self.assertEqual("PUT", put_method)
        self.assertEqual(
            "/client/v4/accounts/" + ACCOUNT_ID + "/r2/buckets/" + BUCKET + "/objects/" + KEY,
            put_path,
        )
        self.assertEqual(body, put_body)
        self.assertEqual(str(len(body)), put_headers["Content-Length"])
        self.assertEqual("text/plain", put_headers["Content-Type"])
        self.assertEqual("private,no-store", put_headers["Cache-Control"])
        self.assertEqual("Standard", put_headers["cf-r2-storage-class"])
        self.assertEqual("Bearer memory-token", put_headers["Authorization"])
        self.assertNotIn("Transfer-Encoding", put_headers)

    def test_auth_output_and_stderr_never_enter_safe_error(self) -> None:
        secret = "oauth-secret-must-not-leak"
        completed = r2.subprocess.CompletedProcess(
            ["wrangler", "auth", "token", "--json"],
            1,
            stdout=secret,
            stderr=secret,
        )
        with patch.object(r2.subprocess, "run", return_value=completed):
            with self.assertRaises(r2.R2TransportError) as caught:
                r2.capture_wrangler_auth_token(Path("/usr/bin/wrangler"))
        self.assertNotIn(secret, str(caught.exception))

    def test_401_refreshes_once_and_uses_new_in_memory_token(self) -> None:
        body = b"refreshable body"
        connection = FakeConnection([FakeResponse(401), FakeResponse(200)])
        transport = make_transport(connection, list_objects=False)
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.bin"
            source.write_bytes(body)
            with patch.object(r2, "capture_wrangler_auth_token", return_value="refreshed-token") as auth:
                transport.put(
                    KEY,
                    source,
                    byte_size=len(body),
                    media_type="application/octet-stream",
                    storage_class="Standard",
                )
        self.assertEqual(1, auth.call_count)
        self.assertEqual("Bearer memory-token", connection.requests[0][2]["Authorization"])
        self.assertEqual("Bearer refreshed-token", connection.requests[1][2]["Authorization"])

    def test_later_token_expiry_gets_a_fresh_refresh_budget(self) -> None:
        body = b"two expiry cycles"
        connection = FakeConnection(
            [FakeResponse(401), FakeResponse(200), FakeResponse(401), FakeResponse(200)]
        )
        transport = make_transport(connection, list_objects=False)
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.bin"
            source.write_bytes(body)
            with patch.object(
                r2,
                "capture_wrangler_auth_token",
                side_effect=["refreshed-token-1", "refreshed-token-2"],
            ) as auth:
                for _ in range(2):
                    transport.put(
                        KEY,
                        source,
                        byte_size=len(body),
                        media_type="application/octet-stream",
                        storage_class="Standard",
                    )
        self.assertEqual(2, auth.call_count)
        self.assertEqual(
            [
                "Bearer memory-token",
                "Bearer refreshed-token-1",
                "Bearer refreshed-token-1",
                "Bearer refreshed-token-2",
            ],
            [request[2]["Authorization"] for request in connection.requests],
        )

    def test_consecutive_401_fails_closed_after_one_refresh(self) -> None:
        body = b"invalid refreshed token"
        connection = FakeConnection([FakeResponse(401), FakeResponse(401)])
        transport = make_transport(connection, list_objects=False)
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.bin"
            source.write_bytes(body)
            with patch.object(
                r2,
                "capture_wrangler_auth_token",
                return_value="still-invalid-token",
            ) as auth:
                with self.assertRaisesRegex(
                    r2.R2TransportError, "authentication failed"
                ):
                    transport.put(
                        KEY,
                        source,
                        byte_size=len(body),
                        media_type="application/octet-stream",
                        storage_class="Standard",
                    )
        self.assertEqual(1, auth.call_count)
        self.assertEqual(2, len(connection.requests))

    def test_429_is_bounded_and_request_spacing_is_applied(self) -> None:
        body = b"retry body"
        clock = FakeClock()
        connection = FakeConnection(
            [FakeResponse(429, headers={"Retry-After": "0"}), FakeResponse(200)]
        )
        transport = make_transport(
            connection,
            clock=clock,
            list_objects=False,
            retry_backoff=0,
            max_retries=1,
        )
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.bin"
            source.write_bytes(body)
            transport.put(
                KEY,
                source,
                byte_size=len(body),
                media_type="application/octet-stream",
                storage_class="Standard",
            )
        self.assertEqual(2, len(connection.requests))
        self.assertTrue(any(duration >= 0.30 for duration in clock.sleeps))

    def test_http_exception_retries_put_with_a_rewound_source(self) -> None:
        body = b"rewind this upload"
        first = FakeConnection([], request_errors=[RemoteDisconnected("closed before response")])
        second = FakeConnection([FakeResponse(200)])
        connections = iter((first, second))
        clock = FakeClock()
        transport = r2.R2RestTransport(
            account_id=ACCOUNT_ID,
            bucket=BUCKET,
            executable=Path("/usr/bin/wrangler"),
            auth_token="memory-token",
            connection_factory=lambda _host, **_options: next(connections),
            clock=clock.now,
            sleep=clock.sleep,
            request_spacing=0,
            retry_backoff=0,
            max_retries=1,
            list_objects=False,
        )
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.bin"
            source.write_bytes(body)
            transport.put(
                KEY,
                source,
                byte_size=len(body),
                media_type="application/octet-stream",
                storage_class="Standard",
            )
        self.assertEqual([body], first.attempted_bodies)
        self.assertEqual([body], [request[3] for request in second.requests])

    def test_http_exception_in_put_response_rewinds_source_within_budget(self) -> None:
        body = b"rewind after response failure"
        first = FakeConnection([FakeResponse(200, read_error=RemoteDisconnected("closed after upload"))])
        second = FakeConnection([FakeResponse(200)])
        connections = iter((first, second))
        transport = r2.R2RestTransport(
            account_id=ACCOUNT_ID,
            bucket=BUCKET,
            executable=Path("/usr/bin/wrangler"),
            auth_token="memory-token",
            connection_factory=lambda _host, **_options: next(connections),
            request_spacing=0,
            retry_backoff=0,
            max_retries=1,
            list_objects=False,
        )
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.bin"
            source.write_bytes(body)
            transport.put(
                KEY,
                source,
                byte_size=len(body),
                media_type="application/octet-stream",
                storage_class="Standard",
            )
        self.assertEqual([body], first.attempted_bodies)
        self.assertEqual([body], [request[3] for request in second.requests])

    def test_incomplete_read_retries_and_removes_partial_fetch(self) -> None:
        first = FakeConnection([PartialThenIncompleteResponse(200, b"partial")])
        full_body = b"complete remote object"
        second = FakeConnection([FakeResponse(200, full_body)])
        connections = iter((first, second))
        transport = r2.R2RestTransport(
            account_id=ACCOUNT_ID,
            bucket=BUCKET,
            executable=Path("/usr/bin/wrangler"),
            auth_token="memory-token",
            connection_factory=lambda _host, **_options: next(connections),
            request_spacing=0,
            retry_backoff=0,
            max_retries=1,
            list_objects=False,
        )
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "remote.bin"
            self.assertTrue(transport.fetch(KEY, output))
            self.assertEqual(full_body, output.read_bytes())
        self.assertEqual(1, len(first.requests))
        self.assertEqual(1, len(second.requests))

    def test_readback_has_a_300_mib_bound_even_for_existing_objects(self) -> None:
        connection = FakeConnection([FakeResponse(200, b"too-large")])
        transport = make_transport(connection, list_objects=False)
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "too-large.bin"
            with patch.object(r2, "MAX_UPLOAD_BYTES", 4):
                with self.assertRaises(r2.R2TransportError):
                    transport.fetch(KEY, output)
            self.assertFalse(output.exists())

    def test_failed_read_removes_partial_destination(self) -> None:
        connection = FakeConnection([FakeResponse(200, read_error=True)])
        transport = make_transport(connection, list_objects=False, max_retries=0)
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "partial.bin"
            with self.assertRaises(r2.R2TransportError):
                transport.fetch(KEY, destination)
            self.assertFalse(destination.exists())

    def test_complete_list_avoids_missing_probe_and_put_adds_key(self) -> None:
        missing_key = "blobs/sha256/bb/" + "b" * 64
        listing = json.dumps(
            list_payload([{"key": KEY}], result_info={"is_truncated": False})
        ).encode()
        connection = FakeConnection(
            [
                FakeResponse(200, listing),
                FakeResponse(200, b"existing"),
                FakeResponse(200),
                FakeResponse(200, b"new remote"),
            ]
        )
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            existing_output = root / "existing.bin"
            new_source = root / "new.bin"
            new_output = root / "new-output.bin"
            new_source.write_bytes(b"new source")
            self.assertTrue(transport.fetch(KEY, existing_output))
            self.assertFalse(transport.fetch(missing_key, root / "missing.bin"))
            transport.put(
                missing_key,
                new_source,
                byte_size=len(b"new source"),
                media_type="application/octet-stream",
                storage_class="Standard",
            )
            self.assertTrue(transport.fetch(missing_key, new_output))
        self.assertEqual(4, len(connection.requests))
        self.assertIn("per_page=1000", connection.requests[0][1])
        self.assertEqual("GET", connection.requests[1][0])
        self.assertEqual("PUT", connection.requests[2][0])
        self.assertEqual("GET", connection.requests[3][0])

    def test_start_after_empty_page_proves_eof_without_result_info(self) -> None:
        missing_key = "blobs/sha256/bb/" + "b" * 64
        first_page = list_payload([{"key": KEY}])
        connection = FakeConnection(
            [
                FakeResponse(200, json.dumps(first_page).encode()),
                FakeResponse(200, json.dumps(list_payload([])).encode()),
            ]
        )
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(missing_key, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))
        self.assertIn("start_after=", connection.requests[1][1])
        self.assertEqual("complete", transport._list_state)

    def test_start_after_walk_requires_ordered_pages(self) -> None:
        key_b = "blobs/sha256/aa/" + "b" * 64
        key_c = "blobs/sha256/aa/" + "c" * 64
        pages = [
            list_payload([{"key": KEY}, {"key": key_b}]),
            list_payload([{"key": key_c}]),
            list_payload([]),
        ]
        connection = FakeConnection(
            [FakeResponse(200, json.dumps(page).encode()) for page in pages]
        )
        transport = make_transport(connection)
        missing_key = "blobs/sha256/zz/" + "z" * 64
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(missing_key, Path(temporary) / "missing.bin"))
        self.assertEqual(3, len(connection.requests))
        self.assertIn("start_after=", connection.requests[1][1])
        self.assertIn("start_after=", connection.requests[2][1])
        self.assertEqual("complete", transport._list_state)

    def test_start_after_ignored_marker_falls_back_to_get_without_cache(self) -> None:
        repeated_page = list_payload([{"key": KEY}])
        connection = FakeConnection(
            [
                FakeResponse(200, json.dumps(repeated_page).encode()),
                FakeResponse(200, json.dumps(repeated_page).encode()),
                FakeResponse(404),
            ]
        )
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(KEY, Path(temporary) / "missing.bin"))
        self.assertEqual(3, len(connection.requests))
        self.assertEqual("unsupported", transport._list_state)
        self.assertIn("/objects/" + KEY, connection.requests[2][1])

    def test_start_after_unsorted_page_falls_back_to_get_without_cache(self) -> None:
        key_b = "blobs/sha256/aa/" + "b" * 64
        unordered_page = list_payload([{"key": key_b}, {"key": KEY}])
        connection = FakeConnection(
            [
                FakeResponse(200, json.dumps(unordered_page).encode()),
                FakeResponse(404),
            ]
        )
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(KEY, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))
        self.assertEqual("unsupported", transport._list_state)
        self.assertIn("/objects/" + KEY, connection.requests[1][1])

    def test_list_prefix_contradiction_falls_back_to_get_without_cache(self) -> None:
        outside_prefix = list_payload([{"key": "other-prefix/" + "a" * 64}])
        connection = FakeConnection(
            [FakeResponse(200, json.dumps(outside_prefix).encode()), FakeResponse(404)]
        )
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(KEY, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))
        self.assertEqual("unsupported", transport._list_state)
        self.assertIn("/objects/" + KEY, connection.requests[1][1])

    def test_empty_page_with_cursor_is_followed_before_eof(self) -> None:
        first_page = list_payload(
            [],
            result_info={"is_truncated": True, "cursor": "cursor-1"},
        )
        final_page = list_payload([], result_info={"is_truncated": False})
        connection = FakeConnection(
            [
                FakeResponse(200, json.dumps(first_page).encode()),
                FakeResponse(200, json.dumps(final_page).encode()),
            ]
        )
        transport = make_transport(connection)
        missing_key = "blobs/sha256/bb/" + "b" * 64
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(missing_key, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))
        self.assertIn("cursor=cursor-1", connection.requests[1][1])
        self.assertEqual("complete", transport._list_state)

    def test_unsupported_list_falls_back_to_ordinary_get(self) -> None:
        connection = FakeConnection([FakeResponse(405), FakeResponse(404)])
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(KEY, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))
        self.assertIn("objects?", connection.requests[0][1])
        self.assertIn("/objects/" + KEY, connection.requests[1][1])

    def test_unproven_truncated_list_falls_back_to_get(self) -> None:
        page = {
            "success": True,
            "errors": [],
            "messages": [],
            "result": [{"key": f"blobs/sha256/aa/{index:064x}"} for index in range(1000)],
            "result_info": {"is_truncated": True},
        }
        connection = FakeConnection(
            [FakeResponse(200, json.dumps(page).encode()), FakeResponse(404)]
        )
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(KEY, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))

    def test_malformed_list_response_falls_back_to_ordinary_get(self) -> None:
        listing = json.dumps(
            {"success": True, "errors": [], "messages": [], "result": "not-a-list"}
        ).encode()
        connection = FakeConnection([FakeResponse(200, listing), FakeResponse(404)])
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(KEY, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))

    def test_failed_list_envelope_falls_back_to_ordinary_get(self) -> None:
        listing = json.dumps(
            {
                "success": False,
                "errors": [{"code": 1000, "message": "denied"}],
                "messages": [],
                "result": [],
            }
        ).encode()
        connection = FakeConnection([FakeResponse(200, listing), FakeResponse(404)])
        transport = make_transport(connection)
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(transport.fetch(KEY, Path(temporary) / "missing.bin"))
        self.assertEqual(2, len(connection.requests))
        self.assertEqual("unsupported", transport._list_state)

    def test_account_bucket_key_and_endpoint_are_validated(self) -> None:
        with self.assertRaises(r2.R2TransportError):
            make_transport(FakeConnection([]), account_id="short")
        with self.assertRaises(r2.R2TransportError):
            r2.R2RestTransport(
                account_id=ACCOUNT_ID,
                bucket=BUCKET,
                executable=Path("/usr/bin/wrangler"),
                auth_token="memory-token",
                endpoint="https://example.test/client/v4",
            )
        with self.assertRaises(r2.R2TransportError):
            make_transport(FakeConnection([])).fetch("../escape", Path("/tmp/never"))


if __name__ == "__main__":
    unittest.main()
