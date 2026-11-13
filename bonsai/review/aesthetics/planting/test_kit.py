import unittest
from unittest.mock import patch

import kit


class SecondReaderScanTest(unittest.TestCase):
    def test_same_page_number_in_another_volume_does_not_count(self):
        reading = {'reader': {'id': 'maker'}, 'conceptions': [], 'links': [], 'left': []}
        review = {'reader': {'id': 'second', 'model': 'model', 'date': '2026-11-10'},
                  'images': [{'loc': 'p', 'n': 5, 'reads': 'as the packet'}], 'accept': True}
        ev = {'scan': {'item': 'volume-a', 'image_offset': 0},
              'scans': {'volume-a': {'item': 'volume-a', 'image_offset': 0},
                        'volume-b': {'item': 'volume-b', 'image_offset': 0}},
              'places': {'p': {'scan': 'volume-b', 'method': 'image', 'from_scan': True, 'leaves': [5]}},
              'images': [{'item': 'volume-a', 'file': 'volume-a-n5.jpg', 'n': 5, 'by': 'second'}]}
        with patch.object(kit, 'part', return_value=review):
            problems = kit.second_gate('.', reading, ev, {'p': {}})
            self.assertTrue(any('every page' in problem for problem in problems))
            self.assertTrue(any('loc, n' in problem for problem in problems))
            ev['images'].append({'item': 'volume-b', 'file': 'volume-b-n5.jpg', 'n': 5, 'by': 'second'})
            self.assertEqual(kit.second_gate('.', reading, ev, {'p': {}}), [])


class LineFaultTest(unittest.TestCase):
    def test_a_planting_that_died_is_the_fault_of_the_line(self):
        died = ("node:fs:562\n  return binding.open(\n                 ^\n\n"
                "Error: ENOENT: no such file or directory, open 'documents/6cbc.json'\n"
                "    at Object.openSync (node:fs:562:18)\n    at readFileSync (node:fs:446:35)\n\nNode.js v22.11.0\n")
        self.assertEqual(kit.thrown(died), "Error: ENOENT: no such file or directory, open 'documents/6cbc.json'")

    def test_a_finding_of_the_planting_is_the_packet_s(self):
        self.assertIsNone(kit.thrown('B:B-I043: the formula is not found in the place\n'))


class EyeAloneTest(unittest.TestCase):
    def test_spelling_of_the_print_changes_no_letters(self):
        self.assertEqual(kit.changed('Per affectum intelligo corporis affectiones quibus coercetur', 'Per Affectum intelligo Corporis affectiones, quibus coërcetur'), [])
        self.assertEqual(kit.changed('Über adæquata', 'Ueber adaequata'), [])

    def test_a_changed_letter_is_named(self):
        self.assertEqual(kit.changed('adaequata possimus esse causa', 'adaequata possumus esse causa'), [('possimus', 'possumus')])

    def test_a_correction_in_a_place_read_from_the_image_rests_on_the_eye(self):
        places = {'work': 'w', 'corrections': [{'place': 'w p1', 'from': 'possimus', 'to': 'possumus'}, {'place': 'w p2', 'from': 'bei', 'to': 'bey'}]}
        ev = {'scan': {'item': 's'}, 'places': {'p1': {'method': 'image'}, 'p2': {'method': 'ocr', 'differences': []}}}
        self.assertEqual([loc for loc, _ in kit.by_eye(places, ev)], ['p1'])
        ev['places']['p2']['differences'] = [{'transcription': 'bey', 'ocr': 'bcy'}]
        self.assertEqual([loc for loc, _ in kit.by_eye(places, ev)], ['p1', 'p2'])

    def test_s_and_f_in_a_long_s_print_rest_on_the_eye(self):
        places = {'work': 'w', 'corrections': [{'place': 'w p1', 'from': 'seinen', 'to': 'feinen'}]}
        ev = {'scan': {'item': 's', 'long_s': True}, 'places': {'p1': {'method': 'ocr', 'differences': []}}}
        self.assertEqual([loc for loc, _ in kit.by_eye(places, ev)], ['p1'])


