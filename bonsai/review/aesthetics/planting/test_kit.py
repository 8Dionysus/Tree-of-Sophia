import unittest
from unittest.mock import patch

import kit


class AgentDateCorrectionTest(unittest.TestCase):
    def test_source_correction_keeps_register_wording_and_is_idempotent(self):
        import importlib.util
        from pathlib import Path
        spec=importlib.util.spec_from_file_location('aesthetics_unify',Path(kit.REVIEW)/'unify.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        agents=[{'id':'person','keys':['B:1'],'dates':['1939–2013','род. 1939']}];corrections=[]
        decision={'target':'B:1','agent':'person','was':'1939–2013','now':'род. 1939','verified_at':['https://example.org/cv.pdf'],'source_sha256':'a'*64,'statement':'The later university CV contradicts the asserted death year.','by':'reviewer'}
        module.apply_agent_date_corrections(agents,corrections,[decision]);module.apply_agent_date_corrections(agents,corrections,[decision])
        self.assertEqual(agents[0]['dates'],['род. 1939']);self.assertEqual(agents[0]['register_dates'],[{'key':'B:1','dates':'1939–2013'}]);self.assertEqual(len(corrections),1)
        with self.assertRaises(ValueError):module.apply_agent_date_corrections([{'id':'person','keys':['B:1'],'dates':['other']}],[],[decision])


class WithdrawnOriginalPageTest(unittest.TestCase):
    def test_wrong_external_image_is_retained_and_needs_a_new_page(self):
        import argparse, base64, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();cache=Path(tmp)/'cache/work';cache.mkdir(parents=True)
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');(cache/'wrong.png').write_bytes(raw)
            image={'item':'external','loc':'p','n':8,'file':'wrong.png','url':'https://example.org/book.pdf','sha256':hashlib.sha256(raw).hexdigest(),'by':'maker'}
            ev={'images':[image],'places':{'p':{'method':'image','scan':'external','images':[8]}}};(path/'evidence.json').write_text(json.dumps(ev));(path/'review.json').write_text(json.dumps({'reader':{'id':'reader'}}))
            args=argparse.Namespace(packet=str(path),loc='p',file='wrong.png',replacement_page=9,reason='The file was rendered from a PDF and recorded as a directly retrieved image.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_page_withdraw(args);after=json.loads((path/'evidence.json').read_text())
                self.assertEqual(after['withdrawn_page_images'][0]['image'],image);self.assertEqual(after['withdrawn_page_images'][0]['review']['reader']['id'],'reader');self.assertEqual(after['places']['p']['images'],[])
                self.assertTrue(any('replacement' in p for p in kit.withdrawn_page_problems(str(path),after,['p'])))
                after['images'].append({'loc':'p','n':9,'file':'pdf-derived.png','by':'maker'})
                self.assertEqual(kit.withdrawn_page_problems(str(path),after,['p']),[])
                (cache/'wrong.png').write_bytes(raw+b'changed');self.assertTrue(any('changed' in p for p in kit.withdrawn_page_problems(str(path),after,['p'])))


class WitnessPageIdentityTest(unittest.TestCase):
    def test_different_renderings_need_bound_independent_assessment(self):
        import argparse, base64, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            first=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            second=first+b'changed rendering';(folder/'maker.png').write_bytes(first);(folder/'reader.png').write_bytes(second)
            maker={'file':'maker.png','url':'https://example.org/maker','sha256':hashlib.sha256(first).hexdigest(),'by':'maker'};other={'file':'reader.png','url':'https://example.org/reader','sha256':hashlib.sha256(second).hexdigest(),'by':'second'}
            page={'page':'652','url':maker['url'],'image_sha256':maker['sha256']};data={'language':'en','witness':{'maker':'maker','edition':'Huerta 1629'},'images':[maker,other],'places':{'p':page},'review':{}}
            file=path/'witnesses/en.json';file.write_text(json.dumps(data))
            self.assertIsNone(kit.second_witness_image(data,page,'reader'))
            args=argparse.Namespace(packet=str(path),language='en',loc='p',page='652',image='reader.png',reader='reader',reader_url='https://archive.org/details/book/page/n669',method='Independently retrieved the archive page as a JPEG rendering.',assessment='I compared the printed page number, surrounding layout, and the passage on both images of this edition and found the same page.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):kit.cmd_witness_page_identity(args)
            reviewed=json.loads(file.read_text());self.assertEqual(kit.second_witness_image(reviewed,page,'reader'),other)
            self.assertIsNone(kit.second_witness_image(reviewed,{**page,'page':'653'},'reader'))
            self.assertIsNone(kit.second_witness_image(reviewed,page,'another-reader'))
            reviewed['witness']['edition']='different edition';self.assertIsNone(kit.second_witness_image(reviewed,page,'reader'))


class WitnessWithdrawalTest(unittest.TestCase):
    def test_retiring_unaccepted_witness_preserves_its_review_and_refuses_accepted(self):
        import argparse, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/ru.json'
            raw=json.dumps({'language':'ru','witness':{'maker':'maker'},'review':{'reader':'reader','finding':'alignment concern'}}).encode();file.write_bytes(raw)
            args=argparse.Namespace(packet=str(path),language='ru',maker='maker',reason='The existing edition requires a different documented source and rights assessment.')
            world={'fragments':{'works':{'work':{'witnesses':{}}},'places':{'work':{}}}}
            marker=Path(str(file)+'.accepted');marker.write_text('{}')
            with patch.object(kit,'world',return_value=world),self.assertRaises(SystemExit):kit.cmd_witness_retire(args)
            marker.unlink()
            with patch.object(kit,'world',return_value=world):kit.cmd_witness_retire(args)
            history=path/'retired-witnesses'/'ru'/hashlib.sha256(raw).hexdigest()
            self.assertEqual((history/'witness.json').read_bytes(),raw);self.assertFalse(file.exists());self.assertEqual(kit.witness_withdrawal_problems(str(path),'ru'),[])
            (history/'witness.json').write_bytes(raw+b' ');self.assertTrue(kit.witness_withdrawal_problems(str(path),'ru'))

    def test_reopen_preserves_original_and_requires_new_reader_review(self):
        import argparse, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/ru.json'
            data={'language':'ru','witness':{'maker':'maker'},'places':{'p':{'text':'words'}},'review':{'reader':'second','places':{'p':{'reads':'as the packet'}}}}
            raw=json.dumps(data).encode();file.write_bytes(raw);marker=Path(str(file)+'.accepted');marker.write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            args=argparse.Namespace(packet=str(path),language='ru',reader='second',finding='The published wording and quotation basis need a further independent assessment.',replace=False)
            world={'fragments':{'works':{'work':{'witnesses':{}}},'places':{'work':{}}}}
            with patch.object(kit,'world',return_value=world):kit.cmd_witness_withdraw(args)
            current=json.loads(file.read_text());self.assertEqual(current['places'],data['places']);self.assertEqual(current['review'],{});self.assertFalse(marker.exists())
            self.assertEqual((path/'witness-history'/'ru'/hashlib.sha256(raw).hexdigest()/'witness.json').read_bytes(),raw)

    def test_unplanted_withdrawal_preserves_exact_receipt_and_rejects_planted_or_tampered(self):
        import argparse, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            file=path/'witnesses/ru.json';marker=Path(str(file)+'.accepted')
            data={'language':'ru','witness':{'maker':'maker'},'places':{'p':{'text':'words'}},'review':{'reader':'second','places':{'p':{'reads':'as the packet'}}}}
            raw=json.dumps(data,ensure_ascii=False).encode();file.write_bytes(raw);digest=hashlib.sha256(raw).hexdigest();receipt=json.dumps({'sha256':digest,'date':'2026-11-27'}).encode();marker.write_bytes(receipt)
            args=argparse.Namespace(packet=str(path),language='ru',reader='second',finding='The later review found that the applicable quotation conditions need separate assessment.',replace=True)
            def world(witnesses):return {'fragments':{'works':{'work':{'witnesses':witnesses}},'places':{'work':{}}}}
            with patch.object(kit,'world',return_value=world({'ru':{}})),self.assertRaises(SystemExit):kit.cmd_witness_withdraw(args)
            self.assertEqual(file.read_bytes(),raw);self.assertEqual(marker.read_bytes(),receipt)
            marker.write_text('{"sha256":"incorrect"}')
            with patch.object(kit,'world',return_value=world({})),self.assertRaises(SystemExit):kit.cmd_witness_withdraw(args)
            marker.write_bytes(receipt)
            with patch.object(kit,'world',return_value=world({})):kit.cmd_witness_withdraw(args)
            history=path/'witness-history'/'ru'/digest
            self.assertEqual((history/'witness.json').read_bytes(),raw);self.assertEqual((history/'acceptance.json').read_bytes(),receipt)
            self.assertFalse(file.exists());self.assertFalse(marker.exists())
            self.assertEqual(kit.witness_withdrawal_problems(str(path),'ru'),[])
            (history/'acceptance.json').write_bytes(receipt+b' ')
            self.assertTrue(kit.witness_withdrawal_problems(str(path),'ru'))


class LanguageSearchPrecedenceTest(unittest.TestCase):
    def test_a_later_accepted_translation_takes_precedence_over_the_parent_search(self):
        import hashlib
        import json
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'author.work';(path/'language-searches').mkdir(parents=True)
            search={'language':'es','search':{'maker':'maker','places':['p1'],'reason':'The documented searches found no suitable text.','tried':[]},'review':{}}
            raw=json.dumps(search).encode();(path/'language-searches/es.json').write_bytes(raw)
            (path/'language-searches/es.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            def world(witnesses):
                return {'fragments':{'works':{'author.work':{'original':'de','witnesses':witnesses}},'places':{'author.work':{'p1':{}}}}}
            es=lambda problems:[problem for problem in problems if problem.startswith('es:')]
            # While the language is missing, the accepted search is what the packet holds for it.
            with patch.object(kit,'world',return_value=world({'de':{}})):
                problems,_=kit.language_coverage(str(path),{},'de',second=True,offline=True)
                self.assertFalse(any('still be missing' in problem for problem in es(problems)))
            # An extension has since brought a Spanish translation: the parent's search gives way to it.
            with patch.object(kit,'world',return_value=world({'de':{},'es':{}})):
                problems,_=kit.language_coverage(str(path),{},'de',second=True,offline=True)
                self.assertEqual(es(problems),[])
            # A search that was never accepted is still checked against the work as it stands.
            (path/'language-searches/es.json.accepted').unlink()
            with patch.object(kit,'world',return_value=world({'de':{},'es':{}})):
                problems,_=kit.language_coverage(str(path),{},'de',second=True,offline=True)
                self.assertTrue(any('still be missing' in problem for problem in es(problems)))


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

    def test_indic_words_and_danda_keep_their_boundaries(self):
        self.assertEqual(kit.letters('नाट्यशास्त्रम्'), 'नाट्यशास्त्रम्')
        self.assertNotEqual(kit.letters('कला'), kit.letters('कल'))
        self.assertEqual(kit.changed('कला', 'कल'), [('कला', 'कल')])
        self.assertEqual(kit.sentences('नाट्यशास्त्रम्।'), [])
        self.assertEqual(kit.sentences('नाट्यशास्त्रम्॥'), [])
        self.assertIsNone(kit.in_transcription('नाट्यशास्त्रम्।', 'प्रथमम्। नाट्यशास्त्रम्।'))
        self.assertEqual([x[0] for x in kit.tokens('一二三。')], ['一','二','三','。'])
        self.assertEqual([x[0] for x in kit.tokens('かな漢字。')], ['か','な','漢','字','。'])
        self.assertEqual(kit.changed('學', '学'), [('學', '学')])
        self.assertEqual(kit.letters('ςϲσ'), kit.letters('σσσ'))

    def test_lookup_folds_preserve_printed_differences_for_review(self):
        self.assertEqual(kit.iast('नाट्यशास्त्रम्'), 'nāṭyaśāstram')
        self.assertEqual(kit.key('ivvenis',script='la'), kit.key('iuuenis',script='la'))
        self.assertNotEqual(kit.key('ivvenis'), kit.key('iuuenis'))
        self.assertEqual(kit.key('學',script='zh'), kit.key('学',script='zh'))
        for transcription, printed, script in [('學','学','zh'), ('ivvenis','iuuenis','la'), ('नाट्यशास्त्रम्','nāṭyaśāstram','sa')]:
            o={'words':[printed], 'at':[0], 'starts':[0], 'printed':{}, 'long_s':False}
            found=kit.against_ocr(transcription,o,script=script)
            self.assertEqual(found['share'],1)
            self.assertEqual(found['differences'][0]['ocr'],printed)

    def test_cjk_correction_without_ocr_support_requires_enlarged_review(self):
        places={'work':'w','corrections':[{'place':'w p1','from':'天地元黄宇宙洪荒','to':'天地玄黄宇宙洪荒'}]}
        ev={'scan':{'item':'s'},'places':{'p1':{'method':'ocr','differences':[{'transcription':'玄','ocr':'天'}]}}}
        self.assertEqual([loc for loc,_ in kit.by_eye(places,ev)], ['p1'])

    def test_cjk_deletion_contradicted_by_ocr_requires_enlarged_review(self):
        places={'work':'w','corrections':[{'place':'w p1','from':'天地玄黄宇宙洪荒','to':'天地黄宇宙洪荒'}]}
        words=list('天地玄黄宇宙洪荒')
        o={'words':words,'at':list(range(len(words))),'starts':[0],'printed':{},'long_s':False}
        comparison=kit.against_ocr('天地黄宇宙洪荒。',o,script='zh')
        ev={'scan':{'item':'s'},'places':{'p1':{'method':'ocr','differences':comparison['differences']}}}
        self.assertEqual([loc for loc,_ in kit.by_eye(places,ev)], ['p1'])
        ev['places']['p1']['differences']=[]
        self.assertEqual(kit.by_eye(places,ev), [])

    def test_quotation_counts_danda_sentences(self):
        proof={'source':'https://example.org/page','page':'1','lines':'1–4','sha256':'a'*64}
        ev={'images':[{'sha256':'a'*64,'by':'maker'}]}
        self.assertTrue(any('exceeds' in p for p in kit.quotation_problems('p','एकम्। द्वे। त्रीणि। चत्वारि।',proof,ev)))

    def test_greek_question_mark_is_a_sentence_boundary(self):
        self.assertEqual(kit.sentences('Τί ἐστι θεός;'), [])
        self.assertEqual(kit.sentences('εἰ ξυμπονήσεις;'), [])
        self.assertTrue(kit.sentences('This is not a complete sentence;'))
        self.assertIsNone(kit.in_transcription('Τί ἐστι θεός;', 'Τί λέγεις; Τί ἐστι θεός;'))
        proof={'source':'https://example.org/page','page':'1','lines':'1–4','sha256':'a'*64}
        ev={'images':[{'sha256':'a'*64,'by':'maker'}]}
        self.assertTrue(any('exceeds' in p for p in kit.quotation_problems('p','Τί; Πῶς; Ποῦ; Πότε;',proof,ev)))

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
                data['witness']['persons'].append('Unsupported provisional name')
                self.assertTrue(any('unsupported provisional participant' in issue for issue in kit.witness_problems(str(path),'en',data,second=True)))
                data['witness']['persons'].pop()
                saved_rights=data['witness']['rights'];data['witness']['rights']={'russia':'legacy','usa':'legacy','eu':'legacy'}
                self.assertTrue(any('rights.eu needs basis' in issue for issue in kit.witness_problems(str(path),'en',data,second=True)))
                data['witness']['rights']='invalid'
                self.assertTrue(any('witness.rights must be an object' in issue for issue in kit.witness_problems(str(path),'en',data,second=True)))
                data['witness']['rights']=saved_rights
                import copy
                bounded=copy.deepcopy(data);author=bounded['witness']['persons'][0];raw=b'<html><body>Author is an ancient anonymous author.</body></html>'
                (folder/'ancient.html').write_bytes(raw);author.pop('qid');author.update({'died':None,'died_before':1000,'source':'https://en.wikipedia.org/wiki/Author','death_bound':{'year':1000,'from':'https://en.wikipedia.org/wiki/Author','note':'The reference describes the author in an ancient historical period.'},'evidence':{'kind':'reference','file':'ancient.html','sha256':hashlib.sha256(raw).hexdigest(),'url':'https://en.wikipedia.org/wiki/Author'}})
                self.assertTrue(any('independent reference assessment' in p for p in kit.witness_problems(str(path),'en',bounded,second=True)))
                bounded['review']['bounds']={kit.page_number_digest(author):{'reader':'reader','assessment':'The reference places this author in antiquity, earlier than the recorded conservative upper bound.'}}
                self.assertEqual(kit.witness_problems(str(path),'en',bounded,second=True),[])
                author['death_bound']['year']=1001;self.assertTrue(any('sourced death bound' in p for p in kit.witness_problems(str(path),'en',bounded,second=True)))
                import argparse, copy
                (path/'witnesses').mkdir();file=path/'witnesses/en.json'
                prior=copy.deepcopy(data);prior['places']['p']['page']='XXX.1 p953a 10–14';prior['places']['p']['corrections']=[{'from':'whole','to':'whole','page':prior['places']['p']['page'],'url':prior['places']['p']['url'],'image_sha256':digest}];file.write_text(json.dumps(prior))
                args=argparse.Namespace(packet=str(path),language='en',loc='p',page='953a',section='XXX.1',line_start=10,line_end=14,reason='The range identifies Bekker lines on this one printed page, rather than multiple printed pages.')
                with patch.object(kit,'packet',return_value=str(path)),self.assertRaises(SystemExit):kit.cmd_witness_cut(args)
                self.assertEqual(json.loads(file.read_text()),prior)
                with patch.object(kit,'packet',return_value=str(path)):kit.cmd_witness_address_correct(args)
                fixed=json.loads(file.read_text());self.assertEqual(fixed['address_corrections'][0]['predecessor'],prior['places']['p']);self.assertEqual(fixed['places']['p']['corrections'][0]['page'],'953a');self.assertEqual(fixed['review'],{})
                self.assertFalse(any('page range' in issue or 'line locator' in issue for issue in kit.witness_problems(str(path),'en',fixed)))
                fixed['places']['p']['page']='953–954';self.assertTrue(any('page range' in issue for issue in kit.witness_problems(str(path),'en',fixed)))
                args.line_end=9
                with patch.object(kit,'packet',return_value=str(path)),self.assertRaises(SystemExit):kit.cmd_witness_address_correct(args)
                (path/'witnesses/en.json.accepted').write_text('{}');args.line_end=14
                with patch.object(kit,'packet',return_value=str(path)),self.assertRaises(SystemExit):kit.cmd_witness_address_correct(args)
                (path/'witnesses/en.json.accepted').unlink()
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
                before=copy.deepcopy(data);file=path/'witnesses/en.json';file.write_text(json.dumps(data))
                correction=argparse.Namespace(packet=str(path),language='en',loc='p',to='statement.',reason='The second page reads statement rather than sentence.',image='page2.jpg',**{'from':'sentence.'})
                kit.cmd_witness_correct(correction)
                kit.cmd_witness_review(argparse.Namespace(packet=str(path),language='en',loc='p',reader='reader',model='gpt-6.1-terra',alignment='Both pages match the full original sentence.'))
                corrected=json.loads(file.read_text());self.assertEqual(corrected['places']['p']['extra_pages'][0]['span'],'statement.')
                self.assertEqual(kit.witness_problems(str(path),'en',corrected,second=True),[])
                spread=copy.deepcopy(before);place=spread['places']['p'];place['page']='p. 1';place['extra_pages'][0].update({'url':place['url'],'image_sha256':place['image_sha256']})
                spread['review']['places']['p']['pages']=[{'page':page['page'],'url':page['url'],'image_sha256':page['image_sha256']} for page in kit.witness_pages(place)]
                self.assertEqual(kit.witness_problems(str(path),'en',spread,second=True),[])
                file.write_text(json.dumps(spread))
                ambiguous=argparse.Namespace(packet=str(path),language='en',loc='p',to='statement.',reason='The spread reads statement rather than sentence.',image='page.jpg',**{'from':'sentence.'})
                with self.assertRaises(SystemExit):kit.cmd_witness_correct(ambiguous)
                ambiguous.page='p. 2';kit.cmd_witness_correct(ambiguous)
                changed=json.loads(file.read_text());self.assertEqual(changed['places']['p']['corrections'][0]['page'],'p. 2')
                kit.cmd_witness_review(argparse.Namespace(packet=str(path),language='en',loc='p',reader='reader',model='gpt-6.1-terra',alignment='Both printed pages of the spread match the original.'))
                self.assertEqual(kit.witness_problems(str(path),'en',json.loads(file.read_text()),second=True),[])
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
    def test_restore_missing_accepted_evidence_requires_exact_recorded_bytes(self):
        import argparse,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            raw=b'Exact source';record={'file':'source.txt','url':'https://example.org/source','sha256':hashlib.sha256(raw).hexdigest()}
            signed=json.dumps({'sources':[record],'images':[]}).encode();file.write_bytes(signed);Path(str(file)+'.accepted').write_text('{}')
            args=argparse.Namespace(packet=str(path),language='en',name='source.txt',file=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                with patch.object(kit,'get',return_value=b'Wrong source'),self.assertRaises(SystemExit):kit.cmd_witness_restore_evidence(args)
                with patch.object(kit,'get',return_value=raw):kit.cmd_witness_restore_evidence(args)
                self.assertEqual((Path(tmp)/'cache/work/witnesses/en/source.txt').read_bytes(),raw);self.assertEqual(file.read_bytes(),signed)
                with self.assertRaises(SystemExit):kit.cmd_witness_restore_evidence(args)

    def test_source_and_image_names_do_not_overwrite_prior_evidence(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            first=b'Old source';(folder/'source.txt').write_bytes(first)
            image=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');(folder/'page.png').write_bytes(image)
            source={'file':'source.txt','url':'https://example.org/old','sha256':hashlib.sha256(first).hexdigest()}
            page={'file':'page.png','url':'https://example.org/page','sha256':hashlib.sha256(image).hexdigest(),'by':'maker'}
            file=path/'witnesses/en.json';raw=json.dumps({'witness':{},'sources':[source],'images':[page],'review':{'reader':'second'}}).encode();file.write_bytes(raw)
            args=argparse.Namespace(packet=str(path),language='en',url='https://example.org/new',name='source.txt',file=None,by='maker')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=b'New source'):
                with self.assertRaises(SystemExit):kit.cmd_witness_fetch(args)
            self.assertEqual((folder/'source.txt').read_bytes(),first);self.assertEqual(file.read_bytes(),raw)
            args.url='https://example.org/other';args.name='page.png'
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=image):
                with self.assertRaises(SystemExit):kit.cmd_witness_image(args)
            self.assertEqual((folder/'page.png').read_bytes(),image);self.assertEqual(file.read_bytes(),raw)

    def test_accepted_witness_commands_cannot_mutate_data_or_fetch_new_bytes(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/ru.json'
            raw=b'{"language":"ru","witness":{},"sources":[],"images":[],"places":{},"review":{}}\n';file.write_bytes(raw);Path(str(file)+'.accepted').write_text('{}')
            args=argparse.Namespace(packet=str(path),language='ru',assessment='The independent assessment is deliberately long enough to pass validation.')
            functions=[kit.cmd_witness_info,kit.cmd_witness_rights,kit.cmd_witness_missing,kit.cmd_witness_candidate,kit.cmd_witness_cut_candidate,kit.cmd_witness_person,kit.cmd_witness_fetch,kit.cmd_witness_image,kit.cmd_witness_cut,kit.cmd_witness_transcribe_page,kit.cmd_witness_add_page,kit.cmd_witness_correct,kit.cmd_witness_review,kit.cmd_witness_anonymous,kit.cmd_witness_anonymous_review,kit.cmd_witness_anonymous_clear,kit.cmd_licence,kit.cmd_licence_review,kit.cmd_mexico_quotation,kit.cmd_mexico_quotation_review]
            with patch.object(kit,'get',side_effect=AssertionError('must not fetch')):
                for function in functions:
                    with self.subTest(command=function.__name__),self.assertRaises(SystemExit):function(args)
                    self.assertEqual(file.read_bytes(),raw)

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
                self.assertIsNone(kit.witness_conflict(str(extension),'ru',data))
                sibling=parent/'extensions'/'witness-only';(sibling/'witnesses').mkdir(parents=True)
                (sibling/'scope.json').write_text(json.dumps({'kind':'witness-only','work':'work'}))
                sibling_file=sibling/'witnesses/ru.json';sibling_raw=json.dumps({'language':'ru','witness':{'edition':'Third book'},'places':{'new':{'text':'Уже занято.'}}}).encode()
                sibling_file.write_bytes(sibling_raw);Path(str(sibling_file)+'.accepted').write_text(json.dumps({'sha256':hashlib.sha256(sibling_raw).hexdigest()}))
                self.assertIn('already covers',kit.witness_conflict(str(extension),'ru',data))

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
                kit.cmd_mexico_quotation_review(argparse.Namespace(packet=str(path),language=None,outcome='approved',assessment='I independently checked the source, integrity, proportion and normal exploitation for these exact places.'))
                _,witness,places,review,maker=kit.mexico_context(str(path),None)
                self.assertEqual(kit.mexico_problems(witness,places,review,maker,second=True,path=str(path)),[])

    def test_quotation_review_is_scoped_to_the_exact_witness_and_text(self):
        import argparse, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            data={'language':'en','witness':{'edition':'A named edition','maker':'maker','rights':{country:{'basis':'public-domain','law':'Public domain under the applicable national rules.'} for country in kit.JURISDICTIONS}},'places':{'p':{'text':'A complete sentence.','url':'https://example.org/page','page':'7'}},'review':{'reader':'reader'}}
            file.write_text(json.dumps(data))
            law=Path(tmp)/'law.pdf';law.write_bytes(b'%PDF-1.4\nTest law source\n%%EOF')
            args=argparse.Namespace(packet=str(path),language='en',law_file=str(law),non_substantial='This short passage is proportionate to the scholarly purpose of the comparison.',normal_exploitation='The selected sentences do not replace the book or affect normal exploitation.',source_integrity='The cited source and author are identified and the page text is unchanged.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_mexico_quotation(args);data=json.loads(file.read_text())
                self.assertTrue(kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True))
                with patch.object(kit,'world',return_value={'fragments':{'places':{'work':{}},'works':{}}}),patch.object(kit,'evidence',return_value={}):
                    self.assertTrue(any('Mexico quotation assessment needs a distinct second reader' in issue for issue in kit.witness_problems(str(path),'en',data,second=True,offline=True)))
                kit.cmd_mexico_quotation_review(argparse.Namespace(packet=str(path),language='en',outcome='pending',assessment='The quotation requires further assessment of the published display and its cumulative extent.'))
                pending=json.loads(file.read_text());self.assertTrue(any('pending' in issue for issue in kit.mexico_problems(pending['witness'],pending['places'],pending['review'],'maker',second=True)))
                kit.cmd_mexico_quotation(args);self.assertEqual(json.loads(file.read_text())['review']['mexico_quotation_history'][0]['outcome'],'pending')
                kit.cmd_mexico_quotation_review(argparse.Namespace(packet=str(path),language='en',outcome='approved',assessment='I independently checked the source, integrity, proportion and normal exploitation for this exact quotation.'))
                data=json.loads(file.read_text());self.assertEqual(data['review']['mexico_quotation_history'][0]['outcome'],'pending');self.assertEqual(kit.mexico_problems(data['witness'],data['places'],data['review'],'maker',second=True,path=str(path),lang='en'),[])
                negative=dict(data['review']);negative['mexico_quotation']={**negative['mexico_quotation'],'outcome':'rejected'}
                self.assertTrue(any('rejected' in issue for issue in kit.mexico_problems(data['witness'],data['places'],negative,'maker',second=True)))
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
            data['review']['mexico_quotation']={'reader':'second','date':'2026-11-19','sha256':kit.page_number_digest(mexico),'assessment':'I independently checked the quotation, source, text integrity and normal exploitation.','outcome':'approved'}
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
        import argparse
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
            manual='immanuel-kant.kritik-der-urteilskraft'
            for child in Path(kit.PACKETS).iterdir():
                if child.name not in (work,manual):(packets/child.name).symlink_to(child,target_is_directory=True)
            # The manual-witness case writes its own files; never link it to an operator packet.
            (packets/manual).mkdir()
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
            (ext/'evidence.json').write_text(json.dumps({'persons':[{'qid':'Q1','died_before':1351,'death_bound':{'from':'wikidata','year':1351}}]}))
            self.assertIn('older Wikidata bound',subprocess.run(command,capture_output=True,text=True).stderr)
            (ext/'evidence.json').unlink()
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
                        'places':{loc:{'text':'Русское предложение.','url':'https://example.org/page','page':'1','line_locator':{'section':'XXX.1','start':10,'end':14},'image_sha256':'a'*64,'extra_pages':[{'page':'2','url':'https://example.org/page2','image_sha256':'b'*64,'span':'предложение.'}],'corrections':[{'from':'Прежнее','to':'Русское','reason':'The source page reads the corrected form.'}]}}}
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
            stale=copy.deepcopy(translated);stale['witness']['persons']=[{'qid':'Q1','died_before':1351,'death_bound':{'from':'wikidata','year':1351}}]
            stale_raw=json.dumps(stale,ensure_ascii=False).encode();(witness_dir/'ru.json').write_bytes(stale_raw);(witness_dir/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(stale_raw).hexdigest()}))
            self.assertIn('older Wikidata bound',subprocess.run(command,capture_output=True,text=True).stderr)
            (witness_dir/'ru.json').write_bytes(raw);(witness_dir/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            self.assertEqual(projected['provenance']['translation_packets'][work]['ru']['corrections'][0]['loc'],loc)
            self.assertEqual(projected['provenance']['translation_packets'][work]['ru']['page_sources'][loc][1]['image_sha256'],'b'*64)
            self.assertTrue(any('XXX.1, 1, lines 10–14, 2' in ref[1] for ref in projected['places'][work][loc]['refs'] if ref[0]=='ru'))
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
            # A separately accepted edition owns only its own disjoint place,
            # including its own citation and jurisdiction-specific rights.
            russian_extra['witness']['edition']='Different translation'
            russian_extra['witness']['attribution']='Another translator'
            russian_extra['witness']['rights']['usa']={'basis':'short-quotation','law':'A separately assessed quotation from this edition.'}
            russian_extra['places']['Test p1000']['emphasis']=[{'text':'Третье','page':'3','url':'https://example.org/page3','image_sha256':'a'*64,'reason':'The exact printed emphasis is meaningful.'}]
            raw_extra=json.dumps(russian_extra,ensure_ascii=False).encode();(ext_witness/'ru.json').write_bytes(raw_extra)
            (ext_witness/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw_extra).hexdigest()}))
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            alt=projected['places'][work]['Test p1000']['witness']['ru']
            self.assertNotEqual(alt,'ru');self.assertEqual(projected['works'][work]['witnesses'][alt]['edition'],'Different translation')
            self.assertEqual(projected['works'][work]['witnesses'][alt]['rights']['usa']['basis'],'short-quotation')
            self.assertEqual(projected['works'][work]['witnesses']['ru']['edition'],'Test edition')
            self.assertEqual(projected['places'][work]['Test p1000']['links']['ru'],'https://example.org/page3')
            self.assertEqual(projected['places'][work]['Test p1000']['emphasis']['ru'],['Третье'])
            self.assertIn('/extensions/test/witnesses/ru.json',projected['provenance']['translation_packets'][work]['ru']['place_sources']['Test p1000'])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            # Adding a translation of an already planted original place needs
            # no invented reading or original-extension acceptance.
            with patch.object(kit,'world',return_value={'fragments':projected}):
                kit.cmd_witness_extension_open(argparse.Namespace(packet=str(parent),name='translation-only'))
            only=parent/'extensions/translation-only';self.assertFalse((only/'accepted.json').exists());self.assertFalse((only/'reading.json').exists())
            other=next(key for key in base['places'] if key!=loc);(only/'witnesses').mkdir()
            third=copy.deepcopy(translated);third['witness']['edition']='Third translation';third['places']={other:{'text':'Четвёртое предложение.','url':'https://example.org/page4','page':'4','transcription':{'kind':'new-translation'}}}
            third_raw=json.dumps(third,ensure_ascii=False).encode();(only/'witnesses/ru.json').write_bytes(third_raw)
            (only/'witnesses/ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(third_raw).hexdigest()}))
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text());third_key=projected['places'][work][other]['witness']['ru']
            self.assertEqual(projected['works'][work]['witnesses'][third_key]['edition'],'Third translation')
            self.assertIn(['ru','Third translation; original source 4'],projected['places'][work][other]['refs'])
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            (only/'witnesses/ru.json.accepted').unlink();subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text());self.assertNotIn(third_key,projected['works'][work]['witnesses'])
            (ext/'accepted.json').unlink()
            translated['places'].pop('Test p999');raw=json.dumps(translated,ensure_ascii=False).encode();(witness_dir/'ru.json').write_bytes(raw)
            (witness_dir/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            (witness_dir/'ru.json.accepted').unlink()
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text())
            self.assertNotIn(alt,projected['works'][work]['witnesses'])
            self.assertNotIn('ru',projected['places'][work][loc]['texts'])
            self.assertTrue(any(item['language']=='ru' for item in projected['works'][work].get('unsearched',[])))
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            self.assertFalse((packets/manual).is_symlink())
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
            with patch.object(kit,'world',return_value={'fragments':projected}):
                kit.cmd_witness_extension_open(argparse.Namespace(packet=str(packets/manual),name='translation-only'))
            manual_only=packets/manual/'extensions/translation-only/witnesses';manual_only.mkdir()
            (manual_only/'ru.json').write_bytes(raw);(manual_only/'ru.json.accepted').write_text(json.dumps({'sha256':hashlib.sha256(raw).hexdigest()}))
            subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text());self.assertEqual(projected['places'][manual][manual_loc]['texts']['ru'],'Русское предложение.')
            subprocess.run(command+['--check'],check=True,capture_output=True,text=True)
            (manual_only/'ru.json.accepted').unlink();subprocess.run(command,check=True,capture_output=True,text=True)
            projected=json.loads((target/'bonsai/aesthetics/fragments.json').read_text());self.assertNotIn('ru',projected['places'][manual][manual_loc]['texts'])

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
        import os
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
            library=json.loads((home/'library.json').read_text())
            library['works']['plato.symposium']['agents'].append('immanuel-kant')
            (home/'library.json').write_text(json.dumps(library,ensure_ascii=False))
            subprocess.run(['node',str(review/'chronology.mjs')],check=True,capture_output=True,text=True)
            result=json.loads((home/'chronology.json').read_text())
            work=result['situations'][result['compositions']['works']['plato.symposium']]
            self.assertEqual(work['dating']['kind'],'unknown-date')
            self.assertNotIn('within the life',work['statement']['en'])
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
            # Refreshing Wikidata must not erase an unresolved, source-visible conflict.
            library=json.loads((home/'library.json').read_text());lives={key:value for key,value in lives.items() if key in library['agents']}
            kant=lives['immanuel-kant'];kant['born']=[];kant['date_conflict']={'field':'born','claims':[{'time':'+1724-01-01T00:00:00Z','source':'https://example.org/one'},{'time':'+1725-01-01T00:00:00Z','source':'https://example.org/two'}]}
            (layer/'lives.json').write_text(json.dumps(lives,ensure_ascii=False))
            shim=target/'mock-fetch.mjs';qid=kant['item']
            shim.write_text('globalThis.fetch=async()=>({ok:true,json:async()=>({entities:{'+json.dumps(qid)+':{claims:{P569:[{rank:"normal",mainsnak:{datavalue:{value:{time:"+1724-01-01T00:00:00Z",precision:9}}}}]}}}})});')
            env={**os.environ,'NODE_OPTIONS':'--import='+str(shim)}
            subprocess.run(['node',str(review/'chronology.mjs'),'--refresh-qualifiers'],check=True,capture_output=True,text=True,env=env)
            refreshed=json.loads((layer/'lives.json').read_text())['immanuel-kant']
            self.assertEqual(refreshed['born'],[]);self.assertEqual(len(refreshed['date_conflict']['claims']),2)
            result=json.loads((home/'chronology.json').read_text());life=result['situations'][result['lives']['immanuel-kant']]
            self.assertEqual(life['dating']['kind'],'unknown-date')

