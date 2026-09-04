from __future__ import annotations

import http.client
import re
import sys
import tempfile
import threading
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
from test_access_contract import write_fixture  # noqa: E402
from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.http_server import make_server  # noqa: E402


@pytest.fixture()
def access_server():
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        write_fixture(root)
        server = make_server(ToSAccessCore.discover(tos_root=root), port=0)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield server
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


def response(server, path: str) -> tuple[http.client.HTTPResponse, bytes]:
    connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
    connection.request("GET", path)
    result = connection.getresponse()
    body = result.read()
    connection.close()
    return result, body


def test_root_security_headers_and_nonce_bind_inline_boot(access_server) -> None:
    result, body = response(access_server, "/")
    headers = {name.lower(): value for name, value in result.getheaders()}
    csp = headers["content-security-policy"]
    nonce_match = re.search(r'<script nonce="([A-Za-z0-9_-]+)">', body.decode("utf-8"))

    assert result.status == 200
    assert nonce_match
    assert f"'nonce-{nonce_match.group(1)}'" in csp
    assert "script-src 'self'" in csp
    assert "'unsafe-inline'" not in csp
    assert "object-src 'none'" in csp
    assert "frame-ancestors 'none'" in csp
    assert headers["permissions-policy"].startswith("tools=(self)")
    assert "camera=()" in headers["permissions-policy"]
    assert headers["cross-origin-opener-policy"] == "same-origin"
    assert headers["cross-origin-embedder-policy"] == "require-corp"
    assert headers["cross-origin-resource-policy"] == "same-origin"
    assert headers["origin-agent-cluster"] == "?1"
    assert headers["referrer-policy"] == "no-referrer"
    assert headers["x-frame-options"] == "DENY"
    assert headers["x-content-type-options"] == "nosniff"


def test_json_and_static_responses_keep_cross_origin_boundaries(access_server) -> None:
    for path in ("/health", "/static/assets/tos-graph.css"):
        result, _ = response(access_server, path)
        headers = {name.lower(): value for name, value in result.getheaders()}
        assert result.status == 200
        assert headers["content-security-policy"].startswith("default-src 'self'")
        assert headers["permissions-policy"].startswith("tools=(self)")
        assert headers["cross-origin-opener-policy"] == "same-origin"
