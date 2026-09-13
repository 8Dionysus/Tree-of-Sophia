"""Real owner command transport plus browser/account boundary negatives."""
import http.client
import hashlib
import json
import os
from pathlib import Path
import socket
import shutil
import subprocess
import secrets
import sys
import threading
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_source_commands as fixtures
import source_command_http as transport


class SourceCommandHttpTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.SourceCommandTests(methodName='runTest')
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.token = 'a' * 64
        self.token_file = self.fixture.root / 'transport-token'
        self.token_file.write_text(self.token)
        self.token_file.chmod(0o600)
        self.origin = 'http://127.0.0.1:44257'
        self.server = transport.SourceCommandServer(
            owner_config=self.fixture.owner, token_file=self.token_file,
            browser_origin=self.origin)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.stop)

    def stop(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(2)

    def request(self, method='POST', path='/commands', value=None, headers=None, raw=None):
        connection = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=5)
        body = json.dumps(value).encode() if raw is None and value is not None else raw
        selected = {'Authorization': self.authorization(method, path, body or b''), 'Origin': self.origin,
                    'Content-Type': 'application/json'}
        selected.update(headers or {})
        try:
            connection.request(method, path, body=body, headers=selected)
            response = connection.getresponse()
            content = response.read()
            return response.status, dict(response.getheaders()), json.loads(content) if content else None
        finally:
            connection.close()

    def authorization(self, method, path, raw, *, timestamp=None, nonce=None):
        timestamp = str(int(time.time())) if timestamp is None else str(timestamp)
        nonce = secrets.token_hex(32) if nonce is None else nonce
        digest = hashlib.sha256(raw).hexdigest()
        proof = transport._signature(self.token, ['tos-request-v1', method, path, timestamp, nonce, digest])
        return f'ToS-HMAC-SHA256 {timestamp}:{nonce}:{digest}:{proof}'

    def test_real_describe_apply_replay_and_revocation_share_owner_grammar(self):
        describe = {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'}
        status, headers, body = self.request(value=describe)
        self.assertEqual(status, 200)
        self.assertEqual(body, self.fixture.describe())
        self.assertEqual(headers['Access-Control-Allow-Origin'], self.origin)
        self.assertEqual(headers['Cache-Control'], 'no-store')
        request = self.fixture.request()
        status, _, changed = self.request(value=request)
        self.assertEqual(status, 200, changed)
        self.assertFalse(changed['replayed'])
        self.assertFalse(changed['grants_admission'])
        status, _, replay = self.request(value=request)
        self.assertEqual(status, 200, replay)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], changed['receipt'])
        self.assertEqual(self.fixture.source.read_bytes(), self.fixture.original_source)
        self.fixture.config['expires_at'] = '2000-01-01T00:00:00Z'
        self.fixture.save_config()
        self.assertEqual(self.request(value=describe)[0], 403)

    def test_auth_origin_host_and_credential_rotation_precede_dispatch(self):
        with patch.object(transport.commands, 'run_local_command') as dispatch:
            for headers in (
                {'Authorization': ''}, {'Authorization': 'Bearer incorrect'},
                {'Origin': 'null'}, {'Origin': 'http://evil.example'},
                {'Origin': 'http://127.0.0.1:44258'}, {'Host': 'evil.example'},
            ):
                status, _, body = self.request(value={}, headers=headers)
                self.assertIn(status, (401, 403))
                self.assertEqual(body['outcome'], 'not-dispatched')
            self.token_file.write_text('b' * 64)
            self.assertEqual(self.request(value={})[0], 401)
            self.token_file.chmod(0o644)
            self.assertEqual(self.request(value={})[0], 403)
            dispatch.assert_not_called()

    def test_framing_json_size_and_paths_fail_without_dispatch(self):
        with patch.object(transport.commands, 'run_local_command') as dispatch:
            for raw, headers, path, expected in (
                (b'{"operation":"a","operation":"b"}', {}, '/commands', 400),
                (b'[]', {}, '/commands', 400),
                (b'{}', {'Transfer-Encoding': 'chunked'}, '/commands', 400),
                (b'', {'Content-Length': str(transport.commands.MAX_COMMAND_BYTES + 1)}, '/commands', 413),
                (b'{}', {'Content-Type': 'text/plain'}, '/commands', 415),
                (b'{}', {}, '/commands?owner-config=/etc/passwd', 404),
                (b'{}', {}, '/api/commands', 404),
            ):
                status, _, body = self.request(raw=raw, headers=headers, path=path)
                self.assertEqual(status, expected, body)
                self.assertEqual(body['outcome'], 'not-dispatched')
            dispatch.assert_not_called()

    def test_duplicate_auth_and_lengths_are_rejected(self):
        for name, values in (('Authorization', [self.authorization('POST', '/commands', b'{}')] * 2),
                             ('Content-Length', ['2', '2'])):
            connection = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=5)
            try:
                connection.putrequest('POST', '/commands')
                connection.putheader('Origin', self.origin)
                connection.putheader('Content-Type', 'application/json')
                if name != 'Authorization':
                    connection.putheader('Authorization', self.authorization('POST', '/commands', b'{}'))
                if name != 'Content-Length':
                    connection.putheader('Content-Length', '2')
                for value in values:
                    connection.putheader(name, value)
                connection.endheaders(b'{}')
                response = connection.getresponse()
                self.assertIn(response.status, (400, 401))
                self.assertEqual(json.loads(response.read())['outcome'], 'not-dispatched')
            finally:
                connection.close()

    def test_catalog_is_the_existing_unauthorized_grammar_and_cors_is_exact(self):
        status, _, body = self.request('GET', '/commands/catalog')
        self.assertEqual(status, 200)
        self.assertEqual(body, transport.commands.discover_commands())
        self.assertFalse(body['grants_admission'])
        status, headers, _ = self.request('OPTIONS', '/commands', headers={
            'Authorization': '', 'Access-Control-Request-Method': 'POST',
            'Access-Control-Request-Headers': 'authorization, content-type'})
        self.assertEqual(status, 204)
        self.assertEqual(headers['Access-Control-Allow-Origin'], self.origin)
        self.assertNotIn('Access-Control-Allow-Credentials', headers)
        self.assertEqual(self.request('OPTIONS', '/commands', headers={
            'Access-Control-Request-Method': 'DELETE',
            'Access-Control-Request-Headers': 'authorization'})[0], 403)

    @unittest.skipUnless(shutil.which('node'), 'browser client contract requires Node')
    def test_real_browser_client_session_prepares_and_applies_through_owner(self):
        modules = fixtures.ROOT / 'access/web/constructor'
        program = '''
          const {createSourceCommandClient}=await import(process.env.TOS_TEST_CLIENT);
          const {createSourceFormSession}=await import(process.env.TOS_TEST_SESSION);
          const client=createSourceCommandClient({origin:process.env.TOS_TEST_ORIGIN,
            token:process.env.TOS_TEST_TOKEN});
          const session=createSourceFormSession(client,{commandId:()=> 'test:browser-owner-wire'});
          const context=(await session.describe()).current;
          const prepared=await session.prepare({formId:context.allowed_form_ids[0],fieldId:'metadata.preferred-name'});
          if(prepared.current.prepared_materialization.state!=='ready')throw new Error('preview unavailable');
          if(prepared.current.prepared_materialization.admission!==null)throw new Error('preview granted admission');
          const result=(await session.commit()).result;
          const replay=(await session.commit()).result;
          process.stdout.write(JSON.stringify({receipt:result.receipt.command_id,
            replayed:replay.replayed,grants_admission:result.grants_admission}));
          client.close();
        '''
        environment = {**os.environ, 'TOS_TEST_CLIENT': (modules / 'source-command-client.mjs').as_uri(),
            'TOS_TEST_SESSION': (modules / 'source-form-session.mjs').as_uri(),
            'TOS_TEST_ORIGIN': 'http://' + self.server.expected_host, 'TOS_TEST_TOKEN': self.token}
        result = subprocess.run(['node', '--input-type=module', '-e', program],
            env=environment, capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {'receipt': 'test:browser-owner-wire',
            'replayed': True, 'grants_admission': False})
        self.assertEqual(self.fixture.source.read_bytes(), self.fixture.original_source)

    def test_prepare_preview_is_the_new_form_with_context_and_does_not_write(self):
        before = self.fixture.target.read_bytes()
        status, _, prepared = self.request(value={'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare', 'form_id': self.fixture.config['allowed_form_ids'][0],
            'field_id': 'metadata.preferred-name'})
        self.assertEqual(status, 200, prepared)
        preview = prepared['prepared_materialization']
        self.assertEqual(preview['form'], transport.commands._form_ref(prepared['prepared_change']['form']))
        self.assertEqual(preview['subject'], prepared['source'])
        self.assertEqual(preview['display_text'], json.loads(self.fixture.original_source)['preferred_label'])
        self.assertEqual(preview['state'], 'ready')
        self.assertIsNone(preview['admission'])
        self.assertTrue(preview['context'])
        self.assertEqual(self.fixture.target.read_bytes(), before)
        self.assertEqual(self.fixture.source.read_bytes(), self.fixture.original_source)

    def test_owner_failures_and_oversized_receipts_do_not_claim_no_write(self):
        for error, expected in ((transport.JournalConflict('private details'), 409),
                                (RuntimeError('private details'), 500)):
            with patch.object(transport.commands, 'run_local_command', side_effect=error):
                status, _, body = self.request(value={})
                self.assertEqual(status, expected)
                self.assertEqual(body['outcome'], 'unconfirmed')
                self.assertNotIn('private details', json.dumps(body))
        with patch.object(transport.commands, 'run_local_command', return_value={'large': 'x' * 200}), \
                patch.object(transport, 'MAX_RESPONSE_BYTES', 128):
            status, _, body = self.request(value={})
            self.assertEqual(status, 502)
            self.assertEqual(body['outcome'], 'unconfirmed')

    def test_startup_rejects_public_tokens_and_nonloopback_origin(self):
        self.token_file.chmod(0o644)
        with self.assertRaises(PermissionError):
            transport.SourceCommandServer(owner_config=self.fixture.owner,
                token_file=self.token_file, browser_origin=self.origin)
        for origin in ('https://evil.example', 'http://127.0.0.1',
                       'http://127.0.0.1:44257/', 'http://user@localhost:44257'):
            with self.assertRaises(ValueError):
                transport._origin(origin)

    def test_signed_requests_bind_body_time_nonce_and_owner_reply(self):
        raw = b'{"schema_version":"tos_local_source_command_v1","operation":"describe"}'
        auth = self.authorization('POST', '/commands', raw)
        status, headers, result = self.request(raw=raw, headers={'Authorization': auth})
        self.assertEqual(status, 200)
        nonce = auth.split(' ')[1].split(':')[1]
        body = json.dumps(result, ensure_ascii=False, allow_nan=False, separators=(',', ':')).encode()
        self.assertEqual(headers['X-ToS-Response-Signature'], transport._signature(
            self.token, ['tos-response-v1', nonce, status, hashlib.sha256(body).hexdigest()]))
        self.assertNotIn(self.token, auth)
        self.assertEqual(self.request(raw=raw, headers={'Authorization': auth})[0], 409)
        with patch.object(transport.commands, 'run_local_command') as dispatch:
            self.assertEqual(self.request(raw=raw+b' ', headers={
                'Authorization': self.authorization('POST', '/commands', raw)})[0], 401)
            self.assertEqual(self.request(raw=raw, headers={
                'Authorization': self.authorization('POST', '/commands', raw, timestamp=int(time.time())-31)})[0], 401)
            self.assertEqual(self.request(raw=raw, headers={
                'Authorization': self.authorization('GET', '/commands/catalog', raw)})[0], 401)
            dispatch.assert_not_called()

    def test_nonce_capacity_fails_closed_without_unbounded_cache_growth(self):
        with patch.object(transport, 'MAX_RECENT_NONCES', 1):
            self.assertEqual(self.request(value={'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})[0], 200)
            self.assertEqual(self.request(value={})[0], 429)
            self.assertEqual(len(self.server.recent_nonces), 1)

    def test_absolute_read_deadline_releases_trickled_header_and_body(self):
        describe = {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'}
        expected = self.fixture.describe()
        with patch.object(transport, 'REQUEST_READ_DEADLINE_SECONDS', 0.2), \
                patch.object(transport.commands, 'run_local_command',
                              wraps=transport.commands.run_local_command) as dispatch:
            slow_header = socket.create_connection(('127.0.0.1', self.server.server_port), timeout=2)
            try:
                slow_header.sendall((f'POST /commands HTTP/1.1\r\n'
                                     f'Host: {self.server.expected_host}\r\n').encode())
                for byte in b'Content-Length: 2\r\n':
                    try:
                        slow_header.sendall(bytes((byte,)))
                    except OSError:
                        break
                    # Each write stays below the five-second idle timeout; the
                    # absolute request deadline must still close this socket.
                    time.sleep(0.04)
                slow_header.settimeout(1)
                self.assertEqual(slow_header.recv(1), b'')
            finally:
                slow_header.close()
            status, _, body = self.request(value=describe)
            self.assertEqual(status, 200, body)
            self.assertEqual(body, expected)

            slow_body = socket.create_connection(('127.0.0.1', self.server.server_port), timeout=2)
            try:
                raw = b'{}'
                authorization = self.authorization('POST', '/commands', raw)
                slow_body.sendall((f'POST /commands HTTP/1.1\r\n'
                                   f'Host: {self.server.expected_host}\r\n'
                                   f'Origin: {self.origin}\r\n'
                                   f'Authorization: {authorization}\r\n'
                                   'Content-Type: application/json\r\n'
                                   'Content-Length: 2\r\n\r\n').encode() + raw[:1])
                # The body remains one byte short while the sender is active,
                # proving this is an absolute deadline rather than idle EOF.
                time.sleep(0.35)
                slow_body.settimeout(1)
                response = bytearray()
                while True:
                    chunk = slow_body.recv(4096)
                    if not chunk:
                        break
                    response.extend(chunk)
                self.assertTrue(response.startswith(b'HTTP/1.0 400'), response[:80])
            finally:
                slow_body.close()
            status, _, body = self.request(value=describe)
            self.assertEqual(status, 200, body)
            self.assertEqual(body, expected)
            self.assertEqual(dispatch.call_count, 2)


if __name__ == '__main__':
    unittest.main()