class TranslationCorrectionTest(unittest.TestCase):
    def test_external_pages_preserve_old_bytes_and_other_scan_default(self):
        import argparse,base64,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');source=Path(tmp)/'page.png';source.write_bytes(raw)
            old={'method':'image','scan':'old','images':[3]};(path/'evidence.json').write_text(json.dumps({'scan':{'item':'old'},'places':{'p':old},'images':[]}));(path/'places.json').write_text(json.dumps({'places':{'p':{}}}));(path/'review.json').write_text(json.dumps({'reader':{'id':'second'}}))
            args=argparse.Namespace(packet=str(path),url='https://example.org/p1',loc='p',n=5,by='maker',file=str(source))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_page(args);first=kit.evidence(str(path));saved=Path(kit.cache(str(path),first['images'][0]['file']));self.assertEqual(first['scan']['item'],'old');self.assertEqual(first['places']['p']['images'],[5]);self.assertEqual(first['page_rebindings'][0]['from'],old);self.assertEqual(json.loads((path/'review.json').read_text()),{})
                args.url='https://example.org/p2';source.write_bytes(raw+b'changed');kit.cmd_page(args);self.assertEqual(saved.read_bytes(),raw);self.assertEqual(len(kit.evidence(str(path))['images']),2)
                (path/'accepted.json').write_text('{}')
                with self.assertRaises(SystemExit):kit.cmd_page(args)
                with self.assertRaises(SystemExit):kit.cmd_image(argparse.Namespace(packet=str(path),scan='old',n=5,loc='p',zoom=None,width=1100,by='maker',file=str(source)))
                with self.assertRaises(SystemExit):kit.cmd_pdf_text(argparse.Namespace(packet=str(path),language=None,source='book.pdf'))
                with self.assertRaises(SystemExit):kit.cmd_pdf_page(argparse.Namespace(packet=str(path),language=None,source='book.pdf',loc='p',page=1,by='maker'))

    def test_image_ocr_preserves_page_model_and_local_origin(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/ru.json'
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            image={'file':'page.png','url':'https://example.org/page.png','sha256':hashlib.sha256(raw).hexdigest(),'by':'maker'}
            file.write_text(json.dumps({'witness':{'maker':'maker'},'sources':[],'images':[image],'review':{'reader':'old'}}))
            modeldir=Path(tmp)/'model';modeldir.mkdir();(modeldir/'rus.traineddata').write_bytes(b'model bytes')
            args=argparse.Namespace(packet=str(path),language='ru',image='page.png',model_dir=str(modeldir),model='rus')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'image_ocr',return_value=('Исходный OCR'.encode(),'5.5.4','leptonica-1.87.0')):
                Path(kit.witness_cache(str(path),'ru','page.png')).write_bytes(raw);kit.cmd_witness_ocr(args)
                result=json.loads(file.read_text());source=result['sources'][0];self.assertEqual(result['review'],{})
                self.assertEqual(source['derived_from']['sha256'],image['sha256']);self.assertNotIn('fetched',source)
                local=lambda name:kit.witness_cache(str(path),'ru',name)
                self.assertEqual(kit.derived_source_problems(result['sources'],local,images=result['images']),[])
                Path(local(source['derived_from']['model_file'])).write_bytes(b'changed')
                self.assertTrue(kit.derived_source_problems(result['sources'],local,images=result['images']))

    def test_joined_ocr_retains_ordered_raw_pages_and_detects_changes(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            images=[{'file':f'p{i}.png','url':f'https://example.org/p{i}.png','sha256':hashlib.sha256(raw+bytes([i])).hexdigest(),'by':'maker'} for i in (1,2)]
            file.write_text(json.dumps({'witness':{'maker':'maker'},'sources':[],'images':images,'review':{}}));model=Path(tmp)/'model';model.mkdir();(model/'eng.traineddata').write_bytes(b'model')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                for image,text in zip(images,[b'header First words catchword',b'catchword last words footer']):
                    Path(kit.witness_cache(str(path),'en',image['file'])).write_bytes(raw+bytes([int(image['file'][1])]))
                    with patch.object(kit,'image_ocr',return_value=(text,'5.5.4','leptonica-1.87.0')):kit.cmd_witness_ocr(argparse.Namespace(packet=str(path),language='en',image=image['file'],model_dir=str(model),model='eng'))
                result=json.loads(file.read_text());sources=result['sources'];spec={'segments':[{'source':sources[0]['file'],'open':'First','close':'words'},{'source':sources[1]['file'],'open':'catchword','close':'words'}],'reason':'The printed page ends with a catchword repeated at the next opening.'};specfile=Path(tmp)/'join.json';specfile.write_text(json.dumps(spec))
                kit.cmd_witness_ocr_join(argparse.Namespace(packet=str(path),language='en',file=str(specfile)));result=json.loads(file.read_text());joined=result['sources'][-1];local=lambda name:kit.witness_cache(str(path),'en',name)
                with self.assertRaises(SystemExit):kit.cmd_witness_source_remove(argparse.Namespace(packet=str(path),language='en',name=sources[0]['file'],reason='Keep the referenced raw OCR source until its joined derivative is withdrawn.'))
                self.assertEqual(Path(local(joined['file'])).read_text(),'First words catchword last words')
                self.assertEqual(kit.derived_source_problems(result['sources'],local,images=result['images']),[])
                result['language']='en';result['places']={'p':{'text':'First words catchword last words','source':joined['file'],'page':'1','url':images[0]['url'],'image_sha256':images[0]['sha256'],'page_span':'First words','extra_pages':[{'page':'2','url':images[1]['url'],'image_sha256':images[1]['sha256'],'span':'catchword last words'}],'alignment':'This text aligns with the original passage.'}};result['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(result))
                with patch.object(kit,'world',return_value={'fragments':{'places':{'work':{'p':{}}}}}):
                    self.assertTrue(any('joined OCR needs an independent' in p for p in kit.witness_problems(str(path),'en',result,second=True)))
                    kit.cmd_witness_ocr_join_review(argparse.Namespace(packet=str(path),language='en',loc='p',reader='second',assessment='I read both pages and checked that the omitted catchword is repeated at the page opening.'))
                    result=json.loads(file.read_text());self.assertFalse(any('joined OCR needs an independent' in p for p in kit.witness_problems(str(path),'en',result,second=True)))
                    result['places']['p']['text']+=' changed';self.assertTrue(any('joined OCR needs an independent' in p for p in kit.witness_problems(str(path),'en',result,second=True)))
                Path(local(sources[1]['file'])).write_text('changed')
                self.assertTrue(kit.derived_source_problems(result['sources'],local,images=result['images']))

    def test_real_scan_image_transforms_preserve_separate_readers(self):
        import argparse,json,tempfile
        from pathlib import Path
        from PIL import Image
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();source=Path(tmp)/'source.jpg'
            Image.new('RGB',(120,200),'white').save(source)
            (path/'evidence.json').write_text(json.dumps({'scan':{'item':'book','ocr':False,'image_offset':0},'places':{'p':{}},'images':[]}))
            args=argparse.Namespace(packet=str(path),n=5,loc='p',width=40,by='maker',file=str(source),scan=None,zoom=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_image(args);maker=kit.evidence(str(path))['images'][0];maker_path=Path(kit.cache(str(path),maker['file']));maker_bytes=maker_path.read_bytes()
                args.by='second';args.zoom='20-40';kit.cmd_image(args)
                records=kit.evidence(str(path))['images'];self.assertEqual(len(records),2)
                self.assertEqual(maker_path.read_bytes(),maker_bytes);self.assertNotEqual(records[0]['file'],records[1]['file'])
                self.assertTrue(kit.valid_page_image(Path(kit.cache(str(path),records[1]['file'])).read_bytes()))

    def test_original_page_images_never_replace_another_readers_bytes(self):
        import argparse,base64,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            (path/'evidence.json').write_text(json.dumps({'scan':{'item':'book','ocr':False,'image_offset':0},'places':{'p':{}},'images':[]}))
            source=Path(tmp)/'source.png';source.write_bytes(raw)
            args=argparse.Namespace(packet=str(path),n=5,loc='p',width=1100,by='maker',file=str(source),scan=None,zoom=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'narrow',side_effect=lambda file,width:Path(file).read_bytes()):
                kit.cmd_image(args);maker=kit.evidence(str(path))['images'][0];maker_path=Path(kit.cache(str(path),maker['file']))
                source.write_bytes(raw+b'other image');args.by='second';kit.cmd_image(args)
                records=kit.evidence(str(path))['images'];self.assertEqual(len(records),2)
                self.assertNotEqual(records[0]['file'],records[1]['file']);self.assertEqual(maker_path.read_bytes(),raw)

    def test_fetching_another_scan_does_not_rebind_existing_place(self):
        import argparse,base64,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');source=Path(tmp)/'source.png';source.write_bytes(raw)
            original={'method':'image','images':[3],'scan':'old'}
            (path/'evidence.json').write_text(json.dumps({'scan':{'item':'old'},'scans':{'new':{'item':'new'}},'places':{'p':original},'images':[]}))
            args=argparse.Namespace(packet=str(path),n=5,loc='p',width=1100,by='maker',file=str(source),scan='new',zoom=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'narrow',side_effect=lambda file,width:Path(file).read_bytes()):kit.cmd_image(args)
            result=kit.evidence(str(path));self.assertEqual(result['places']['p'],original);self.assertEqual(result['images'][0]['item'],'new')

    def test_rebind_preserves_history_and_requires_recomparison(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();folder=Path(tmp)/'cache/work';folder.mkdir(parents=True)
            image=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');(folder/'external.img').write_bytes(image)
            records=[]
            for role,name,raw in [('scan-ocr','book.ocr.txt',b'A complete sentence.'),('scan-index','book.index.json',b'[[0]]')]:
                (folder/name).write_bytes(raw);records.append({'role':role,'file':name,'sha256':hashlib.sha256(raw).hexdigest()})
            old={'transcription':'source.txt','scan':'external','method':'image','images':[5],'whole':{'stands':'transcription'}};page={'file':'external.img','url':'https://archive.org/download/book/page/n5.jpg','sha256':hashlib.sha256(image).hexdigest(),'item':'external','n':5,'by':'maker','loc':'p'}
            ev={'sources':records,'scan':{'item':'external','ocr':False},'scans':{'book':{'item':'book','ocr':True,'image_offset':0}},'places':{'p':old},'images':[page]}
            (path/'evidence.json').write_text(json.dumps(ev));(path/'places.json').write_text(json.dumps({'work':'work','entry':{'original':'en'},'places':{'p':{'texts':{'en':'A complete sentence.'},'links':{}}}}))
            prior={'reader':{'id':'second'},'images':[{'loc':'p','n':5,'reads':'as the packet'}]};(path/'review.json').write_text(json.dumps(prior))
            args=argparse.Namespace(packet=str(path),loc='p',item='book',image='external.img',reason='The original Archive image belongs to the registered indexed scan.')
            found={'leaves':[5],'printed':['7'],'share':1.0,'differences':[]}
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'ocr',return_value={}),patch.object(kit,'against_ocr',return_value={'leaves':[6]}):
                with self.assertRaises(SystemExit):kit.cmd_scan_rebind(args)
                self.assertEqual(kit.evidence(str(path))['places']['p'],old)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'ocr',return_value={'available':True}),patch.object(kit,'against_ocr',return_value=found):
                kit.cmd_scan_rebind(args);new=kit.evidence(str(path))
                self.assertTrue(new['places']['p']['scan_rebind_pending']);self.assertEqual(new['scan_rebindings'][0]['from'],old);self.assertEqual(new['scan_rebindings'][0]['review'],prior)
                self.assertEqual(len(new['images']),2);self.assertIn('external',new['scans']);self.assertEqual(new['scan']['item'],'external');self.assertEqual(json.loads((path/'review.json').read_text()),{})
                kit.cmd_compare(argparse.Namespace(packet=str(path)))
                self.assertNotIn('scan_rebind_pending',kit.evidence(str(path))['places']['p'])

    def test_added_page_binds_words_and_invalidates_review(self):
        import argparse, base64, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            (folder/'page2.jpg').write_bytes(raw);digest=hashlib.sha256(raw).hexdigest()
            data={'language':'en','images':[{'file':'page1.jpg','url':'https://example.org/page1','by':'maker','sha256':'a'*64},{'file':'page2.jpg','url':'https://example.org/page2','by':'maker','sha256':digest}],
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

    def test_second_image_cannot_overwrite_maker_bytes(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            file.write_text(json.dumps({'witness':{},'sources':[{'file':'text.txt'}],'images':[{'file':'maker.jpg','by':'maker'}]}))
            args=argparse.Namespace(packet=str(path),language='en',url='https://example.org/page',name='maker.jpg',by='second',file=None)
            with self.assertRaises(SystemExit):kit.cmd_witness_image(args)
            args.name='text.txt'
            with self.assertRaises(SystemExit):kit.cmd_witness_image(args)
            self.assertEqual(json.loads(file.read_text())['images'][0]['file'],'maker.jpg')

    def test_two_printed_pages_can_share_a_spread_image(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            (folder/'spread.png').write_bytes(raw);digest=hashlib.sha256(raw).hexdigest();url='https://example.org/spread'
            data={'language':'en','images':[{'file':'spread.png','url':url,'by':'maker','sha256':digest}], 'places':{'p':{'text':'A complete sentence.','page':'p. 54','url':url,'image_sha256':digest}},'review':{'reader':'old'}}
            file=path/'witnesses/en.json';file.write_text(json.dumps(data))
            args=argparse.Namespace(packet=str(path),language='en',loc='p',page='p. 55',image='spread.png',span='sentence.',first_span='A complete')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):kit.cmd_witness_add_page(args)
            actual=json.loads(file.read_text());self.assertEqual(actual['places']['p']['extra_pages'][0]['url'],url)
            self.assertEqual(actual['places']['p']['extra_pages'][0]['image_sha256'],digest);self.assertEqual(actual['review'],{})

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
    def test_scan_rebind_check_preserves_original_language_coverage_entry(self):
        import json,shutil,tempfile
        from pathlib import Path
        name='leon-battista-alberti.de-re-aedificatoria'
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/name;shutil.copytree(Path(kit.PACKETS)/name,path)
            expected=json.loads((path/'places.json').read_text())['entry']
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'language_coverage',wraps=kit.language_coverage) as coverage:
                kit.gate(str(path),second=True,offline=True)
                self.assertEqual(coverage.call_args.args[1],expected)

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

class UnicodeSourceUrlTest(unittest.TestCase):
    def test_unicode_path_is_escaped_without_escaping_existing_percent_encoding(self):
        from unittest.mock import MagicMock
        response=MagicMock();response.__enter__.return_value.read.return_value=b'ok'
        with patch.object(kit.urllib.request,'urlopen',return_value=response) as open_url:
            self.assertEqual(kit.get('https://sa.wikipedia.org/wiki/महिमभट्टः?x=%20',tries=1),b'ok')
            actual=open_url.call_args.args[0].full_url
            self.assertTrue(actual.isascii());self.assertIn('%E0%A4',actual);self.assertTrue(actual.endswith('?x=%20'))


class HtmlTranscriptionTest(unittest.TestCase):
    def test_mediawiki_content_is_not_hidden_by_skin_toc_class(self):
        raw=b'<html class="vector-feature-toc-not-available"><body><nav>Navigation</nav><div class="mw-parser-output"><div class="toc">Contents</div><p>A complete sentence.</p></div><footer>Footer</footer></body></html>'
        text=kit.plain(raw,'source.html')
        self.assertIn('A complete sentence.',text)
        self.assertNotIn('Navigation',text)
        self.assertNotIn('Contents',text)
        self.assertNotIn('Footer',text)

class PreparedLanguageHandoffTest(unittest.TestCase):
    def test_only_independently_reviewed_unchanged_witness_is_accepted(self):
        import json,tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp:
            packet=Path(tmp)/'work';(packet/'witnesses').mkdir(parents=True)
            file=packet/'witnesses/en.json';maker={'language':'en','witness':{'maker':'maker'},'sources':[{'file':'book'}],'places':{'p':{'text':'source text'}},'images':[{'by':'maker','file':'page'}],'review':{}}
            file.write_text(json.dumps(maker));calls=[]
            def kit_call(*args):calls.append(args);return 0,'gate: open'
            def reader(work,role,prompt):
                self.assertEqual(role,'language-second');current=json.loads(file.read_text())
                current['images'].append({'by':'second','file':'page-second'});current['review']={'reader':'other'};file.write_text(json.dumps(current));return 'reader-thread',None
            with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'kit',side_effect=kit_call),patch.object(drive,'codex',side_effect=reader):
                stop,feedback,changed=drive.review_prepared_languages('work','owes: en: witness is research until independently accepted',set())
            self.assertIsNone(stop);self.assertEqual(feedback,[]);self.assertTrue(changed)
            self.assertTrue(any(call[0]=='witness-gate' and '--accept' in call for call in calls))
            self.assertEqual(len(list((Path(tmp)/'out/work').glob('witness-en-before-*.json'))),1)

    def test_maker_gate_waits_for_independent_witness_acceptance(self):
        import json,tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp:
            packet=Path(tmp)/'work';(packet/'witnesses').mkdir(parents=True);file=packet/'witnesses/en.json';events=[]
            data={'language':'en','witness':{'maker':'maker'},'sources':[],'places':{'p':{'text':'source text'}},'images':[],'review':{}}
            file.write_text(json.dumps(data));accepted=False
            def kit_call(*args):
                nonlocal accepted
                if args[0]=='gate':return (0,'gate: open') if accepted else (0,'owes: en: witness is research until independently accepted\ngate: open, owes 1')
                if args[0]=='witness-gate' and '--accept' in args:accepted=True
                return 0,'gate: open'
            def reader(work,role,prompt,resume=None):
                events.append(role)
                if role=='language-second':
                    current=json.loads(file.read_text());current['review']={'reader':'second'};file.write_text(json.dumps(current))
                return role+'-thread',None
            with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'kit',side_effect=kit_call),patch.object(drive,'codex',side_effect=reader),patch.object(drive,'have',return_value=False):
                thread,result=drive.until_open('work','maker','first','{gate}',3)
            self.assertEqual((thread,result),('maker-thread','open'))
            self.assertEqual(events,['maker','language-second']);self.assertTrue(accepted)

    def test_reviewed_search_is_admitted_only_after_its_second_gate(self):
        import json,tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp:
            packet=Path(tmp)/'work';(packet/'language-searches').mkdir(parents=True);file=packet/'language-searches/es.json'
            file.write_text(json.dumps({'language':'es','search':{'maker':'maker','tried':[]},'review':{}}));calls=[]
            def reader(work,role,prompt):
                data=json.loads(file.read_text());data['review']={'reader':'other'};file.write_text(json.dumps(data));return 'reader-thread',None
            def gate(*args):calls.append(args);return 0,'gate: open'
            with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'kit',side_effect=gate),patch.object(drive,'codex',side_effect=reader):
                stop,feedback,changed=drive.review_prepared_languages('work','gate: open, owes 1',set())
            self.assertIsNone(stop);self.assertTrue(changed);self.assertEqual(feedback,[])
            self.assertTrue(any(call[0]=='language-search-gate' and '--second' in call for call in calls))
            self.assertTrue(any(call[0]=='language-search-accept' for call in calls))

    def test_failed_review_is_not_retried_until_maker_evidence_changes(self):
        import json,tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp:
            packet=Path(tmp)/'work';(packet/'witnesses').mkdir(parents=True);file=packet/'witnesses/en.json'
            file.write_text(json.dumps({'language':'en','witness':{'maker':'maker'},'sources':[],'places':{'p':{'text':'original'}},'images':[],'review':{}}));calls=[];attempted=set()
            def reader(work,role,prompt):
                calls.append(role);data=json.loads(file.read_text());data['review']={'reader':'second'};file.write_text(json.dumps(data));return 'thread',None
            def gate(*args):return (1,'review incomplete') if '--second' in args else (0,'gate: open')
            with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'kit',side_effect=gate),patch.object(drive,'codex',side_effect=reader),patch.object(drive,'language_reader_message',return_value='finding'):
                for _ in range(2):drive.review_prepared_languages('work','owes: en: witness is research until independently accepted',attempted)
                self.assertEqual(len(calls),1)
                data=json.loads(file.read_text());data['places']['p']['text']='corrected';file.write_text(json.dumps(data))
                drive.review_prepared_languages('work','owes: en: witness is research until independently accepted',attempted)
                self.assertEqual(len(calls),2)

    def test_reader_created_acceptance_marker_is_a_conflict(self):
        import json,tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp:
            packet=Path(tmp)/'work';(packet/'language-searches').mkdir(parents=True);file=packet/'language-searches/es.json'
            file.write_text(json.dumps({'language':'es','search':{'maker':'maker'},'review':{}}));calls=[]
            def reader(work,role,prompt):
                Path(str(file)+'.accepted').write_text('{}');return 'reader-thread',None
            def gate(*args):calls.append(args);return 0,'gate: open'
            with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'kit',side_effect=gate),patch.object(drive,'codex',side_effect=reader):
                stop,feedback,changed=drive.review_prepared_languages('work','gate: open, owes 1',set())
            self.assertEqual(stop,'language-review-conflict');self.assertFalse(changed);self.assertTrue(feedback)
            self.assertFalse(any(call[0]=='language-search-accept' for call in calls))

    def test_reader_cannot_replace_the_makers_evidence(self):
        import json,tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp:
            packet=Path(tmp)/'work';(packet/'witnesses').mkdir(parents=True);file=packet/'witnesses/en.json'
            file.write_text(json.dumps({'language':'en','witness':{'maker':'maker'},'sources':[],'places':{'p':{'text':'original'}},'images':[],'review':{}}));calls=[]
            def reader(work,role,prompt):
                data=json.loads(file.read_text());data['places']['p']['text']='changed';file.write_text(json.dumps(data));return 'reader-thread',None
            def gate(*args):calls.append(args);return 0,'gate: open'
            with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'kit',side_effect=gate),patch.object(drive,'codex',side_effect=reader):
                stop,feedback,changed=drive.review_prepared_languages('work','owes: en: witness is research until independently accepted',set())
            self.assertEqual(stop,'language-review-conflict');self.assertFalse(changed);self.assertTrue(feedback)
            snapshot=next((Path(tmp)/'out/work').glob('witness-en-before-*.json'));self.assertEqual(json.loads(snapshot.read_text())['places']['p']['text'],'original')
            self.assertFalse(any('--accept' in call for call in calls))

