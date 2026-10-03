"""Private fixture adapter over retained genuine synthetic owner OCR evidence.

No OCR execution or stronger-owner signature is manufactured here. The native
creator re-records authenticated retained evidence under a fresh test selection.
"""
from pathlib import Path
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
import shutil

BASE = Path(os.environ['TOS_PRIVATE_JOURNAL_V6_FIXTURE_ROOT']).resolve()
PREFIX = 'ToS/source-witnesses/owner-local/sid-77777777777777777777777777777777/'
OLD_PACKAGE = PREFIX + 'layers/raw-ocr/'
NEW_PACKAGE = PREFIX + 'layers/journal-v6-retained-ocr/'
LAYER_ID = 'tos.text-layer.sid-99999999999999999999999999999999'


def encoded(body):
    return (json.dumps(body, sort_keys=True, ensure_ascii=False, separators=(',', ':'))+'\n').encode()


def write(path, body):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(encoded(body))
    path.chmod(0o600)


def prepare(repository, root):
    public, private = root/'v6-public', root/'v6-private'
    def checked_copy(source, destination):
        entries=list(source.rglob('*'))
        assert len(entries) <= 512
        total=0
        for path in entries:
            assert not path.is_symlink()
            if path.is_file():
                size=path.stat().st_size
                assert size <= 2_097_152
                total+=size
                assert total <= 8_388_608
        shutil.copytree(source,destination)
        for path in entries:
            if path.is_file():
                copy=destination/path.relative_to(source)
                assert hashlib.sha256(path.read_bytes()).digest() == hashlib.sha256(copy.read_bytes()).digest()
    checked_copy(BASE/'source', public)
    shutil.copytree(repository/'ToS/contracts',public/'ToS/contracts',dirs_exist_ok=True)
    private.mkdir(mode=0o700)
    # Only private rights are inputs; no old journal or authored layer is copied.
    checked_copy(BASE/'private'/PREFIX/'rights', private/PREFIX/'rights')
    context = root/'v6-context.json'
    write(context, {'schema_version':'tos_owner_local_source_context_v1',
        'public_root':str(public),'private_root':str(private),'private_prefix':PREFIX,
        'store_id':'sid-77777777777777777777777777777777'})
    config = json.loads((BASE/'private'/OLD_PACKAGE/'source-create-owner-configuration.json').read_bytes())
    expiry = (datetime.now(timezone.utc)+timedelta(hours=1)).isoformat()
    config.update(uid=os.getuid(), principal_id='test:journal-v6-retained-evidence',
        authority_ref='test:journal-v6-new-layer-selection', expires_at=expiry,
        source_context_ref=str(context), source_path=NEW_PACKAGE+'source-text-layer.v1.json',
        identities={'layer_id':LAYER_ID,'provenance_event_id':'tos.event.sid-99999999999999999999999999999999'})
    for name in ('source_access','derivation_access','material'):
        config[name]['expires_at']=expiry
        config[name]['authority_ref']='test:journal-v6-'+name
    config['maker']['agent_ref']='test:journal-v6-retained-authenticated-ocr'
    # Separate synthetic scope record; never amend the earlier layer's rights.
    rights_ref=PREFIX+'rights/journal-v6-synthetic-layer.json'
    rights=json.loads((BASE/'private'/PREFIX/'rights/synthetic-new-ocr-layer.json').read_bytes())
    rights.update(rights_id='tos.rights.synthetic.journal-v6-retained-ocr',scope_refs=[LAYER_ID],
        assessed_at=datetime.now(timezone.utc).isoformat(),
        assessed_by={'maker_type':'model','agent_ref':'test:journal-v6-fixture'},
        assessment_status='not_assessed',review_status='unreviewed')
    write(private/rights_ref,rights)
    for binding in config['derivation_access']['rights_record_refs']:
        if binding['ref'].startswith(PREFIX):
            binding.update(ref=rights_ref,sha256=hashlib.sha256((private/rights_ref).read_bytes()).hexdigest())
    owner=root/'v6-create-owner.json';write(owner,config)
    initial={'public':str(public),'private':str(private),'context':str(context),
        'owner':str(owner),'assessment_owner':str(root/'v6-assessment-owner.json'),
        'source_ref':config['source_path'],'expiry':expiry,
        'original_receipt_sha256':config['material']['receipt_sha256'],
        'original_signature_sha256':config['material']['signature_sha256'],
        'original_owner_source_ref':config['material']['owner_source_ref'],
        'image_path':json.loads((BASE/'synthetic-own-disclosed-owner.json').read_bytes())['native_text_layers'][0]['image_access']['path']}
    write(root/'v6-fixture-state.json',initial)
    return initial


def finish(repository, root):
    import sys
    sys.path[:0]=[str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
    from knowledge_assessment import Record
    state=json.loads((root/'v6-fixture-state.json').read_bytes())
    layer_path=Path(state['private'])/state['source_ref'];raw=layer_path.read_bytes();layer=json.loads(raw)
    owner=json.loads((BASE/'synthetic-own-disclosed-owner.json').read_bytes())
    owner.update(uid=os.getuid(),principal_id='test:journal-v6-read-only',
        journal_directory=str(root/'v6-new-journal'),source_context_ref=state['context'])
    (root/'v6-new-journal').mkdir(mode=0o700)
    selection=owner['native_text_layers'][0]
    selected=selection['binding']['text_layer']
    selected.update(layer_id=LAYER_ID,layer_version=1,record_ref=state['source_ref'],record_sha256=hashlib.sha256(raw).hexdigest())
    for name in ('image_access','payload_access'):
        selection[name]['expires_at']=state['expiry']
        selection[name]['authority_ref']='test:journal-v6-'+name
    selection['source_access']['authority_ref']='test:journal-v6-private-source'
    # Keep OCR/image local; semantic/model disclosure is a separate authority.
    selection['disclosure_access']=None
    record=Record.from_payload(LAYER_ID,1,layer)
    oldscope=next(iter(owner['subjects'].values()))
    oldscope['record']=record.ref
    oldscope['maker_id']=layer['derivation']['maker']['agent_ref']
    owner['subjects']={LAYER_ID:oldscope}
    write(Path(state['assessment_owner']),owner)
    return {**state,'subject':record.ref,'image_sha256':selection['image_access']['sha256'],
        'layer_sha256':selected['record_sha256'],'package':str(layer_path.parent)}
