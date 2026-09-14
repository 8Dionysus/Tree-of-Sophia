"""Exercise the actual assembly boundary that excludes old private text."""
import hashlib
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('fragment_builder', Path(__file__).with_name('build-fragments.py'))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)

class RecordingLibraryTests(unittest.TestCase):
    def fixture(self):
        def bi(text): return {'ru': text, 'en': text}
        version = {'paragraphs': ['Complete unit.'], 'textSha256': hashlib.sha256(b'Complete unit.').hexdigest(), 'rights': {'uses': ['local-reading', 'video-display']}, 'sourceUrl': 'https://example.org/new'}
        passage = {'id': 'one', 'title': bi('New edition'), 'status': 'available', 'complete': True, 'versions': {'ru': version, 'en': version}}
        nodes = [{'id': identity, 'kind': 'work' if identity == 'work' else 'fragment', 'parentId': None if identity == 'work' else 'part-3', 'title': bi(identity), 'body': bi('private old body'), 'quote': bi('PRIVATE_QUOTE'), 'exact': {'ru': 'PRIVATE_EXACT'}, 'sourceRefs': [{'ref': '/private/old/path', 'label': 'old'}], 'speaker': bi('old speaker')} for identity in builder.SOURCE_IDS]
        nodes.extend([{'id': 'part-3', 'kind': 'part', 'parentId': 'work', 'title': bi('Part III'), 'exact': {'ru': 'PRIVATE_PARENT_TEXT'}}, {'id': 'unlisted-archival-text', 'exact': {'ru': 'PRIVATE_UNLISTED_TEXT'}}])
        source = {'rootId': 'work', 'nodes': nodes}
        bindings = [{'nodeId': identity, 'passageIds': ['one'], 'context': bi('New selected section')} for identity in builder.SOURCE_IDS]
        return source, [passage], bindings

    def test_assembly_keeps_parent_closure_but_never_copies_private_text_or_paths(self):
        source, passages, bindings = self.fixture()
        _, catalog_bytes, library = builder.assemble(source, passages, bindings)
        import json
        raw = json.dumps(library)
        self.assertNotIn('PRIVATE_', raw)
        self.assertNotIn('/private/', raw)
        self.assertNotIn('private old body', raw)
        self.assertNotIn('unlisted-archival-text', raw)
        self.assertEqual({item['id'] for item in library['nodes']}, set(builder.SOURCE_IDS) | {'part-3'})
        self.assertEqual(library['fragmentCatalog']['sha256'], hashlib.sha256(catalog_bytes).hexdigest())
        self.assertEqual(source['nodes'][0]['exact']['ru'], 'PRIVATE_EXACT')

    def test_missing_complete_unit_or_video_basis_stops_assembly(self):
        source, passages, bindings = self.fixture()
        passages[0]['versions']['en']['rights']['uses'] = ['local-reading']
        with self.assertRaisesRegex(ValueError, 'Video display'):
            builder.assemble(source, passages, bindings)

    def test_original_is_explicit_integrity_checked_and_added_to_source_navigation(self):
        from copy import deepcopy
        source, passages, bindings = self.fixture()
        passage = passages[0]
        translations = deepcopy(passage['versions'])
        original = deepcopy(passage['versions']['en'])
        original['paragraphs'] = ['Original source unit.']
        original['textSha256'] = hashlib.sha256(b'Original source unit.').hexdigest()
        original['sourceUrl'] = 'https://example.org/original'
        passage['versions']['la'] = original
        with self.assertRaisesRegex(ValueError, 'declare its original'):
            builder.assemble(source, passages, bindings)
        passage['originalLanguage'] = 'la'
        catalog, _, library = builder.assemble(source, passages, bindings)
        self.assertEqual(catalog['passages'][0]['versions']['la'], original)
        self.assertEqual({code: passage['versions'][code] for code in ['ru', 'en']}, translations)
        self.assertTrue(any(ref['ref'] == original['sourceUrl'] for node in library['nodes'] for ref in node['sourceRefs']))
        original['paragraphs'].append('Unreviewed extra text.')
        with self.assertRaisesRegex(ValueError, 'Text digest mismatch'):
            builder.assemble(source, passages, bindings)

if __name__ == '__main__': unittest.main()
