"""Preserve identity, evidence addresses and admission boundaries on import."""
from __future__ import annotations

import copy
import gzip
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from xml.sax.saxutils import escape
from zipfile import ZipFile, ZipInfo

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))

import source_registry_common as common  # noqa: E402
from source_registry_ooxml import workbook  # noqa: E402
from source_registry_values import (  # noqa: E402
    extract_reported_links, load_profile, normalize_record_values,
)

S = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
P = "http://schemas.openxmlformats.org/package/2006/relationships"
PROFILE = REPO_ROOT / "mechanics/source-witnessing/parts/witness-route/config/registry-normalization.v1.json"


def zipped(path, parts):
    """Use fixed ZIP metadata so fixture changes reflect changed source bytes."""
    with ZipFile(path, "w") as archive:
        for name, value in parts.items():
            archive.writestr(ZipInfo(name), value.encode("utf-8"))


def inline(address, value):
    return (f'<c r="{address}" t="inlineStr"><is><t xml:space="preserve">'
            f'{escape(value)}</t></is></c>')


def sheet_xml(headers, rows, *, fidelity=False):
    header = '<row r="2">' + ''.join(
        inline(f'{common.column_name(i)}2', value)
        for i, value in enumerate(headers, 1)) + '</row>'
    body = []
    for index, values in enumerate(rows, 3):
        cells = ''.join(inline(f'{common.column_name(i)}{index}', value)
                        for i, value in enumerate(values, 1) if value is not None)
        if fidelity and index == 3:
            cells = (inline('A3', values[0]) + '<c r="B3" t="s"><v>0</v></c>'
                     '<c r="C3" s="1"><v>46268</v></c>'
                     '<c r="D3"><f>1+1</f><v>2</v></c>'
                     '<c r="E3" t="inlineStr"><is><t/></is></c>')
        body.append(f'<row r="{index}" hidden="{int(fidelity and index == 3)}">{cells}</row>')
    extra = ('<hyperlinks><hyperlink ref="B3" r:id="link1" display="Source"/></hyperlinks>'
             '<mergeCells count="1"><mergeCell ref="F5:G5"/></mergeCells>') if fidelity else ''
    columns = '<cols><col min="5" max="5" hidden="1" width="15"/></cols>' if fidelity else ''
    return (f'<worksheet xmlns="{S}" xmlns:r="{R}">{columns}<sheetData>'
            '<row r="1">' + inline('A1', 'Third corpus explanatory preamble') + '</row>'
            + header + ''.join(body) + '</sheetData>' + extra + '</worksheet>')


def make_workbook(path, rows, *, gaps=(), fidelity=False, unknown=False):
    headers = ['Entry code', 'Caption', 'Reviewed', 'Calculation', 'Empty note', 'Sparse note']
    if unknown:
        headers[1] = 'An unmapped future heading'
    parts = {
        'xl/workbook.xml': (
            f'<workbook xmlns="{S}" xmlns:r="{R}"><workbookPr date1904="0"/>'
            '<sheets><sheet name="Entries" sheetId="1" r:id="r1"/>'
            '<sheet name="Missing" sheetId="2" state="hidden" r:id="r2"/></sheets></workbook>'),
        'xl/_rels/workbook.xml.rels': (
            f'<Relationships xmlns="{P}"><Relationship Id="r1" Target="worksheets/items.xml"/>'
            '<Relationship Id="r2" Target="worksheets/missing.xml"/></Relationships>'),
        'xl/styles.xml': (
            f'<styleSheet xmlns="{S}"><cellXfs count="2"><xf numFmtId="0"/>'
            '<xf numFmtId="14"/></cellXfs></styleSheet>'),
        'xl/sharedStrings.xml': (
            f'<sst xmlns="{S}"><si><r><t xml:space="preserve">Recorded </t></r>'
            '<r><t>name</t></r></si></sst>'),
        'xl/worksheets/items.xml': sheet_xml(headers, rows, fidelity=fidelity),
        'xl/worksheets/missing.xml': sheet_xml(['Subject', 'Problem'], gaps),
    }
    if fidelity:
        parts['xl/worksheets/_rels/items.xml.rels'] = (
            f'<Relationships xmlns="{P}"><Relationship Id="link1" '
            'Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" '
            'Target="https://example.org/item?x=1&amp;y=2" TargetMode="External"/></Relationships>')
    zipped(path, parts)


