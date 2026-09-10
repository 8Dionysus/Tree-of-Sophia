"""Bounded second planting: retained upstream metadata, then reviewed source templates."""
from pathlib import Path
import sys, json, hashlib, time
from datetime import datetime, timezone
from urllib.request import Request, urlopen
import xml.etree.ElementTree as ET
root=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(root/'scripts'))
import prepare_registry_sources as first
from prepare_philosophy_source_planting import prepare_anchor
from source_registry_common import read, PACKET
base=Path(__file__).resolve().parents[1]
rel=base.relative_to(root).as_posix()
pin='341e309c821d5eca8c976bebca77c28b10bad58f'
repo='PerseusDL/canonical-greekLit'
ns={'t':'http://www.tei-c.org/ns/1.0','c':'http://chs.harvard.edu/xmlns/cts'}
names=['Euthyphro','Apology','Crito','Phaedo','Cratylus','Theaetetus','Sophist','Statesman','Parmenides','Philebus','Symposium','Phaedrus']
sha=lambda b:hashlib.sha256(b).hexdigest()
raw=lambda p:f'https://raw.githubusercontent.com/{repo}/{pin}/{p}'
stamp=lambda:datetime.now(timezone.utc).isoformat()
write=first.write

def capture(name,url,header=False,tree=False):
 p=base/'evidence'/name; p.parent.mkdir(exist_ok=True)
 receipt=p.with_name(p.name+'.receipt.json')
 if p.exists():
  r=read(receipt)
  if r['url']!=url or sha(p.read_bytes())!=r['retained_sha256']:raise ValueError('metadata identity changed')
  return p.read_bytes(),r
 start=stamp(); tick=time.monotonic()
 with urlopen(Request(url,headers={'User-Agent':'Tree-of-Sophia-source-preparation'}),timeout=40) as response:
  b=b''
  if header:
   while b'</teiHeader>' not in b and len(b)<100000:b+=response.read(4096)
   if b'</teiHeader>' not in b:raise ValueError('no bounded TEI header')
   received=len(b); b=b.split(b'</teiHeader>',1)[0]+b'</teiHeader>'
  else:
   b=response.read(8_000_001);received=len(b)
  status,final=response.status,response.url
 response_digest=sha(b)
 if tree:
  obj=json.loads(b)
  if obj['truncated']:raise ValueError('upstream Git tree truncated')
  obj['tree']=[x for x in obj['tree'] if x['path'].startswith('data/tlg0059/') and any(f'/tlg{i:03}/' in x['path'] for i in range(1,13)) and ('perseus-grc' in x['path'] or x['path'].endswith('__cts__.xml'))]
  obj['selection_only']=True;b=first.encode(obj)
 r={'url':url,'final_url':final,'http_status':status,'started_at':start,'ended_at':stamp(),'elapsed_seconds':time.monotonic()-tick,'retained_sha256':sha(b),'retained_byte_size':len(b),'retained_ref':f'{rel}/evidence/{name}','response_sha256':response_digest,'received_bytes':received,'corpus_payload_fetched':header,'acquisition_scope':'metadata prefix only; complete corpus file not retained' if header else 'metadata/license','response_digest_scope':'retained header prefix' if header else 'complete response'}
 p.write_bytes(b);write(receipt,r);return b,r

def metadata():
 capture('perseus-README.md',raw('README.md'))
 capture('perseus-license.md',raw('license.md'))
 tree,_=capture('perseus-tree.json',f'https://api.github.com/repos/{repo}/git/trees/{pin}?recursive=1',tree=True)
 for row in json.loads(tree)['tree']:
  if row['path'].endswith('__cts__.xml'):capture('perseus-'+row['path'].split('/')[-2]+'-cts.xml',raw(row['path']))
  else:capture('perseus-'+Path(row['path']).stem+'-header.xml',raw(row['path']),header=True)
 print('Retained metadata for twelve pinned TEI files.')

