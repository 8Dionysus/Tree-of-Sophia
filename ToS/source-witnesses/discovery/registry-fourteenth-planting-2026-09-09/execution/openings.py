from metadata import *
pin='d6d54741b7f2ddfeca82f02c3f95eb3990b4e351'
rows=[]
for language in ['pli','en']:
 for n in range(1,12):
  d=f'sn{n}'
  tree=json.loads((e/('bilara-'+d+('-en' if language=='en' else '')+'-tree.json')).read_text())
  for f in tree['tree']:
   assert f['type']=='blob' and f['path'].endswith('.json')
   rows.append({**f,'upstream_path':('root/pli/ms' if language=='pli' else 'translation/en/sujato')+'/sutta/sn/'+d+'/'+f['path'],'language':language,'district':d})
assert len(rows)==542
(b/'candidate-files.json').write_text(json.dumps(rows,indent=2)+'\n')
def one(row):
 p=e/(row['path']+'.opening');url='https://raw.githubusercontent.com/suttacentral/bilara-data/'+pin+'/'+row['upstream_path']
 if p.exists():return
 now=lambda:datetime.now(timezone.utc).isoformat();start=now();tick=time.monotonic();size=min(1536,row['size']-1)
 for attempt in range(3):
  try:
   with urlopen(Request(url,headers={'User-Agent':'Tree-of-Sophia-source-preparation'}),timeout=45) as z:body=z.read(size);status=z.status;final=z.url
   break
  except OSError:
   if attempt==2:raise
 assert len(body)==size and body.startswith(b'{');p.write_bytes(body);p.with_name(p.name+'.receipt.json').write_text(json.dumps({'url':url,'final_url':final,'http_status':status,'started_at':start,'ended_at':now(),'elapsed_seconds':time.monotonic()-tick,'retained_ref':p.relative_to(r).as_posix(),'retained_sha256':hashlib.sha256(body).hexdigest(),'retained_byte_size':len(body),'corpus_payload_fetched':True,'acquisition_scope':'bounded incomplete text opening for UID/title/language assessment; complete source not retained'},indent=2)+'\n')
with ThreadPoolExecutor(max_workers=4) as ex:list(ex.map(one,rows))
print(f'{len(rows)} incomplete openings captured; no complete new files')
