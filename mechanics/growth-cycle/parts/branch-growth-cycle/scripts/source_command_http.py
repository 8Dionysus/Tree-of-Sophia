"""Explicit loopback transport for the existing source-owner command grammar.

Access and grant issuance use their own APIs. The process owner selects one
protected command configuration and a separate private transport credential.
Every command still enters run_local_command and rechecks the current grant.
"""
from __future__ import annotations

import argparse
import hashlib
import hmac
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import re
import stat
import time
from urllib.parse import urlsplit

import source_commands as commands
from assessment_journal import JournalBusy, JournalConflict, _json_object, _owned_path

MAX_RESPONSE_BYTES = 4 * 1024 * 1024
SOCKET_TIMEOUT_SECONDS = 5
# The idle timeout remains the per-read guard; this caps the complete
# request-line, header and body read without bounding owner execution.
REQUEST_READ_DEADLINE_SECONDS = 30
TOKEN_PATTERN = re.compile(r'[a-f0-9]{64}\Z')
AUTH_PATTERN = re.compile(r'ToS-HMAC-SHA256 ([0-9]{10,13}):([a-f0-9]{64}):([a-f0-9]{64}):([a-f0-9]{64})\Z')
AUTH_WINDOW_SECONDS = 30
MAX_RECENT_NONCES = 1024


def _signature(token, fields):
    message = json.dumps(fields, ensure_ascii=True, separators=(',', ':')).encode('ascii')
    return hmac.new(bytes.fromhex(token), message, hashlib.sha256).hexdigest()


def _credential(path: Path) -> str:
    with os.fdopen(_owned_path(path), 'rb') as stream:
        info = os.fstat(stream.fileno())
        if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
            raise PermissionError('transport credential must be private to the process owner')
        raw = stream.read(66)
    token = raw.decode('ascii').removesuffix('\n')
    if TOKEN_PATTERN.fullmatch(token) is None:
        raise ValueError('transport credential must contain exactly 64 lowercase hexadecimal characters')
    return token


def _origin(value: str) -> str:
    parsed = urlsplit(value)
    if (parsed.scheme != 'http' or parsed.hostname not in {'127.0.0.1', 'localhost'}
            or parsed.username is not None or parsed.password is not None
            or parsed.path or parsed.query or parsed.fragment or parsed.port is None
            or not 1 <= parsed.port <= 65535
            or value != f'http://{parsed.hostname}:{parsed.port}'):
        raise ValueError('browser origin must be one exact loopback HTTP origin with a port')
    return value


class SourceCommandServer(HTTPServer):
    """One bounded synchronous command at a time, with no implicit retry.

    Socket timeouts limit idle network IO, and one absolute request-read
    deadline limits a complete request without bounding owner execution time.
    The owner handler retains its own work, transaction and recovery bounds.
    No arbitrary root, configuration, token or executable comes from a request.
    """

    request_queue_size = 4

    def __init__(self, *, owner_config: Path, token_file: Path,
                 browser_origin: str, port: int = 0):
        self.owner_config = Path(owner_config)
        self.token_file = Path(token_file)
        self.browser_origin = _origin(browser_origin)
        self.recent_nonces = {}
        # Check path protection at startup; do not execute describe or load
        # source material merely to start the transport.
        os.close(_owned_path(self.owner_config))
        _credential(self.token_file)
        super().__init__(('127.0.0.1', port), SourceCommandHandler)
        self.expected_host = f'127.0.0.1:{self.server_port}'

    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(SOCKET_TIMEOUT_SECONDS)
        return connection, address


class _RequestReader:
    """Small exact reader that refreshes the idle timeout per socket read."""

    def __init__(self, connection, handler):
        self.connection = connection
        self.handler = handler
        self.buffer = bytearray()
        self.closed = False

    def readable(self):
        return True

    def _receive(self):
        self.handler._prepare_request_read()
        chunk = self.connection.recv(8192)
        if chunk:
            self.buffer.extend(chunk)
        return chunk

    def readline(self, size=-1):
        if self.closed:
            return b''
        while True:
            boundary = self.buffer.find(b'\n')
            if boundary >= 0:
                end = boundary + 1
                if size >= 0:
                    end = min(end, size)
                value = bytes(self.buffer[:end])
                del self.buffer[:end]
                return value
            if size >= 0 and len(self.buffer) >= size:
                value = bytes(self.buffer[:size])
                del self.buffer[:size]
                return value
            if not self._receive():
                value = bytes(self.buffer)
                self.buffer.clear()
                return value

    def read(self, size=-1):
        if self.closed:
            return b''
        if size < 0:
            raise ValueError('unbounded request reads are not supported')
        while len(self.buffer) < size:
            if not self._receive():
                break
        value = bytes(self.buffer[:size])
        del self.buffer[:size]
        # The only bounded body consumer is do_POST. Restore the ordinary
        # idle timeout before hashing, dispatch and response writes.
        self.handler._finish_request_read()
        return value

    def close(self):
        self.closed = True


