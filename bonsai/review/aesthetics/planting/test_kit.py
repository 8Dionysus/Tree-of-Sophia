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
        self.assertEqual(kit.modern_russian('Ея книга.'), 'Её книга.')
        self.assertEqual(kit.modern_russian('Безсильная речь.'), 'Бессильная речь.')
        self.assertEqual(kit.modern_russian('великаго мудреца.'), 'великого мудреца.')
        self.assertEqual(kit.modern_russian('Великаго мудреца.',old_source=True), 'Великого мудреца.')
        self.assertFalse(kit.old_russian_source('Современная история.'))
        self.assertTrue(kit.old_russian_source('Бѣлый свѣтъ.'))


class SentenceBoundaryTest(unittest.TestCase):
    def test_uncased_script_and_exact_source_boundaries(self):
        self.assertEqual(kit.sentences('这是一句完整的话。'),[])
        self.assertEqual(kit.sentences('「这是一个句子。」'),[])
        self.assertEqual(kit.sentences('هذه جملة كاملة؟'),[])
        self.assertTrue(kit.sentences('начало внутри.'))
        self.assertTrue(kit.sentences('这是不完整的话'))
        source='这是第一句。这是第二句。'
        self.assertIsNone(kit.in_transcription('这是第二句。',source))
        self.assertIsNotNone(kit.in_transcription('第二句。',source))
        proof={'source':'https://example.org/page','page':'1','lines':'1–4','sha256':'a'*64}
        ev={'images':[{'sha256':'a'*64,'by':'maker'}]}
        self.assertTrue(any('exceeds' in p for p in kit.quotation_problems('p','一句。二句。三句。四句。',proof,ev)))