def prepare():
 review=base/'SOURCE_AND_RIGHTS_REVIEW.md'
 if not review.is_file():raise ValueError('source-visible owner review required')
 snapshot_root=root/PACKET/'snapshots'/read(root/PACKET/'current.json')['snapshot_id'];snapshot=snapshot_root/'snapshot.json'
 doc=None
 for d in read(snapshot)['documents']:
  value=read(snapshot_root/d)
  if value['document_id']=='A25':doc=value
 if doc is None:raise ValueError('A25 normalization absent')
 records={r['source_record_id']:r for r in doc['records']}
 anchor=prepare_anchor(root,atlas_row_id='A25',source_table_index=14,source_row_index=1,source_label='Scaife Viewer / Perseus')
 trees=read(base/'evidence/perseus-tree.json')['tree'];targets=[];packages=[];plans=[]
 receipts=[read(p) for p in sorted((base/'evidence').glob('*.receipt.json'))]
 assessed_at=stamp()
 for i,title in enumerate(names,1):
  slug=title.lower();group=f'tlg{i:03}'
  rows=[x for x in trees if f'/{group}/' in x['path'] and 'perseus-grc' in x['path']]
  if len(rows)!=1:raise ValueError('Greek edition target ambiguous')
  row=rows[0]; filename=Path(row['path']).name;urn='urn:cts:greekLit:'+Path(filename).stem
  header=base/'evidence'/('perseus-'+Path(filename).stem+'-header.xml');header_xml=ET.fromstring(header.read_bytes()+b'\n</TEI>')
  cts=ET.parse(base/'evidence'/f'perseus-{group}-cts.xml').getroot()
  if not any(x.get('urn')==urn for x in cts):raise ValueError('CTS edition absent')
  source_desc=' '.join(' '.join(header_xml.find('t:teiHeader/t:fileDesc/t:sourceDesc',ns).itertext()).split())
  original=f'A25-R{i+9:03}';lead=records[original]
  source={'corpus':'table-i','document_id':'A25','original_source_id':original,'entry_id':lead['record_id'],'source_locator':lead['source']}
  identity=f'plato.{slug}';exp='grc-perseus-burnet';edition='perseus-'+pin[:12];item='git-tei-xml'
  w=f'ToS/source-witnesses/works/plato/{slug}';e=f'{w}/expressions/{exp}';d=f'{e}/editions/{edition}';it=f'{d}/items/{item}'
  ids={'work':f'tos.work.{identity}','expression':f'tos.expression.{identity}.{exp}','edition':f'tos.edition.{identity}.{exp}.{edition}','item':f'tos.item.{identity}.{exp}.{edition}.{item}'}
  paths={'work':w+'/work.json','expression':e+'/expression.json','edition':d+'/edition.json','item':it+'/item.json','item_root':it}
  target=dict(slug=slug,title=title,family='plato',provider='perseus',repository=repo,pin=pin,language='grc',script='Grek',expression=exp,edition=edition,item=item,registry_sources=[source],branch_target_slug=slug,ids=ids,paths=paths,coverage={'kind':'perseus-tei-work','cts_urn':urn,'header_prefix_sha256':sha(header.read_bytes())},version_description=f'Perseus Ancient Greek TEI/EpiDoc transcription of {title}; exact CTS edition {urn}, Git commit {pin}. Supplied source description: {source_desc}',responsibility='John Burnet: named print editor. Perseus Project, Tufts University: digital transcription and encoding; retain the full supplied header credits.',limits=['The supplied Greek edition is a source-language text, not a manuscript facsimile or ToS critical reconstruction.','A complete pinned file does not establish completeness of the critical apparatus or all recensions.','Dialogue speakers are not automatically assertions of Plato; dramatic voice, chronology and doctrinal confidence remain unassessed.','Only this edition of this work is planted; translations, other editions, remaining Plato and Aristotle works stay open.'],files=[{'upstream_path':row['path'],'basename':filename,'git_blob_sha1':row['sha'],'byte_size':row['size'],'media_type':'application/tei+xml','url':raw(row['path'])}],byte_size=row['size'])
  refs=[f'{rel}/manifest.json',f'{rel}/SOURCE_AND_RIGHTS_REVIEW.md',f'{rel}/evidence/perseus-README.md',f'{rel}/evidence/perseus-license.md',header.relative_to(root).as_posix(),f'{rel}/evidence/perseus-{group}-cts.xml',f'https://github.com/{repo}/tree/{pin}']
  license_uri='https://creativecommons.org/licenses/by-sa/4.0/'
  restrictions=['Retain supplied Tufts/Perseus, editor and contributor attribution and all header notices.','Link CC BY-SA 4.0; identify any future modifications.','Shared adaptations must retain the applicable share-alike license; impose no additional restrictions.','Offer any future source modifications to Perseus as requested in its README; this intake changes no source bytes.']
  rid='tos.rights.'+ids['item'].removeprefix('tos.item.')
  rights={'schema_version':'tos_rights_record_v1','rights_id':rid,'scope_refs':[ids['item']],'assessment_status':'licensed','rights_statement_uri':license_uri,'license_uri':license_uri,'jurisdictions_reviewed':['MX'],'source_refs':refs+[license_uri],'layer_assessments':[],'permissions':['Acquire and retain the exact positively licensed source file locally.','Process the supplied digital text with the stated attribution and license conditions.'],'restrictions':restrictions,'visibility':'local_only','redistribution_posture':'authorized_with_conditions','derivative_posture':'allowed_with_conditions','assessed_by':{'maker_type':'model','agent_ref':'model:codex'},'assessed_at':assessed_at,'rationale':'The exact repository grants CC BY-SA 4.0 unless otherwise indicated. Each retained file header and CTS metadata was separately inspected for a conflicting exception. Positive license basis for this local operation; no copyright-term calculation or universal legal opinion. Rights apply only within the supplier authority.','review_status':'unreviewed','review_refs':[f'{rel}/SOURCE_AND_RIGHTS_REVIEW.md'],'access_request_ref':None,'record_version':1,'supersedes_rights_ref':None}
  for name,role,scope in [('digital-text','embedded_text',[ids['expression']]),('edition','editing',[ids['edition']]),('encoding','edition_presentation',[ids['item']]),('metadata','metadata',[ids['item']])]:
   rights['layer_assessments'].append({'layer_id':rid+'.layer.'+name,'layer_role':role,'scope_refs':scope,'assessment_status':'licensed','assessment_basis':'license','rights_statement_uri':license_uri,'license_uri':license_uri,'jurisdictions_reviewed':['MX'],'source_refs':refs+[license_uri],'rights_holder_refs':[f'https://github.com/{repo}'],'permissions':rights['permissions'],'restrictions':restrictions,'redistribution_posture':'authorized_with_conditions','derivative_posture':'allowed_with_conditions','server_processing_posture':'authorized_with_conditions','term':{'calculation_status':'not_applicable','basis':'Positive license; no expiry calculation relied upon.','starts_on':None,'ends_on':None,'uncertainty':'Supplier scope only.'},'uncertainty':'No human legal review or manuscript rights assessment.','assessed_at':assessed_at,'review_status':'unreviewed','rationale':'Exact supplied Perseus digital layer under the repository license; no broader ancient authorship or textual acceptance claim.'})
  package=first.prepare_package(target,assessed_at,evidence_refs=refs,rights_assessment=rights)
  for kind in ('expression','edition','item'):package['records'][paths[kind]]['external_identifiers']=[{'scheme':'CTS','value':urn,'source_ref':f'{rel}/evidence/perseus-{group}-cts.xml','status':'verified'}]
  package['records'][paths['work']]['external_identifiers']=[{'scheme':'CTS','value':'urn:cts:greekLit:tlg0059.'+group,'source_ref':f'{rel}/evidence/perseus-{group}-cts.xml','status':'verified'}]
  targets.append(target);packages.append(package)
  plans.append({'target_slug':slug,'registry_source_record_id':original,'anchor_preparation':anchor,'scope_relationship':'bounded-constituent-work','scope_rationale':f'The {title} Ancient Greek edition is one exact work within the branch Scaife/Perseus source need.','remaining_controls':target['limits']})
 package_bytes=b''.join((json.dumps(p,ensure_ascii=False,separators=(',',':'))+'\n').encode() for p in packages)
 (base/'prepared-source-packages.jsonl').write_bytes(package_bytes)
 branch_ref='ToS/philosophy/source-planting-preparation/classical-athenian-plato-works-20260908.json'
 write(root/branch_ref,{'schema_version':'tos_source_planting_preparation_batch_v1','status':'prepared-not-planted','review_scope':'Exact A25 branch fit and version boundary only','reviewer_ref':'model:codex','targets':plans})
 manifest={'schema_version':'tos_registry_first_planting_preparation_v1','status':'prepared-not-acquired','source_registry_snapshot_ref':snapshot.relative_to(root).as_posix(),'source_registry_snapshot_sha256':sha(snapshot.read_bytes()),'branch_preparation_ref':branch_ref,'provider_pins':{'perseus':pin},'metadata_observations':receipts,'targets':targets,'prepared_packages_ref':f'{rel}/prepared-source-packages.jsonl','prepared_packages_sha256':sha(package_bytes),'totals':{'works':12,'expressions':12,'payload_files':12,'payload_bytes':sum(t['byte_size'] for t in targets),'bibliographic_claims':36},'authority_boundary':'Preparation does not establish local custody, textual or semantic acceptance, canon or public release.'}
 write(base/'manifest.json',manifest);print(json.dumps(manifest['totals']))

if __name__=='__main__':
 if sys.argv[1:]==['metadata']:metadata()
 elif sys.argv[1:]==['prepare']:prepare()
 else:raise SystemExit('choose metadata or prepare')
