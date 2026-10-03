"""Maintained native fixture consumer; no external provider is contacted."""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import unittest
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/"scripts"))
import acquisition_batch as native
from tests import test_acquisition_batch as oracle_fixture

@unittest.skipUnless(os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN") or shutil.which("tos-native-owner-command"), "native owner product not selected")
class NativeAcquisitionTests(unittest.TestCase):
    def setUp(self):
        self.fixture=oracle_fixture.AcquisitionBatchTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)
    def acquire(self,fetcher,sha):
        f=self.fixture
        return native.acquire_batch(manifest_path=f.manifest_path,metadata_root=f.metadata,output_root=f.output,expected_manifest_sha256=sha,fetcher=fetcher,max_attempts=1)
    def test_real_consumer_failure_resume_and_immutable_custody(self):
        f=self.fixture
        bodies,sha=f._write_manifest(count=2)
        urls=list(bodies)
        calls=[]
        def fetch(payload):
            url=payload['file_ref'];calls.append(url)
            if url==urls[0]: raise native.SourceFetchError("isolated fixture failure")
            return bodies[url]
        first=self.acquire(fetch,sha)
        self.assertEqual('partially-acquired-not-admitted',first['status'])
        oracle_output=f.root/'oracle-prepared'
        oracle_fixture.acquisition.prepare_batch(manifest_path=f.manifest_path,metadata_root=f.metadata,output_root=oracle_output,expected_manifest_sha256=sha)
        preparation=json.loads((f.output/'receipts/preparation.json').read_bytes())
        for ref in ('manifest.json','receipts/preparation.json',preparation['provenance_delta_ref']):
            self.assertEqual((oracle_output/ref).read_bytes(),(f.output/ref).read_bytes(),ref)
        calls.clear()
        result=self.acquire(lambda p:(calls.append(p['file_ref']) or bodies[p['file_ref']]),sha)
        self.assertEqual('acquired-not-admitted',result['status'])
        self.assertEqual([urls[0]],calls)
        self.assertEqual('verified',native.verify_local(output_root=f.output)['status'])
        cli=subprocess.run([sys.executable,str(ROOT/'scripts/acquisition_batch.py'),'verify-local','--output-root',str(f.output)],capture_output=True,text=True,timeout=30)
        self.assertEqual(0,cli.returncode,cli.stderr)
        self.assertEqual('verified',json.loads(cli.stdout)['status'])
        handoff=json.loads((f.output/result['handoff_ref']).read_bytes())
        self.assertEqual('not-admitted',handoff['admission_status'])
        self.assertEqual('not-published',handoff['publication_status'])
        for p in (f.output/'payload').rglob('*'):
            if p.is_file(): self.assertEqual(0o444,p.stat().st_mode&0o7777)
        for name in ('','source','payload','receipts'):
            self.assertEqual(0o700,(f.output/name).stat().st_mode&0o7777)
        # A byte-identical writable destination remains a custody conflict.
        payload=next(p for p in (f.output/'payload').rglob('*') if p.is_file())
        payload.chmod(0o644)
        calls.clear()
        conflict=self.acquire(lambda p:(calls.append(p['file_ref']) or bodies[p['file_ref']]),sha)
        self.assertEqual('partially-acquired-not-admitted',conflict['status'])
        self.assertEqual([],calls)
    def test_control_selection_and_private_roots_are_rechecked(self):
        f=self.fixture;bodies,sha=f._write_manifest(count=1)
        calls=[]
        with self.assertRaises(native.AcquisitionBatchError):
            native.acquire_batch(manifest_path=f.manifest_path,metadata_root=f.metadata,output_root=f.output,expected_manifest_sha256=sha,fetcher=lambda p:(calls.append(p['file_ref']) or bodies[p['file_ref']]),max_attempts=-1)
        self.assertEqual([],calls)
        self.assertFalse(f.output.exists())
        native.prepare_batch(manifest_path=f.manifest_path,metadata_root=f.metadata,output_root=f.output,expected_manifest_sha256=sha)
        manifest=json.loads(f.manifest_path.read_bytes())
        ref=manifest['selection'][0]['rights']['ref']
        selected=f.output/'source'/ref
        original=selected.read_bytes();selected.write_bytes(original+b' ')
        calls=[]
        with self.assertRaises(native.AcquisitionBatchError):
            self.acquire(lambda p:(calls.append(p['file_ref']) or bodies[p['file_ref']]),sha)
        self.assertEqual([],calls)
        selected.write_bytes(original)
        (f.output/'payload').chmod(0o755)
        with self.assertRaises(native.AcquisitionBatchError):
            self.acquire(lambda p:(calls.append(p['file_ref']) or bodies[p['file_ref']]),sha)
        self.assertEqual([],calls)
    def test_interrupted_preparation_recovers_only_manifest_owned_shape(self):
        f=self.fixture;bodies,sha=f._write_manifest(count=1)
        native.prepare_batch(manifest_path=f.manifest_path,metadata_root=f.metadata,output_root=f.output,expected_manifest_sha256=sha)
        (f.output/'receipts/preparation.json').unlink()
        self.assertEqual('acquired-not-admitted',self.acquire(lambda p:bodies[p['file_ref']],sha)['status'])
        # Recovery must preserve evidence and refuse deletion after custody.
        (f.output/'receipts/preparation.json').unlink()
        calls=[]
        with self.assertRaises(native.AcquisitionBatchError):
            self.acquire(lambda p:(calls.append(p['file_ref']) or bodies[p['file_ref']]),sha)
        self.assertEqual([],calls)
        self.assertTrue(any(p.is_file() for p in (f.output/'payload').rglob('*')))
    def test_no_follow_and_single_link_payload_check(self):
        f=self.fixture;bodies,sha=f._write_manifest(count=1)
        self.acquire(lambda p:bodies[p['file_ref']],sha)
        payload=next(p for p in (f.output/'payload').rglob('*') if p.is_file())
        outside=f.root/'hardlink';os.link(payload,outside)
        self.assertEqual('incomplete',native.verify_local(output_root=f.output)['status'])
        outside.unlink()
        body=payload.read_bytes();payload.unlink();outside.write_bytes(body);outside.chmod(0o444);payload.symlink_to(outside)
        self.assertEqual('incomplete',native.verify_local(output_root=f.output)['status'])

    def test_exact_manifest_bytes_survive_wire_expansion(self):
        f=self.fixture;f._write_manifest(count=1)
        raw=f.manifest_path.read_bytes()+b' '*(7*1024*1024)
        f.manifest_path.write_bytes(raw)
        context=native.load_manifest(f.manifest_path,expected_sha256=hashlib.sha256(raw).hexdigest())
        self.assertEqual(raw,context.raw_manifest)
        self.assertEqual(json.loads(raw),context.manifest)

    def test_selected_claim_profile_public_api_before_fetch(self):
        # Reuse the maintained public-API case and its real metadata fixture.
        # Its reads and profile checks execute in Rust; no FD internals are mocked.
        with patch.object(oracle_fixture, "acquisition", native):
            self.fixture.test_selected_source_claim_carrier_uses_its_declared_profile_before_fetch()

    def test_default_native_http_transport_isolated_fixture(self):
        from http.server import BaseHTTPRequestHandler, HTTPServer
        import threading
        f=self.fixture;bodies,sha=f._write_manifest(count=2)
        manifest=json.loads(f.manifest_path.read_bytes())
        requests=[]
        class Provider(BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                if self.path=="/failure":
                    self.send_error(503);return
                body=bodies[self.path[1:]]
                self.send_response(200);self.send_header("Content-Length",str(len(body)));self.end_headers();self.wfile.write(body)
            def log_message(self,*args): pass
        server=HTTPServer(("127.0.0.1",0),Provider)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            for index,selection in enumerate(manifest['selection']):
                payload=selection['payload_files'][0]
                path='failure' if index==0 else payload['file_ref']
                payload['provider_url']=f"http://127.0.0.1:{server.server_port}/{path}"
            f.manifest_path.write_bytes((json.dumps(manifest)+"\n").encode())
            sha=hashlib.sha256(f.manifest_path.read_bytes()).hexdigest()
            result=native.acquire_batch(manifest_path=f.manifest_path,metadata_root=f.metadata,output_root=f.output,expected_manifest_sha256=sha,max_attempts=1)
            self.assertEqual('partially-acquired-not-admitted',result['status'])
            self.assertEqual(2,len(requests))
            self.assertEqual('not-admitted',result['admission_status'])
        finally:
            server.shutdown();server.server_close();thread.join(timeout=2)