class OriginalPersonBoundTest(unittest.TestCase):
    def test_qid_reference_and_independent_reader_are_bound_to_source(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();url='https://en.wikipedia.org/wiki/Ancient_Author'
            entity={'entities':{'Q123':{'labels':{'en':{'value':'Ancient Author'}},'claims':{},'sitelinks':{'enwiki':{'url':url}}}}}
            raw=json.dumps(entity).encode();article=b'<html>Ancient Author was active in the early eleventh century.</html>'
            args=argparse.Namespace(packet=str(path),role='author',qid='Q123',name='Ancient Author',ru=None,died=None,died_before=1224,bound_note='The linked article dates activity to the early eleventh century; the exclusive bound allows a 122-year lifespan.',source=url,remove=False,wartime=None,rehabilitated=None,bounded=False)
            def fetch(target):return raw if 'Special:EntityData' in target else article
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',side_effect=fetch):
                kit.cmd_person(args);person=json.loads((path/'evidence.json').read_text())['persons'][0]
                self.assertIsNone(person['died']);self.assertEqual(person['died_before'],1224)
                self.assertEqual(kit.original_bound_problems(str(path),person),[])
                stripped=dict(person);stripped.pop('bound_reference');self.assertTrue(kit.original_bound_problems(str(path),stripped,offline=True))
                (path/'reading.json').write_text(json.dumps({'reader':{'id':'maker'}}));(path/'review.json').write_text(json.dumps({'reader':{'id':'reader'}}))
                self.assertTrue(kit.original_bound_problems(str(path),person,second=True))
                kit.cmd_person_bound_review(argparse.Namespace(packet=str(path),role='author',qid='Q123',reader='reader',assessment='The linked source places the author in the early eleventh century, supporting this conservative bound.'))
                self.assertEqual(kit.original_bound_problems(str(path),person,second=True),[])
                Path(kit.cache(str(path),person['bound_reference']['file'])).write_bytes(b'changed')
                self.assertTrue(kit.original_bound_problems(str(path),person,second=True))
                args.source='https://en.wikipedia.org/wiki/Another'
                with self.assertRaises(SystemExit):kit.cmd_person(args)

class WitnessAncientBoundTest(unittest.TestCase):
    def test_linked_reference_preserves_qid_and_detects_changed_source(self):
        import argparse,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            url='https://en.wikipedia.org/wiki/Example';qid='Q123'
            entity={'entities':{qid:{'claims':{},'sitelinks':{'enwiki':{'url':url}}}}}
            raw=json.dumps(entity).encode();person={'role':'author','qid':qid,'name':{'en':'Example'},'born':None,'died':None,'source':'https://www.wikidata.org/wiki/Q123','evidence':{'file':'entity.json','sha256':hashlib.sha256(raw).hexdigest(),'url':'https://www.wikidata.org/wiki/Special:EntityData/Q123.json'}}
            file.write_text(json.dumps({'witness':{'maker':'maker','persons':[person]},'review':{'reader':'old'}}))
            args=argparse.Namespace(packet=str(path),language='en',role='author',qid=qid,name='Example',source=url,died_before=1200,bound_note='The source places the author in a period earlier than this conservative bound.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=b'<html>Example lived in antiquity.</html>'):
                Path(kit.witness_cache(str(path),'en','entity.json')).write_bytes(raw)
                kit.cmd_witness_person_bound(args);data=json.loads(file.read_text());person=data['witness']['persons'][0]
                self.assertEqual(person['qid'],qid);self.assertIsNone(person['died']);self.assertEqual(data['review'],{})
                self.assertEqual(kit.witness_linked_bound_problems(str(path),'en',person),[])
                stripped=dict(person);stripped.pop('bound_reference');self.assertTrue(kit.witness_linked_bound_problems(str(path),'en',stripped,offline=True))
                data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(data))
                kit.cmd_witness_bound_review(argparse.Namespace(packet=str(path),language='en',role='author',name='Example',reader='second',assessment='The reference places this person in antiquity, earlier than the conservative bound.'))
                self.assertIn(kit.page_number_digest(person),json.loads(file.read_text())['review']['bounds'])
                Path(kit.witness_cache(str(path),'en',person['bound_reference']['file'])).write_bytes(b'changed')
                self.assertTrue(kit.witness_linked_bound_problems(str(path),'en',person))
                person.pop('died_before');self.assertTrue(kit.witness_linked_bound_problems(str(path),'en',person,offline=True))
                Path(str(file)+'.accepted').write_text('{}')
                with self.assertRaises(SystemExit):kit.cmd_witness_person_bound(args)

    def test_source_bound_keeps_unknown_death_and_anonymity(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            file.write_text(json.dumps({'witness':{'persons':[]},'review':{'reader':'old'}}))
            args=argparse.Namespace(packet=str(path),language='en',role='author',name='Anonymous Author',url='https://en.wikipedia.org/wiki/Example',ru=None,born=None,died=None,died_before=1200,bound_note='The reference establishes this author lived in an ancient historical period.',anonymous=True,file=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=b'<html>Anonymous Author lived in antiquity.</html>'):
                kit.cmd_witness_person_source(args)
            data=json.loads(file.read_text());person=data['witness']['persons'][0]
            self.assertIsNone(person['died']);self.assertEqual(person['died_before'],1200);self.assertTrue(person['anonymous']);self.assertEqual(data['review'],{})
            self.assertFalse(kit.witness_public_domain_by_dates([person],1900)['eu'])
            self.assertFalse(kit.witness_public_domain_by_dates([person],1900)['russia'])
            data['witness']['maker']='maker';data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(data))
            review=argparse.Namespace(packet=str(path),language='en',role='author',name='Anonymous Author',reader='second',assessment='The reference places this author in antiquity, earlier than the conservative bound.')
            kit.cmd_witness_bound_review(review);data=json.loads(file.read_text())
            self.assertIn(kit.page_number_digest(person),data['review']['bounds'])
            args.died=1180
            with self.assertRaises(SystemExit):kit.cmd_witness_person_source(args)
            args.died=None;args.role='translator'
            with self.assertRaises(SystemExit):kit.cmd_witness_person_source(args)

class NationalArchiveReferenceTest(unittest.TestCase):
    def test_estonian_archive_is_trusted_without_trusting_lookalike_hosts(self):
        self.assertTrue(kit.trusted('https://www.eha.ee/fondiloend/frames/fond_prop.php?id=3024'))
        self.assertFalse(kit.trusted('https://eha.ee.example.org/fondiloend/'))
        self.assertFalse(kit.trusted('http://www.eha.ee/fondiloend/'))

    def test_refresh_keeps_legal_status_and_previous_source(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            old={'role':'editor','name':{'en':'Maxim Rehbinder','ru':'Maxim Rehbinder'},'rehabilitated':1990,'rehabilitation_source':'https://example.org/rehabilitation','evidence':{'kind':'reference','file':'old.html'}}
            file.write_text(json.dumps({'witness':{'persons':[old]},'review':{'reader':'previous'}}))
            args=argparse.Namespace(packet=str(path),language='en',role='editor',name='Maxim Rehbinder',url='https://www.eha.ee/archive',ru=None,born=1873,died=1914,died_before=None,bound_note=None,anonymous=False,file=None)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=b'<html>Maxim Rehbinder 1873 1914</html>'):kit.cmd_witness_person_source(args)
            data=json.loads(file.read_text());self.assertEqual(data['witness']['persons'][0]['rehabilitated'],1990)
            self.assertEqual(data['person_corrections'][0]['removed'],old);self.assertEqual(data['review'],{})

class MultilingualWikidataNameTest(unittest.TestCase):
    def test_witness_uses_mul_label_without_inventing_dates(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            file.write_text(json.dumps({'witness':{'persons':[]},'review':{}}))
            entity={'entities':{'Q18910535':{'labels':{'mul':{'value':'Joseph Ward Swain'}},'claims':{}}}}
            args=argparse.Namespace(packet=str(path),language='en',role='translator',qid='Q18910535',bounded=False)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=json.dumps(entity).encode()):kit.cmd_witness_person(args)
            person=json.loads(file.read_text())['witness']['persons'][0]
            self.assertEqual(person['name']['en'],'Joseph Ward Swain');self.assertIsNone(person['died'])
            data=json.loads(file.read_text());data['witness']['persons'][0].update({'rehabilitated':1990,'rehabilitation_source':'https://example.org/rehabilitation'});file.write_text(json.dumps(data))
            entity['entities']['Q18910535']['descriptions']={'en':{'value':'Translator'}}
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=json.dumps(entity).encode()):kit.cmd_witness_person(args)
            current=json.loads(file.read_text());updated=current['witness']['persons'][0]
            self.assertEqual(updated['rehabilitated'],1990);self.assertNotEqual(person['evidence']['file'],updated['evidence']['file'])
            self.assertTrue((Path(tmp)/'cache/work/witnesses/en'/person['evidence']['file']).exists());self.assertTrue(current['person_corrections'])