class RussianExtensionTest(unittest.TestCase):
    def test_extension_normalizes_without_editing_parent(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            parent=Path(tmp)/'work';path=parent/'extensions'/'first';path.mkdir(parents=True)
            cache=Path(tmp)/'cache';folder=cache/'work'/'extensions'/'first';folder.mkdir(parents=True)
            (folder/'source.txt').write_text('Бѣлый свѣтъ. Великаго мудреца.',encoding='utf8')
            (path/'places.json').write_text(json.dumps({'work':'work','places':{}}))
            (path/'evidence.json').write_text(json.dumps({'sources':[{'file':'source','text':'source.txt','role':'transcription'}],'places':{}}))
            parent_witness={'edition':'Existing Russian edition','rights':{'reasoning':'original record'}}
            world={'fragments':{'works':{'work':{'original':'ru','witnesses':{'ru':parent_witness}}}}}
            args=argparse.Namespace(packet=str(path),file='source',open='Бѣлый',close='свѣтъ.',nth=None,drop=None,lang=None,loc='p1')
            with patch.object(kit,'CACHE',str(cache)),patch.object(kit,'world',return_value=world):kit.cmd_cut(args)
            place=json.loads((path/'places.json').read_text())['places']['p1']
            self.assertEqual(place['texts']['ru'],'Белый свет.')
            self.assertEqual(place['normalization']['ru']['source_text'],'Бѣлый свѣтъ.')
            self.assertEqual(parent_witness['rights']['reasoning'],'original record')
            args.open='Великаго';args.close='мудреца.';args.loc='p2'
            with patch.object(kit,'CACHE',str(cache)),patch.object(kit,'world',return_value=world):kit.cmd_cut(args)
            second=json.loads((path/'places.json').read_text())['places']['p2']
            self.assertEqual(second['texts']['ru'],'Великого мудреца.')
            self.assertEqual(second['normalization']['ru']['source_text'],'Великаго мудреца.')

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
            import base64
            image=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            source=b'A whole sentence.'
            (folder/'text.txt').write_bytes(source);(folder/'page.jpg').write_bytes(image);(folder/'second.jpg').write_bytes(image)
            digest=hashlib.sha256(image).hexdigest()
            people=[{'role':'author','name':{'en':'Author'},'qid':'Q1','died':1800,'source':'https://www.wikidata.org/wiki/Q1'},
                    {'role':'translator','name':{'en':'Translator'},'qid':'Q2','died':1800,'source':'https://www.wikidata.org/wiki/Q2'}]
            import json
            for person in people:
                qid=person['qid']; filename=f'wikidata-{qid}.json'
                entity={'entities':{qid:{'labels':{'en':{'value':person['name']['en']}},'claims':{'P570':[{'mainsnak':{'datavalue':{'value':{'time':'+1800-01-01T00:00:00Z','precision':11}}}}]}}}}
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
                import struct, zlib
                chunk=b'tEXtpage\x00two';image2=image[:-12]+struct.pack('>I',len(chunk)-4)+chunk+struct.pack('>I',zlib.crc32(chunk))+image[-12:];digest2=hashlib.sha256(image2).hexdigest()
                (folder/'page2.jpg').write_bytes(image2);(folder/'second2.jpg').write_bytes(image2)
                data['places']['p']['page']='pp. 1–2'
                self.assertTrue(any('page range' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['places']['p']['extra_pages']=[{'page':'p. 2','url':'https://example.org/page2','image_sha256':digest2,'span':'sentence.'}]
                data['places']['p']['page_span']='A whole'
                data['images'].extend([{'file':'page2.jpg','url':'https://example.org/page2','sha256':digest2,'by':'maker'},{'file':'second2.jpg','url':'https://example.org/page2','sha256':digest2,'by':'second'}])
                self.assertTrue(any('review every page' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['review']['places']['p']['pages']=[{'page':'pp. 1–2','url':'https://example.org/page','image_sha256':digest},{'page':'p. 2','url':'https://example.org/page2','image_sha256':digest2}]
                self.assertEqual(kit.witness_problems(str(path),'en',data,second=True),[])
                import argparse, copy
                before=copy.deepcopy(data);(path/'witnesses').mkdir();file=path/'witnesses/en.json';file.write_text(json.dumps(data))
                correction=argparse.Namespace(packet=str(path),language='en',loc='p',to='statement.',reason='The second page reads statement rather than sentence.',image='page2.jpg',**{'from':'sentence.'})
                kit.cmd_witness_correct(correction)
                kit.cmd_witness_review(argparse.Namespace(packet=str(path),language='en',loc='p',reader='reader',model='gpt-6.1-terra',alignment='Both pages match the full original sentence.'))
                corrected=json.loads(file.read_text());self.assertEqual(corrected['places']['p']['extra_pages'][0]['span'],'statement.')
                self.assertEqual(kit.witness_problems(str(path),'en',corrected,second=True),[])
                data=before
                data['places']['p']['page_span']='A'
                self.assertTrue(any('spans must cover' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['places']['p']['page_span']='A whole'
                data['images'].pop();self.assertTrue(any('second reader page image' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['images'].pop();data['places']['p'].pop('extra_pages');data['places']['p'].pop('page_span');data['places']['p']['page']='1';data['review']['places']['p'].pop('pages')
                place=data['places']['p'];place.pop('source')
                place['transcription']={'kind':'human-page','reason':'The page image has no usable machine transcription of this passage.','original_text':'A whole sentence.'}
                self.assertEqual(kit.witness_problems(str(path),'en',data,second=True),[])
                data['images'][1]['url']='https://example.org/other-page'
                self.assertTrue(any('second reader page image' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['images'][1]['url']='https://example.org/page'
                place['transcription']['reason']='short'
                self.assertTrue(any('human page transcription' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                place.pop('transcription');place['source']='text.txt'
                license_url='https://creativecommons.org/licenses/by/4.0/'
                license_raw=('Publisher grants '+license_url.replace('https://','http://')).encode();(folder/'licence.html').write_bytes(license_raw)
                source_record={'file':'licence.html','url':'https://example.org/exact-edition','sha256':hashlib.sha256(license_raw).hexdigest()}
                data['sources'].append(source_record)
                data['witness']['rights']['eu']={'basis':'licensed','law':'Open licence covers this specific edition.'}
                self.assertTrue(any('licensed basis' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['witness']['licence_evidence']={'edition':'Book','licence_url':license_url,'source':source_record,'explanation':'The publisher licenses this exact edition under CC BY 4.0.'}
                data['review']['licence']={'reader':'reader','source_sha256':source_record['sha256'],'licence_url':license_url,'assessment':'The publisher confirms this exact edition and its licence terms.'}
                self.assertEqual(kit.witness_problems(str(path),'en',data,second=True),[])
                data['review']['licence']['reader']='maker'
                self.assertTrue(any('independent review' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                data['review']['licence']['reader']='reader'
                (folder/'licence.html').write_bytes(b'Changed publisher page')
                self.assertTrue(any('licensed source changed' in p for p in kit.witness_problems(str(path),'en',data,second=True)))
                (folder/'licence.html').write_bytes(license_raw)
                data['witness']['rights']['eu']={'basis':'public-domain','law':'Public domain according to applicable law.'}
                data['images'][1]['url']='https://example.org/other-page'
                self.assertTrue(any('second reader page image' in item for item in kit.witness_problems(str(path),'en',data,second=True)))
                data['images'][1]['url']='https://example.org/page'
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

class LicensedOriginalTest(unittest.TestCase):
    def test_licence_commands_bind_source_and_independent_reader(self):
        import argparse, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();cache=Path(tmp)/'cache';folder=cache/'work';folder.mkdir(parents=True)
            url='https://creativecommons.org/licenses/by/4.0/'
            raw=('Exact publisher edition: '+url).encode();(folder/'publisher.html').write_bytes(raw)
            source={'file':'publisher.html','url':'https://publisher.example/edition','sha256':hashlib.sha256(raw).hexdigest()}
            entry={'original':'en','witnesses':{'en':{'edition':'2026 journal version','rights':{'eu':{'basis':'licensed'}}}}}
            (path/'places.json').write_text(json.dumps({'entry':entry}));(path/'evidence.json').write_text(json.dumps({'sources':[source]}))
            (path/'reading.json').write_text(json.dumps({'reader':{'id':'maker'}}));(path/'review.json').write_text(json.dumps({'reader':{'id':'second'}}))
            with patch.object(kit,'CACHE',str(cache)):
                kit.cmd_licence(argparse.Namespace(packet=str(path),language=None,source='publisher.html',url=url,explanation='The publisher licenses the exact journal edition under CC BY 4.0.'))
                kit.cmd_licence_review(argparse.Namespace(packet=str(path),language=None,assessment='The publisher confirms that this exact version has the stated open licence.'))
                witness=json.loads((path/'places.json').read_text())['entry']['witnesses']['en'];review=json.loads((path/'review.json').read_text())
                self.assertEqual(kit.license_problems(witness,[source],lambda name:folder/name,review,'maker',True),[])
                witness['edition']='A different edition'
                self.assertTrue(kit.license_problems(witness,[source],lambda name:folder/name,review,'maker',True))

class AcceptedLicenceEvidenceTest(unittest.TestCase):
    def test_accept_keeps_the_transcription_used_to_prove_a_licence(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir()
            entry={'original':'en','witnesses':{'en':{'licence_evidence':{'source':{'file':'publisher.xml'}}}}}
            (path/'places.json').write_text(json.dumps({'entry':entry}))
            (path/'evidence.json').write_text(json.dumps({'sources':[{'file':'text.txt','role':'transcription'},{'file':'publisher.xml','role':'transcription'}],'places':{'p':{'transcription':'text.txt'}}}))
            (path/'reading.json').write_text(json.dumps({'reader':{'id':'maker'},'conceptions':[],'links':[],'left':[]}))
            (path/'review.json').write_text(json.dumps({'reader':{'id':'second'}}))
            args=argparse.Namespace(packet=str(path),offline=False,basis=None,by='test')
            with patch.object(kit,'gate',return_value=([],'open')):kit.cmd_accept(args)
            evidence=json.loads((path/'evidence.json').read_text())
            self.assertEqual({source['file'] for source in evidence['sources']},{'text.txt','publisher.xml'})

class DuplicateWitnessAcceptanceTest(unittest.TestCase):
    def test_extension_cannot_sign_language_already_accepted_by_parent(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            parent=Path(tmp)/'work';extension=parent/'extensions'/'new';(extension/'witnesses').mkdir(parents=True)
            (parent/'witnesses').mkdir();(parent/'witnesses/ru.json').write_text('{}')
            (parent/'witnesses/ru.json.accepted').write_text('{}')
            file=extension/'witnesses/ru.json';file.write_text(json.dumps({'language':'ru','witness':{}}))
            args=argparse.Namespace(packet=str(extension),language='ru',second=True,offline=False,accept=True)
            with patch.object(kit,'witness_problems',return_value=[]):
                with self.assertRaises(SystemExit):kit.cmd_witness_gate(args)
            self.assertFalse(Path(str(file)+'.accepted').exists())

    def test_same_edition_adds_only_new_places(self):
        import hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            parent=Path(tmp)/'work';extension=parent/'extensions'/'new';(extension/'witnesses').mkdir(parents=True)
            (parent/'witnesses').mkdir();file=parent/'witnesses/ru.json'
            original={'language':'ru','witness':{'edition':'Same book','url':'https://example.org','maker':'first'},'places':{'old':{'text':'Оригинал.'}}}
            raw=json.dumps(original).encode();file.write_bytes(raw);Path(str(file)+'.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            data={'language':'ru','witness':{'edition':'Same book','url':'https://example.org','maker':'second'},'places':{'new':{'text':'Новое.'}}}
            fragments={'works':{'work':{'witnesses':{'ru':original['witness']}}},'provenance':{'translation_packets':{'work':{'ru':{'record':'parent'}}}}}
            with patch.object(kit,'world',return_value={'fragments':fragments}):
                self.assertIsNone(kit.witness_conflict(str(extension),'ru',data))
                data['places']['old']={'text':'Другое.'}
                self.assertIn('already covers',kit.witness_conflict(str(extension),'ru',data))
                data['places'].pop('old');data['witness']['edition']='Other book'
                self.assertIn('different edition',kit.witness_conflict(str(extension),'ru',data))

    def test_packet_signs_no_witness_if_a_later_witness_conflicts(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            for lang in ('ru','es'):(path/'witnesses'/f'{lang}.json').write_text(json.dumps({'language':lang}))
            args=argparse.Namespace(packet=str(path),offline=False,basis=None,by='test')
            with patch.object(kit,'gate',return_value=([],'open')),patch.object(kit,'witness_problems',return_value=[]),patch.object(kit,'witness_conflict',side_effect=[None,'conflict']):
                with self.assertRaises(SystemExit):kit.cmd_accept(args)
            self.assertEqual(list((path/'witnesses').glob('*.accepted')),[])

class MexicoQuotationTest(unittest.TestCase):
    def test_original_extension_keeps_its_own_reviewed_assessment(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work'/'extensions'/'extra';path.mkdir(parents=True)
            (path/'places.json').write_text(json.dumps({'work':'work','places':{'p':{'texts':{'en':'A sentence.'}}}}))
            (path/'evidence.json').write_text('{}');(path/'reading.json').write_text(json.dumps({'reader':{'id':'maker'}}))
            (path/'review.json').write_text(json.dumps({'reader':{'id':'second','model':'model','date':'2026-11-19'}}))
            original={'edition':'Same edition','rights':{'usa':{'basis':'short-quotation'}}}
            world={'fragments':{'works':{'work':{'original':'en','witnesses':{'en':original}}}}}
            law=Path(tmp)/'law.pdf';law.write_bytes(b'%PDF-1.4\nTest law source\n%%EOF')
            args=argparse.Namespace(packet=str(path),language=None,law_file=str(law),non_substantial='This passage is proportionate to the scholarly purpose of the comparison.',normal_exploitation='The selected sentences do not replace the book or affect normal exploitation.',source_integrity='The cited source and author are identified and the page text is unchanged.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'world',return_value=world):
                kit.cmd_mexico_quotation(args)
                self.assertIn('mexico_quotation',json.loads((path/'evidence.json').read_text()))
                self.assertNotIn('mexico_quotation',original)
                kit.cmd_mexico_quotation_review(argparse.Namespace(packet=str(path),language=None,assessment='I independently checked the source, integrity, proportion and normal exploitation for these exact places.'))
                _,witness,places,review,maker=kit.mexico_context(str(path),None)
                self.assertEqual(kit.mexico_problems(witness,places,review,maker,second=True,path=str(path)),[])
                self.assertTrue(kit.same_witness({**original,'mexico_quotation':{'scope_sha256':'one'}},{**original,'mexico_quotation':{'scope_sha256':'two'}}))

    def test_quotation_review_is_scoped_to_the_exact_witness_and_text(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            data={'language':'en','witness':{'edition':'A named edition','maker':'maker','rights':{'usa':{'basis':'short-quotation'}}},'places':{'p':{'text':'A complete sentence.','url':'https://example.org/page','page':'7'}},'review':{'reader':'reader'}}
            file.write_text(json.dumps(data))
            law=Path(tmp)/'law.pdf';law.write_bytes(b'%PDF-1.4\nTest law source\n%%EOF')
            args=argparse.Namespace(packet=str(path),language='en',law_file=str(law),non_substantial='This short passage is proportionate to the scholarly purpose of the comparison.',normal_exploitation='The selected sentences do not replace the book or affect normal exploitation.',source_integrity='The cited source and author are identified and the page text is unchanged.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_mexico_quotation(args);data=json.loads(file.read_text())
                self.assertTrue(kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True))
                kit.cmd_mexico_quotation_review(argparse.Namespace(packet=str(path),language='en',assessment='I independently checked the source, integrity, proportion and normal exploitation for this exact quotation.'))
                data=json.loads(file.read_text());self.assertEqual(kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True,path=str(path),lang='en'),[])
                Path(kit.witness_cache(str(path),'en',data['witness']['mexico_quotation']['law_file'])).write_bytes(b'changed')
                self.assertTrue(any('law source changed' in p for p in kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True,path=str(path),lang='en')))
            data=json.loads(file.read_text());self.assertEqual(kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True),[])
            data['places']['p']['text']='A different sentence.'
            self.assertTrue(any('current exact witness and places' in p for p in kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True)))
            data['review']['reader']='maker'
            self.assertTrue(any('distinct second reader' in p for p in kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True)))

class AnonymousTranslationTest(unittest.TestCase):
    def test_commands_bind_sources_review_and_allow_clearing(self):
        import argparse, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);cache=Path(tmp)/'cache';folder=cache/'work'/'witnesses'/'en';folder.mkdir(parents=True)
            sources=[]
            for name in ('edition.html','catalogue.html'):
                raw=('Evidence '+name).encode();(folder/name).write_bytes(raw);sources.append({'file':name,'url':'https://example.org/'+name,'sha256':hashlib.sha256(raw).hexdigest()})
            file=path/'witnesses/en.json';file.write_text(json.dumps({'language':'en','witness':{'edition':'1902 edition','year':1902,'maker':'maker'},'sources':sources,'review':{}}))
            info={'first_publication_year':1875,'lawful_publication':'A catalogue establishes lawful publication of this exact translation.',
                  'identity_research':'Independent catalogues and later editions were searched for disclosure of the translator.',
                  'same_version':'The passages and bibliographic history establish the same translation in both editions.',
                  'searches':[{'url':f'https://example.org/search/{i}','outcome':'The catalogue was searched for the translator identity.'} for i in range(3)],'sources':[x['file'] for x in sources]}
            evidence=Path(tmp)/'evidence.json';evidence.write_text(json.dumps(info))
            with patch.object(kit,'CACHE',str(cache)):
                kit.cmd_witness_anonymous(argparse.Namespace(packet=str(path),language='en',file=str(evidence)))
                data=json.loads(file.read_text());data['review']={'reader':'second'};file.write_text(json.dumps(data))
                kit.cmd_witness_anonymous_review(argparse.Namespace(packet=str(path),language='en',assessment='I checked the edition, publication date and the identity searches independently.'))
                data=json.loads(file.read_text());self.assertEqual(data['review']['anonymous']['sha256'],kit.anonymous_digest(data['witness']['anonymous_translation']))
                kit.cmd_witness_anonymous_clear(argparse.Namespace(packet=str(path),language='en'))
                data=json.loads(file.read_text());self.assertNotIn('anonymous_translation',data['witness']);self.assertEqual(data['review'],{})

    def test_eu_anonymous_term_requires_version_sources_search_and_independent_review(self):
        import datetime, hashlib, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();cache=Path(tmp)/'cache';folder=cache/'work'/'witnesses'/'en';folder.mkdir(parents=True)
            sources=[]
            for name,text in [('edition.html',b'First publication and the exact edition.'),('catalogue.html',b'Translation identity research.')]:
                (folder/name).write_bytes(text);sources.append({'file':name,'url':'https://example.org/'+name,'sha256':hashlib.sha256(text).hexdigest()})
            author={'role':'author','qid':'Q1','name':{'en':'Author'},'died':1800,'source':'https://www.wikidata.org/wiki/Q1',
                    'evidence':{'file':'author.json','sha256':'a'*64,'url':'https://www.wikidata.org/wiki/Special:EntityData/Q1.json'}}
            proof={'edition':'Anonymous edition','first_publication_year':datetime.date.today().year-71,
                   'lawful_publication':'The catalogue and edition establish the first lawful publication of this translation.',
                   'identity_research':'The identified catalogues and subsequent editions were searched for a disclosed identity.',
                   'same_version':'The wording and edition records establish that the first publication is the same translation.',
                   'searches':[{'url':f'https://example.org/search/{i}','outcome':'The named catalogue was searched for the translator identity.'} for i in range(3)],
                   'sources':sources}
            rights={'russia':{'basis':'short-quotation','law':'Quotation for the documented scholarly purpose.'},
                    'usa':{'basis':'short-quotation','law':'Quotation for the documented scholarly purpose.'},
                    'eu':{'basis':'public-domain','law':'The anonymous translation term has expired.'}}
            data={'language':'en','witness':{'maker':'maker','edition':'Anonymous edition','attribution':'Author and anonymous translator','licence':'Public domain','url':'https://example.org/edition','year':1900,'persons':[author],'rights':rights,'anonymous_translation':proof},
                  'sources':sources+[{'file':'text.txt','url':'https://example.org/text','sha256':'b'*64}],
                  'images':[{'file':'page.jpg','url':'https://example.org/page','sha256':'c'*64,'by':'maker'},{'file':'page2.jpg','url':'https://example.org/page','sha256':'c'*64,'by':'second'}],
                  'places':{'p':{'text':'A whole sentence.','source':'text.txt','page':'1','url':'https://example.org/page','lines':'1–2','image_sha256':'c'*64,'alignment':'This is the corresponding complete original sentence.'}},
                  'review':{'reader':'second','model':'gpt-6.1-terra','date':'2026-11-12','places':{'p':{'reads':'as the packet','image_sha256':'c'*64,'alignment':'This is the corresponding complete original sentence.'}}}}
            # Use a sufficiently old edition while retaining an exact dynamic threshold.
            data['witness']['year']=datetime.date.today().year-71
            data['review']['anonymous']={'reader':'second','sha256':kit.anonymous_digest(proof),'assessment':'I checked the same edition, lawful publication and the identity searches independently.'}
            mexico={'law_url':kit.MEXICO_LFDA,'law_file':'mexico-law-'+('a'*64)+'.pdf','law_sha256':'a'*64,'scope_sha256':kit.mexico_scope(data['witness'],data['places']),
                    'non_substantial':'The selected passage is proportionate to this particular scholarly purpose.',
                    'normal_exploitation':'The quotation does not replace the edition or affect its normal exploitation.',
                    'source_integrity':'The source, attribution and exact page are given and the text is unchanged.'}
            data['witness']['mexico_quotation']=mexico
            data['review']['mexico_quotation']={'reader':'second','date':'2026-11-19','sha256':kit.page_number_digest(mexico),'assessment':'I independently checked the quotation, source, text integrity and normal exploitation.'}
            with patch.object(kit,'CACHE',str(cache)),patch.object(kit,'world',return_value={'fragments':{'places':{'work':{'p':{}}}}}):
                self.assertEqual(kit.witness_problems(str(path),'en',data,second=True,offline=True),[])
                del data['witness']['mexico_quotation']
                self.assertTrue(any('Mexico quotation assessment' in p for p in kit.witness_problems(str(path),'en',data,second=True,offline=True)))
                data['witness']['mexico_quotation']=mexico
                self.assertEqual(kit.anonymous_problems(str(path),'en',data,second=True),[])
                proof['identity_research']+=' Changed.'
                self.assertTrue(any('independent assessment' in x for x in kit.witness_problems(str(path),'en',data,second=True,offline=True)))
                data['review']['anonymous']['sha256']=kit.anonymous_digest(proof)
                proof['first_publication_year']=datetime.date.today().year-70;data['witness']['year']=proof['first_publication_year']
                data['review']['anonymous']['sha256']=kit.anonymous_digest(proof)
                self.assertTrue(any('rights.eu is not established' in x for x in kit.witness_problems(str(path),'en',data,second=True,offline=True)))
                proof['first_publication_year']=datetime.date.today().year-71;data['witness']['year']=proof['first_publication_year'];data['review']['anonymous']['sha256']=kit.anonymous_digest(proof)
                rights['russia']={'basis':'public-domain','law':'A claimed Russian public-domain term.'}
                self.assertTrue(any('rights.russia is not established' in x for x in kit.witness_problems(str(path),'en',data,second=True,offline=True)))
                (folder/'edition.html').write_bytes(b'Changed version.')
                self.assertTrue(any('source changed' in x for x in kit.anonymous_problems(str(path),'en',data,second=True)))

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

class LanguageSearchTest(unittest.TestCase):
    def test_whole_language_requires_complete_search_and_independent_review(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir()
            world={'fragments':{'works':{'work':{'original':'de','witnesses':{'de':{}}}},'places':{'work':{'p':{}}}}}
            record={'maker':'maker','reason':'The documented searches did not find an eligible translation.','tried':[{'site':site,'url':'https://example.org/'+site,'outcome':'The catalogue was searched and no matching edition was found.'} for site in sorted(kit.LOCATOR_SEARCHES)]}
            source=Path(tmp)/'search.json';source.write_text(json.dumps(record))
            with patch.object(kit,'world',return_value=world):
                kit.cmd_language_search(argparse.Namespace(packet=str(path),language='es',file=str(source)))
                file=path/'language-searches/es.json';data=json.loads(file.read_text())
                self.assertEqual(kit.language_search_problems(str(path),'es',data),[])
                self.assertTrue(kit.language_search_problems(str(path),'es',data,second=True))
                with self.assertRaises(SystemExit):kit.cmd_language_search_review(argparse.Namespace(packet=str(path),language='es',reader='maker',model='model',assessment='I checked every recorded search independently and found the same result.'))
                kit.cmd_language_search_review(argparse.Namespace(packet=str(path),language='es',reader='second',model='model',assessment='I checked every recorded search independently and found the same result.'))
                kit.cmd_language_search_accept(argparse.Namespace(packet=str(path),language='es'))
                data=json.loads(file.read_text());self.assertEqual(kit.language_search_problems(str(path),'es',data,second=True),[])
                data['search']['reason']='A different unreviewed search conclusion appears here.'
                self.assertTrue(kit.language_search_problems(str(path),'es',data,second=True))
                extension=path/'extensions'/'test';extension.mkdir(parents=True)
                self.assertTrue(any('parent packet' in p for p in kit.language_search_problems(str(extension),'es',data)))
                world['fragments']['places']['work']['new']={}
                self.assertTrue(any('every current original place' in p for p in kit.language_search_problems(str(path),'es',data)))

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
            world=kit.world(but=json.loads((source/'reading.json').read_text())['reader']['id']);world['fragments']=projected
            with patch.object(kit,'world',return_value=world):
                problems,_=kit.gate(str(parent),second=True,offline=True)
                self.assertEqual(problems,[])
                (ext/'evidence.json').write_text('{}')
                original=projected['works'][work]['original'];rights=projected['works'][work]['witnesses'][original]['rights'];previous=rights['usa']
                rights['usa']={'basis':'short-quotation','law':'Quotation for a documented scholarly purpose.'}
                (ext/'accepted.json').unlink()
                problems,_=kit.gate(str(ext),second=True,offline=True)
                self.assertTrue(any('Mexico quotation assessment' in problem for problem in problems))
                rights['usa']=previous;(ext/'accepted.json').write_text('{}')
                projected['places'][work]['Unowned p998']=copy.deepcopy(place)
                problems,_=kit.gate(str(parent),second=True,offline=True)
                self.assertTrue(any('drop entry from places.json' in problem for problem in problems))
                del projected['places'][work]['Unowned p998']
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            (ext/'accepted.json').unlink()
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertNotIn('Test p999',projected['places'][work])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            import hashlib
            search_dir=parent/'language-searches';search_dir.mkdir()
            search={'language':'es','search':{'places':sorted(base['places']),'reason':'The documented searches found no suitable text.','tried':[]},'review':{}}
            search_raw=json.dumps(search).encode();search_file=search_dir/'es.json';search_file.write_bytes(search_raw)
            (search_dir/'es.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(search_raw).hexdigest()}))
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertTrue(any(item['language']=='es' for item in projected['works'][work]['absent']))
            self.assertNotIn('es',projected['works'][work]['witnesses'])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            search_file.write_bytes(search_raw+b' ')
            self.assertIn('search changed since review',subprocess.run(command,capture_output=True,text=True).stderr)
            (search_dir/'es.json.accepted').unlink()
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertFalse(any(item['language']=='es' and item.get('reason')==search['search']['reason'] for item in projected['works'][work].get('absent',[])))
            witness_dir=parent/'witnesses';witness_dir.mkdir()
            loc=next(iter(base['places']))
            rights={country:{'basis':'public-domain','law':'Public domain according to the applicable law.'} for country in kit.JURISDICTIONS}
            translated={'language':'ru','witness':{'edition':'Test edition','attribution':'Test author','url':'https://example.org','rights':rights},
                        'places':{loc:{'text':'Русское предложение.','url':'https://example.org/page','page':'1','image_sha256':'a'*64,'extra_pages':[{'page':'2','url':'https://example.org/page2','image_sha256':'b'*64,'span':'предложение.'}],'corrections':[{'from':'Прежнее','to':'Русское','reason':'The source page reads the corrected form.'}]}}}
            translated['witness']['mexico_quotation']={'scope_sha256':'parent'}
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
            self.assertEqual(projected['provenance']['translation_packets'][work]['ru']['page_sources'][loc][1]['image_sha256'],'b'*64)
            self.assertTrue(any('1, 2' in ref[1] for ref in projected['places'][work][loc]['refs'] if ref[0]=='ru'))
            self.assertFalse(any(item['language']=='ru' for item in projected['works'][work].get('unsearched',[])))
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            # An accepted translation cannot keep the packet-owned original alive.
            (parent/'accepted.json').unlink()
            subprocess.run(command,check=True,capture_output=True,text=True)
            withdrawn=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertNotIn(work,withdrawn['works'])
            self.assertNotIn(work,withdrawn['places'])
            (parent/'accepted.json').symlink_to(source/'accepted.json')
            subprocess.run(command,check=True,capture_output=True,text=True)
            restored=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertEqual(restored['places'][work][loc]['texts']['ru'],'Русское предложение.')
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            # Parent translations may refer to newly introduced extension places;
            # accepted witnesses inside that extension must also be projected.
            (ext/'accepted.json').write_text('{}')
            (ext/'evidence.json').write_text(json.dumps({'places':{'Test p999':{'quotation':{'justification':'This longer quotation is necessary to retain the complete argument.'}}}}))
            translated['places']['Test p999']={'text':'Второе предложение.','url':'https://example.org/page2','page':'2'}
            raw=json.dumps(translated,ensure_ascii=False).encode();(witness_dir/'ru.json').write_bytes(raw)
            (witness_dir/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            ext_witness=ext/'witnesses';ext_witness.mkdir()
            extension_places['places']['Test p1000']=copy.deepcopy(place)
            (ext/'places.json').write_text(json.dumps(extension_places,ensure_ascii=False))
            russian_extra=copy.deepcopy(translated);russian_extra['witness']['maker']='another maker'
            russian_extra['witness']['mexico_quotation']={'scope_sha256':'extension'}
            russian_extra['places']={'Test p1000':{'text':'Третье предложение.','url':'https://example.org/page3','page':'3'}}
            raw_extra=json.dumps(russian_extra,ensure_ascii=False).encode();(ext_witness/'ru.json').write_bytes(raw_extra)
            (ext_witness/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw_extra).hexdigest()}))
            spanish=copy.deepcopy(translated);spanish['language']='es'
            spanish['places']={'Test p999':{'text':'Una frase completa.','url':'https://example.org/es','page':'3'}}
            raw_es=json.dumps(spanish,ensure_ascii=False).encode();(ext_witness/'es.json').write_bytes(raw_es)
            (ext_witness/'es.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw_es).hexdigest()}))
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertEqual(projected['places'][work]['Test p999']['texts']['ru'],'Второе предложение.')
            self.assertEqual(projected['places'][work]['Test p1000']['texts']['ru'],'Третье предложение.')
            self.assertEqual(len(projected['provenance']['translation_packets'][work]['ru']['mexico_quotations']),2)
            self.assertEqual(len(projected['provenance']['translation_packets'][work]['ru']['records']),2)
            self.assertEqual(projected['places'][work]['Test p999']['texts']['es'],'Una frase completa.')
            self.assertIn('/extensions/test/witnesses/es.json',projected['provenance']['translation_packets'][work]['es']['record'])
            self.assertIn('complete argument',projected['places'][work]['Test p999']['quotation_justification'][base['entry']['original']])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            (ext/'accepted.json').unlink()
            translated['places'].pop('Test p999');raw=json.dumps(translated,ensure_ascii=False).encode();(witness_dir/'ru.json').write_bytes(raw)
            (witness_dir/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
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
            lives['immanuel-kant']['died']=[{'time':'+1804-01-01T00:00:00Z','precision':9,'approximate':True,'qualifiers':['Q5727902']}]
            lives['immanuel-kant']['born'][0].update({'time':'+1720-01-01T00:00:00Z','precision':8})
            lives['immanuel-kant']['born'][0]['bounds']={'earliest':[{'time':'+1723-01-01T00:00:00Z','precision':9}],'latest':[{'time':'+1725-01-01T00:00:00Z','precision':9}]}
            lives['immanuel-kant']['born'][0]['approximate']=True
            (layer/'lives.json').write_text(json.dumps(lives,ensure_ascii=False))
            subprocess.run(['node',str(review/'chronology.mjs')],check=True,capture_output=True,text=True)
            result=json.loads((home/'chronology.json').read_text());life=result['situations'][result['lives']['immanuel-kant']]
            self.assertEqual(life['dating']['certainty'],'approximate')
            self.assertEqual(life['dating']['interval']['start'],'1723')
            self.assertIn('approximately',life['dating']['source_wording']['text'])

class TranslationCorrectionTest(unittest.TestCase):
    def test_added_page_binds_words_and_invalidates_review(self):
        import argparse, base64, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            (folder/'page2.jpg').write_bytes(raw);digest=hashlib.sha256(raw).hexdigest()
            data={'language':'en','images':[{'file':'page2.jpg','url':'https://example.org/page2','by':'maker','sha256':digest}],
                  'places':{'p':{'text':'The sentence continues here.','page':'37','url':'https://example.org/page1','image_sha256':'a'*64}},'review':{'reader':'old'}}
            file=path/'witnesses/en.json';file.write_text(json.dumps(data))
            args=argparse.Namespace(packet=str(path),language='en',loc='p',page='38',image='page2.jpg',span='continues here.',first_span='The sentence')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):kit.cmd_witness_add_page(args)
            result=json.loads(file.read_text());self.assertEqual(result['places']['p']['extra_pages'][0]['image_sha256'],digest)
            self.assertEqual(result['review'],{})
            result['images'].append({'file':'second1.jpg','url':'https://example.org/page1','by':'second','sha256':'a'*64})
            file.write_text(json.dumps(result))
            review=argparse.Namespace(packet=str(path),language='en',loc='p',reader='reader',model='model',alignment='Both pages match the original place.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                with self.assertRaises(SystemExit):kit.cmd_witness_review(review)
                result['images'].append({'file':'second2.jpg','url':'https://example.org/page2','by':'second','sha256':digest});file.write_text(json.dumps(result))
                kit.cmd_witness_review(review)
            self.assertEqual(len(json.loads(file.read_text())['review']['places']['p']['pages']),2)
            correction=argparse.Namespace(packet=str(path),language='en',loc='p',to='there.',reason='The second page reads there rather than here.',image='page2.jpg',**{'from':'here.'})
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):kit.cmd_witness_correct(correction)
            changed=json.loads(file.read_text());self.assertEqual(changed['places']['p']['corrections'][0]['page'],'38');self.assertEqual(changed['review'],{})
            self.assertEqual(changed['places']['p']['extra_pages'][0]['span'],'continues there.')
            self.assertEqual(changed['places']['p']['text'],'The sentence continues there.')

    def test_page_transcription_records_image_and_invalidates_review(self):
        import argparse, base64, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            (folder/'page.jpg').write_bytes(raw);digest=hashlib.sha256(raw).hexdigest()
            data={'language':'en','witness':{},'images':[{'file':'page.jpg','url':'https://example.org/page','by':'maker','sha256':digest}],'places':{},'review':{'reader':'old-reader'}}
            file=path/'witnesses/en.json';file.write_text(json.dumps(data))
            args=argparse.Namespace(packet=str(path),language='en',loc='p',image='page.jpg',page='86',address_kind=None,address_value=None,text='A whole sentence.',reason='This image-only page has no usable transcription of the passage.',alignment='This sentence matches the original place.',lines=None,justification=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'world',return_value={'fragments':{'places':{'work':{'p':{}}}}}):kit.cmd_witness_transcribe_page(args)
            result=json.loads(file.read_text());place=result['places']['p']
            self.assertEqual(place['transcription']['kind'],'human-page');self.assertNotIn('source',place)
            self.assertEqual(place['image_sha256'],digest);self.assertEqual(result['review'],{})

    def test_correction_preserves_page_binding_and_invalidates_review(self):
        import argparse
        import hashlib
        import json
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            import base64
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            (folder/'page.jpg').write_bytes(raw);digest=hashlib.sha256(raw).hexdigest()
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

class AcceptedPageNumberTest(unittest.TestCase):
    def test_reviewed_correction_reverses_the_selected_repeated_text(self):
        text='😀 A sentence. Another sentence.'
        correction={'from':';','to':'.','offset':len(text)-1}
        self.assertEqual(kit.reverse_reviewed_correction(text,correction),'😀 A sentence. Another sentence;')
        self.assertIsNone(kit.reverse_reviewed_correction(text,{**correction,'offset':2}))

    def test_accepted_correction_gate_rejects_changed_review(self):
        import json, shutil, tempfile
        from pathlib import Path
        name='leon-battista-alberti.de-re-aedificatoria'
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/name;shutil.copytree(Path(kit.PACKETS)/name,path)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                problems,_=kit.gate(str(path),second=True,offline=True);self.assertEqual(problems,[])
                file=path/'accepted.json';accepted=json.loads(file.read_text());accepted['text_correction_reviews']=[];file.write_text(json.dumps(accepted))
                problems,_=kit.gate(str(path),second=True,offline=True)
                self.assertTrue(any('not bound to the accepted packet review' in problem for problem in problems))

    def test_accepted_text_correction_keeps_review_and_predecessor(self):
        import argparse, base64, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir()
            (path/'places.json').write_text(json.dumps({'work':'work','entry':{'original':'la'},'places':{'loc':{'texts':{'la':'gratiam auctoritatem vendicat'}}},'corrections':[]}))
            (path/'accepted.json').write_text(json.dumps({'basis':'original acceptance'}))
            (path/'evidence.json').write_text(json.dumps({'scan':{'item':'book'},'places':{'loc':{'images':[3]}}}))
            image=Path(tmp)/'page.png';image.write_bytes(base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII='))
            args=argparse.Namespace(packet=str(path),loc='loc',to='autoritatem',n=3,url='https://archive.org/download/book/page/n3.jpg',by='maker',second='reader',second_model='model',assessment='The page reads autoritatem without the letter c.',file=str(image),**{'from':'auctoritatem'})
            prior_places=(path/'places.json').read_bytes();prior_accept=(path/'accepted.json').read_bytes()
            args.to=''
            with self.assertRaises(SystemExit):kit.cmd_accepted_text_correct(args)
            self.assertEqual((path/'places.json').read_bytes(),prior_places)
            self.assertEqual((path/'accepted.json').read_bytes(),prior_accept)
            args.to='autoritatem'
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):kit.cmd_accepted_text_correct(args)
            places=json.loads((path/'places.json').read_text());accepted=json.loads((path/'accepted.json').read_text());correction=places['corrections'][0]
            self.assertEqual(places['places']['loc']['texts']['la'],'gratiam autoritatem vendicat')
            self.assertEqual(correction['from'],'auctoritatem');self.assertEqual(correction['offset'],len('gratiam '));self.assertEqual(accepted['basis'],'original acceptance')
            self.assertEqual(accepted['text_correction_reviews'][0]['sha256'],kit.page_number_digest(correction))

    def test_corrected_number_keeps_predecessor_and_review(self):
        import argparse, base64, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();scan={'item':'book','ocr':True,'image_offset':0}
            ev={'scan':scan,'scans':{'book':scan},'places':{'loc':{'scan':'book','method':'ocr','leaves':[218],'printed':['183'],'differences':[]}}}
            (path/'evidence.json').write_text(json.dumps(ev));(path/'accepted.json').write_text(json.dumps({'by':'original','basis':'original acceptance'}))
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            image=Path(tmp)/'page.png';image.write_bytes(raw)
            args=argparse.Namespace(packet=str(path),loc=['loc'],leaf=218,old='183',new='184',url='https://archive.org/download/book/page/n218.jpg',by='maker',second='reader',second_model='model',assessment='Both definitions and printed page 184 are visible on this page.',file=str(image))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_page_number_correct(args);ev=json.loads((path/'evidence.json').read_text());accepted=json.loads((path/'accepted.json').read_text());entry=ev['places']['loc']
                self.assertEqual(entry['printed'],['184']);self.assertEqual(entry['page_number_corrections'][0]['from'],'183');self.assertEqual(accepted['basis'],'original acceptance')
                self.assertEqual(kit.page_number_problems(str(path),ev,'loc',entry),[])
                places={'places':{'loc':{'texts':{'en':'A whole sentence.'}}}}
                found={'share':1,'leaves':[218],'printed':['183'],'differences':[]}
                with patch.object(kit,'ocr',return_value={'available':True}),patch.object(kit,'against_ocr',return_value=found):
                    fresh=kit.compare(str(path),ev,places,'en');self.assertEqual(fresh['loc']['printed'],['184'])
                    ev['places']['loc']['from_scan']=True
                    fresh=kit.compare(str(path),ev,places,'en');self.assertEqual(fresh['loc']['method'],'image');self.assertEqual(fresh['loc']['printed'],['184'])
                    self.assertEqual(kit.page_number_problems(str(path),ev,'loc',fresh['loc']),[])
                accepted['page_number_reviews']=[];(path/'accepted.json').write_text(json.dumps(accepted))
                self.assertTrue(any('accepted packet review' in p for p in kit.page_number_problems(str(path),ev,'loc',entry)))

class StructuredAddressTest(unittest.TestCase):
    def test_section_identifies_a_lawful_unpaginated_source(self):
        address={'kind':'section','value':'III.2','url':'https://example.org/chapter#section-2'}
        self.assertTrue(kit.valid_address(address))
        self.assertFalse(kit.valid_address({**address,'value':''}))
        self.assertFalse(kit.valid_address({**address,'url':'https://libgen.rs/book'}))
        self.assertFalse(kit.valid_address({**address,'kind':'guess'}))

class WikidataDatePrecisionTest(unittest.TestCase):
    def test_decade_and_conflicting_years_do_not_establish_a_cutoff(self):
        def claim(year,precision=11,rank='normal'):
            return {'rank':rank,'mainsnak':{'datavalue':{'value':{'time':f'+{year}-01-01T00:00:00Z','precision':precision}}}}
        self.assertIsNone(kit.entity_year({'claims':{'P570':[claim(1950,8)]}},'P570'))
        approximate=claim(1950);approximate['qualifiers']={'P1480':[{'datavalue':{'value':{'id':'Q5727902'}}}]}
        self.assertIsNone(kit.entity_year({'claims':{'P570':[approximate]}},'P570'))
        interval=claim(1950);interval['mainsnak']['datavalue']['value']['after']=1
        self.assertIsNone(kit.entity_year({'claims':{'P570':[interval]}},'P570'))
        bounded=claim(1950);bounded['qualifiers']={'P1319':[{'datavalue':{'value':{'time':'+1949-01-01T00:00:00Z'}}}]}
        self.assertIsNone(kit.entity_year({'claims':{'P570':[bounded]}},'P570'))
        calendar=claim(1910);calendar['qualifiers']={'P1480':[{'datavalue':{'value':{'id':'Q27055388'}}}]}
        self.assertEqual(kit.entity_year({'claims':{'P570':[calendar]}},'P570'),1910)
        calendar['mainsnak']['datavalue']['value']['time']='+1910-12-25T00:00:00Z'
        self.assertIsNone(kit.entity_year({'claims':{'P570':[calendar]}},'P570'))
        calendar['mainsnak']['datavalue']['value'].update({'time':'+1910-01-01T00:00:00Z','precision':9})
        self.assertIsNone(kit.entity_year({'claims':{'P570':[calendar]}},'P570'))
        julian=claim(1910);julian['mainsnak']['datavalue']['value'].update({'time':'+1910-12-25T00:00:00Z','calendarmodel':'http://www.wikidata.org/entity/Q1985786'})
        self.assertIsNone(kit.entity_year({'claims':{'P570':[julian]}},'P570'))
        julian['mainsnak']['datavalue']['value']['time']='+1910-07-01T00:00:00Z'
        self.assertEqual(kit.entity_year({'claims':{'P570':[julian]}},'P570'),1910)
        julian['mainsnak']['datavalue']['value']['time']='+1711-04-26T00:00:00Z'
        self.assertEqual(kit.entity_year({'claims':{'P570':[julian]}},'P570'),1711)
        julian['mainsnak']['datavalue']['value']['calendarmodel']='http://www.wikidata.org/entity/Q1985787'
        self.assertIsNone(kit.entity_year({'claims':{'P570':[julian]}},'P570'))
        self.assertIsNone(kit.entity_year({'claims':{'P570':[claim(1950),claim(1951)]}},'P570'))
        self.assertEqual(kit.entity_year({'claims':{'P570':[claim(1950),claim(1951,11,'preferred')]}},'P570'),1951)

class PageImageTest(unittest.TestCase):
    def test_html_or_empty_response_is_not_page_evidence(self):
        self.assertFalse(kit.valid_page_image(b'<html>Access denied</html>'))
        self.assertFalse(kit.valid_page_image(b''))

class QueueRightsWaveTest(unittest.TestCase):
    def test_partial_public_domain_and_unknown_dates_remain_distinct(self):
        w={'works':{'work':{'agents':['author'],'year':1928}},'agents':{'author':{'dates':[]}},
           'fragments':{'works':{}},'field':{'conceptions':[]},
           'lives':{'author':{'item':'Q1','died':[{'time':'+1995-01-01T00:00:00Z','precision':11}]}}}
        self.assertEqual(kit.wave(w,'work'),'free-some')
        w['lives']['author']['died']=[]
        self.assertEqual(kit.wave(w,'work'),'free-some')
        w['works']['work']['year']=None
        self.assertEqual(kit.wave(w,'work'),'quotation')
        w['works']['work']['year']=1900
        w['lives']['author']['died']=[{'time':'+1900-01-01T00:00:00Z','precision':11}]
        self.assertEqual(kit.wave(w,'work'),'free-all')
        w['lives']['author']['died']=[{'time':'+1900-01-01T00:00:00Z','precision':8}]
        self.assertEqual(kit.wave(w,'work'),'free-some')

class LanguageCoverageTest(unittest.TestCase):
    def test_missing_language_needs_exhaustive_search_and_a_book_locator(self):
        record={'reason':'No lawful text was found in the searched sources.',
                'edition':'A named edition, publisher and year.','book_url':'https://example.org/book',
                'tried':[{'site':site,'url':'https://example.org/search','outcome':'No usable text was found here.'} for site in kit.LOCATOR_SEARCHES]}
        self.assertEqual(kit.missing_language_problems(record),[])
        record['tried'].pop()
        self.assertTrue(any('search outcomes' in issue for issue in kit.missing_language_problems(record)))
        self.assertTrue(kit.missing_language_problems(None))

    def test_explicit_basis_does_not_skip_second_gate(self):
        import argparse
        import tempfile
        with tempfile.TemporaryDirectory() as path:
            args=argparse.Namespace(packet=path,basis='Partial reading accepted',offline=False,by='test')
            with patch.object(kit,'gate',return_value=(['missing second review'],'shut')) as gate:
                with self.assertRaises(SystemExit):kit.cmd_accept(args)
                self.assertTrue(gate.call_args.kwargs['second'])

class ScanRegistrationTest(unittest.TestCase):
    def test_missing_archive_item_is_not_registered_and_unused_scan_can_be_withdrawn(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)
            args=argparse.Namespace(packet=tmp,item='missing',dir=None,files=None,long_s=False)
            with patch.object(kit,'get',return_value=b'{}'):
                with self.assertRaises(SystemExit):kit.cmd_ia(args)
            self.assertFalse((path/'evidence.json').exists())
            scan={'item':'missing','ocr':False}
            (path/'evidence.json').write_text(json.dumps({'scan':scan,'scans':{'missing':scan},'places':{},'sources':[]}))
            kit.cmd_scan_withdraw(argparse.Namespace(packet=tmp,item='missing',reason='The Archive metadata does not identify this item.'))
            record=json.loads((path/'evidence.json').read_text())
            self.assertEqual(record['scans'],{})
            self.assertEqual(record['withdrawn_scans'][0]['scan'],scan)

class HtmlTranscriptionTest(unittest.TestCase):
    def test_mediawiki_content_is_not_hidden_by_skin_toc_class(self):
        raw=b'<html class="vector-feature-toc-not-available"><body><nav>Navigation</nav><div class="mw-parser-output"><div class="toc">Contents</div><p>A complete sentence.</p></div><footer>Footer</footer></body></html>'
        text=kit.plain(raw,'source.html')
        self.assertIn('A complete sentence.',text)
        self.assertNotIn('Navigation',text)
        self.assertNotIn('Contents',text)
        self.assertNotIn('Footer',text)

class ReopenUnplantedTest(unittest.TestCase):
    def test_reopen_keeps_predecessor_and_requires_new_review(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir()
            original={'accepted.json':{'by':'operator'},'reading.json':{'reader':{'id':'maker'}},'review.json':{'reader':{'id':'second'},'accept':True},'places.json':{'places':{}},'evidence.json':{'sources':[]}}
            for name,value in original.items():(path/name).write_text(json.dumps(value))
            args=argparse.Namespace(packet=str(path),reader='second',finding='The page says only when, which the reading omits.')
            world={'fragments':{'works':{},'provenance':{}},'field':{'conceptions':[],'links':[]}}
            with patch.object(kit,'world',return_value=world):kit.cmd_reopen(args)
            self.assertFalse((path/'accepted.json').exists())
            self.assertEqual(json.loads((path/'review.json').read_text()),{})
            self.assertEqual(json.loads((path/'reopen-history.json').read_text())[0]['predecessor'],original)
            (path/'accepted.json').write_text(json.dumps(original['accepted.json']))
            (path/'review.json').write_text(json.dumps(original['review.json']))
            (path/'accepted.json').write_text(json.dumps({'by':'operator','page_number_reviews':[{'sha256':'a'*64}]}))
            with patch.object(kit,'world',return_value=world),self.assertRaises(SystemExit):kit.cmd_reopen(args)
            self.assertTrue((path/'accepted.json').exists())
            (path/'accepted.json').write_text(json.dumps(original['accepted.json']))
            world['fragments']['works']['work']={}
            with patch.object(kit,'world',return_value=world),self.assertRaises(SystemExit):kit.cmd_reopen(args)
            self.assertTrue((path/'accepted.json').exists())

class UnknownParticipantTest(unittest.TestCase):
    def test_source_backed_unknown_death_does_not_establish_personal_term(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'packet';path.mkdir()
            args=argparse.Namespace(packet=str(path),role='author',qid=None,name='Rebecca Journey',ru=None,died=None,source='https://example.org/article',remove=False,wartime=None,rehabilitated=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=b'<html><body>Rebecca Journey</body></html>'):
                kit.cmd_person(args)
                person=json.loads((path/'evidence.json').read_text())['persons'][0]
                self.assertIsNone(person['died'])
                self.assertEqual(person['identity_evidence']['url'],args.source)
                self.assertFalse(kit.public_domain_by_dates([person],2025)['eu'])
                self.assertFalse(kit.public_domain_by_dates([person],2025)['russia'])
                args.name='Another Author'
                with patch.object(kit,'get',return_value=b'<html><body>Another Author</body></html>'):
                    kit.cmd_person(args)
                other=json.loads((path/'evidence.json').read_text())['persons'][1]
                self.assertNotEqual(person['identity_evidence']['file'],other['identity_evidence']['file'])
                self.assertTrue(Path(kit.cache(str(path),person['identity_evidence']['file'])).is_file())

    def test_remove_wrong_witness_identity_keeps_history_and_clears_review(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp);(path/'witnesses').mkdir()
            file=path/'witnesses/ru.json'
            wrong={'role':'author','qid':'Q358'};right={'role':'author','qid':'Q35802'}
            file.write_text(json.dumps({'witness':{'persons':[wrong,right]},'review':{'reader':'second'}}))
            args=argparse.Namespace(packet=tmp,language='ru',role='author',qid='Q358',reason='This identifier refers to an unrelated heritage site.')
            kit.cmd_witness_person_remove(args)
            result=json.loads(file.read_text())
            self.assertEqual(result['witness']['persons'],[right])
            self.assertEqual(result['person_corrections'][0]['removed'],wrong)
            self.assertEqual(result['review'],{})
            (path/'witnesses/ru.json.accepted').write_text('{}')
            args.qid='Q35802'
            with self.assertRaises(SystemExit):kit.cmd_witness_person_remove(args)

class InterfaceCoverageGateTest(unittest.TestCase):
    def test_new_work_requires_evidence_and_old_work_records_debt(self):
        import tempfile
        with tempfile.TemporaryDirectory() as path:
            problems,debts=kit.language_coverage(path,{'absent':[]},'en')
            self.assertEqual(len(problems),4);self.assertEqual(debts,[])
            self.assertTrue(any(item.startswith('zh-Hans:') for item in problems))
            problems,debts=kit.language_coverage(path,{'absent':[]},'en',legacy_work=True)
            self.assertEqual(problems,[]);self.assertEqual(len(debts),4)
            entry={'absent':[],'witnesses':{'ru':{}}}
            problems,debts=kit.language_coverage(path,entry,'en',legacy_work=True,held=True)
            self.assertEqual(problems,[]);self.assertFalse(any(item.startswith('ru:') for item in debts))
            import json
            from pathlib import Path
            (Path(path)/'places.json').write_text(json.dumps({'places':{'old':{},'new':{}}}))
            held={'fragments':{'places':{Path(path).name:{'old':{'texts':{'ru':'перевод'}}}}}}
            with patch.object(kit,'world',return_value=held):
                problems,debts=kit.language_coverage(path,entry,'en',legacy_work=True,held=True)
            self.assertEqual(problems,[])
            self.assertTrue(any(item.startswith('ru new:') for item in debts))
            self.assertFalse(any(item.startswith('ru old:') for item in debts))

class PartialWitnessCoverageTest(unittest.TestCase):
    def test_new_work_accounts_for_every_missing_translated_place(self):
        import json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp);(path/'witnesses').mkdir()
            (path/'places.json').write_text(json.dumps({'places':{'p1':{},'p2':{}}}))
            file=path/'witnesses/ru.json';witness={'language':'ru','places':{'p1':{}}}
            file.write_text(json.dumps(witness))
            entry={'absent':[]};gap={'reason':'No lawful text was found in the searched sources.',
                'edition':'A named edition, publisher and year.','book_url':'https://example.org/book',
                'tried':[{'site':site,'url':'https://example.org/search','outcome':'No usable text was found here.'} for site in kit.LOCATOR_SEARCHES]}
            with patch.object(kit,'witness_problems',return_value=[]):
                problems,_=kit.language_coverage(str(path),entry,'en')
                self.assertIn('ru p2: no search record',problems)
                witness['missing_places']={'p2':gap};file.write_text(json.dumps(witness))
                problems,_=kit.language_coverage(str(path),entry,'en')
                self.assertFalse(any(p.startswith('ru p2:') for p in problems))
                witness.pop('missing_places');file.write_text(json.dumps(witness))
                problems,debts=kit.language_coverage(str(path),entry,'en',legacy_work=True)
                self.assertFalse(problems)
                self.assertTrue(any(p.startswith('ru: witness is research') for p in debts))
                import hashlib
                (path/'witnesses/ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(file.read_bytes()).hexdigest()}))
                problems,debts=kit.language_coverage(str(path),entry,'en',legacy_work=True)
                self.assertFalse(problems)
                self.assertIn('ru p2: no search record',debts)

class CoauthorIdentityTest(unittest.TestCase):
    def test_distinct_qids_cannot_be_collapsed_by_matching_names(self):
        a={'qid':'Q1','name':{'en':'Alex Smith'}}
        b={'qid':'Q2','name':{'en':'Alex Smith'}}
        self.assertFalse(kit.same_participant(a,b))
        self.assertTrue(kit.same_participant(a,{'name':{'en':'Alex Smith'}}))
        self.assertFalse(kit.same_participant(a,{'name':{'en':'Alex Jones'}}))

class CatalogueIdentityTest(unittest.TestCase):
    def test_work_author_and_wikidata_identity_must_agree(self):
        import json
        import shutil
        import subprocess
        import tempfile
        from pathlib import Path
        root=Path(kit.ROOT);original=Path(kit.REVIEW)
        with tempfile.TemporaryDirectory() as tmp:
            target=Path(tmp);review=target/'bonsai/review/aesthetics';layer=review/'layer';layer.mkdir(parents=True)
            (target/'bonsai/lib').symlink_to(root/'bonsai/lib',target_is_directory=True)
            (review/'planting/packets').mkdir(parents=True)
            shutil.copy2(original/'chronology.mjs',review/'chronology.mjs')
            agents=[{'id':'karl','name':{'en':'Karl Rosenkranz','ru':'Карл Розенкранц'}},{'id':'other','name':{'en':'Different Person','ru':'Другой'}},{'id':'dates','name':{'en':'Dated Author','ru':'Автор'}},{'id':'uncertain','name':{'en':'Uncertain Author','ru':'Автор'}}]
            works=[{'id':'karl.book','agents':['karl'],'title':{'original':'Book'}},{'id':'other.book','agents':['other'],'title':{'original':'Other Book'}},{'id':'dates.book','agents':['dates'],'title':{'original':'Dated Book'}},{'id':'uncertain.book','agents':['uncertain'],'title':{'original':'Uncertain Book'}}]
            for name,value in [('agents',agents),('works',works),('sources',[]),('lives',{})]:(layer/(name+'.json')).write_text(json.dumps(value,ensure_ascii=False))
            home=target/'bonsai/aesthetics';home.mkdir();(home/'library.json').write_text('{"agents":{},"works":{}}')
            script="""
import {pathToFileURL} from 'node:url';
globalThis.setTimeout=fn=>{fn();return 0};
globalThis.fetch=async input=>{
 const url=new URL(String(input));let data;
 if(url.pathname==='/search.json'){
   const name=url.searchParams.get('author'),title=url.searchParams.get('title');
   data={docs:[{key:'/works/OL123W',title,author_name:[name],author_key:[name==='Dated Author'?'OL456A':name==='Uncertain Author'?'OL789A':'OL123A']}]};
 }else if(url.pathname==='/authors/OL123A.json')data={name:'Karl Rosenkranz',remote_ids:{viaf:'12345'}};
 else if(url.pathname==='/authors/OL456A.json')data={name:'Dated Author',birth_date:'1900',death_date:'1980'};
 else if(url.pathname==='/authors/OL789A.json')data={name:'Uncertain Author',birth_date:'circa 1900',death_date:'1980'};
 else if(url.hostname==='query.wikidata.org')data={results:{bindings:[{person:{value:'http://www.wikidata.org/entity/'+(url.searchParams.get('query').includes('P214')?'Q62315':'Q77')}}]}};
 else if(url.pathname.includes('Special:EntityData/Q62315'))data={entities:{Q62315:{labels:{en:{value:'Karl Rosenkranz'}},claims:{P31:[{mainsnak:{datavalue:{value:{id:'Q5'}}}}],P569:[{qualifiers:{P1480:[{datavalue:{value:{id:'Q5727902'}}}],P1319:[{datavalue:{value:{time:'+1804-01-01T00:00:00Z',precision:9}}}],P1326:[{datavalue:{value:{time:'+1806-01-01T00:00:00Z',precision:9}}}]},mainsnak:{datavalue:{value:{time:'+1805-01-01T00:00:00Z',precision:9}}}}],P570:[{mainsnak:{datavalue:{value:{time:'+1879-01-01T00:00:00Z',precision:9}}}}]}}}};
 else if(url.pathname.includes('Special:EntityData/Q77'))data={entities:{Q77:{labels:{en:{value:'Dated Author'}},claims:{P31:[{mainsnak:{datavalue:{value:{id:'Q5'}}}}],P569:[{mainsnak:{datavalue:{value:{time:'+1900-01-01T00:00:00Z',precision:9}}}}],P570:[{mainsnak:{datavalue:{value:{time:'+1980-01-01T00:00:00Z',precision:9}}}}]}}}};
 else throw new Error('unexpected URL '+url);
 return new Response(JSON.stringify(data),{status:200,headers:{'Content-Type':'application/json'}});
};
process.argv=['node','chronology.mjs','--fetch-queue-catalogue'];
await import(pathToFileURL(process.argv[3]??'%MODULE%').href);
""".replace('%MODULE%',str(review/'chronology.mjs'))
            subprocess.run(['node','--input-type=module','-e',script],check=True,capture_output=True,text=True)
            lives=json.loads((layer/'lives.json').read_text());self.assertEqual(lives['karl']['item'],'Q62315')
            self.assertTrue(lives['karl']['born'][0]['approximate'])
            self.assertEqual(lives['karl']['born'][0]['qualifiers'],['Q5727902'])
            self.assertEqual(lives['karl']['born'][0]['bounds']['earliest'][0]['time'],'+1804-01-01T00:00:00Z')
            self.assertEqual(lives['karl']['born'][0]['bounds']['latest'][0]['time'],'+1806-01-01T00:00:00Z')
            self.assertEqual(lives['karl']['identity_catalogue']['work'],'/works/OL123W')
            self.assertEqual(lives['karl']['identity_catalogue']['authority'],['P214','12345'])
            self.assertIsNone(lives['other']['item']);self.assertEqual(lives['other']['status'],'unresolved')
            self.assertEqual(lives['dates']['item'],'Q77');self.assertEqual(lives['dates']['identity_catalogue']['matched_years'],{'born':1900,'died':1980})
            self.assertIsNone(lives['uncertain']['item']);self.assertEqual(lives['uncertain']['status'],'unresolved')

class OrcidIdentityTest(unittest.TestCase):
    def test_exact_publication_author_and_unique_orcid_are_required(self):
        import json, shutil, subprocess, tempfile
        from pathlib import Path
        root=Path(kit.ROOT);original=Path(kit.REVIEW)
        with tempfile.TemporaryDirectory() as tmp:
            target=Path(tmp);review=target/'bonsai/review/aesthetics';layer=review/'layer';layer.mkdir(parents=True)
            (target/'bonsai/lib').symlink_to(root/'bonsai/lib',target_is_directory=True)
            (review/'planting/packets').mkdir(parents=True);shutil.copy2(original/'chronology.mjs',review/'chronology.mjs')
            agents=[{'id':'matched','name':{'en':'Ada Example'}},{'id':'wrong','name':{'en':'Wrong Name'}},{'id':'ambiguous','name':{'en':'Ambiguous Name'}}]
            works=[{'id':a['id']+'.paper','agents':[a['id']],'title':{'original':a['id']+' article'}} for a in agents]
            for name,value in [('agents',agents),('works',works),('sources',[]),('lives',{})]:(layer/(name+'.json')).write_text(json.dumps(value))
            home=target/'bonsai/aesthetics';home.mkdir();(home/'library.json').write_text('{"agents":{},"works":{}}')
            script="""
import {pathToFileURL} from 'node:url';
globalThis.setTimeout=fn=>{fn();return 0};
globalThis.fetch=async input=>{
 const url=new URL(String(input));let data;
 if(url.hostname==='api.crossref.org'){
   const title=url.searchParams.get('query.title');
   const author=title.startsWith('matched')?{given:'Ada',family:'Example',ORCID:'https://orcid.org/0000-0001-2345-6789'}:title.startsWith('wrong')?{given:'Different',family:'Name',ORCID:'https://orcid.org/0000-0001-2345-6789'}:{given:'Ambiguous',family:'Name',ORCID:'https://orcid.org/0000-0001-2345-6789'};
   const authors=title.startsWith('ambiguous')?[author,{...author,ORCID:'https://orcid.org/0000-0001-2345-679X'}]:[author];
   data={message:{items:[{DOI:'10.1/test',title:[title],author:authors}]}};
 }else if(url.hostname==='query.wikidata.org')data={results:{bindings:[{person:{value:'http://www.wikidata.org/entity/Q123'}}]}};
 else if(url.pathname.includes('Special:EntityData/Q123'))data={entities:{Q123:{labels:{en:{value:'Ada Example'}},claims:{P569:[{mainsnak:{datavalue:{value:{time:'+1900-01-01T00:00:00Z',precision:9}}}}]}}}};
 else throw new Error('unexpected URL '+url);
 return new Response(JSON.stringify(data),{status:200});
};
process.argv=['node','chronology.mjs','--fetch-queue-orcid'];
await import(pathToFileURL('%MODULE%').href);
""".replace('%MODULE%',str(review/'chronology.mjs'))
            subprocess.run(['node','--input-type=module','-e',script],check=True,capture_output=True,text=True)
            lives=json.loads((layer/'lives.json').read_text())
            self.assertEqual(lives['matched']['item'],'Q123')
            self.assertEqual(lives['matched']['identity_orcid']['doi'],'10.1/test')
            self.assertIsNone(lives['wrong']['item']);self.assertIsNone(lives['ambiguous']['item'])

class NewWorkLanguageIntegrationTest(unittest.TestCase):
    def test_new_work_needs_each_language_and_second_readings(self):
        import copy
        import json
        import shutil
        import tempfile
        from pathlib import Path
        work='alexander-gottlieb-baumgarten.aesthetica';source=Path(kit.PACKETS)/work
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/work;path.mkdir()
            for name in ('places.json','reading.json','evidence.json','review.json'):shutil.copy2(source/name,path/name)
            places=json.loads((path/'places.json').read_text());ev=json.loads((path/'evidence.json').read_text())
            rights=kit.rights_lines(ev);rights['reasoning']=places['entry']['witnesses']['la']['rights']['reasoning']
            places['entry']['witnesses']['la']['rights']=rights;places['entry']['absent']=[]
            (path/'places.json').write_text(json.dumps(places,ensure_ascii=False))
            kit.OWES.clear();problems,_=kit.gate(str(path),offline=True)
            self.assertTrue(any(item.startswith('ru: no search record') for item in problems))
            directory=path/'witnesses';directory.mkdir();loc=next(iter(places['places']))
            author=copy.deepcopy(next(person for person in ev['persons'] if person['role']=='author'))
            for person in (author,):
                person['evidence']={'file':'person.json','sha256':'a'*64,'url':f"https://www.wikidata.org/wiki/Special:EntityData/{person['qid']}.json"}
            translator={'role':'translator','qid':'Q2','name':{'en':'Translator'},'died':1800,'source':'https://www.wikidata.org/wiki/Q2',
                        'evidence':{'file':'translator.json','sha256':'b'*64,'url':'https://www.wikidata.org/wiki/Special:EntityData/Q2.json'}}
            by_country={key:{'basis':'public-domain','law':'The relevant public-domain statutory term.'} for key in kit.JURISDICTIONS}
            for language in ('ru','en','es','de','zh-Hans'):
                data={'language':language,'witness':{'maker':'maker','edition':'A cited edition','attribution':'Author and translator','licence':'Public domain','url':'https://example.org/book','year':1900,'persons':[author,translator], 'rights':by_country,'normalization':'Modern Russian source and documented normalization.'},
                      'sources':[{'file':'text.txt','url':'https://example.org/text','sha256':'c'*64}],
                      'images':[{'file':'page.jpg','url':'https://example.org/page','sha256':'d'*64,'by':'maker'}, {'file':'second.jpg','url':'https://example.org/page','sha256':'d'*64,'by':'second'}],
                      'places':{place:{'text':'A whole sentence.','source':'text.txt','page':'1','url':'https://example.org/page','image_sha256':'d'*64,'alignment':'The sentence corresponds to the original place.'} for place in places['places']},
                      'review':{'reader':'second','model':'gpt-6.1-terra','date':'2026-11-12','places':{place:{'reads':'as the packet','image_sha256':'d'*64,'alignment':'The sentence corresponds to the original place.'} for place in places['places']}}}
                (directory/(language+'.json')).write_text(json.dumps(data,ensure_ascii=False))
            kit.OWES.clear();problems,state=kit.gate(str(path),offline=True)
            self.assertEqual(problems,[]);self.assertEqual(state,'open')
            kit.OWES.clear();problems,state=kit.gate(str(path),second=True,offline=True)
            self.assertEqual(problems,[]);self.assertEqual(state,'open')
            russian=json.loads((directory/'ru.json').read_text());russian['review']={}
            (directory/'ru.json').write_text(json.dumps(russian,ensure_ascii=False))
            kit.OWES.clear();problems,_=kit.gate(str(path),second=True,offline=True)
            self.assertTrue(any('second reader' in item for item in problems))
            (directory/'ru.json').unlink()
            kit.OWES.clear();problems,_=kit.gate(str(path),offline=True)
            self.assertTrue(any(item.startswith('ru: no search record') for item in problems))
