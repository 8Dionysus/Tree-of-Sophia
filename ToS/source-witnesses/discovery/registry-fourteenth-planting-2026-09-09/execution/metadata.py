from pathlib import Path
from urllib.request import Request,urlopen
from datetime import datetime,timezone
from concurrent.futures import ThreadPoolExecutor
import json,time,hashlib
r=next(p for p in Path(__file__).resolve().parents if (p/'scripts/acquire_registry_sources.py').is_file());b=r/'ToS/source-witnesses/discovery/registry-fourteenth-planting-2026-09-09';e=b/'evidence';e.mkdir(parents=True,exist_ok=True)
def capture(name,url):
 p=e/name
 if p.exists():return json.loads(p.read_text())
 now=lambda:datetime.now(timezone.utc).isoformat();start=now();tick=time.monotonic()
 with urlopen(Request(url,headers={'User-Agent':'Tree-of-Sophia-source-preparation'}),timeout=45) as z:body=z.read(8000001);status=z.status;final=z.url
 assert len(body)<8000001
 p.write_bytes(body);v={'url':url,'final_url':final,'http_status':status,'started_at':start,'ended_at':now(),'elapsed_seconds':time.monotonic()-tick,'retained_ref':p.relative_to(r).as_posix(),'retained_sha256':hashlib.sha256(body).hexdigest(),'retained_byte_size':len(body),'corpus_payload_fetched':False,'acquisition_scope':'complete immutable Git tree metadata'};p.with_name(p.name+'.receipt.json').write_text(json.dumps(v,indent=2)+'\n');x=json.loads(body);assert not x.get('truncated');return x

if __name__=='__main__':
 jobs=[('bilara-sn-tree.json','7f36c43e1d760e5934fc1a8b6b8af588e7c457e9'),('bilara-sn-en-tree.json','04128dd163b1099e8289acebfc0a41ef18b5ac9f')]
 for name,sha in jobs:
  x=capture(name,'https://api.github.com/repos/suttacentral/bilara-data/git/trees/'+sha)
  for row in x['tree']:
   if row['path'] in {f'sn{n}' for n in range(1,12)}:
    assert row['type']=='tree'
    subname='bilara-'+row['path']+('-en' if '-en-' in name else '')+'-tree.json'
    sub=capture(subname,'https://api.github.com/repos/suttacentral/bilara-data/git/trees/'+row['sha'])
    assert all(z['type']=='blob' for z in sub['tree'])
    print(subname,len(sub['tree']),sum(z['size'] for z in sub['tree']),flush=True)