class UnidentifiedTranslatorTest(unittest.TestCase):
    def test_unidentified_hand_needs_short_quotation_and_independent_source_review(self):
        import argparse,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);folder=Path(tmp)/'cache/work/witnesses/es';folder.mkdir(parents=True)
            raw=b'<html>Published edition, translator not named.</html>';digest=hashlib.sha256(raw).hexdigest();(folder/'edition.html').write_bytes(raw)
            rights={country:{'basis':'short-quotation','law':'A separately assessed territorial quotation basis.'} for country in kit.JURISDICTIONS}
            data={'witness':{'maker':'maker','edition':'Exact publication','rights':rights,'persons':[]},'sources':[{'file':'edition.html','url':'https://example.org/edition','sha256':digest}],'review':{}}
            file=path/'witnesses/es.json';file.write_text(json.dumps(data));details=Path(tmp)/'identity.json'
            details.write_text(json.dumps({'finding':'The publisher source and catalogues do not name the individual translator of this edition.','sources':['edition.html'],'searches':[{'url':'https://example.org/catalogue','outcome':'No translator is identified in this catalogue record.'},{'url':'https://example.org/publisher','outcome':'The publisher does not identify an individual translator.'}]}))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_witness_unidentified_translator(argparse.Namespace(packet=str(path),language='es',file=str(details)))
                data=json.loads(file.read_text());self.assertTrue(kit.unidentified_translator_problems(str(path),'es',data,second=True))
                data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(data))
                kit.cmd_witness_unidentified_review(argparse.Namespace(packet=str(path),language='es',reader='second',assessment='The exact source and independent catalogue searches do not identify the translator.'))
                data=json.loads(file.read_text());self.assertEqual(kit.unidentified_translator_problems(str(path),'es',data,second=True),[])
                data['witness']['rights']['eu']['basis']='public-domain';self.assertTrue(kit.unidentified_translator_problems(str(path),'es',data,second=True))
                data=json.loads(file.read_text());data['witness']['unidentified_translator']['finding']+=' changed';self.assertTrue(kit.unidentified_translator_problems(str(path),'es',data,second=True))
                kit.cmd_witness_unidentified_clear(argparse.Namespace(packet=str(path),language='es',reason='A subsequent source identified the translator of this edition.'))
                data=json.loads(file.read_text());self.assertNotIn('unidentified_translator',data['witness']);self.assertTrue(data['translator_corrections']);self.assertEqual(data['review'],{})

