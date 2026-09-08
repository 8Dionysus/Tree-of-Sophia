"""Coverage must distinguish selected versions, possible matches and current custody."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from build_source_registry_coverage import assess_target, classify_record


class RegistryCoverageTests(unittest.TestCase):
    def fixture(self, root):
        def write(ref, obj):
            path = root / ref
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(obj) + '\n')
        ids = {kind: 'tos.' + kind + '.fixture' for kind in ('work', 'expression', 'edition', 'item')}
        paths = {kind: 'source/' + kind + '.json' for kind in ids}
        paths['item_root'] = 'source/item'
        target = {'title': 'A selected version', 'ids': ids, 'paths': paths, 'files': [{'basename': 'original.xml', 'byte_size': 4}]}
        for kind in ids:
            record = {'record_id': ids[kind]}
            if kind == 'expression': record['work_ref'] = ids['work']
            if kind == 'edition': record['embodies_expression_refs'] = [ids['expression']]
            write(paths[kind], record)
        digest = hashlib.sha256(b'exact'[:4]).hexdigest()
        file = {'file_id': 'tos.file.sha256.' + digest, 'original_basename': 'original.xml', 'relative_path': 'payload/original.xml', 'byte_size': 4, 'sha256': digest}
        manifest = {'item_id': ids['item'], 'embodiment_ref': ids['edition'], 'payload_files': [file], 'provenance_ref': 'source/item/provenance.jsonl', 'acquisition_event_ref': 'event:acquisition'}
        write('source/item/item.manifest.json', manifest)
        write('source/item/provenance.jsonl', {'event_id': 'event:acquisition', 'event_type': 'acquisition', 'status': 'completed', 'outputs': [{'ref': file['file_id'], 'sha256': digest}]})
        plants = {ids['work']: [('branch/source-planting.json', {'source_witness': {'record_ref': paths['work']}, 'status': 'source_witness_planted'})]}
        return target, plants, write

    def test_recorded_acquisition_is_portable_and_local_existence_is_separate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target, plants, _ = self.fixture(root)
            portable = assess_target(root, target, plants)
            self.assertEqual(portable['status'], 'selected_version_planted')
            self.assertNotIn('local_now', portable)
            live = assess_target(root, target, plants, verify_local=True)
            self.assertEqual(live['status'], 'selected_version_planted')
            self.assertEqual(live['local_now']['files'][0]['state'], 'missing_in_this_checkout')
            payload = root / 'source/item/payload/original.xml'; payload.parent.mkdir()
            payload.write_bytes(b'exac')
            self.assertEqual(assess_target(root, target, plants, verify_local=True)['local_now']['state'], 'verified')
            payload.write_bytes(b'bad!')
            self.assertEqual(assess_target(root, target, plants, verify_local=True)['local_now']['files'][0]['state'], 'fixity_mismatch')

    def test_preparation_and_acquisition_do_not_establish_a_branch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target, _, _ = self.fixture(root)
            self.assertEqual(assess_target(root, target, {})['status'], 'acquired_version_needs_branch')
            (root / 'source/item/item.manifest.json').unlink()
            self.assertEqual(assess_target(root, target, {})['status'], 'prepared_version_not_installed')

    def test_wrong_identity_or_unbound_acquisition_output_stops_projection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target, plants, write = self.fixture(root)
            original = json.loads((root / target['paths']['expression']).read_text())
            write(target['paths']['expression'], {**original, 'work_ref': 'tos.work.other'})
            with self.assertRaisesRegex(ValueError, 'another work'): assess_target(root, target, plants)
            write(target['paths']['expression'], original)
            write('source/item/provenance.jsonl', {'event_id': 'event:acquisition', 'event_type': 'acquisition', 'status': 'completed', 'outputs': []})
            with self.assertRaisesRegex(ValueError, 'output digest'): assess_target(root, target, plants)

    def test_possible_match_never_promotes_and_one_version_never_exhausts_a_lead(self):
        record = {'record_id': 'registry:corpus', 'source_record_id': 'R001', 'corpus_id': 'fixture', 'document_id': 'D1', 'kind': 'registry', 'owner_matches': [{'owner_ref': 'work.json'}]}
        possible = classify_record(record, [])
        self.assertEqual(possible['status'], 'possible_owner_correspondence')
        self.assertFalse(possible['lead_scope_exhausted'])
        linked = classify_record(record, [{'status': 'selected_version_planted', 'work_id': 'one-of-many'}])
        self.assertEqual(linked['status'], 'selected_versions_planted')
        self.assertFalse(linked['lead_scope_exhausted'])
        record['owner_matches'] = []
        self.assertEqual(classify_record(record, [])['status'], 'not_yet_reconciled')

if __name__ == '__main__': unittest.main()