class WholeSentencesTest(unittest.TestCase):
    def test_a_quoted_sentence_is_a_whole_sentence(self):
        self.assertEqual(kit.sentences('‘Now, here, you see, it takes all the running you can do, to keep in the same place.’'), [])
        self.assertEqual(kit.sentences('„Das ist das Numinose.“'), [])
        self.assertEqual(kit.sentences('«Tout est dit.»'), [])

    def test_a_fragment_is_still_named(self):
        self.assertTrue(any('capital' in problem for problem in kit.sentences('‘and so on.’')))
        self.assertTrue(any('close a sentence' in problem for problem in kit.sentences('It takes all the running you can do’')))

    def test_a_place_may_run_to_2400_characters(self):
        self.assertEqual(kit.sentences('A' + 'a' * 2141 + '.'), [])
        self.assertTrue(kit.sentences('A' + 'a' * 2400 + '.'))


class QuotedPlaceTest(unittest.TestCase):
    def test_a_place_keeps_the_opening_mark_of_the_transcription(self):
        source = 'The Queen said this. ‘Now, here, you see, it takes all the running you can do, to keep in the same place.’ Alice looked.'
        self.assertIsNone(kit.in_transcription('‘Now, here, you see, it takes all the running you can do, to keep in the same place.’', source))
        source = 'Er schreibt: „Das ist das Numinose.“ Dann folgt mehr.'
        self.assertIsNone(kit.in_transcription('„Das ist das Numinose.“', source))

    def test_a_mark_the_transcription_lacks_is_still_named(self):
        source = 'The Queen said this. Now, here, you see, it takes all the running you can do, to keep in the same place. Alice looked.'
        self.assertIsNotNone(kit.in_transcription('‘Now, here, you see, it takes all the running you can do, to keep in the same place.’', source))


if __name__ == '__main__':
    unittest.main()

class JurisdictionDatesTest(unittest.TestCase):
    def test_boundaries_are_relative_to_the_running_year(self):
        self.assertEqual(kit.public_domain_by_dates([{'died': 1951}], 1930, 2026),
                         {'russia': True, 'usa': True, 'eu': True})
        self.assertEqual(kit.public_domain_by_dates([{'died': 1952}], 1931, 2026),
                         {'russia': False, 'usa': False, 'eu': True})
        self.assertTrue(kit.public_domain_by_dates([{'died': 1955, 'wartime': False}], 1930, 2026)['russia'])
        self.assertFalse(kit.public_domain_by_dates([{'died': 1956, 'wartime': False}], 1930, 2026)['eu'])
        self.assertTrue(all(kit.public_domain_by_dates([{'died': 1952}], 1931, 2027).values()))

    def test_every_hand_and_rehabilitation_must_be_counted(self):
        people = [{'died': 1900}, {'died': 1956}]
        self.assertFalse(kit.public_domain_by_dates(people, 1900, 2026)['eu'])
        self.assertFalse(kit.public_domain_by_dates([{'died': 1930, 'rehabilitated': 1990}], 1900, 2026)['russia'])
        self.assertFalse(kit.public_domain_by_dates([{'died': None}], 1900, 2026)['russia'])
        self.assertFalse(kit.public_domain_by_dates([], None, 2026)['usa'])

class RussianSpellingTest(unittest.TestCase):
    def test_real_old_spelling_and_original_wording(self):
        self.assertEqual(kit.modern_russian('Ея бѣлый свѣтъ и безсильныя силы.'), 'Её белый свет и бессильные силы.')
        self.assertEqual(kit.modern_russian('Історія великаго народа.'), 'История великого народа.')
        self.assertEqual(kit.modern_russian('Ѵ'), 'И')
        self.assertEqual(kit.modern_russian('История и свет.'), 'История и свет.')


class QuotationEvidenceTest(unittest.TestCase):
    def test_second_reader_must_read_same_image(self):
        digest='a'*64
        proof={'source':'https://example.org/page','page':'12','lines':'4–5','sha256':digest}
        ev={'images':[{'sha256':digest,'by':'maker'}]}
        self.assertEqual(kit.quotation_problems('p', 'A whole sentence.', proof, ev), [])
        self.assertTrue(kit.quotation_problems('p', 'A whole sentence.', proof, ev, second=True))
        ev['images'].append({'sha256':digest,'by':'second'})
        self.assertEqual(kit.quotation_problems('p', 'A whole sentence.', proof, ev, second=True), [])
        self.assertTrue(kit.quotation_problems('p', 'A. B. C. D.', proof, ev))

