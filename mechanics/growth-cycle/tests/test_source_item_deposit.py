"""Durable byte, no-replace, privacy and resource boundaries independent of corpus."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys
import struct
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
for directory in (ROOT / 'scripts', MECHANIC, Path(__file__).parent):
    sys.path.insert(0, str(directory))
import source_item_deposit as deposit
from test_source_item_commands import epub_bytes


class DepositTests(unittest.TestCase):
    def setUp(self):
        fixture = tempfile.TemporaryDirectory()
        control = tempfile.TemporaryDirectory()
        self.addCleanup(fixture.cleanup)
        self.addCleanup(control.cleanup)
        self.root = Path(fixture.name)
        self.control = Path(control.name)
        self.input = self.root / 'input.epub'
        self.input.write_bytes(epub_bytes())
        self.payload = self.root / 'canonical'
        self.payload.mkdir()
        self.metadata = self.root / 'metadata'
        self.metadata.mkdir()
        self.config = {'source_root': str(self.metadata), 'payload_root': str(self.payload),
            'input_path': str(self.input), 'recovery_root': str(self.control),
            'payload_authority_ref': 'test-only:exact-local-copy', 'payload_expires_at': '2099-01-01T00:00:00Z',
            'item_source_path': 'ToS/source-witnesses/works/test/editions/test/items/test/item.json',
            'payload_basename': 'source.epub', 'original_basename': 'upstream.epub',
            'file_id': 'tos.file.synthetic.deposit', 'media_type': 'application/epub+zip',
            'byte_size': self.input.stat().st_size, 'sha256': hashlib.sha256(self.input.read_bytes()).hexdigest()}
        self.observed = deposit.observe(self.config)
        self.request = {'inventory': self.observed['inventory'], 'inventory_limitation': self.observed['limitation']}
        self.identifier = 'sha256:' + '1' * 64
        self.original = self.input.read_bytes()

    def run_deposit(self, **kwargs):
        return deposit.ensure_deposit(self.config, self.request, self.identifier, authorize=lambda: None, **kwargs)

    def test_exact_copy_private_retention_and_replay(self):
        first = self.run_deposit()
        self.assertEqual(self.run_deposit(), first)
        self.assertEqual(self.input.read_bytes(), self.original)
        self.assertEqual(deposit.destination(self.config).read_bytes(), self.original)
        self.assertEqual(list(self.metadata.iterdir()), [])
        receipt = deposit.public_receipt(first)
        self.assertEqual(receipt['observation_interval'], first['observation_interval'])
        interval = receipt['observation_interval']
        self.assertLessEqual(deposit.source._instant(interval['started_at']), deposit.source._instant(interval['ended_at']))
        self.assertLessEqual(deposit.source._instant(interval['ended_at']), deposit.source._instant(receipt['started_at']))
        self.assertNotIn('observation_interval', first['observation'])
        raw = json.dumps(receipt).encode()
        # The private stage is persisted with sorted keys; the selected public
        # carrier must keep identical bytes before and after that round trip.
        self.assertEqual(raw, json.dumps(deposit.public_receipt(deposit.read_stage(self.config, self.identifier))).encode())
        for path in (self.input, self.payload, self.control):
            self.assertNotIn(str(path).encode(), raw)
        self.assertEqual((self.control / self.identifier[7:]).stat().st_mode & 0o777, 0o700)
        self.assertEqual((self.control / self.identifier[7:] / deposit.STAGE_FILE).stat().st_mode & 0o777, 0o600)

    def test_authority_is_rechecked_after_read_only_observation_before_retention(self):
        calls = 0
        def authorize():
            nonlocal calls
            calls += 1
            if calls == 2:
                raise PermissionError('synthetic authority expiry during read-only observation')
        with self.assertRaises(PermissionError):
            deposit.ensure_deposit(self.config, self.request, self.identifier, authorize=authorize)
        self.assertEqual(list(self.control.iterdir()), [])
        self.assertEqual(list(self.payload.iterdir()), [])
        self.assertEqual(self.input.read_bytes(), self.original)

    def test_interrupted_prefix_resumes_without_deleting_original(self):
        original_write = os.write
        count = 0
        def interrupted(fd, raw):
            nonlocal count
            result = original_write(fd, raw)
            count += 1
            if count == 2:
                raise OSError('synthetic interruption after partial write')
            return result
        with patch.object(deposit, 'CHUNK', 16), patch.object(deposit.os, 'write', interrupted):
            with self.assertRaises(OSError):
                self.run_deposit()
        stage = deposit.read_stage(self.config, self.identifier)
        self.assertEqual(stage['state'], 'copying')
        self.assertFalse(deposit.destination(self.config).exists())
        self.run_deposit()
        self.assertEqual(deposit.destination(self.config).read_bytes(), self.original)
        self.assertEqual(self.input.read_bytes(), self.original)

    def test_crash_after_no_replace_publication_resumes_by_bound_inode(self):
        original_save = deposit._save
        def interrupted(config, identifier, stage, **kwargs):
            if stage['state'] == 'deposited':
                raise OSError('synthetic crash before final stage receipt')
            return original_save(config, identifier, stage, **kwargs)
        with patch.object(deposit, '_save', interrupted):
            with self.assertRaises(OSError):
                self.run_deposit()
        self.assertEqual(deposit.read_stage(self.config, self.identifier)['state'], 'copying')
        self.assertEqual(self.run_deposit()['state'], 'deposited')
        self.assertEqual(deposit.destination(self.config).stat().st_nlink, 1)

    def test_equal_foreign_target_is_never_adopted(self):
        target = deposit.destination(self.config)
        target.parent.mkdir(parents=True)
        target.write_bytes(self.original)
        with self.assertRaises(ValueError):
            self.run_deposit()
        self.assertIsNone(deposit.read_stage(self.config, self.identifier))
        self.assertEqual(target.read_bytes(), self.original)

    def test_replaced_deposit_and_input_symlink_fail_closed(self):
        stage = self.run_deposit()
        target = deposit.destination(self.config)
        replacement = target.with_name('replacement')
        replacement.write_bytes(self.original)
        replacement.replace(target)
        with self.assertRaises(ValueError):
            deposit.verify_deposit(self.config, stage)
        self.input.rename(self.input.with_name('original'))
        self.input.symlink_to(self.input.with_name('original'))
        with self.assertRaises((OSError, ValueError)):
            deposit.observe(self.config)

    def test_zip_budget_blocks_enumerator_and_retains_bytes_without_claiming_inventory(self):
        with patch.object(deposit, 'MAX_ZIP_MEMBERS', 2), patch.object(deposit, 'build_file_inventory') as enumerator:
            observed = deposit.observe(self.config)
            self.assertIsNone(observed['inventory'])
            enumerator.assert_not_called()
            self.request = {'inventory': None, 'inventory_limitation': observed['limitation']}
            stage = self.run_deposit()
        self.assertEqual(stage['state'], 'deposited')
        self.assertIsNotNone(stage['observation']['limitation'])
        self.assertEqual(deposit.destination(self.config).read_bytes(), self.original)

    def test_private_root_overlap_and_relaxed_companion_permissions_rejected(self):
        for path in (self.metadata, self.payload, self.root, self.input):
            with self.subTest(path=path):
                with self.assertRaises((PermissionError, OSError)):
                    deposit.validate_config({**self.config, 'recovery_root': str(path)})
        self.run_deposit()
        companion = self.control / self.identifier[7:]
        companion.chmod(0o755)
        with self.assertRaises(PermissionError):
            deposit.read_stage(self.config, self.identifier)
        companion.chmod(0o700)
        (companion / deposit.STAGE_FILE).chmod(0o644)
        with self.assertRaises(PermissionError):
            deposit.read_stage(self.config, self.identifier)

    def test_zip64_locator_cannot_override_small_eocd_before_parser(self):
        for comment_size in (0, 65535):
            with self.subTest(comment_size=comment_size):
                locator = struct.pack('<4sIQI', b'PK\x06\x07', 0, 0, 1)
                end = struct.pack('<4s4H2IH', b'PK\x05\x06', 0, 0, 0, 0, 0, 0, comment_size)
                raw = b'prefix' + locator + end + b'x' * comment_size
                self.input.write_bytes(raw)
                config = {**self.config, 'byte_size': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
                with patch.object(deposit.zipfile, 'ZipFile') as parser:
                    observed = deposit.observe(config)
                    self.assertIsNone(observed['inventory'])
                    self.assertEqual(observed['limitation'], 'inventory-unavailable:InventoryBuildError')
                    parser.assert_not_called()

    def test_exotic_compression_is_rejected_before_decoder_creation(self):
        for compression in (zipfile.ZIP_LZMA, zipfile.ZIP_BZIP2):
            with self.subTest(compression=compression):
                with zipfile.ZipFile(self.input, 'w', compression=compression) as archive:
                    archive.writestr('member', b'x')
                config = {**self.config, 'byte_size': self.input.stat().st_size,
                          'sha256': hashlib.sha256(self.input.read_bytes()).hexdigest()}
                with patch.object(deposit.zipfile, '_get_decompressor') as decoder:
                    observed = deposit.observe(config)
                    self.assertIsNone(observed['inventory'])
                    self.assertEqual(observed['limitation'], 'inventory-unavailable:InventoryBuildError')
                    decoder.assert_not_called()

    def test_rollback_preserves_original_and_completed_deposit(self):
        self.run_deposit()
        result = deposit.rollback_retained(self.config, self.identifier)
        self.assertEqual(result['state'], 'rolled-back-retained')
        self.assertEqual(self.input.read_bytes(), self.original)
        self.assertEqual(deposit.destination(self.config).read_bytes(), self.original)
        with self.assertRaises(ValueError):
            self.run_deposit()


if __name__ == '__main__':
    unittest.main()
