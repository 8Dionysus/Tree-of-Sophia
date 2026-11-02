#!/usr/bin/env python3
"""Project the retained bonsai witness units into native public corpus inputs.

No textual normalization: each source unit is concatenated in chapter order;
anchors select its exact retained bytes. The concatenation is a delivery layer,
not a claim that the four DTA editions are one bibliographic edition.
"""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'access/public-corpus/bonsai-witnesses'
B = ROOT / 'bonsai/zarathustra'
manifest = json.loads((B / 'witnesses.json').read_text())
parts = {'I': 0, 'II': 1, 'III': 2, 'IV': 3}
chapters = sorted((B / 'witnesses').glob('*.json'), key=lambda p: (parts[p.stem.split('.')[0]], int(p.stem.split('.')[1])))

def digest(data):
    return hashlib.sha256(data).hexdigest()

def write(path, value):
    data = (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode()
    path.write_bytes(data)
    return digest(data)

OUT.mkdir(exist_ok=True)
selection_path = ROOT / 'access/public-corpus/selection.v1.json'
selection = json.loads(selection_path.read_text())
doc = next(d for d in selection['documents'] if d['id'] == manifest['corpus']['documentId'])
for lang, version_id in [('de', 'dta-first-editions-de'), ('ru', 'wikisource-antonovsky-1913-ru')]:
    witness = manifest['witnesses'][lang]
    source_ref = f'access/public-corpus/bonsai-witnesses/{version_id}.txt'
    packet_ref = f'access/public-corpus/bonsai-witnesses/{version_id}.packet.json'
    rights_ref = f'access/public-corpus/bonsai-witnesses/{version_id}.rights.json'
    records, structures, source = [], [], ''
    for path in chapters:
        chapter = json.loads(path.read_text())
        rows = chapter['units'].get(lang, [])
        if not rows:
            continue
        structures.append({'id': f'bonsai.{lang}.{chapter["chapter"]}', 'label': chapter['titles'].get(lang) or chapter['chapter'], 'unitId': rows[0][0], 'level': 1, 'parentId': None})
        for row in rows:
            unit_id, kind, raw = row[:3]
            start = len(source)
            source += raw
            records.append((unit_id, kind, raw, start, len(source), chapter['chapter']))
    text_bytes = source.encode()
    revision = digest(text_bytes)
    (ROOT / source_ref).write_bytes(text_bytes)
    file_id = f'tos.file.sha256.{revision}'
    item_id = f'tos.item.zarathustra.bonsai-witness.{version_id}'
    seg = f'tos.text-segmentation.zarathustra.bonsai-witness.{version_id}.v1'
    attribution = witness['attribution']
    url = witness['sources'][0]['url']
    license_url = 'https://creativecommons.org/licenses/by-sa/4.0/'
    rights = {'schema_version':'tos_bonsai_public_rights_projection_v1','rights_id':f'tos.rights.zarathustra.bonsai-witness.{version_id}', 'scope_refs':[item_id,file_id], 'assessment_status':'licensed', 'jurisdictions_reviewed':['US','EU','RU'], 'source_refs':[s['url'] for s in witness['sources']] + [license_url], 'permissions':['Attribution: '+attribution,'Public reuse under CC BY-SA 4.0.'], 'restrictions':['Retain attribution, source links and share-alike license.','Source transcription and alignment remain subject to their stated limitations.'], 'visibility':'public_payload','redistribution_posture':'authorized_with_conditions','derivative_posture':'allowed_with_conditions','license_uri':license_url,'review_status':'unreviewed','rationale':'Projection of the licence and rights basis recorded in bonsai/zarathustra/witnesses.json; no textual or canon acceptance.'}
    rights_sha = write(ROOT / rights_ref, rights)
    anchors = []
    units = []
    for n,(uid,kind,raw,start,end,chapter) in enumerate(records,1):
        aid = f'tos.anchor.zarathustra.bonsai-witness.{version_id}.{n}'
        anchors.append({'anchor_ref':aid,'text_layer_ref':source_ref,'selector':{'type':'text_position','start':start,'end':end,'position_unit':'unicode_code_point','interval':'half_open'},'exact_sha256':digest(raw.encode()),'anchor_role':'content'})
        units.append({'unit_id':uid,'unit_kind':'verse' if kind=='v' else 'paragraph','ordered_anchor_refs':[aid]})
    # DTA is a stand-local composite of four distinct source expressions;
    # its delivery scope does not replace the source identities in witnesses.json.
    expression = witness['tree'][0] if lang == 'ru' else 'tos.expression.zarathustra.bonsai-composite.dta-first-editions-de'
    edition = ('tos.edition.friedrich-nietzsche.also-sprach-zarathustra.saint-petersburg-zhizn-dlya-vsekh-1913'
               if lang == 'ru' else 'tos.edition.zarathustra.bonsai-composite.dta-first-editions-de')
    packet = {'schema_version':'tos_bonsai_public_text_packet_v1','packet_id':f'tos.bonsai-public-text-packet.zarathustra.{version_id}.v1', 'source_scope':{'work_ref':doc['id'],'expression_ref':expression,'edition_ref':edition,'item_ref':item_id,'file_ref':file_id,'file_sha256':revision}, 'source_layer':{'text_layer_ref':source_ref,'text_layer_sha256':revision,'visibility':'public','publication_authorized':True}, 'rights_and_visibility':{'source_visibility':'public','packet_visibility':'public','effective_visibility':'public','private_source_used':False,'publication_authorized':True,'rights_record_refs':[rights_ref]},'anchors':anchors,'units':units,'segmentations':[{'segmentation_id':seg,'ordered_unit_refs':[row[0] for row in records]}]}
    packet_sha = write(ROOT / packet_ref,packet)
    version = {'id':version_id,'label':witness['label']['en'],'language':lang,'role':'source_language' if lang=='de' else 'translation','packetRef':packet_ref,'packetSha256':packet_sha,'segmentationRef':seg,'rightsRef':rights_ref,'rightsSha256':rights_sha,'sourceUrl':url,'attribution':attribution,'license':{'label':witness['licence'],'url':license_url},'structure':structures}
    if lang == 'de':
        # DWDSmor results are form analyses, not a judgment about either
        # passage. Reuse only byte-identical forms occurring in this layer.
        analysis = json.loads((ROOT / 'access/public-corpus/morphology.de.v1.json').read_text())
        forms = set(re.findall(r'[^\W\d_]+', source, flags=re.UNICODE))
        analysis['forms'] = {form: result for form, result in analysis['forms'].items() if form in forms}
        analysis['unknownForms'] = [form for form in analysis['unknownForms'] if form in forms]
        analysis['textLayerSha256'] = revision
        analysis['analysisVersion'] += '-exact-form-reuse-dta-v1'
        analysis['method'] = 'Exact matching forms reused from the retained Gutenberg DWDSmor analysis; candidates are unranked and are not contextual judgments.'
        analysis['coverage'] = {'knownForms':len(analysis['forms']), 'unknownForms':len(analysis['unknownForms'])}
        analysis_ref = f'access/public-corpus/bonsai-witnesses/{version_id}.morphology.json'
        version['analysisRef'] = analysis_ref
        version['analysisSha256'] = write(ROOT / analysis_ref, analysis)
    doc['versions'] = [v for v in doc['versions'] if v['id'] != version_id] + [version]
    manifest['corpus']['dta' if lang=='de' else 'ru'] = {'versionId':version_id,'revision':revision}
write(selection_path,selection)
write(B / 'witnesses.json',manifest)