class SourceCommandHandler(BaseHTTPRequestHandler):
    server: SourceCommandServer

    def setup(self):
        super().setup()
        original = self.rfile
        self.rfile = _RequestReader(self.connection, self)
        original.close()

    def _prepare_request_read(self):
        deadline = getattr(self, '_request_read_deadline', None)
        if deadline is None:
            self.connection.settimeout(SOCKET_TIMEOUT_SECONDS)
            return
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('request read deadline exceeded')
        self.connection.settimeout(min(SOCKET_TIMEOUT_SECONDS, remaining))

    def _finish_request_read(self):
        self._request_read_deadline = None
        self.connection.settimeout(SOCKET_TIMEOUT_SECONDS)

    def handle_one_request(self):
        self._request_read_deadline = time.monotonic() + REQUEST_READ_DEADLINE_SECONDS
        try:
            super().handle_one_request()
        finally:
            self._finish_request_read()

    def send_response(self, code, message=None):
        # Do not let the request-read deadline become an owner execution or
        # response-write deadline when a request did not have a body read.
        self._finish_request_read()
        return super().send_response(code, message)

    def log_message(self, *_args):
        # Request headers, source payloads and configuration paths are private.
        pass

    def _reply(self, status, value):
        raw = json.dumps(value, ensure_ascii=False, allow_nan=False,
                         separators=(',', ':')).encode('utf-8')
        if len(raw) > MAX_RESPONSE_BYTES:
            status = 502
            raw = b'{"status":"error","code":"response-budget","outcome":"unconfirmed"}'
        self.send_response(status)
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Content-Length', str(len(raw)))
        self.send_header('Cache-Control', 'no-store')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.send_header('Connection', 'close')
        if getattr(self, '_authenticated', None) is not None:
            token, nonce, _body_digest = self._authenticated
            signature = _signature(token, ['tos-response-v1', nonce, status, hashlib.sha256(raw).hexdigest()])
            self.send_header('X-ToS-Response-Signature', signature)
        if self.headers.get_all('Origin') == [self.server.browser_origin]:
            self.send_header('Access-Control-Allow-Origin', self.server.browser_origin)
            self.send_header('Access-Control-Expose-Headers', 'X-ToS-Response-Signature')
            self.send_header('Vary', 'Origin')
        self.end_headers()
        self.close_connection = True
        try:
            self.wfile.write(raw)
        except (BrokenPipeError, ConnectionResetError):
            # A disconnected client does not cancel or roll back a command.
            pass

    def _error(self, status, code, *, dispatched=False):
        self._reply(status, {'status': 'error', 'code': code,
                            'outcome': 'unconfirmed' if dispatched else 'not-dispatched'})

    def _boundary(self, *, authenticate=True):
        if self.headers.get_all('Host') != [self.server.expected_host]:
            self._error(403, 'host-not-allowed')
            return False
        origins = self.headers.get_all('Origin')
        if origins is not None and origins != [self.server.browser_origin]:
            self._error(403, 'origin-not-allowed')
            return False
        if authenticate:
            auth = self.headers.get_all('Authorization')
            try:
                token = _credential(self.server.token_file)
            except (OSError, ValueError, UnicodeError):
                self._error(403, 'transport-credential-unavailable')
                return False
            match = AUTH_PATTERN.fullmatch(auth[0]) if auth is not None and len(auth) == 1 else None
            if match is None:
                self._error(401, 'transport-authentication-required')
                return False
            timestamp, nonce, body_digest, supplied = match.groups()
            expected = _signature(token, ['tos-request-v1', self.command, self.path, timestamp, nonce, body_digest])
            now = time.time()
            if abs(now - int(timestamp)) > AUTH_WINDOW_SECONDS or not hmac.compare_digest(supplied, expected):
                self._error(401, 'transport-authentication-required')
                return False
            self._authenticated = (token, nonce, body_digest)
            # A signed request is single-use even if its later body is rejected.
            # Retain its nonce until its timestamp can no longer pass freshness.
            recent = self.server.recent_nonces
            for retained, expiry in tuple(recent.items()):
                if expiry < now:
                    del recent[retained]
            if nonce in recent:
                self._error(409, 'transport-nonce-replayed')
                return False
            if len(recent) >= MAX_RECENT_NONCES:
                self._error(429, 'transport-authentication-capacity')
                return False
            recent[nonce] = int(timestamp) + AUTH_WINDOW_SECONDS
        return True

    def do_OPTIONS(self):
        if not self._boundary(authenticate=False):
            return
        if (self.path not in {'/commands', '/commands/catalog'}
                or self.headers.get_all('Origin') != [self.server.browser_origin]
                or self.headers.get_all('Access-Control-Request-Method') !=
                    ['POST' if self.path == '/commands' else 'GET']):
            self._error(403, 'preflight-not-allowed')
            return
        requested = self.headers.get_all('Access-Control-Request-Headers') or []
        if len(requested) != 1 or not set(requested[0].lower().replace(' ', '').split(',')) <= {
            'authorization', 'content-type'
        }:
            self._error(403, 'preflight-headers-not-allowed')
            return
        self.send_response(204)
        self.send_header('Access-Control-Allow-Origin', self.server.browser_origin)
        self.send_header('Access-Control-Allow-Methods', 'POST' if self.path == '/commands' else 'GET')
        self.send_header('Access-Control-Allow-Headers', 'Authorization, Content-Type')
        self.send_header('Vary', 'Origin')
        self.send_header('Cache-Control', 'no-store')
        self.send_header('Content-Length', '0')
        self.send_header('Connection', 'close')
        self.end_headers()
        self.close_connection = True

    def do_GET(self):
        if not self._boundary():
            return
        if self.path != '/commands/catalog':
            self._error(404, 'route-not-found')
            return
        if (self.headers.get_all('Transfer-Encoding') is not None
                or self.headers.get_all('Content-Length') not in (None, ['0'])
                or self._authenticated[2] != hashlib.sha256(b'').hexdigest()):
            self._error(400, 'invalid-body-framing')
            return
        self._reply(200, commands.discover_commands())

    def do_POST(self):
        if not self._boundary():
            return
        if self.path != '/commands':
            self._error(404, 'route-not-found')
            return
        lengths = self.headers.get_all('Content-Length')
        if (self.headers.get_all('Transfer-Encoding') is not None or lengths is None
                or len(lengths) != 1 or re.fullmatch(r'[0-9]{1,8}', lengths[0]) is None):
            self._error(400, 'invalid-body-framing')
            return
        length = int(lengths[0])
        if length > commands.MAX_COMMAND_BYTES:
            self._error(413, 'request-budget')
            return
        content_types = self.headers.get_all('Content-Type')
        if (content_types is None or len(content_types) != 1 or content_types[0].lower() not in {
            'application/json', 'application/json; charset=utf-8'
        }):
            self._error(415, 'json-required')
            return
        try:
            raw = self.rfile.read(length)
            if len(raw) != length:
                raise ValueError('incomplete body')
            if hashlib.sha256(raw).hexdigest() != self._authenticated[2]:
                self._error(401, 'request-digest-mismatch')
                return
            request = _json_object(raw)
        except (OSError, ValueError, UnicodeError, RecursionError):
            self._error(400, 'invalid-command-json')
            return
        try:
            result = commands.run_local_command(self.server.owner_config, request)
        except PermissionError:
            self._error(403, 'owner-permission-denied', dispatched=True)
        except (JournalBusy, JournalConflict):
            self._error(409, 'owner-conflict', dispatched=True)
        except ValueError:
            self._error(422, 'owner-command-rejected', dispatched=True)
        except Exception:
            # Never claim rollback after an unknown owner or delivery failure.
            self._error(500, 'owner-outcome-unconfirmed', dispatched=True)
        else:
            self._reply(200, result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--owner-config', type=Path, required=True)
    parser.add_argument('--token-file', type=Path, required=True)
    parser.add_argument('--browser-origin', required=True)
    parser.add_argument('--port', type=int, required=True)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error('port must be in 1..65535')
    with SourceCommandServer(owner_config=args.owner_config, token_file=args.token_file,
                             browser_origin=args.browser_origin, port=args.port) as server:
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
