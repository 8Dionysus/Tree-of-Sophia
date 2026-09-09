from pathlib import Path
from urllib.request import Request,urlopen
from datetime import datetime,timezone
from concurrent.futures import ThreadPoolExecutor
import json,time,hashlib
r=next(p for p in Path(__file__).resolve().parents if (p/'scripts/acquire_registry_sources.py').is_file());b=r/'ToS/source-witnesses/discovery/registry-thirteenth-planting-2026-09-09';e=b/'evidence';e.mkdir(parents=True,exist_ok=True)
def capture(name,url):
 p=e/name
 if p.exists():return json.loads(p.read_text())
 now=lambda:datetime.now(timezone.utc).isoformat();start=now();tick=time.monotonic()
 with urlopen(Request(url,headers={'User-Agent':'Tree-of-Sophia-source-preparation'}),timeout=45) as z:body=z.read(8000001);status=z.status;final=z.url
 assert len(body)<8000001
 p.write_bytes(body);v={'url':url,'final_url':final,'http_status':status,'started_at':start,'ended_at':now(),'elapsed_seconds':time.monotonic()-tick,'retained_ref':p.relative_to(r).as_posix(),'retained_sha256':hashlib.sha256(body).hexdigest(),'retained_byte_size':len(body),'corpus_payload_fetched':False,'acquisition_scope':'complete immutable Git tree metadata'};p.with_name(p.name+'.receipt.json').write_text(json.dumps(v,indent=2)+'\n');x=json.loads(body);assert not x.get('truncated');return x
if __name__=='__main__':
 jobs=[('bilara-dn-tree.json','985db01adf1ce6e3371159f5960892fe3691dca8'),('bilara-mn-tree.json','f0e956618fd089176cb797125a68a8b9ea59a3b2'),('bilara-root-tree.json','d6d54741b7f2ddfeca82f02c3f95eb3990b4e351')]
 def go(job):
  name,sha=job;x=capture(name,'https://api.github.com/repos/suttacentral/bilara-data/git/trees/'+sha);print(name,len(x['tree']),sum(z.get('size',0) for z in x['tree']));print([(z['path'],z['sha']) for z in x['tree']] if 'root-tree' in name else '')
 with ThreadPoolExecutor(max_workers=3) as ex:list(ex.map(go,jobs))