class NewTranslationTest(unittest.TestCase):
    def test_complete_new_translation_passes_the_real_witness_gate(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);folder=Path(tmp)/'cache/work/witnesses/es';folder.mkdir(parents=True)
            image=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');digest=hashlib.sha256(image).hexdigest()
            for name in ('maker.png','second.png'):(folder/name).write_bytes(image)
            declaration=b'<html>Translator licenses New translation 2026 under https://creativecommons.org/licenses/by/4.0/</html>';(folder/'grant.html').write_bytes(declaration)
            url='https://example.org/grant';license_url='https://creativecommons.org/licenses/by/4.0/'
            source={'file':'grant.html','url':url,'sha256':hashlib.sha256(declaration).hexdigest()}
            people=[]
            for qid,name,role in [('Q1','Author','author')]:
                raw=json.dumps({'entities':{qid:{'labels':{'en':{'value':name}},'claims':{}}}}).encode();filename=qid+'.json';(folder/filename).write_bytes(raw)
                people.append({'role':role,'qid':qid,'name':{'en':name},'born':None,'died':None,'source':'https://www.wikidata.org/wiki/'+qid,'evidence':{'file':filename,'url':'https://www.wikidata.org/wiki/Special:EntityData/'+qid+'.json','sha256':hashlib.sha256(raw).hexdigest()}})
            rights={country:{'basis':'licensed','law':'The licence applies in this named territory.'} for country in kit.JURISDICTIONS};original_rights={country:{'basis':'public-domain'} for country in kit.JURISDICTIONS}
            world={'fragments':{'works':{'work':{'original':'en','witnesses':{'en':{'edition':'Original','rights':original_rights}}}},'places':{'work':{'p':{'texts':{'en':'The original sentence.'},'links':{'en':'https://example.org/original'}}}}}}
            data={'language':'es','witness':{'maker':'maker','edition':'New translation 2026','attribution':'New translation by Translator','licence':'CC BY 4.0','url':url,'year':2026,'rights':rights,'persons':people},'sources':[source],'images':[{'file':'maker.png','url':'https://example.org/page','sha256':digest,'by':'maker'},{'file':'second.png','url':'https://example.org/page','sha256':digest,'by':'second'}],'places':{},'review':{}}
            file=path/'witnesses/es.json';file.write_text(json.dumps(data));args=argparse.Namespace(packet=str(path),language='es',loc='p',creator='Translator',text='Una oración completa.',image='maker.png',page='7',alignment='This complete sentence corresponds to the original.',source_assessment='The original source has been researched and is public domain in the named territories.',image_assessment='The image is the actual printed page of the original source.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'world',return_value=world):
                kit.cmd_licence(argparse.Namespace(packet=str(path),language='es',source='grant.html',url=license_url,explanation='The named translator grants this exact new translation under the licence.'))
                kit.cmd_witness_creator(argparse.Namespace(packet=str(path),language='es',name='Translator'))
                kit.cmd_witness_new_translation(args)
                self.assertTrue(kit.witness_problems(str(path),'es',json.loads(file.read_text()),second=True))
                kit.cmd_witness_review(argparse.Namespace(packet=str(path),language='es',loc='p',reader='second',model='model',alignment='The translation faithfully follows the complete original sentence.'))
                kit.cmd_licence_review(argparse.Namespace(packet=str(path),language='es',assessment='The translator declares this licence for the exact new translation.'))
                kit.cmd_witness_new_translation_review(argparse.Namespace(packet=str(path),language='es',loc='p',reader='second',assessment='The exact original page and translation correspond, and the original edition permits this adaptation.'))
                self.assertEqual(kit.witness_problems(str(path),'es',json.loads(file.read_text()),second=True),[])

    def test_new_translation_binds_author_licence_original_and_independent_review(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);folder=Path(tmp)/'cache/work/witnesses/es';folder.mkdir(parents=True)
            image=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');digest=hashlib.sha256(image).hexdigest();(folder/'page.png').write_bytes(image)
            declaration=b'Translator: New translation 2026';(folder/'grant.html').write_bytes(declaration)
            source_rights={country:{'basis':'public-domain'} for country in kit.JURISDICTIONS};translation_rights={country:{'basis':'licensed'} for country in kit.JURISDICTIONS}
            world={'fragments':{'works':{'work':{'original':'en','witnesses':{'en':{'edition':'Original','rights':source_rights}}}},'places':{'work':{'p':{'texts':{'en':'The original sentence.'},'links':{'en':'https://example.org/original'}}}}}}
            data={'language':'es','witness':{'maker':'maker','edition':'New translation 2026','attribution':'New translation by Translator','url':'https://example.org/licence','rights':translation_rights,'persons':[{'role':'translator','name':{'en':'Translator'},'source':'https://example.org/translator'}],'licence_evidence':{'source':{'file':'grant.html','url':'https://example.org/licence','sha256':hashlib.sha256(declaration).hexdigest()}}},'sources':[],'images':[{'file':'page.png','url':'https://example.org/page','sha256':digest,'by':'maker'}],'places':{},'review':{}}
            file=path/'witnesses/es.json';file.write_text(json.dumps(data));args=argparse.Namespace(packet=str(path),language='es',loc='p',creator='Translator',text='Una oración completa.',image='page.png',page='7',alignment='This complete sentence corresponds to the original.',source_assessment='The original source has been independently researched and is public domain in the named territories.',image_assessment='The image is the actual printed page of the original source.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'world',return_value=world):
                kit.cmd_witness_new_translation(args);data=json.loads(file.read_text());place=data['places']['p']
                self.assertNotIn('source',place);self.assertEqual(place['transcription']['kind'],'new-translation')
                self.assertTrue(kit.new_translation_problems(str(path),'es',data,'p',second=True))
                data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(data))
                kit.cmd_witness_new_translation_review(argparse.Namespace(packet=str(path),language='es',loc='p',reader='second',assessment='The exact original page and translation correspond, and the original edition permits this adaptation.'))
                data=json.loads(file.read_text());self.assertEqual(kit.new_translation_problems(str(path),'es',data,'p',second=True),[])
                data['places']['p']['text']='Otra oración completa.';self.assertTrue(kit.new_translation_problems(str(path),'es',data,'p',second=True));data=json.loads(file.read_text())
                world['fragments']['places']['work']['p']['texts']['en']='Different original sentence.'
                self.assertTrue(kit.new_translation_problems(str(path),'es',data,'p',second=True))
                world['fragments']['places']['work']['p']['texts']['en']='The original sentence.'
                source_rights['eu']['basis']='short-quotation';self.assertTrue(kit.new_translation_problems(str(path),'es',data,'p',second=True))
                source_rights['eu']['basis']='licensed';world['fragments']['works']['work']['witnesses']['en']['licence_evidence']={'licence_url':'https://creativecommons.org/licenses/by-nd/4.0/'}
                self.assertTrue(any('adaptation' in issue for issue in kit.new_translation_problems(str(path),'es',data,'p',second=True)))
                (folder/'grant.html').write_bytes(b'Somebody else and another edition')
                self.assertTrue(any('creator declaration' in issue for issue in kit.new_translation_problems(str(path),'es',data,'p',second=True)))


class OriginalBrowserCaptureTest(unittest.TestCase):
    def test_original_web_section_needs_separate_capture_and_review(self):
        import argparse, base64, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();folder=Path(tmp)/'cache/work';folder.mkdir(parents=True)
            html=b'<html><body><p>A complete sentence.</p></body></html>';digest=hashlib.sha256(html).hexdigest();url='https://example.org/article'
            (folder/'article.html').write_bytes(html);(folder/'article.html.txt').write_text(kit.plain(html,'article.html'))
            image=Path(tmp)/'capture.png';image.write_bytes(base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII='))
            source={'role':'transcription','file':'article.html','text':'article.html.txt','url':url,'sha256':digest}
            (path/'evidence.json').write_text(json.dumps({'scan':{'item':'old'},'sources':[source],'places':{'p':{'transcription':'article.html','scan':'old','images':[9]}},'images':[]}))
            (path/'places.json').write_text(json.dumps({'work':'work','entry':{'original':'en'},'places':{'p':{'texts':{'en':'A complete sentence.'}}}}))
            (path/'reading.json').write_text(json.dumps({'reader':{'id':'maker'}}))
            (path/'review.json').write_text(json.dumps({'reader':{'id':'second','model':'model','date':'2026-11-28'},'images':[{'loc':'p','n':1,'reads':'as the packet'}],'accept':True}))
            args=argparse.Namespace(packet=str(path),loc='p',source='article.html',file=str(image),by='maker',reader='maker',captured_at='2026-11-28T05:00:00+00:00',n=1,address_kind='section',address_value='Discussion')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=html):
                kit.cmd_original_browser_capture(args)
                ev=kit.evidence(str(path));places=kit.part(str(path),'places.json');reading=kit.part(str(path),'reading.json')
                self.assertEqual(ev['scan']['item'],'old');self.assertEqual(ev['places']['p']['images'],[1]);self.assertTrue(ev['page_rebindings'][0]['review']['accept'])
                self.assertEqual(kit.original_browser_problems(str(path),ev,reading,places),[])
                self.assertEqual(kit.part(str(path),'review.json')['images'],[])
                self.assertTrue(kit.original_browser_problems(str(path),ev,reading,places,second=True))
                image.write_bytes(image.read_bytes()+b'second capture')
                args.by='second';args.reader='second';kit.cmd_original_browser_capture(args)
                review=kit.part(str(path),'review.json');review['images']=[{'loc':'p','n':1,'reads':'as the packet'}];review['accept']=True;kit.save(str(path/'review.json'),review)
                ev=kit.evidence(str(path));self.assertEqual(kit.original_browser_problems(str(path),ev,reading,places,second=True),[])
                maker_image=next(i for i in ev['images'] if i['by']=='maker');second_image=next(i for i in ev['images'] if i['by']=='second');self.assertNotEqual(maker_image['sha256'],second_image['sha256'])
                quote={'source':url,'page':'Discussion','lines':'section','sha256':maker_image['sha256']}
                self.assertEqual(kit.quotation_problems('p','A complete sentence.',quote,ev,second=True,path=str(path)),[])
                ev['places']['p']['quotation']=quote
                self.assertEqual(kit.second_gate(str(path),reading,ev,places['places']),[])
                places['places']['p']['addresses']['en']['value']='Another section'
                self.assertTrue(kit.original_browser_problems(str(path),ev,reading,places,second=True))
                places=kit.part(str(path),'places.json');second=next(i for i in ev['images'] if i['by']=='second');(folder/second['file']).write_bytes(b'changed')
                self.assertTrue(kit.original_browser_problems(str(path),ev,reading,places,second=True))

class PrintedParticipantTest(unittest.TestCase):
    def test_image_identity_requires_independent_page_and_preserves_old_person(self):
        import argparse, base64, hashlib, json, tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True)
            folder=Path(tmp)/'cache/work/witnesses/ru';folder.mkdir(parents=True)
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');digest=hashlib.sha256(raw).hexdigest()
            (folder/'maker.png').write_bytes(raw);(folder/'second.png').write_bytes(raw)
            old={'role':'editor','name':{'en':'M. Epstein'},'died':None};url='https://archive.org/example'
            data={'witness':{'maker':'maker','persons':[old]},'images':[{'file':'maker.png','by':'maker','url':url,'sha256':digest}],'review':{'reader':'old'}}
            file=path/'witnesses/ru.json';file.write_text(json.dumps(data))
            args=argparse.Namespace(packet=str(path),language='ru',role='editor',old_name='M. Epstein',name='М. А. Эпштейн',died=1901,image='maker.png',page='501',wording='М. А. Эпштейнъ († 1901)',assessment='The printed page names the contributor and directly gives the year of death.',reason='Replace the provisional identity with exactly what the printed page states.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                data['witness']['persons'][0]['rehabilitated']=1990;file.write_text(json.dumps(data))
                with self.assertRaises(SystemExit):kit.cmd_witness_person_image(args)
                self.assertEqual(json.loads(file.read_text())['witness']['persons'][0]['rehabilitated'],1990)
                data['witness']['persons'][0].pop('rehabilitated');file.write_text(json.dumps(data))
                kit.cmd_witness_person_image(args)
                current=json.loads(file.read_text());person=current['witness']['persons'][0]
                self.assertEqual(current['person_corrections'][0]['removed'],old);self.assertEqual(current['review'],{})
                self.assertTrue(kit.image_identity_problems(str(path),'ru',current,person,second=True))
                current['images'].append({'file':'second.png','by':'second','url':url,'sha256':digest});current['review']={'reader':'second','places':{'p':{}}};file.write_text(json.dumps(current))
                review=argparse.Namespace(packet=str(path),language='ru',name=args.name,reader='second',assessment='The same printed page identifies the named contributor and the printed year of death.')
                kit.cmd_witness_image_identity_review(review);current=json.loads(file.read_text())
                self.assertEqual(kit.image_identity_problems(str(path),'ru',current,person,second=True),[])
                person['died']=1902;self.assertTrue(kit.image_identity_problems(str(path),'ru',current,person,second=True))
                (folder/'second.png').write_bytes(b'changed');person['died']=1901
                self.assertTrue(kit.image_identity_problems(str(path),'ru',current,person,second=True))

class WitnessEmphasisTest(unittest.TestCase):
    def test_page_emphasis_requires_distinct_scoped_review(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');digest=hashlib.sha256(raw).hexdigest();(folder/'page.png').write_bytes(raw)
            place={'text':'Something uncanny in real experience is conditioned thus.','page':'401','url':'https://example.org/page','image_sha256':digest}
            data={'witness':{'maker':'maker'},'images':[{'file':'page.png','url':place['url'],'sha256':digest,'by':'maker'}],'places':{'p':place},'review':{'reader':'old'}}
            file=path/'witnesses/en.json';file.write_text(json.dumps(data))
            args=argparse.Namespace(packet=str(path),language='en',loc='p',text='real experience',page=None,reason='The printed italics distinguish actual experience from imagined events.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_witness_emphasis(args);data=json.loads(file.read_text());self.assertEqual(data['review'],{})
                self.assertTrue(kit.witness_emphasis_problems(str(path),'en',data,'p',second=True))
                data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(data))
                kit.cmd_witness_emphasis_review(argparse.Namespace(packet=str(path),language='en',loc='p',reader='second',assessment='The independent reader saw italics on these exact words in this printed edition.'))
                data=json.loads(file.read_text());self.assertEqual(kit.witness_emphasis_problems(str(path),'en',data,'p',second=True),[])
                data['places']['p']['text']='The meaning of real experience differs.'
                self.assertTrue(kit.witness_emphasis_problems(str(path),'en',data,'p',second=True))
                kit.cmd_witness_emphasis_withdraw(argparse.Namespace(packet=str(path),language='en',loc='p',text='real experience',reason='The later reading found this typographic claim was mistaken.'))
                data=json.loads(file.read_text());self.assertNotIn('emphasis',data['places']['p']);self.assertEqual(data['review'],{});self.assertEqual(data['emphasis_corrections'][0]['removed']['text'],'real experience')

class BrowserWitnessCaptureTest(unittest.TestCase):
    def test_separate_captures_keep_html_lineage_reader_and_image_integrity(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/zh-Hans.json'
            html=b'<html><body>Lawful electronic edition chapter.</body></html>';url='https://example.org/chapter';source={'file':'chapter.html','url':url,'sha256':hashlib.sha256(html).hexdigest()}
            page=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
            maker_file=Path(tmp)/'maker.png';second_file=Path(tmp)/'second.png';maker_file.write_bytes(page);second_file.write_bytes(page+b'different-browser-render')
            file.write_text(json.dumps({'witness':{'maker':'maker'},'sources':[source],'images':[],'places':{},'review':{}}))
            args=argparse.Namespace(packet=str(path),language='zh-Hans',source='chapter.html',file=str(maker_file),by='maker',reader='maker',captured_at='2026-11-28T05:00:00+00:00')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=html):
                Path(kit.witness_cache(str(path),'zh-Hans','chapter.html')).write_bytes(html);kit.cmd_witness_browser_capture(args)
                data=json.loads(file.read_text());maker=data['images'][0];self.assertEqual(maker['capture']['method'],'codex-iab-screenshot')
                args.by='second';args.file=str(second_file);args.reader='second';kit.cmd_witness_browser_capture(args)
                data=json.loads(file.read_text());second=data['images'][1];self.assertNotEqual(second['sha256'],maker['sha256'])
                page_ref={'url':url,'image_sha256':maker['sha256']};self.assertEqual(kit.second_witness_image(data,page_ref,'second'),second)
                self.assertIsNone(kit.second_witness_image(data,page_ref,'maker'));self.assertEqual(kit.browser_capture_problems(str(path),'zh-Hans',data,second),[])
                data['places']['p']={'text':'这是原文。','page':'chapter 5','url':url,'image_sha256':maker['sha256']};file.write_text(json.dumps(data,ensure_ascii=False))
                kit.cmd_witness_review(argparse.Namespace(packet=str(path),language='zh-Hans',loc='p',reader='second',model='model',alignment='The complete Chinese passage matches the original location.'))
                self.assertEqual(json.loads(file.read_text())['review']['places']['p']['image_sha256'],maker['sha256'])
                second_path=Path(kit.witness_cache(str(path),'zh-Hans',second['file']));second_path.write_bytes(b'changed');self.assertTrue(kit.browser_capture_problems(str(path),'zh-Hans',data,second))
                args.reader='maker'
                with self.assertRaises(SystemExit):kit.cmd_witness_browser_capture(args)
                args.reader='second';args.captured_at='2026-11-28'
                with self.assertRaises(SystemExit):kit.cmd_witness_browser_capture(args)
                args.captured_at='2026-11-28T05:00:00+00:00'
                with patch.object(kit,'get',return_value=b'<html>different</html>'),self.assertRaises(SystemExit):kit.cmd_witness_browser_capture(args)


class QuotationBoundaryTest(unittest.TestCase):
    def test_external_attribution_and_period_are_reviewed_but_not_added_to_quote(self):
        import argparse,base64,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/es.json'
            page=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');page_hash=hashlib.sha256(page).hexdigest()
            text='“Una frase completa”';suffix=' (traducción del autor).';raw=(text+suffix).encode();digest=hashlib.sha256(raw).hexdigest();url='https://example.org/page'
            data={'witness':{'maker':'maker'},'sources':[{'file':'text.txt','url':'https://example.org/text','sha256':digest}],'images':[{'by':'maker','file':'page.png','url':url,'sha256':page_hash}],'places':{'p':{'text':text,'source':'text.txt','url':url,'image_sha256':page_hash}},'review':{'reader':'old'}};file.write_text(json.dumps(data,ensure_ascii=False))
            args=argparse.Namespace(packet=str(path),language='es',loc='p',suffix=suffix,reason='The printed period follows the translator attribution outside the quoted words.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                Path(kit.witness_cache(str(path),'es','text.txt')).write_bytes(raw);Path(kit.witness_cache(str(path),'es','page.png')).write_bytes(page)
                kit.cmd_witness_sentence_boundary(args);result=json.loads(file.read_text());self.assertEqual(result['places']['p']['text'],text);self.assertEqual(result['review'],{})
                self.assertTrue(kit.witness_boundary_problems(str(path),'es',result,'p',second=True))
                result['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(result,ensure_ascii=False))
                kit.cmd_witness_boundary_review(argparse.Namespace(packet=str(path),language='es',loc='p',reader='second',assessment='The page puts the period after the parenthetical attribution and outside the quote.'))
                result=json.loads(file.read_text());self.assertEqual(kit.witness_boundary_problems(str(path),'es',result,'p',second=True),[])
                result['places']['p']['sentence_boundary']['suffix']=' (inventado).';self.assertTrue(kit.witness_boundary_problems(str(path),'es',result,'p'))
                args.suffix=' (inventado).'
                with self.assertRaises(SystemExit):kit.cmd_witness_sentence_boundary(args)


class OriginalHumanPageTest(unittest.TestCase):
    def test_original_page_requires_same_image_and_independent_scoped_reading(self):
        import argparse,base64,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();raw=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=');image=Path(tmp)/'page.png';image.write_bytes(raw)
            (path/'places.json').write_text(json.dumps({'work':'work','entry':{'original':'zh'},'places':{}}));(path/'reading.json').write_text(json.dumps({'reader':{'id':'maker'}}))
            args=argparse.Namespace(packet=str(path),loc='dream p57',url='https://example.org/page.png',n=57,text='昔者莊周夢為胡蝶。',reader='maker',label_ru='Сон, с. 57',label_en='Dream, p. 57',why='The printed edition differs from the available transcription; the actual page was read directly.',punctuation='Punctuation is editorial and follows the printed sentence boundary.',file=str(image))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_transcribe_original_page(args);ev=kit.evidence(str(path));places=kit.part(str(path),'places.json');reading=kit.part(str(path),'reading.json');source=ev['sources'][0]
                self.assertEqual(source['kind'],'human-page');self.assertNotIn('fetched',source);self.assertEqual(places['places']['dream p57']['texts']['zh'],args.text)
                self.assertEqual(kit.human_page_problems(str(path),ev,reading,places),[])
                self.assertTrue(kit.human_page_problems(str(path),ev,reading,places,second=True))
                kit.cmd_page(argparse.Namespace(packet=str(path),url=args.url,n=57,loc=args.loc,by='second',file=str(image)))
                (path/'review.json').write_text(json.dumps({'reader':{'id':'second'},'images':[{'loc':args.loc,'n':57,'reads':'as the packet'}]}))
                kit.cmd_original_page_review(argparse.Namespace(packet=str(path),loc=args.loc,reader='second',assessment='I compared the characters and the declared editorial punctuation against the same page.'))
                ev=kit.evidence(str(path));self.assertEqual(kit.human_page_problems(str(path),ev,reading,places,second=True),[])
                second=next(item for item in ev['images'] if item.get('by')=='second');second_file=Path(kit.cache(str(path),second['file']));second_file.write_bytes(b'changed')
                self.assertTrue(kit.human_page_problems(str(path),ev,reading,places,second=True));second_file.write_bytes(raw)
                places['places'][args.loc]['texts']['zh']='Different text.';(path/'places.json').write_text(json.dumps(places))
                self.assertTrue(kit.human_page_problems(str(path),ev,reading,places,second=True))
                Path(kit.cache(str(path),source['file'])).write_bytes(b'changed');self.assertTrue(kit.human_page_problems(str(path),ev,reading,places))
                with self.assertRaises(SystemExit):kit.cmd_transcribe_original_page(args)


class PublisherIdentityTest(unittest.TestCase):
    def test_collective_translation_does_not_invent_individual_translators(self):
        import argparse,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json';url='https://example.org/article'
            raw=b'<html><body><p>Alice One and Bob Two</p><p>"A whole sentence" (our translation).</p></body></html>';digest=hashlib.sha256(raw).hexdigest()
            rights={c:{'basis':'licensed','law':'Source licence and jurisdiction reviewed separately.'} for c in kit.JURISDICTIONS}
            data={'witness':{'maker':'maker','edition':'The article','attribution':'Translation attributed to the authors Alice One and Bob Two','rights':rights,'persons':[{'role':'author'}]},'sources':[{'file':'article.html','url':url,'sha256':digest}],'places':{'p':{'text':'“A whole sentence”'}},'review':{}}
            file.write_text(json.dumps(data));spec=Path(tmp)/'collective.json';spec.write_text(json.dumps({'members':['Alice One','Bob Two'],'source':'article.html','claim':'our translation','assessment':'The authors attribute this exact passage to their collective without naming an individual translator.'}))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                Path(kit.witness_cache(str(path),'en','article.html')).write_bytes(raw);kit.cmd_witness_collective(argparse.Namespace(packet=str(path),language='en',loc='p',file=str(spec)))
                data=json.loads(file.read_text());self.assertFalse(any(p.get('role')=='translator' for p in data['witness']['persons']));self.assertTrue(kit.collective_translation_problems(str(path),'en',data,second=True))
                data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(data))
                kit.cmd_witness_collective_review(argparse.Namespace(packet=str(path),language='en',loc='p',reader='second',assessment='I checked both named authors, the exact source statement and the territorial rights evidence.'))
                data=json.loads(file.read_text());self.assertEqual(kit.collective_translation_problems(str(path),'en',data,second=True),[])
                data['language']='en'
                with patch.object(kit,'world',return_value={'fragments':{'places':{'work':{'p':{}}}}}):
                    issues=kit.witness_problems(str(path),'en',data,second=True)
                    self.assertNotIn('translator is missing',issues);self.assertTrue(any('licensed basis' in issue for issue in issues))
                data['witness']['rights']['eu']['basis']='public-domain';self.assertTrue(any('cannot establish public domain' in p for p in kit.collective_translation_problems(str(path),'en',data)))
                data['witness']['rights']['eu']['basis']='licensed';data['witness']['edition']='Another edition';self.assertTrue(kit.collective_translation_problems(str(path),'en',data,second=True))
                kit.cmd_witness_collective_clear(argparse.Namespace(packet=str(path),language='en',reason='A later source establishes a different attribution and requires a renewed independent reading.'))
                cleared=json.loads(file.read_text());self.assertNotIn('collective_translation',cleared['witness']);self.assertTrue(cleared['collective_corrections'][0]['review']);self.assertEqual(cleared['review'],{})

    def test_catalogue_identity_requires_independent_edition_bound_review(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/en.json'
            file.write_text(json.dumps({'witness':{'maker':'maker','edition':'Selected writings 1972','persons':[]},'review':{},'places':{}}))
            source=Path(tmp)/'source.html';source.write_text('<p>Patrick Edward Charvet; P.E. Charvet; Selected writings on art and artists, 1972.</p>')
            args=argparse.Namespace(packet=str(path),language='en',role='translator',name='Patrick Edward Charvet',url='https://ci.nii.ac.jp/author/DA06887074',ru=None,born=None,died=None,died_before=None,bound_note=None,anonymous=False,file=str(source))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                kit.cmd_witness_person_source(args);data=json.loads(file.read_text());person=data['witness']['persons'][0]
                self.assertIsNone(person['died']);self.assertTrue(kit.reference_identity_problems(data,person,True))
                data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(data))
                review=argparse.Namespace(packet=str(path),language='en',role='translator',name=args.name,reader='second',assessment='I checked the catalogue identity, initials and the specified 1972 edition together.')
                kit.cmd_witness_reference_review(review);data=json.loads(file.read_text());self.assertEqual(kit.reference_identity_problems(data,person,True),[])
                data['witness']['edition']='Another edition';self.assertTrue(kit.reference_identity_problems(data,person,True))

    def test_exact_edition_identity_preserves_predecessor_and_requires_review(self):
        import argparse,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/ru.json'
            raw='<html><p>Переводчик: Парахневич Елена Валерьевна</p><p>ISBN: 978-5-17-179033-2</p></html>'.encode()
            source={'file':'publisher.html','url':'https://ast.ru/book/propaganda-893044/','sha256':hashlib.sha256(raw).hexdigest()}
            old={'role':'translator','name':{'en':'Е. Парахневич'},'born':None,'died':None,'source':'https://search.rsl.ru/record/1','evidence':{'kind':'reference'}}
            data={'witness':{'maker':'maker','edition':'АСТ ISBN 978-5-17-179033-2','persons':[old]},'sources':[source],'review':{'reader':'old'}};file.write_text(json.dumps(data,ensure_ascii=False))
            args=argparse.Namespace(packet=str(path),language='ru',old_name='Е. Парахневич',full_name='Парахневич Елена Валерьевна',source='publisher.html',isbn='978-5-17-179033-2',reason='The exact edition publisher names this translator in full; the previous source used initials.')
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                Path(kit.witness_cache(str(path),'ru','publisher.html')).write_bytes(raw)
                kit.cmd_witness_person_publisher(args);result=json.loads(file.read_text());person=result['witness']['persons'][0]
                self.assertEqual(result['person_corrections'][0]['removed'],old);self.assertEqual(result['review'],{});self.assertIsNone(person['died'])
                self.assertTrue(kit.publisher_identity_problems(str(path),'ru',result,person,second=True))
                result['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};file.write_text(json.dumps(result,ensure_ascii=False))
                kit.cmd_witness_publisher_review(argparse.Namespace(packet=str(path),language='ru',name=args.full_name,reader='second',assessment='I checked the exact ISBN and the named translator on the publisher edition card.'))
                result=json.loads(file.read_text());self.assertEqual(kit.publisher_identity_problems(str(path),'ru',result,person,second=True),[])
                result['witness']['edition']='A different edition ISBN 9785000000000';self.assertTrue(kit.publisher_identity_problems(str(path),'ru',result,person,offline=True))
                Path(kit.witness_cache(str(path),'ru','publisher.html')).write_bytes(b'changed');self.assertTrue(kit.publisher_identity_problems(str(path),'ru',json.loads(file.read_text()),person))


class WitnessPlaceWithdrawalTest(unittest.TestCase):
    def test_withdrawal_preserves_place_and_review_and_does_not_claim_a_search(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/ru.json'
            place={'text':'The passage.','page':'144','image_sha256':'a'*64,'transcription':{'original_text':'The passage.'}}
            data={'witness':{},'sources':[{'file':'book.txt'}],'images':[{'file':'p144.jpg'}],'places':{'p':place,'q':{'text':'Other.'}},'review':{'reader':'second','places':{'p':{'reads':'differs'}}}}
            file.write_text(json.dumps(data));args=argparse.Namespace(packet=str(path),language='ru',loc='p',reason='This edition omits the antecedent necessary to translate the passage accurately.')
            kit.cmd_witness_place_withdraw(args);result=json.loads(file.read_text());record=result['withdrawn_places'][0]
            self.assertEqual(record['place'],place);self.assertEqual(record['review'],data['review']);self.assertEqual(result['review'],{})
            self.assertNotIn('p',result['places']);self.assertIn('q',result['places']);self.assertFalse(result.get('missing_places'))
            self.assertEqual(result['sources'],data['sources']);self.assertEqual(result['images'],data['images'])
            with self.assertRaises(SystemExit):kit.cmd_witness_place_withdraw(args)
            file.write_text(json.dumps(data));Path(str(file)+'.accepted').write_text('{}')
            with self.assertRaises(SystemExit):kit.cmd_witness_place_withdraw(args)


class SourcePersonRemovalTest(unittest.TestCase):
    def test_removal_preserves_evidence_and_resets_review(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/de.json'
            person={'role':'author','name':{'en':'Pope, Alexander'},'source':'https://d-nb.info/example','evidence':{'kind':'reference','sha256':'a'*64}}
            other={'role':'author','name':{'en':'Alexander Pope'},'qid':'Q164047'}
            data={'witness':{'persons':[person,other]},'review':{'reader':'second'}};file.write_text(json.dumps(data))
            args=argparse.Namespace(packet=str(path),language='de',role='author',name='Pope, Alexander',reason='This inverted catalogue identity is replaced by the sourced Wikidata identity.')
            duplicate={**data,'witness':{'persons':[person,person,other]}};file.write_text(json.dumps(duplicate))
            with self.assertRaises(SystemExit):kit.cmd_witness_person_remove_source(args)
            self.assertEqual(json.loads(file.read_text()),duplicate);file.write_text(json.dumps(data))
            kit.cmd_witness_person_remove_source(args);result=json.loads(file.read_text())
            self.assertEqual(result['witness']['persons'],[other]);self.assertEqual(result['person_corrections'][0]['removed'],person);self.assertEqual(result['review'],{})
            with self.assertRaises(SystemExit):kit.cmd_witness_person_remove_source(args)
            file.write_text(json.dumps(data));Path(str(file)+'.accepted').write_text('{}')
            with self.assertRaises(SystemExit):kit.cmd_witness_person_remove_source(args)


class WitnessSourceRemovalTest(unittest.TestCase):
    def test_unused_source_has_history_and_referenced_source_cannot_be_removed(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';(path/'witnesses').mkdir(parents=True);file=path/'witnesses/ru.json'
            data={'witness':{},'sources':[{'file':'old.txt','url':'https://example.org/text','sha256':'a'*64}],'places':{'p':{'source':'old.txt'}},'review':{'reader':'second'}}
            file.write_text(json.dumps(data));args=argparse.Namespace(packet=str(path),language='ru',name='old.txt',reason='The page transcription has replaced this unused source.')
            with self.assertRaises(SystemExit):kit.cmd_witness_source_remove(args)
            self.assertEqual(json.loads(file.read_text()),data)
            data['places']['p'].pop('source');file.write_text(json.dumps(data));kit.cmd_witness_source_remove(args)
            result=json.loads(file.read_text());self.assertEqual(result['sources'],[]);self.assertEqual(result['source_corrections'][0]['removed'],data['sources'][0]);self.assertEqual(result['review'],{})
            data['places']['p']['source']='old.txt';file.write_text(json.dumps(data));Path(str(file)+'.accepted').write_text('{}')
            with self.assertRaises(SystemExit):kit.cmd_witness_source_remove(args)


class ReopenedFindingRouteTest(unittest.TestCase):
    def test_finding_requires_maker_and_independent_current_review(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp);finding={'reader':'second','finding':'The thesis omits the essential words only when from the printed passage.'}
            def put(name,value):(path/name).write_text(json.dumps(value))
            put('reopen-history.json',[finding]);put('reading.json',{'reader':{'id':'maker'},'conceptions':[]});put('places.json',{'places':{}})
            self.assertTrue(kit.reopen_problems(tmp))
            response=argparse.Namespace(packet=tmp,index=1,maker='maker',assessment='The source says only when; the current thesis now preserves this condition in both languages.')
            kit.cmd_reopen_response(response);self.assertEqual(kit.reopen_problems(tmp),[]);self.assertTrue(kit.reopen_problems(tmp,second=True))
            review={'reader':{'id':'second','model':'gpt-6.1-terra','date':'2026-11-27'},'accept':False};put('review.json',review)
            answer=argparse.Namespace(packet=tmp,index=1,reader='second',resolution='unresolved',assessment='The page still says only when while the revised thesis omits that condition entirely.')
            kit.cmd_reopen_review(answer);self.assertEqual(kit.reopen_problems(tmp,second=True),[])
            self.assertTrue(kit.reopen_unresolved(tmp));self.assertIn('unresolved',kit.reopen_problems(tmp)[0])
            with patch.object(kit,'gate',return_value=([],'open')),self.assertRaises(SystemExit):kit.cmd_accept(argparse.Namespace(packet=tmp,offline=True,by='line',basis=None))
            self.assertFalse((path/'accepted.json').exists())
            with patch.object(kit,'by_eye',return_value=[]):
                self.assertEqual(kit.second_gate(tmp,{'reader':{'id':'maker'},'conceptions':[],'links':[],'left':[]},{'places':{}},{},locator_only=True),[])
            kit.cmd_reopen_response(response);self.assertFalse(kit.reopen_unresolved(tmp))
            previous=json.loads((path/'reopen-resolutions.json').read_text())[kit.page_number_digest(finding)]['prior'][0]
            self.assertFalse(previous['review']['resolved']);self.assertIn('omits',previous['review']['assessment'])
            review['accept']=True;put('review.json',review);answer.resolution='resolved'
            answer.assessment='The page says only when and both versions of the revised thesis preserve this condition.'
            kit.cmd_reopen_review(answer);self.assertEqual(kit.reopen_problems(tmp,second=True),[])
            reading=json.loads((path/'reading.json').read_text());reading['reader'].update(model='gpt-6.1-terra',effort='high');put('reading.json',reading)
            review['reader'].update(model='gpt-6.1-terra',effort='high');put('review.json',review);self.assertEqual(kit.reopen_problems(tmp,second=True),[])
            resolutions=json.loads((path/'reopen-resolutions.json').read_text());put('accepted.json',{'reopen_sha256':kit.page_number_digest({'history':[finding],'resolutions':resolutions})})
            self.assertEqual(kit.reopen_problems(tmp,second=True),[])
            review['accept']=False;put('review.json',review);self.assertTrue(kit.reopen_problems(tmp,second=True))
            put('reading.json',{'reader':{'id':'maker'},'conceptions':[{'id':'changed'}]});self.assertTrue(kit.reopen_problems(tmp))
            with self.assertRaises(SystemExit):kit.cmd_reopen_response(response)

class PageDifferenceAcceptanceTest(unittest.TestCase):
    def test_a_negative_page_reading_cannot_be_accepted_directly(self):
        import argparse,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp)/'review.json').write_text(json.dumps({'images':[{'loc':'p','reads':'differs','note':'The printed page says different words.'}]}))
            with patch.object(kit,'gate',return_value=([],'open')),self.assertRaises(SystemExit):
                kit.cmd_accept(argparse.Namespace(packet=tmp,offline=True,by='line',basis=None))
            self.assertFalse((Path(tmp)/'accepted.json').exists())

class ReopenedFindingDriverTest(unittest.TestCase):
    def test_settled_reopened_reading_gets_maker_and_fresh_independent_review(self):
        import json,tempfile
        from pathlib import Path
        import drive
        for final_accept in (True,False):
            with self.subTest(final_accept=final_accept),tempfile.TemporaryDirectory() as tmp:
                packet=Path(tmp)/'work';packet.mkdir();events=[];calls=[]
                (packet/'reopen-history.json').write_text(json.dumps([{'finding':'The source condition must be preserved.'}]))
                (packet/'reading.json').write_text(json.dumps({'reader':{'id':'maker'},'conceptions':[{'id':'kept'},{'id':'disputed'}],'links':[]}))
                def kit_call(*args):
                    calls.append(args[0])
                    if args[0]=='settle':
                        data=json.loads((packet/'reading.json').read_text());data['conceptions'].pop();(packet/'reading.json').write_text(json.dumps(data))
                    return 0,'gate: open'
                def reader(work,role,first,again,turns,**kwargs):
                    events.append((role,first))
                    if role=='second':
                        count=sum(event[0]=='second' for event in events)
                        (packet/'review.json').write_text(json.dumps({'reader':{'id':'second'},'accept':final_accept if count==2 else False,'images':[]}))
                    return role+'-thread','open'
                with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'state',return_value={}),patch.object(drive,'kit',side_effect=kit_call),patch.object(drive,'until_open',side_effect=reader),patch.object(drive,'have',side_effect=lambda w,n:n in ('places.json','reading.json')),patch.object(drive,'RETURNS',0):
                    result=drive.run('work')
                self.assertEqual([event[0] for event in events],['second','maker','second'])
                self.assertIn('reopen-response',events[1][1]);self.assertIn('reopen-review',events[2][1])
                self.assertEqual(result,'accepted-in-part' if final_accept else 'reopen-disputed')
                self.assertEqual('accept' in calls,final_accept)
                self.assertTrue((Path(tmp)/'out/work/review-settled-1.json').exists())

    def test_negative_second_reading_returns_finding_to_maker(self):
        import json,tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp:
            packet=Path(tmp)/'work';packet.mkdir();seen=[];calls=[]
            (packet/'reopen-history.json').write_text(json.dumps([{'finding':'The source condition is still missing.'}]))
            def kit_call(*args):
                if args[0]=='gate':
                    calls.append(args);return (0,'gate: open') if len(calls)==1 else (1,'- reopen 1: second reader found the issue unresolved; maker must respond again: the condition is missing')
                return 0,''
            def reader(work,role,first,again,turns,**kwargs):
                seen.append((role,first))
                if role=='second':(packet/'review.json').write_text(json.dumps({'accept':False}));return None,'open'
                return None,'stuck'
            with patch.object(drive,'PACKETS',tmp),patch.object(drive,'OUT',str(Path(tmp)/'out')),patch.object(drive,'state',return_value={}),patch.object(drive,'kit',side_effect=kit_call),patch.object(drive,'until_open',side_effect=reader),patch.object(drive,'have',side_effect=lambda w,n:n in ('places.json','reading.json')):
                self.assertEqual(drive.run('work'),'maker-stuck')
            self.assertEqual([role for role,_ in seen],['second','maker'])
            self.assertIn('the condition is missing',seen[1][1])

class TranslationSourceBlockTest(unittest.TestCase):
    def test_original_ocr_does_not_resolve_a_translation_search(self):
        import json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir()
            (path/'places.json').write_text(json.dumps({'entry':{'original':'grc'}}))
            (path/'evidence.json').write_text(json.dumps({'scan':{'item':'book','ocr':True}}))
            block={'ground':'sources','reason':'The translation passage and its corresponding page remain unlocated.','tried':[{'url':'https://example.org/catalogue','outcome':'The catalogue does not show the requested translation page.'}]}
            file=path/'blocked.json';file.write_text(json.dumps(block))
            self.assertEqual(kit.gate(str(path))[1],'shut')
            block.update(source_scope='translation',language='ru');file.write_text(json.dumps(block))
            self.assertEqual(kit.gate(str(path)),([],'blocked'))
            block['tried']=[];file.write_text(json.dumps(block));self.assertEqual(kit.gate(str(path))[1],'shut')
            block['tried']=[{'url':'https://example.org','outcome':'The source was searched for the relevant translation.'}];block['language']='grc';file.write_text(json.dumps(block))
            self.assertEqual(kit.gate(str(path))[1],'shut')

class ReaderOutputReservationTest(unittest.TestCase):
    def test_existing_and_concurrent_outputs_are_never_overwritten(self):
        import concurrent.futures, tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp, patch.object(drive,'OUT',tmp):
            home=Path(tmp)/'work';home.mkdir()
            (home/'01-maker.jsonl').write_text('previous event log')
            (home/'02-maker.md').write_text('previous final answer')
            def reserve(_):
                n,log,last,handle=drive.reserve_reader_outputs('work','maker',1)
                with handle:handle.write(str(n))
                return n,log,last
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:allocated=list(pool.map(reserve,range(4)))
            self.assertEqual(len({n for n,_,_ in allocated}),4)
            self.assertEqual((home/'01-maker.jsonl').read_text(),'previous event log')
            self.assertEqual((home/'02-maker.md').read_text(),'previous final answer')
            self.assertTrue(all(Path(log).read_text()==str(n) and Path(last).exists() for n,log,last in allocated))
            def finish(n):drive.state('work',_session={'n':n,'tokens':{'input_tokens':1}})
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:list(pool.map(finish,range(20)))
            state=drive.state('work')
            self.assertEqual(len(state['sessions']),20)
            self.assertEqual(state['tokens']['input_tokens'],20)

class ReviewHistoryTest(unittest.TestCase):
    def test_retried_review_never_overwrites_an_earlier_run(self):
        import tempfile
        from pathlib import Path
        import drive
        with tempfile.TemporaryDirectory() as tmp, patch.object(drive,'OUT',tmp):
            review=Path(tmp)/'review.json';review.write_bytes(b'first reading')
            first=Path(drive.preserve_review('work',str(review),'review-0'))
            review.write_bytes(b'later reading');second=Path(drive.preserve_review('work',str(review),'review-0'))
            self.assertNotEqual(first,second);self.assertEqual(first.read_bytes(),b'first reading');self.assertEqual(second.read_bytes(),b'later reading');self.assertFalse(review.exists())


class DerivedPdfTest(unittest.TestCase):
    def test_pdf_transcription_keeps_its_fetched_predecessor(self):
        import argparse, hashlib, json, shutil, tempfile
        from pathlib import Path
        if not shutil.which('pdftotext'):self.skipTest('pdftotext is not installed')
        stream=b'BT /F1 12 Tf 72 720 Td (A complete sentence.) Tj ET'
        objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',b'<< /Length '+str(len(stream)).encode()+b' >>\nstream\n'+stream+b'\nendstream']
        pdf=bytearray(b'%PDF-1.4\n');offsets=[]
        for index,obj in enumerate(objects,1):
            offsets.append(len(pdf));pdf.extend(f'{index} 0 obj\n'.encode()+obj+b'\nendobj\n')
        xref=len(pdf);pdf.extend(b'xref\n0 6\n0000000000 65535 f \n')
        for offset in offsets:pdf.extend(f'{offset:010d} 00000 n \n'.encode())
        pdf.extend(f'trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();source={'file':'book.pdf','url':'https://example.org/book.pdf','role':'other','sha256':hashlib.sha256(pdf).hexdigest()}
            (path/'evidence.json').write_text(json.dumps({'sources':[source]}))
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')):
                Path(kit.cache(str(path),'book.pdf')).write_bytes(pdf)
                kit.cmd_pdf_text(argparse.Namespace(packet=str(path),source='book.pdf',language=None))
                sources=json.loads((path/'evidence.json').read_text())['sources'];derived=sources[-1]
                self.assertIn('A complete sentence.',Path(kit.cache(str(path),derived['file'])).read_text())
                self.assertEqual(derived['derived_from']['sha256'],source['sha256'])
                self.assertEqual(kit.derived_source_problems(sources,lambda n:kit.cache(str(path),n)),[])
                # Identical PDF bytes from another registered source keep separate lineages.
                ev=json.loads((path/'evidence.json').read_text());other=dict(source,file='copy.pdf',url='https://example.org/copy.pdf')
                ev['sources'].append(other);(path/'evidence.json').write_text(json.dumps(ev))
                Path(kit.cache(str(path),'copy.pdf')).write_bytes(pdf)
                kit.cmd_pdf_text(argparse.Namespace(packet=str(path),source='copy.pdf',language=None))
                sources=json.loads((path/'evidence.json').read_text())['sources']
                self.assertEqual(len([item for item in sources if item.get('derived_from')]),2)
                self.assertEqual(kit.derived_source_problems(sources,lambda n:kit.cache(str(path),n)),[])
                if shutil.which('pdftoppm'):
                    (path/'places.json').write_text(json.dumps({'places':{'p1':{}}}))
                    ev=json.loads((path/'evidence.json').read_text());ev['scan']={'item':'old'};(path/'evidence.json').write_text(json.dumps(ev))
                    with patch.object(kit,'get',return_value=bytes(pdf)):
                        for by in ('maker','second'):
                            kit.cmd_pdf_page(argparse.Namespace(packet=str(path),source='book.pdf',page=1,language=None,loc='p1',by=by))
                    ev=json.loads((path/'evidence.json').read_text());images=ev['images']
                    self.assertEqual(ev['scan']['item'],'old')
                    self.assertEqual(images[0]['sha256'],images[1]['sha256'])
                    self.assertEqual(images[0]['derived_from']['sha256'],source['sha256'])
                    read=[{'loc':'p1','n':1,'reads':'as the packet'}]
                    self.assertEqual(kit.pdf_page_review_problems(ev,{'p1':{}},read),[])
                    second=next(item for item in ev['images'] if item['by']=='second');self.assertEqual(second['independent_fetch']['sha256'],source['sha256'])
                    changed=dict(second);changed.pop('independent_fetch');self.assertTrue(kit.derived_image_problems(ev['sources'],[changed],lambda n:kit.cache(str(path),n),offline=True))
                    kit.OWES.clear();self.assertEqual(kit.derived_image_problems(ev['sources'],[changed],lambda n:kit.cache(str(path),n),offline=True,legacy=True),[]);self.assertTrue(any('independent fetch' in issue for issue in kit.OWES));kit.OWES.clear()
                    with patch.object(kit,'get',return_value=b'%PDF-different'),self.assertRaises(SystemExit):kit.cmd_pdf_page(argparse.Namespace(packet=str(path),source='book.pdf',page=1,language=None,loc='p1',by='second'))
                    different=[{'loc':'p1','n':1,'reads':'differs','note':'The printed page reads different words.'}]
                    self.assertEqual(kit.pdf_page_review_problems(ev,{'p1':{}},different),[])
                    review={'reader':{'id':'second','model':'gpt-6.1-terra','date':'2026-11-27'},'images':different,'accept':False}
                    (path/'review.json').write_text(json.dumps(review))
                    with patch.object(kit,'by_eye',return_value=[]):
                        self.assertEqual(kit.second_gate(str(path),{'reader':{'id':'maker'},'conceptions':[],'links':[],'left':[]},ev,{'p1':{}}),[])
                    self.assertTrue(kit.pdf_page_review_problems(dict(ev,images=images[:1]),{'p1':{}},read))
                    self.assertTrue(kit.pdf_page_review_problems(ev,{'p1':{}},[]))
                    self.assertEqual(kit.derived_image_problems(sources,images,lambda n:kit.cache(str(path),n)),[])
                    altered=[dict(images[0],derived_from=dict(images[0]['derived_from'],page=0))]
                    self.assertTrue(kit.derived_image_problems(sources,altered,lambda n:kit.cache(str(path),n),offline=True))
                Path(kit.cache(str(path),'book.pdf')).write_bytes(b'%PDF-changed')
                self.assertTrue(kit.derived_source_problems(sources,lambda n:kit.cache(str(path),n)))
                if shutil.which('pdftoppm'):self.assertTrue(kit.derived_image_problems(sources,images,lambda n:kit.cache(str(path),n)))
                # A translation stores its independent source and page evidence in its witness file.
                witness_path=Path(kit.witness_file(str(path),'en'));witness_path.parent.mkdir(exist_ok=True)
                witness_path.write_text(json.dumps({'language':'en','sources':[source],'images':[],'review':{'reader':'old'}}))
                Path(kit.witness_cache(str(path),'en','book.pdf')).write_bytes(pdf)
                kit.cmd_pdf_text(argparse.Namespace(packet=str(path),source='book.pdf',language='en'))
                with patch.object(kit,'world',return_value={'fragments':{'places':{'work':{}}}}), self.assertRaises(SystemExit):
                    kit.cmd_witness_cut(argparse.Namespace(packet=str(path),language='en',source='book.pdf',loc='p1'))
                if shutil.which('pdftoppm'):
                    kit.cmd_pdf_page(argparse.Namespace(packet=str(path),source='book.pdf',page=1,language='en',loc=None,by='maker'))
                    data=json.loads(witness_path.read_text());self.assertFalse(data['review'])
                    self.assertEqual(kit.derived_image_problems(data['sources'],data['images'],lambda n:kit.witness_cache(str(path),'en',n)),[])
                    (Path(str(witness_path)+'.accepted')).write_text('{}')
                    with self.assertRaises(SystemExit):kit.cmd_pdf_page(argparse.Namespace(packet=str(path),source='book.pdf',page=1,language='en',loc=None,by='maker'))

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
            result['witness']['persons'].insert(0,'Unsupported name');result['review']={'reader':'second'};file.write_text(json.dumps(result))
            legacy=argparse.Namespace(packet=tmp,language='ru',name='Unsupported name',reason='An unsupported duplicate of the independently sourced participant.')
            kit.cmd_witness_person_remove_legacy(legacy)
            result=json.loads(file.read_text());self.assertEqual(result['witness']['persons'],[right])
            self.assertEqual(result['person_corrections'][-1]['removed'],'Unsupported name');self.assertEqual(result['review'],{})
            (path/'witnesses/ru.json.accepted').write_text('{}')
            with self.assertRaises(SystemExit):kit.cmd_witness_person_remove_legacy(legacy)
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


class AncientBoundTest(unittest.TestCase):
    def claim(self, time, precision, circa=False, latest=None):
        qualifiers = {}
        if circa:
            qualifiers['P1480'] = [{'datavalue': {'value': {'id': 'Q5727902'}}}]
        if latest:
            qualifiers['P1326'] = [{'datavalue': {'value': {'time': latest, 'precision': 9}}}]
        return {'rank': 'normal', 'mainsnak': {'datavalue': {'value': {'time': time, 'precision': precision}}}, 'qualifiers': qualifiers}

    def test_circa_death_gives_a_bound_and_no_year(self):
        entity = {'claims': {'P570': [self.claim('-0015-00-00T00:00:00Z', 9, circa=True)]}}
        self.assertIsNone(kit.entity_year(entity, 'P570'))
        bound = kit.death_bound(entity)
        self.assertEqual(bound['year'], -15 + kit.LONGEST_LIFE + 2);self.assertEqual(bound['semantics'],'exclusive')
        person = {'died': None, 'died_before': bound['year'], 'death_bound': bound}
        dates = kit.public_domain_by_dates([person, {'died': 1916}], 1899, year=2026)
        self.assertEqual(dates, {'russia': True, 'usa': True, 'eu': True})

    def test_floruit_and_latest_date_bound_the_death(self):
        floruit = {'claims': {'P1317': [self.claim('+1000-00-00T00:00:00Z', 7)]}}
        self.assertEqual(kit.death_bound(floruit)['year'], 1100 + kit.LONGEST_LIFE + 2)   # include the partial calendar year and the exclusive bound
        active = {'claims': {'P1317': [self.claim('+1750-00-00T00:00:00Z', 9)]}}
        self.assertIsNone(kit.death_bound(active))                                     # an activity year is no death year
        earliest = {'claims': {'P570': [self.claim('+1700-00-00T00:00:00Z', 9)]}}
        earliest['claims']['P570'][0]['qualifiers'] = {'P1319': [{'datavalue': {'value': {'time': '+1700-00-00T00:00:00Z', 'precision': 9}}}]}
        self.assertIsNone(kit.death_bound(earliest))                                   # «not before 1700» bounds nothing above
        earliest['claims']['P570'][0]['qualifiers']['P1326'] = [{'datavalue': {'value': {'time': '+1720-00-00T00:00:00Z', 'precision': 9}}}]
        self.assertEqual(kit.death_bound(earliest)['year'], 1721)
        presumed = {'claims': {'P570': [self.claim('+1050-00-00T00:00:00Z', 9)]}}
        presumed['claims']['P570'][0]['qualifiers'] = {'P1480': [{'datavalue': {'value': {'id': 'Q18122778'}}}]}
        self.assertIsNone(kit.death_bound(presumed))                                   # a presumed date leaves the bound to a reference
        coarse = {'claims': {'P570': [self.claim('+1700-00-00T00:00:00Z', 7)]}}
        coarse['claims']['P570'][0]['qualifiers'] = {'P1326': [{'datavalue': {'value': {'time': '+1700-00-00T00:00:00Z', 'precision': 7}}}]}
        self.assertIsNone(kit.death_bound(coarse))                                     # an inclusive latest year 1800 is not before 1800
        uncertain = {'claims': {'P570': [self.claim('+1700-00-00T00:00:00Z', 9)]}}
        uncertain['claims']['P570'][0]['mainsnak']['datavalue']['value']['after'] = 300
        self.assertIsNone(kit.death_bound(uncertain))                                  # 1700 or up to three centuries later
        uncertain['claims']['P570'][0]['mainsnak']['datavalue']['value']['after'] = 20
        self.assertEqual(kit.death_bound(uncertain)['year'], 1721)
        self.assertEqual(kit.death_bound(uncertain)['claims'][0]['after'], 20)
        julian = {'claims': {'P570': [self.claim('+1700-12-25T00:00:00Z', 11)]}}
        julian['claims']['P570'][0]['mainsnak']['datavalue']['value']['calendarmodel'] = 'http://www.wikidata.org/entity/Q1985786'
        self.assertEqual(kit.death_bound(julian)['year'], 1702)                        # 25 December 1700 (Julian) is 5 January 1701
        julian['claims']['P570'][0]['mainsnak']['datavalue']['value'].update({'time': '+1700-00-00T00:00:00Z', 'precision': 9})
        self.assertEqual(kit.death_bound(julian)['year'], 1702)                        # the Julian year 1700 reaches into Gregorian 1701
        self.assertEqual(kit.death_bound(julian)['claims'][0]['calendar'],'http://www.wikidata.org/entity/Q1985786')
        latest = {'claims': {'P570': [self.claim('+1050-00-00T00:00:00Z', 7, latest='+1070-00-00T00:00:00Z')]}}
        self.assertEqual(kit.death_bound(latest)['year'], 1071)

    def test_no_bound_for_modern_or_unclaimed_hands(self):
        self.assertIsNone(kit.death_bound({'claims': {}}))
        self.assertIsNone(kit.death_bound({'claims': {'P570': [self.claim('+1900-00-00T00:00:00Z', 7)]}}))
        self.assertIsNone(kit.death_year({'died': None, 'died_before': 35}))
        self.assertIsNone(kit.death_year({'died': None, 'died_before': 1850, 'death_bound': {'from': 'wikidata'}}))
        self.assertFalse(kit.public_domain_by_dates([{'died': None}], 1899, year=2026)['eu'])


class ExclusiveWikidataBoundTest(unittest.TestCase):
    def test_an_unsupported_calendar_does_not_establish_a_death_bound(self):
        value={'time':'+00000001350-01-01T00:00:00Z','precision':9,'calendarmodel':'http://www.wikidata.org/entity/Q123'}
        entity={'claims':{'P570':[{'mainsnak':{'datavalue':{'value':value}}}]}}
        self.assertIsNone(kit.death_bound(entity))

    def test_original_and_witness_require_exclusive_bound_and_renewed_review(self):
        import argparse,hashlib,json,tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'work';path.mkdir();(path/'witnesses').mkdir();folder=Path(tmp)/'cache/work/witnesses/en';folder.mkdir(parents=True)
            qid='Q372772';value={'time':'+1350-00-00T00:00:00Z','precision':9,'calendarmodel':'http://www.wikidata.org/entity/Q1985786'}
            entity={'entities':{qid:{'labels':{'en':{'value':'Kenkō'}},'claims':{'P570':[{'mainsnak':{'datavalue':{'value':value}}}]}}}};raw=json.dumps(entity).encode()
            (path/'evidence.json').write_text(json.dumps({'persons':[]}));(path/'reading.json').write_text(json.dumps({'reader':{'id':'maker'}}));(path/'review.json').write_text(json.dumps({'reader':{'id':'second'}}))
            args=argparse.Namespace(packet=str(path),role='author',qid=qid,name=None,ru=None,died=None,died_before=None,bound_note=None,source=None,remove=False,wartime=None,rehabilitated=None,bounded=True)
            with patch.object(kit,'CACHE',str(Path(tmp)/'cache')),patch.object(kit,'get',return_value=raw),patch.object(kit,'write_rights'):
                kit.cmd_person(args);person=kit.evidence(str(path))['persons'][0]
                self.assertEqual(person['died_before'],1352);self.assertEqual(person['death_bound']['claims'][0]['calendar'],value['calendarmodel'])
                self.assertEqual(kit.original_bound_problems(str(path),person),[]);self.assertTrue(kit.original_bound_problems(str(path),person,second=True))
                kit.cmd_person_bound_review(argparse.Namespace(packet=str(path),role='author',qid=qid,reader='second',assessment='The Julian year can reach Gregorian 1351, so the exclusive bound is before 1352.'))
                self.assertEqual(kit.original_bound_problems(str(path),person,second=True),[])
                legacy=json.loads(json.dumps(person));legacy['died_before']=1351;legacy['death_bound']['year']=1351;legacy['death_bound'].pop('semantics')
                self.assertTrue(kit.original_bound_problems(str(path),legacy,offline=True))
                witness=path/'witnesses/en.json';witness.write_text(json.dumps({'witness':{'maker':'maker','persons':[]},'review':{}}))
                kit.cmd_witness_person(argparse.Namespace(packet=str(path),language='en',role='author',qid=qid,bounded=True))
                data=json.loads(witness.read_text());hand=data['witness']['persons'][0];self.assertEqual(hand['died_before'],1352)
                self.assertEqual(kit.witness_linked_bound_problems(str(path),'en',hand),[])
                data['review']={'reader':'second','places':{'p':{'reads':'as the packet'}}};witness.write_text(json.dumps(data))
                kit.cmd_witness_bound_review(argparse.Namespace(packet=str(path),language='en',role='author',name='Kenkō',reader='second',assessment='The Julian year can reach Gregorian 1351, so the exclusive bound is before 1352.'))
                self.assertIn(kit.page_number_digest(hand),json.loads(witness.read_text())['review']['bounds'])
                legacy=json.loads(json.dumps(hand));legacy['died_before']=1351;legacy['death_bound']['year']=1351;legacy['death_bound'].pop('semantics')
                self.assertTrue(kit.witness_linked_bound_problems(str(path),'en',legacy,offline=True))
                (path/'accepted.json').write_text('{}')
                with self.assertRaises(SystemExit):kit.cmd_person(args)

class SourceBoundaryTest(unittest.TestCase):
    def test_spanish_opening_punctuation_preserves_the_source_boundary(self):
        for text in ('¿Cómo se explica la belleza?','¡Qué hermoso es!'):
            self.assertEqual(kit.sentences(text),[])
            self.assertIsNone(kit.in_transcription(text,'Una frase anterior. '+text+' Otra frase.'))
            self.assertIsNotNone(kit.in_transcription(text,'Esto sigue sin terminar '+text+' Otra frase.'))
        self.assertTrue(kit.sentences('¿cómo se explica?'))

    def test_latin_sentence_may_open_in_lower_case(self):
        self.assertEqual(kit.sentences('illud autem nobile est.', language='la'), [])
        self.assertTrue(kit.sentences('illud autem nobile est.'))
        self.assertTrue(kit.sentences('lowercase english sentence.', language='en'))

    def test_note_marker_after_a_full_stop_closes_the_sentence(self):
        source = 'Das ist der erste Satz. 220) Der nächste Satz folgt hier.'
        self.assertIsNone(kit.in_transcription('Der nächste Satz folgt hier.', source))
        numbered = 'Vom vorigen Abschnitt. 2) Kunst wird von der Wissenschaft unterschieden.'
        self.assertIsNone(kit.in_transcription('Kunst wird von der Wissenschaft unterschieden.', numbered))
        inside = 'Im Abschnitt 3) folgt die Kunst, die man unterscheidet.'
        self.assertIsNotNone(kit.in_transcription('folgt die Kunst, die man unterscheidet.', inside))

    def test_declared_encoding_is_read_and_bytes_are_kept(self):
        arabic = 'كتاب الشعر'
        raw = ('<html><head><meta http-equiv="Content-Type" content="text/html; charset=windows-1256"></head><body><p>' + arabic + '</p></body></html>').encode('cp1256')
        self.assertIn(arabic, kit.plain(raw, 'page.html'))
        self.assertEqual(kit.declared_charset(raw), 'windows-1256')
        self.assertIn('Kitab', kit.plain('<html><body><p>Kitab</p></body></html>'.encode(), 'page.html'))


class PdfAndDriverTest(unittest.TestCase):
    def test_pdf_licence_is_read_from_its_text_layer(self):
        import shutil, subprocess as sp, tempfile, os
        if not shutil.which('pdftotext'):
            self.skipTest('pdftotext is absent')
        pdf = (b'%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n'
               b'3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 144]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj\n'
               b'4 0 obj<</Length 66>>stream\nBT /F1 12 Tf 20 100 Td (Licensed under CC BY-NC 4.0. A whole sentence.) Tj ET\nendstream endobj\n'
               b'5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n')
        self.assertTrue(kit.source_names_licence(pdf, 'https://creativecommons.org/licenses/by-nc/4.0/'))
        self.assertFalse(kit.source_names_licence(pdf, 'https://creativecommons.org/licenses/by/4.0/'))

    def test_open_gate_with_owes_goes_to_the_maker_first(self):
        import drive
        calls = []
        def fake_kit(*args):
            calls.append(args)
            if args[0] == 'gate':
                return 0, 'gate: open, owes 2\n- owes rights.mexico\n- owes ru search'
            return 0, ''
        with patch.object(drive, 'kit', side_effect=fake_kit), patch.object(drive, 'have', side_effect=lambda work, name: name in ('places.json', 'reading.json')), \
             patch.object(drive, 'state', return_value={}), patch.object(drive, 'until_open', return_value=(None, 'maker-stuck')) as until:
            result = drive.run('work-x')
        self.assertEqual(result, 'maker-stuck')
        first = until.call_args_list[0]
        self.assertEqual(first.args[1], 'maker')
        self.assertIn('owes rights.mexico', first.args[2])

    def test_a_maker_does_not_hand_on_a_packet_that_owes(self):
        import drive
        gates = iter([(1, 'gate: shut\n- place missing'), (0, 'gate: open, owes 1\n- owes ru search'), (0, 'gate: open')])
        calls = []
        def fake_kit(*args):
            calls.append(args)
            return next(gates) if args[0] == 'gate' else (0, '')
        with patch.object(drive, 'kit', side_effect=fake_kit), patch.object(drive, 'codex', return_value=('t', None)), \
             patch.object(drive, 'have', return_value=False):
            thread, got = drive.until_open('work-x', 'maker', 'first', '{gate}', 5)
        self.assertEqual(got, 'open')
        self.assertEqual(sum(1 for c in calls if c[0] == 'gate'), 3)
        self.assertEqual(sum(1 for c in calls if c[0] == 'stamp'), 1)