def make_report(path):
    zipped(path, {
        'word/document.xml': (
            f'<w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>'
            '<w:p><w:r><w:t>R1</w:t><w:tab/><w:t>Witness</w:t><w:br/></w:r>'
            '<w:hyperlink r:id="rLink"><w:r><w:t>Read source</w:t></w:r></w:hyperlink>'
            '<w:r><w:instrText> HYPERLINK "https://example.org/field" </w:instrText></w:r></w:p>'
            '<w:tbl><w:tr><w:tc><w:p><w:r><w:t>R1 in table</w:t></w:r></w:p></w:tc>'
            '<w:tc><w:p><w:r><w:t>R10 must not match R1</w:t></w:r></w:p></w:tc></w:tr></w:tbl>'
            '<w:p><w:r><w:t>R10</w:t></w:r></w:p></w:body></w:document>'),
        'word/_rels/document.xml.rels': (
            f'<Relationships xmlns="{P}"><Relationship Id="rLink" '
            'Target="https://example.org/report" TargetMode="External"/></Relationships>'),
        'word/footnotes.xml': (
            f'<w:footnotes xmlns:w="{W}"><w:footnote w:id="1"><w:p><w:r>'
            '<w:t>Footnote evidence</w:t></w:r></w:p></w:footnote></w:footnotes>'),
    })


class SourceRegistryNormalizationTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name)
        self.inputs = self.repo / 'incoming'
        self.inputs.mkdir()
        self.packet = self.repo / 'packet'
        self.packet.mkdir()
        self.manifest = {'corpora': [], 'documents': []}
        profile = json.loads(PROFILE.read_text(encoding='utf-8'))
        profile['registry'] = {
            'Entry code': copy.deepcopy(profile['registry']['record_id']),
            'Caption': copy.deepcopy(profile['registry']['work_or_corpus_title']),
            'Reviewed': copy.deepcopy(profile['registry']['checked_at']),
            'Calculation': copy.deepcopy(profile['registry']['notes']),
            'Empty note': copy.deepcopy(profile['registry']['notes']),
            'Sparse note': copy.deepcopy(profile['registry']['notes']),
        }
        profile['gaps'] = {
            'Subject': copy.deepcopy(profile['gaps']['candidate']),
            'Problem': copy.deepcopy(profile['gaps']['reason']),
        }
        (self.repo / 'third-adapter.json').write_text(json.dumps(profile), encoding='utf-8')
        patcher = mock.patch.object(common, 'ROOT', self.repo)
        patcher.start()
        self.addCleanup(patcher.stop)

    def add_document(self, corpus='third', document='new-doc', rows=None,
                     *, gaps=(), fidelity=False, unknown=False):
        filename = f'{corpus}-{document}'
        xlsx = self.inputs / f'{filename}.xlsx'
        report = self.inputs / f'{filename}.docx'
        make_workbook(xlsx, rows if rows is not None else [('R1', 'A work')],
                      gaps=gaps, fidelity=fidelity, unknown=unknown)
        make_report(report)
        spec = {
            'corpus_id': corpus, 'document_id': document, 'adapter': 'third-adapter.json',
            'sheets': [{'name': 'Entries', 'kind': 'registry', 'header_row': 2},
                       {'name': 'Missing', 'kind': 'gaps', 'header_row': 2}],
        }
        for kind, path in [('xlsx', xlsx), ('docx', report)]:
            sha = hashlib.sha256(path.read_bytes()).hexdigest()
            spec[kind] = {'source_path': path.name, 'sha256': sha,
                          'original_path': f'originals/{sha}.{kind}'}
        self.manifest['documents'].append(spec)
        if corpus not in {item['corpus_id'] for item in self.manifest['corpora']}:
            self.manifest['corpora'].append({'corpus_id': corpus})
        self.save_manifest()
        return spec

    def save_manifest(self):
        (self.packet / 'input.manifest.json').write_text(json.dumps(self.manifest), encoding='utf-8')

    def rewrite_rows(self, spec, rows):
        path = self.inputs / spec['xlsx']['source_path']
        make_workbook(path, rows)
        sha = hashlib.sha256(path.read_bytes()).hexdigest()
        spec['xlsx']['sha256'] = sha
        spec['xlsx']['original_path'] = f'originals/{sha}.xlsx'
        self.save_manifest()

    def snapshot_document(self, summary, index=0):
        return common.read(self.packet / 'snapshots' / summary['snapshot_id'] /
                           summary['documents'][index])

    def snapshot_delta(self, summary):
        return json.loads((self.packet / 'snapshots' / summary['snapshot_id'] / 'delta.json').read_text())

    def test_third_corpus_adapter_and_originals_replay_are_idempotent(self):
        spec = self.add_document(gaps=[('Missing work', 'Edition is not identified')])
        first = common.run(self.packet, self.inputs)
        original_outputs = {p.relative_to(self.packet): p.read_bytes()
                            for p in self.packet.rglob('*') if p.is_file()}
        second = common.run(self.packet, self.inputs)
        self.assertEqual(first, second)
        self.assertEqual(original_outputs, {p.relative_to(self.packet): p.read_bytes()
                                          for p in self.packet.rglob('*') if p.is_file()})
        self.assertEqual(first, common.run(self.packet, check=True))
        self.assertEqual(1, first['counts']['registry'])
        self.assertEqual(1, first['counts']['gaps'])
        document = self.snapshot_document(first)
        self.assertEqual(['Entries', 'Missing'], [sheet['name'] for sheet in document['sheets']])
        self.assertEqual('hidden', document['sheets'][1]['state'])
        self.assertTrue(all(row['assessment_status'] == 'reported_unreviewed'
                            for row in document['records']))
        for kind in ('xlsx', 'docx'):
            self.assertEqual((self.inputs / spec[kind]['source_path']).read_bytes(),
                             (self.packet / spec[kind]['original_path']).read_bytes())

    def test_gzip_wrapper_drift_does_not_change_canonical_snapshot(self):
        self.add_document()
        summary = common.run(self.packet, self.inputs)
        links = self.packet / 'snapshots' / summary['snapshot_id'] / 'links.json.gz'
        raw = gzip.decompress(links.read_bytes())
        alternate = gzip.compress(raw, compresslevel=1)
        self.assertNotEqual(links.read_bytes(), alternate)
        links.write_bytes(alternate)
        self.assertEqual(summary, common.run(self.packet, check=True))

    def test_same_source_id_in_different_corpora_is_not_merged(self):
        self.add_document(corpus='first')
        self.add_document(corpus='third')
        summary = common.run(self.packet, self.inputs)
        records = [self.snapshot_document(summary, i)['records'][0] for i in range(2)]
        self.assertEqual(['R1', 'R1'], [r['source_record_id'] for r in records])
        self.assertNotEqual(records[0]['record_id'], records[1]['record_id'])
        correspondences = common.read(self.packet / 'snapshots' / summary['snapshot_id'] /
                                      'correspondences.json.gz')
        self.assertEqual('unreviewed_possible_correspondence_no_identity_merge', correspondences[0]['status'])

    def test_version_delta_names_changed_field_and_retains_old_original(self):
        spec = self.add_document()
        old_original = spec['xlsx']['original_path']
        before = common.run(self.packet, self.inputs)
        old_record = self.snapshot_document(before)['records'][0]
        self.rewrite_rows(spec, [('R1', 'Corrected title')])
        after = common.run(self.packet, self.inputs)
        new_record = self.snapshot_document(after)['records'][0]
        delta = self.snapshot_delta(after)
        self.assertNotEqual(before['snapshot_id'], after['snapshot_id'])
        self.assertEqual(old_record['record_id'], new_record['record_id'])
        self.assertNotEqual(old_record['occurrence_id'], new_record['occurrence_id'])
        self.assertEqual(before['snapshot_id'], delta['previous_snapshot_id'])
        self.assertEqual(['Caption'], delta['changed'][0]['changed_fields'])
        self.assertEqual([], delta['added'])
        self.assertEqual([], delta['removed'])
        self.assertTrue((self.packet / old_original).exists())
        common.run(self.packet, self.inputs)
        self.assertEqual(delta, self.snapshot_delta(after))

    def test_anonymous_reorder_preserves_content_identity_and_edit_is_not_a_claimed_match(self):
        spec = self.add_document(rows=[(None, 'Alpha'), (None, 'Beta')])
        before = common.run(self.packet, self.inputs)
        old_ids = {r['record_id'] for r in self.snapshot_document(before)['records']}
        self.rewrite_rows(spec, [(None, 'Beta'), (None, 'Alpha')])
        reordered = common.run(self.packet, self.inputs)
        delta = self.snapshot_delta(reordered)
        self.assertEqual(old_ids, set(delta['relocated']))
        self.assertEqual([], delta['added'])
        self.assertEqual([], delta['removed'])
        self.rewrite_rows(spec, [(None, 'Beta'), (None, 'Alpha edited')])
        edited = common.run(self.packet, self.inputs)
        delta = self.snapshot_delta(edited)
        self.assertEqual(1, len(delta['added']))
        self.assertEqual(1, len(delta['removed']))
        self.assertEqual([], delta['changed'])
        for record in self.snapshot_document(edited)['records']:
            self.assertIn('source_record_id_absent_content_identity_only', record['issues'])

    def test_indistinguishable_anonymous_rows_expose_identity_ambiguity(self):
        self.add_document(rows=[(None, 'Same text'), (None, 'Same text')])
        summary = common.run(self.packet, self.inputs)
        records = self.snapshot_document(summary)['records']
        self.assertNotEqual(records[0]['record_id'], records[1]['record_id'])
        for record in records:
            self.assertIn('indistinguishable_anonymous_occurrences', record['issues'])

    def test_unknown_field_fails_before_publishing_or_copying_input(self):
        self.add_document(unknown=True)
        with self.assertRaisesRegex(ValueError, 'Unmapped registry fields'):
            common.run(self.packet, self.inputs)
        self.assertFalse((self.packet / 'current.json').exists())
        self.assertFalse((self.packet / 'originals').exists())

    def test_raw_cell_trace_retains_formula_cache_sparse_and_empty_cells(self):
        spec = self.add_document(fidelity=True)
        summary = common.run(self.packet, self.inputs)
        record = self.snapshot_document(summary)['records'][0]
        fields = {f['source_field']: f for f in record['raw_fields']}
        self.assertEqual('xl/worksheets/items.xml', record['source']['part'])
        self.assertEqual(3, record['source']['row'])
        self.assertEqual(spec['xlsx']['sha256'], record['source']['original_sha256'])
        self.assertEqual('Recorded name', fields['Caption']['value'])
        self.assertEqual('0', fields['Caption']['xml_value'])
        self.assertEqual('1+1', fields['Calculation']['formula'])
        self.assertEqual(2, fields['Calculation']['value'])
        self.assertEqual('2', fields['Calculation']['xml_value'])
        self.assertEqual('E3', fields['Empty note']['cell'])
        self.assertEqual('', fields['Empty note']['value'])
        self.assertEqual('absent', fields['Sparse note']['type'])
        self.assertEqual('F3', fields['Sparse note']['cell'])
        reported = {f['source_field']: f for f in record['reported_fields']}
        self.assertEqual('day', reported['Reviewed']['value']['precision'])
        self.assertEqual('2026-09-03', reported['Reviewed']['value']['value'])

    def test_docx_paragraph_table_footnote_link_and_record_mentions_are_addressable(self):
        self.add_document()
        summary = common.run(self.packet, self.inputs)
        document = self.snapshot_document(summary)
        parts = {p['part']: p for p in document['report_parts']}
        blocks = {b['xml_path']: b for b in parts['word/document.xml']['blocks']}
        first = blocks['/document[1]/body[1]/p[1]']
        self.assertEqual('R1\tWitness\nRead source', first['text'])
        self.assertEqual(document['records'][0]['record_id'], first['record_mentions'][0])
        self.assertEqual('https://example.org/report', first['hyperlinks'][0]['target'])
        self.assertEqual('External', first['hyperlinks'][0]['target_mode'])
        self.assertEqual([' HYPERLINK "https://example.org/field" '], first['field_instructions'])
        self.assertEqual('table', blocks['/document[1]/body[1]/tbl[1]']['kind'])
        self.assertEqual('R1 in table', blocks['/document[1]/body[1]/tbl[1]/tr[1]/tc[1]/p[1]']['text'])
        self.assertEqual([], blocks['/document[1]/body[1]/p[2]']['record_mentions'])
        footnote = parts['word/footnotes.xml']['blocks'][0]
        self.assertEqual('/footnotes[1]/footnote[1]/p[1]', footnote['xml_path'])
        self.assertEqual('Footnote evidence', footnote['text'])
        self.assertTrue(footnote['occurrence_id'].startswith('tos-occurrence:'))

    def test_ooxml_reader_retains_hidden_and_hyperlink_metadata(self):
        spec = self.add_document(fidelity=True)
        sheets = workbook(self.inputs / spec['xlsx']['source_path'])
        representation = json.dumps(sheets, ensure_ascii=False)
        self.assertIn('https://example.org/item?x=1&y=2', representation)
        self.assertIn('link1', representation)
        row = next(r for r in sheets[0]['rows'] if r['row'] == 3)
        self.assertTrue(row.get('hidden') or row.get('attributes', {}).get('hidden') in ('1', 'true'))
        self.assertTrue(sheets[0].get('columns') or sheets[0].get('hidden_columns'))
        self.assertEqual(['F5:G5'], sheets[0]['merges'])

    def test_manifest_digest_and_adapter_path_boundaries_fail_closed(self):
        spec = self.add_document()
        spec['xlsx']['sha256'] = '0' * 64
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'input changed'):
            common.run(self.packet, self.inputs)
        spec['adapter'] = '../outside-adapter.json'
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'path escapes owner root'):
            common.run(self.packet, self.inputs)

    def test_document_identity_cannot_escape_snapshot_output_directory(self):
        spec = self.add_document()
        spec['document_id'] = '../../../../escaped'
        self.save_manifest()
        with self.assertRaises(ValueError):
            common.build(self.packet, self.inputs)

    def test_formula_change_with_same_cached_value_is_visible_in_delta(self):
        spec = self.add_document(fidelity=True)
        before = common.run(self.packet, self.inputs)
        path = self.inputs / spec['xlsx']['source_path']
        with ZipFile(path) as archive:
            parts = {name: archive.read(name).decode('utf-8') for name in archive.namelist()}
        parts['xl/worksheets/items.xml'] = parts['xl/worksheets/items.xml'].replace(
            '<f>1+1</f><v>2</v>', '<f>2*1</f><v>2</v>')
        zipped(path, parts)
        sha = hashlib.sha256(path.read_bytes()).hexdigest()
        spec['xlsx']['sha256'] = sha
        spec['xlsx']['original_path'] = f'originals/{sha}.xlsx'
        self.save_manifest()
        after = common.run(self.packet, self.inputs)
        self.assertNotEqual(before['snapshot_id'], after['snapshot_id'])
        delta = self.snapshot_delta(after)
        self.assertEqual(1, len(delta['changed']))
        self.assertEqual(['Calculation'], delta['changed'][0]['changed_fields'])

    def test_docx_only_edit_names_changed_block_without_changing_registry_identity(self):
        spec = self.add_document()
        before = common.run(self.packet, self.inputs)
        old_record = self.snapshot_document(before)['records'][0]
        old_sha = spec['docx']['sha256']
        old_original = spec['docx']['original_path']
        path = self.inputs / spec['docx']['source_path']
        with ZipFile(path) as archive:
            parts = {name: archive.read(name).decode('utf-8') for name in archive.namelist()}
        parts['word/document.xml'] = parts['word/document.xml'].replace(
            '<w:t>Witness</w:t>', '<w:t>Revised witness assessment</w:t>')
        zipped(path, parts)
        new_sha = hashlib.sha256(path.read_bytes()).hexdigest()
        spec['docx']['sha256'] = new_sha
        spec['docx']['original_path'] = f'originals/{new_sha}.docx'
        self.save_manifest()
        after = common.run(self.packet, self.inputs)
        self.assertNotEqual(before['snapshot_id'], after['snapshot_id'])
        self.assertEqual(old_record, self.snapshot_document(after)['records'][0])
        delta = self.snapshot_delta(after)
        self.assertEqual([], delta['changed'])
        self.assertEqual([], delta['added'])
        self.assertEqual([], delta['removed'])
        self.assertEqual([{'corpus_id': 'third', 'document_id': 'new-doc',
                           'kind': 'docx', 'before_sha256': old_sha,
                           'after_sha256': new_sha}], delta['files'])
        self.assertEqual([{'corpus_id': 'third', 'document_id': 'new-doc',
                           'changed_block_locators': [
                               ['word/document.xml', '/document[1]/body[1]/p[1]']]}], delta['reports'])
        self.assertTrue((self.packet / old_original).exists())


class SourceRegistryLexicalBoundariesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.profile = load_profile(PROFILE)

    def field(self, name, value):
        return normalize_record_values('registry', {name: value}, self.profile)['reported_fields'][0]

    def test_qualified_language_list_keeps_nested_semicolon_and_source_role(self):
        field = self.field('language', 'Sanskrit (Devanāgarī; переключение транслитерации); '
                           'English metadata; Pāli; Greek')
        self.assertEqual(['sa', 'en', 'pi'], [item['language_code'] for item in field['value']])
        self.assertEqual(['(Devanāgarī; переключение транслитерации)'], field['value'][0]['qualifiers'])
        self.assertEqual('metadata', field['value'][1]['role'])
        self.assertEqual(['Greek'], field['unparsed_fragments'])

    def test_format_process_container_and_carrier_are_separate_reported_facets(self):
        field = self.field('formats', 'DjVu; JP2; MOBI; HOCR; HTML ZIP; OCR; Git; page images; print')
        values = field['value']
        self.assertEqual(['djvu', 'jpeg2000', 'mobi', 'hocr'], [v['format'] for v in values[:4]])
        self.assertEqual(('html', 'zip'), (values[4]['format'], values[4]['container']))
        self.assertEqual('ocr', values[5]['reported_process'])
        self.assertNotIn('format', values[5])
        self.assertEqual('git', values[6]['transport'])
        self.assertNotIn('format', values[6])
        self.assertEqual('page_images', values[7]['content_kind'])
        self.assertEqual('print', values[8]['carrier'])

    def test_temporal_ranges_keep_era_approximation_precision_and_unresolved_scope(self):
        for raw, expected in [
            ('c. 2600–1900 BCE', {'year_start': 2600, 'year_end': 1900, 'era': 'BCE',
                                'precision': 'year', 'approximate': True}),
            ('45 BCE', {'year_start': 45, 'year_end': 45, 'era': 'BCE', 'precision': 'year'}),
            ('413–426', {'year_start': 413, 'year_end': 426, 'era': 'unspecified'}),
            ('late 2nd–early 3rd c.', {'century_start': 2, 'century_end': 3,
                                     'start_part': 'late', 'end_part': 'early',
                                     'precision': 'century', 'era': 'unspecified'}),
            ('First millennium CE', {'millennium': 1, 'precision': 'millennium', 'era': 'CE'}),
        ]:
            with self.subTest(raw=raw):
                result = self.field('original_date', raw)
                self.assertEqual(raw, result['value']['reported_statement'])
                expression = result['value']['expressions'][0]
                self.assertEqual('unspecified', expression['scope'])
                for key, value in expected.items():
                    self.assertEqual(value, expression[key])
        compound = 'composition II–III c.; surviving Coptic witness IV c.'
        self.assertEqual([compound], self.field('original_date', compound)['unparsed_fragments'])
        unknown = self.field('original_date', 'Не установлена')['value']
        self.assertEqual('not_established', unknown['reported_date_status'])
        self.assertEqual([], unknown['expressions'])

    def test_public_link_extractor_preserves_exact_spans_and_url_path_parameters(self):
        raw = ('Sources: HTTPS://example.org/a?x=one;two;https://example.org/b\n'
               'https://example.org/c#section')
        links = extract_reported_links(raw)
        self.assertEqual(['HTTPS://example.org/a?x=one;two', 'https://example.org/b',
                          'https://example.org/c#section'], [item['url'] for item in links])
        for link in links:
            self.assertEqual(link['url'], raw[slice(*link['source_span'])])


if __name__ == '__main__':
    unittest.main()
