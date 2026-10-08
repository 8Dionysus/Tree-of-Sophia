"""Fresh PageOCR consumer over genuinely signed synthetic producer evidence.

Setup creates source metadata and current test grants only. It never mocks
owner verification, runs OCR/rendering, copies a historical layer, or supplies
a stronger-owner signature. Native creation and comparison own the actual case.
"""
from copy import deepcopy
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import sys

LAYER_ID = 'tos.text-layer.sid-cccccccccccccccccccccccccccccccc'


def encoded(value):
    return (json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'))+'\n').encode()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(encoded(value))
    path.chmod(0o600)


def prepare(repository, root):
    sys.path[:0] = [str(repository/'scripts'), str(repository/'tests'),
        str(repository/'mechanics/growth-cycle/tests'),
        str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
    from test_native_text_binding import NativeTextBindingFixture
    sys.path[:0] = [str(repository/'scripts'), str(repository/'tests'),
        str(repository/'mechanics/growth-cycle/tests'),
        str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
    from test_occurrence_growth import copy_contracts
    from source_text_layer_proposal import derivation_policy
    # Source factories only: no mocked owner, layer writer or oracle command.
    public, private = root/'page-public', root/'page-private'
    public.mkdir(mode=0o700);private.mkdir(mode=0o700)
    fixture = NativeTextBindingFixture(public)
    copy_contracts(public)
    for ref in (fixture.packet_ref,fixture.layer_ref,fixture.anchor_ref,fixture.content_ref):
        (public/ref).unlink()
    prefix = 'ToS/source-witnesses/owner-local/sid-'+'9'*32+'/'
    context = root/'page-context.json'
    write(context,{'schema_version':'tos_owner_local_source_context_v1',
        'public_root':str(public),'private_root':str(private),'private_prefix':prefix,'store_id':'sid-'+'9'*32})
    descriptor = Path(os.environ['TOS_PRIVATE_JOURNAL_PAGE_INPUTS_JSON'])
    assert descriptor.is_file() and not descriptor.is_symlink() and descriptor.stat().st_size <= 131072
    inputs = json.loads(descriptor.read_bytes())
    assert inputs['schema'] == 'journal_page_native_consumer_inputs_v1'
    material = deepcopy(inputs['material'])
    receipt_path = Path(material['receipt_root'])/'receipt.json'
    assert receipt_path.is_file() and not receipt_path.is_symlink() and receipt_path.stat().st_size <= 131072
    receipt_raw = receipt_path.read_bytes()
    assert hashlib.sha256(receipt_raw).hexdigest() == material['receipt_sha256']
    receipt = json.loads(receipt_raw)
    assert receipt['schema_version'] == 'tos_retained_pdf_page_ocr_execution_v1'
    assert receipt['status'] == 'completed' and receipt['returncode'] == 0
    assert receipt['owner']['source_ref'] == material['owner_source_ref']
    assert receipt['owner']['adapter_sha256'] == material['adapter_sha256']
    binding = receipt['input_representation']
    assert binding == material['input_representation']
    assert binding['source_file_sha256'] != binding['input_sha256']
    scope = receipt['source_scope']
    from native_owner_ocr import validate_page_binding
    validate_page_binding(binding, source_scope=scope)
    # Preserve the genuinely signed owned IDs in provisional source records.
    # Item identity remains the selected synthetic PDF's exact owner.
    assert scope['item_ref'] == fixture.ids['item']
    fixture.ids.update({kind:scope[kind+'_ref'] for kind in fixture.ids})
    for kind, ref in fixture.refs.items():
        record = json.loads((public/ref).read_bytes())
        record['record_id'] = fixture.ids[kind]
        if kind == 'expression':
            record['work_ref'] = fixture.ids['work']
        elif kind == 'edition':
            record['embodies_expression_refs'] = [fixture.ids['expression']]
        fixture.write_json(ref, record)
    fixture.manifest['embodiment_ref'] = fixture.ids['edition']
    assert {kind+'_ref':value for kind,value in fixture.ids.items()} == {
        key:value for key,value in scope.items() if key not in {'file_ref','file_sha256'}}
    source_pdf = Path(inputs['source_pdf'])
    assert source_pdf.is_file() and not source_pdf.is_symlink() and source_pdf.stat().st_size <= 131072
    pdf = source_pdf.read_bytes()
    assert len(pdf) <= 131072 and pdf.startswith(b'%PDF-')
    assert hashlib.sha256(pdf).hexdigest() == scope['file_sha256']
    payload_root = root/'page-payload-owner'
    selected_pdf = payload_root/Path(fixture.item_home).relative_to('ToS/source-witnesses')/'payload/source-page.pdf'
    selected_pdf.parent.mkdir(parents=True,mode=0o700)
    selected_pdf.write_bytes(pdf)
    selected_pdf.chmod(0o600)
    fixture.manifest['payload_files'].append({'file_id':scope['file_ref'],
        'relative_path':'payload/source-page.pdf','original_basename':'source-page.pdf',
        'media_type':'application/pdf','byte_size':len(pdf),'sha256':scope['file_sha256'],
        'fixity_verified_at':datetime.now(timezone.utc).isoformat()})
    fixture.rights['scope_refs'].append(scope['file_ref'])
    fixture.write_json(fixture.manifest_ref,fixture.manifest)
    fixture.write_json(fixture.rights_ref,fixture.rights)
    expiry = (datetime.now(timezone.utc)+timedelta(hours=1)).isoformat()
    config = {'uid':os.getuid(),'source_context_ref':str(context),
        'source_record_refs':dict(fixture.refs),
        'source_access':{'read_scope':'exact_acquired_file','access_allowed':True,'payload_root':str(payload_root)},
        'derivation_access':{'derivation_allowed':True,'content_visibility':'local_only'},
        'maker':{'maker_type':'software'},'limits':{'max_output_bytes':131072,'max_seconds':60}}
    reference = prefix+'layers/journal-page-current/source-text-layer.v1.json'
    target = private/reference
    current = private
    package_parts = Path(reference).parent.parts
    for part in package_parts[:-1]:
        current /= part
        current.mkdir(mode=0o700,exist_ok=True)
        assert not current.is_symlink() and current.stat().st_uid == os.getuid()
        assert current.stat().st_mode & 0o777 == 0o700
    package = current/package_parts[-1]
    assert package == target.parent
    assert not package.exists() and not package.is_symlink()
    config.update(schema_version='tos_local_text_layer_record_owner_page_ocr_v1',
        principal_id='test:journal-current-page-ocr', authority_ref='test:owned-synthetic-page-ocr-layer',
        expires_at=expiry, source_path=reference,
        allowed_operations=['text-layer.record-owner-page-ocr'], source_scope=scope,
        identities={'layer_id':LAYER_ID,'provenance_event_id':'tos.event.sid-cccccccccccccccccccccccccccccccc'},
        policy=derivation_policy('text-layer.record-owner-page-ocr'),language='de')
    config['source_record_sha256'] = {kind:fixture.file_digest(ref) for kind,ref in fixture.refs.items()}
    config['manifest_sha256'] = fixture.file_digest(fixture.manifest_ref)
    config['source_access'].update(byte_size=len(pdf),expires_at=expiry,
        authority_ref='test:owned-synthetic-exact-page-pdf')
    rights_ref = prefix+'rights/journal-page-current-layer.json'
    rights = deepcopy(fixture.rights)
    rights.update(rights_id='tos.rights.synthetic.journal-page-current-layer',scope_refs=[LAYER_ID],
        assessment_status='not_assessed',review_status='unreviewed')
    write(private/rights_ref,rights)
    config['derivation_access'].update(operation='ocr', expires_at=expiry,
        authority_ref='test:owned-synthetic-page-derivation', rights_record_refs=[
        {'ref':fixture.rights_ref,'sha256':fixture.file_digest(fixture.rights_ref)},
        {'ref':rights_ref,'sha256':hashlib.sha256((private/rights_ref).read_bytes()).hexdigest()}])
    config['maker'].update(agent_ref=config['principal_id'],
        method='tos.owner-retained-page-ocr-record.v1',version='1')
    anchor = deepcopy(fixture.anchor)
    anchor.update(anchor_id='tos.anchor.sid-cccccccccccccccccccccccccccccccc',passage_id=None,
        resolution_status='locator_only',review_status='unreviewed',review_ref=None)
    anchor['target'].update(item_id=scope['item_ref'],file_id=scope['file_ref'],
        file_sha256=scope['file_sha256'],media_type='application/pdf')
    anchor['selector_payload'] = {'kind':'selector_expression','expression':{'mode':'single','selector':{
        'state':{'state_type':'digest_state','representation_ref':fixture.item_home+'/payload/source-page.pdf',
            'representation_sha256':scope['file_sha256'],'media_type':'application/pdf'},
        'selector':{'type':'page_region','page_identity':{'page_number':binding['page_number']},
            'x':0,'y':0,'width':1,'height':1,'coordinate_space':'normalized_0_1'}}}}
    anchor_ref = prefix+'source-page-anchor.current.json'
    write(private/anchor_ref,anchor)
    config['input'] = {'kind':'retained_pdf_page','anchor':{'anchor_id':anchor['anchor_id'],
        'record_ref':anchor_ref,'record_sha256':hashlib.sha256((private/anchor_ref).read_bytes()).hexdigest()}}
    material.update(authority_ref='test:current-authenticated-page-material',expires_at=expiry)
    config['material'] = material
    owner = root/'page-create-owner.json';write(owner,config)
    state = {'public':str(public),'private':str(private),'context':str(context),
        'owner':str(owner),'assessment_owner':str(root/'page-assessment-owner.json'),
        'source_ref':reference,'expiry':expiry,'source_record_refs':dict(fixture.refs),
        'payload_access':config['source_access'],'image_path':inputs['image_path'],
        'source_pdf_path':str(selected_pdf),'input_representation':binding,
        'original_receipt_sha256':material['receipt_sha256'],
        'original_signature_sha256':material['signature_sha256'],
        'original_owner_source_ref':material['owner_source_ref']}
    write(root/'page-fixture-state.json',state)
    return state


def finish(repository, root):
    sys.path[:0] = [str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
    from knowledge_assessment import Record
    state = json.loads((root/'page-fixture-state.json').read_bytes())
    layer_path = Path(state['private'])/state['source_ref'];raw=layer_path.read_bytes();layer=json.loads(raw)
    record = Record.from_payload(LAYER_ID,1,layer)
    selected = {'record_ref':state['source_ref'],'record_sha256':hashlib.sha256(raw).hexdigest(),
        'layer_id':LAYER_ID,'layer_version':1}
    image = state['input_representation']
    selection = {'binding':{'schema_version':'tos_native_text_layer_binding_v1',
        'text_layer':selected,'source_record_refs':state['source_record_refs']},
        'origin_id':'synthetic-current-retained-page-ocr',
        'source_access':{'read_scope':'exact_owner_local','access_allowed':True,
            'authority_ref':'test:owned-synthetic-page-source-metadata'},
        'payload_access':state['payload_access'],
        'comparison_profile':'tos_retained_page_ocr_image_comparison_v1',
        'image_access':{'read_scope':'exact_retained_page','access_allowed':True,
            'authority_ref':'test:owned-synthetic-exact-page-image','expires_at':state['expiry'],
            'path':state['image_path'],'byte_size':image['input_bytes'],'sha256':image['input_sha256'],
            'page_number':image['page_number'],'source_file_ref':image['source_file_ref'],
            'source_file_sha256':image['source_file_sha256'],'processing_boundary':'local_only',
            'width_pixels':image['width_pixels'],'height_pixels':image['height_pixels']},
        'disclosure_access':None}
    policy = Record.from_payload('tos.policy.knowledge-assessment',3,
        json.loads((repository/'ToS/doctrine/semantic-interchange/assessment-policy.v3.json').read_bytes()))
    journal = Path(state['private'])/'page-new-journal';journal.mkdir(mode=0o700)
    owner = {'schema_version':'tos_local_assessment_owner_v6','uid':os.getuid(),
        'principal_id':'test:journal-current-page-comparison','execution_profile':None,
        'policy':{'id':policy.id,'version':policy.version,'payload':policy.payload,'origin_id':policy.origin_id},
        'authorities':[],'competencies':[],'records':[],
        'subjects':{LAYER_ID:{'record':record.ref,'assertion_layer':'textual_observation','risk':'low',
            'languages':['de'],'maker_id':layer['derivation']['maker']['agent_ref'],
            'requested_use':'text-layer:citation','access_allowed':True}},
        'journal_directory':str(journal),'source_context_ref':state['context'],
        'source_records':[],'owner_local_source_records':[],'native_text_units':[],
        'native_text_layers':[selection],'quality_dependencies':{}}
    write(Path(state['assessment_owner']),owner)
    return {**state,'subject':record.ref,'image_sha256':image['input_sha256'],
        'layer_sha256':selected['record_sha256'],'package':str(layer_path.parent)}