class TranslationWitnessTest(unittest.TestCase):
    def test_source_page_and_independent_second_reader_are_required(self):
        import hashlib
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir()
            cache=Path(tmp)/'cache'; cache.mkdir()
            folder=cache/'work'/'witnesses'/'en';folder.mkdir(parents=True)
            source=b'A whole sentence.';image=b'page image'
            (folder/'text.txt').write_bytes(source);(folder/'page.jpg').write_bytes(image);(folder/'second.jpg').write_bytes(image)
            digest=hashlib.sha256(image).hexdigest()
            people=[{'role':'author','name':{'en':'Author'},'qid':'Q1','died':1800,'source':'https://www.wikidata.org/wiki/Q1'},
                    {'role':'translator','name':{'en':'Translator'},'qid':'Q2','died':1800,'source':'https://www.wikidata.org/wiki/Q2'}]
            import json
            for person in people:
                qid=person['qid']; filename=f'wikidata-{qid}.json'
                entity={'entities':{qid:{'claims':{'P570':[{'mainsnak':{'datavalue':{'value':{'time':'+1800-01-01T00:00:00Z'}}}}]}}}}
                raw=json.dumps(entity).encode();(folder/filename).write_bytes(raw)
                person['evidence']={'file':filename,'sha256':hashlib.sha256(raw).hexdigest(),'url':f'https://www.wikidata.org/wiki/Special:EntityData/{qid}.json'}
            reference=b'<html><body>Translator (1800)</body></html>'; (folder/'translator.html').write_bytes(reference)
            people[1].pop('qid');people[1]['source']='https://en.wikipedia.org/wiki/Translator'
            people[1]['evidence']={'kind':'reference','file':'translator.html','sha256':hashlib.sha256(reference).hexdigest(),'url':people[1]['source']}
            rights={key:{'basis':'public-domain','law':'Public domain by the applicable law.'} for key in kit.JURISDICTIONS}
            data={'language':'en','witness':{'maker':'maker','edition':'Book','attribution':'Author','licence':'Public domain','url':'https://example.org/book','year':1900,'persons':people,'rights':rights},
                  'sources':[{'file':'text.txt','url':'https://example.org/text','sha256':hashlib.sha256(source).hexdigest()}],
                  'images':[{'file':'page.jpg','url':'https://example.org/page','sha256':digest,'by':'maker'}, {'file':'second.jpg','url':'https://example.org/page','sha256':digest,'by':'second'}],
                  'places':{'p':{'text':'A whole sentence.','source':'text.txt','page':'1','url':'https://example.org/page','image_sha256':digest,'alignment':'Corresponds to the complete original sentence.'}},
                  'review':{'reader':'reader','model':'gpt-6.1-terra','date':'2026-11-12','places':{'p':{'reads':'as the packet','image_sha256':digest,'alignment':'This sentence matches the complete original.'}}}}
            with patch.object(kit,'CACHE',str(cache)), patch.object(kit,'world',return_value={'fragments':{'places':{'work':{'p':{}}}}}):
                self.assertEqual(kit.witness_problems(str(path),'en',data,second=True),[])
                source=b'A whole sentence;';(folder/'text.txt').write_bytes(source)
                data['sources'][0]['sha256']=hashlib.sha256(source).hexdigest()
                data['places']['p']['corrections']=[{'from':';','to':'.','offset':len('A whole sentence'),'reason':'The printed page ends this sentence with a full stop.','url':'https://example.org/page','page':'1','image_sha256':digest}]
                self.assertEqual(kit.witness_problems(str(path),'en',data,second=True),[])
                data['places']['p']['corrections'][0]['image_sha256']='f'*64
                self.assertTrue(any('correction needs' in item for item in kit.witness_problems(str(path),'en',data,second=True)))
                data['places']['p']['corrections'][0]['image_sha256']=digest
                data['review']['reader']='maker'
                self.assertTrue(any('different second reader' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['review']['reader']='reader';(folder/'text.txt').write_bytes(b'Other words.')
                self.assertTrue(any('changed' in p for p in kit.witness_problems(str(path),'en',data,second=True)))

class LocatorEvidenceTest(unittest.TestCase):
    def test_locator_needs_a_documented_search_and_a_read_book(self):
        loc='Section p1'
        proof={'url':'https://example.org/book','reference':'Book, p. 1','maker_read':'The reader examined this copy in a library.',
               'searches':[{'site':site,'url':'https://example.org/search','outcome':'No readable copy was found.'} for site in kit.LOCATOR_SEARCHES]}
        ev={'places':{loc:{'locator':proof}}};place={'url':proof['url']}
        self.assertEqual(kit.locator_problems(loc,place,ev),[])
        proof['searches'].pop()
        self.assertTrue(any('search outcomes missing' in item for item in kit.locator_problems(loc,place,ev)))

class RussianPageComparisonTest(unittest.TestCase):
    def test_ocr_and_transcription_are_compared_in_modern_spelling(self):
        o={'long_s':False,'words':['Ея','бѣлый','свѣтъ','.'], 'at':[0,4,11,16], 'starts':[0], 'printed':{0:'1'}}
        result=kit.against_ocr('Её белый свет.',o,modern_ru=True)
        self.assertEqual(result['share'],1.0)
        self.assertEqual(result['differences'],[])
        self.assertIsNone(kit.in_transcription('Её белый свет.',kit.modern_russian('Ея бѣлый свѣтъ.')))

class ExtensionScopeTest(unittest.TestCase):
    def test_extension_has_a_separate_cache_and_only_the_chosen_work(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            parent=Path(tmp)/'work';ext=parent/'extensions'/'first';ext.mkdir(parents=True)
            (ext/'scope.json').write_text('{"keys":["A:1"]}',encoding='utf8')
            task={'readings':[{'key':'A:1'},{'key':'A:2'}],'links':[{'key':'L:1'}]}
            self.assertEqual(kit.work_of(str(ext)),'work')
            self.assertEqual(kit.packet_selector(str(ext)),'work/extensions/first')
            scoped=kit.scoped_task(str(ext),task)
            self.assertEqual([item['key'] for item in scoped['readings']],['A:1'])
            self.assertEqual(scoped['links'],[])
            self.assertEqual(len(task['readings']),2)

class LawfulSourceTest(unittest.TestCase):
    def test_named_shadow_libraries_are_not_accepted(self):
        self.assertTrue(kit.lawful_url('https://www.wikidata.org/wiki/Q1'))
        for url in ('https://libgen.rs/book','https://annas-archive.org/book','https://dokumen.pub/book','https://x.z-lib.org/book','http://example.org/book'):
            self.assertFalse(kit.lawful_url(url),url)

class ExtensionPlantingTest(unittest.TestCase):
    def test_extension_planting_is_reversible_and_idempotent(self):
        import copy
        import json
        import os
        import shutil
        import subprocess
        import tempfile
        from pathlib import Path
        root=Path(kit.ROOT)
        work='friedrich-nietzsche.die-geburt-der-tragoedie-aus-dem-geiste'
        source=Path(kit.PACKETS)/work
        with tempfile.TemporaryDirectory() as tmp:
            target=Path(tmp);review=target/'bonsai/review/aesthetics';review.mkdir(parents=True)
            (target/'bonsai/lib').symlink_to(root/'bonsai/lib',target_is_directory=True)
            for name in ('layer','inputs','readings'):(review/name).symlink_to(Path(kit.REVIEW)/name,target_is_directory=True)
            (review/'formula-span.mjs').symlink_to(Path(kit.REVIEW)/'formula-span.mjs')
            shutil.copy2(Path(kit.REVIEW)/'plant.mjs',review/'plant.mjs')
            shutil.copytree(root/'bonsai/aesthetics',target/'bonsai/aesthetics')
            (target/'ToS/source-witnesses').mkdir(parents=True)
            (target/'ToS/source-witnesses/works').symlink_to(root/'ToS/source-witnesses/works',target_is_directory=True)
            (target/'access/web/public').mkdir(parents=True)
            (target/'access/web/public/public-corpus').symlink_to(root/'access/web/public/public-corpus',target_is_directory=True)
            packets=review/'planting/packets';packets.mkdir(parents=True)
            for child in Path(kit.PACKETS).iterdir():
                if child.name!=work:(packets/child.name).symlink_to(child,target_is_directory=True)
            parent=packets/work;parent.mkdir()
            for name in ('places.json','reading.json','evidence.json','review.json','accepted.json'):(parent/name).symlink_to(source/name)
            ext=parent/'extensions'/'test';ext.mkdir(parents=True)
            reading=json.loads((source/'reading.json').read_text());reading['reader']['id']='test-extension-reader'
            conception=copy.deepcopy(reading['conceptions'][0]);conception['id']=work+':test-extension';conception['formula']=[{'place':'Test p999'}]
            reading['conceptions']=[conception];reading['links']=[];reading['left']=[]
            base=json.loads((source/'places.json').read_text());place=copy.deepcopy(next(iter(base['places'].values())))
            extension_places={'work':work,'places':{'Test p999':place},'corrections':[],'checked_on_the_page_image':['test page']}
            (ext/'reading.json').write_text(json.dumps(reading,ensure_ascii=False))
            (ext/'places.json').write_text(json.dumps(extension_places,ensure_ascii=False))
            (ext/'review.json').write_text(json.dumps({'reader':{'id':'test-second','date':'2026-11-12','model':'test'}}))
            (ext/'accepted.json').write_text('{}')
            command=['node',str(review/'plant.mjs')]
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertIn('Test p999',projected['places'][work])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            (ext/'accepted.json').unlink()
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertNotIn('Test p999',projected['places'][work])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            import hashlib
            witness_dir=parent/'witnesses';witness_dir.mkdir()
            loc=next(iter(base['places']))
            rights={country:{'basis':'public-domain','law':'Public domain according to the applicable law.'} for country in kit.JURISDICTIONS}
            translated={'language':'ru','witness':{'edition':'Test edition','attribution':'Test author','url':'https://example.org','rights':rights},
                        'places':{loc:{'text':'Русское предложение.','url':'https://example.org/page','page':'1','corrections':[{'from':'Прежнее','to':'Русское','reason':'The source page reads the corrected form.'}]}}}
            raw=json.dumps(translated,ensure_ascii=False).encode();(witness_dir/'ru.json').write_bytes(raw)
            (witness_dir/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            (witness_dir/'ru.json').write_bytes(raw+b' ')
            changed=subprocess.run(command,capture_output=True,text=True)
            self.assertNotEqual(changed.returncode,0)
            self.assertIn('changed since review',changed.stderr)
            (witness_dir/'ru.json').write_bytes(raw)
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertEqual(projected['places'][work][loc]['texts']['ru'],'Русское предложение.')
            self.assertEqual(projected['provenance']['translation_packets'][work]['ru']['corrections'][0]['loc'],loc)
            self.assertFalse(any(item['language']=='ru' for item in projected['works'][work].get('unsearched',[])))
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            (witness_dir/'ru.json.accepted').unlink()
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertNotIn('ru',projected['places'][work][loc]['texts'])
            self.assertTrue(any(item['language']=='ru' for item in projected['works'][work].get('unsearched',[])))
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            manual='immanuel-kant.kritik-der-urteilskraft'
            manual_dir=packets/manual/'witnesses';manual_dir.mkdir(parents=True)
            manual_loc=next(iter(projected['places'][manual]))
            translated['places']={manual_loc:{'text':'Русское предложение.','url':'https://example.org/page','page':'1'}}
            raw=json.dumps(translated,ensure_ascii=False).encode();(manual_dir/'ru.json').write_bytes(raw)
            (manual_dir/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertEqual(projected['places'][manual][manual_loc]['texts']['ru'],'Русское предложение.')
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            (manual_dir/'ru.json.accepted').unlink()
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertNotIn('ru',projected['places'][manual][manual_loc]['texts'])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)

class CandidatePrivacyTest(unittest.TestCase):
    def test_unreviewed_research_text_stays_in_local_cache(self):
        import argparse
        import json
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            (path/'witnesses/en.json').write_text(json.dumps({'language':'en','witness':{},'review':{}}))
            candidate=Path(tmp)/'candidate.json';candidate.write_text(json.dumps({'work':'work','places':{'p':{'text':'Unreviewed source words.'}}}))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_witness_candidate(argparse.Namespace(packet=str(path),language='en',file=str(candidate)))
                stored=(path/'witnesses/en.json').read_text()
                self.assertNotIn('Unreviewed source words.',stored)
                self.assertTrue((Path(tmp)/'cache/work/witnesses/en/research-candidate.json').exists())

class LivingAuthorChronologyTest(unittest.TestCase):
    def test_known_birth_without_death_remains_an_unknown_lifetime(self):
        import json
        import shutil
        import subprocess
        import tempfile
        from pathlib import Path
        root=Path(kit.ROOT); original=Path(kit.REVIEW)
        with tempfile.TemporaryDirectory() as tmp:
            target=Path(tmp);review=target/'bonsai/review/aesthetics';layer=review/'layer';layer.mkdir(parents=True)
            (target/'bonsai/lib').symlink_to(root/'bonsai/lib',target_is_directory=True)
            (review/'planting/packets').mkdir(parents=True)
            shutil.copy2(original/'chronology.mjs',review/'chronology.mjs')
            for name in ('agents','works','sources'):(layer/(name+'.json')).symlink_to(original/'layer'/(name+'.json'))
            lives=json.loads((original/'layer/lives.json').read_text());lives['immanuel-kant']['died']=[]
            (layer/'lives.json').write_text(json.dumps(lives,ensure_ascii=False))
            home=target/'bonsai/aesthetics';home.mkdir();shutil.copy2(root/'bonsai/aesthetics/library.json',home/'library.json')
            subprocess.run(['node',str(review/'chronology.mjs')],check=True,capture_output=True,text=True)
            result=json.loads((home/'chronology.json').read_text());life=result['situations'][result['lives']['immanuel-kant']]
            self.assertEqual(life['dating']['kind'],'unknown-date')
            self.assertIn('born',life['dating']['source_wording']['text'])
            self.assertIn('no death year recorded',life['dating']['source_wording']['text'])

class TranslationCorrectionTest(unittest.TestCase):
    def test_correction_preserves_page_binding_and_invalidates_review(self):
        import argparse
        import hashlib
        import json
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            raw=b'page image';(folder/'page.jpg').write_bytes(raw);digest=hashlib.sha256(raw).hexdigest()
            data={'language':'en','images':[{'file':'page.jpg','url':'https://example.org/page','by':'maker','sha256':digest}],
                  'places':{'p':{'text':'A sentence;','page':'391','url':'https://example.org/page','image_sha256':digest}},
                  'review':{'reader':'old-reader'}}
            file=path/'witnesses/en.json';file.write_text(json.dumps(data))
            args=argparse.Namespace(packet=str(path),language='en',loc='p',to='.',reason='The image ends this sentence with a full stop.',**{'from':';'})
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):kit.cmd_witness_correct(args)
            result=json.loads(file.read_text());self.assertEqual(result['places']['p']['text'],'A sentence.')
            self.assertEqual(result['places']['p']['corrections'][0]['from'],';')
            self.assertEqual(result['places']['p']['corrections'][0]['offset'],len('A sentence'))
            self.assertEqual(result['places']['p']['corrections'][0]['image_sha256'],digest)
            self.assertEqual(result['review'],{})

class StructuredAddressTest(unittest.TestCase):
    def test_section_identifies_a_lawful_unpaginated_source(self):
        address={'kind':'section','value':'III.2','url':'https://example.org/chapter#section-2'}
        self.assertTrue(kit.valid_address(address))
        self.assertFalse(kit.valid_address({**address,'value':''}))
        self.assertFalse(kit.valid_address({**address,'url':'https://libgen.rs/book'}))
        self.assertFalse(kit.valid_address({**address,'kind':'guess'}))
