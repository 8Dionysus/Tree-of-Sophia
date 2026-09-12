from metadata import *
pin='d6d54741b7f2ddfeca82f02c3f95eb3990b4e351'
x=json.loads((e/'bilara-root-tree.json').read_text());path=[]
for component in ['translation','en','sujato','sutta']:
 sha=next(z['sha'] for z in x['tree'] if z['path']==component and z['type']=='tree');path.append(component);x=capture('bilara-'+'-'.join(path)+'-tree.json','https://api.github.com/repos/suttacentral/bilara-data/git/trees/'+sha)
for district in ['dn','mn']:
 sha=next(z['sha'] for z in x['tree'] if z['path']==district);tree=capture('bilara-'+district+'-en-tree.json','https://api.github.com/repos/suttacentral/bilara-data/git/trees/'+sha);print(district,len(tree['tree']),sum(z.get('size',0) for z in tree['tree']))
def rawcapture(name):
 p=e/('bilara-'+name)
 if p.exists():return
 url='https://raw.githubusercontent.com/suttacentral/bilara-data/'+pin+'/'+name;now=lambda:datetime.now(timezone.utc).isoformat();start=now();tick=time.monotonic()
 with urlopen(Request(url,headers={'User-Agent':'Tree-of-Sophia-source-preparation'}),timeout=45) as z:body=z.read(2000001);status=z.status;final=z.url
 assert len(body)<2000001;p.write_bytes(body);p.with_name(p.name+'.receipt.json').write_text(json.dumps({'url':url,'final_url':final,'http_status':status,'started_at':start,'ended_at':now(),'elapsed_seconds':time.monotonic()-tick,'retained_ref':p.relative_to(r).as_posix(),'retained_sha256':hashlib.sha256(body).hexdigest(),'retained_byte_size':len(body),'corpus_payload_fetched':False,'acquisition_scope':'complete repository publication metadata or license'},indent=2)+'\n')
with ThreadPoolExecutor(max_workers=4) as ex:list(ex.map(rawcapture,['README.md','LICENSE.md','_author.json','_edition.json','_publication.json','_publication-v2.json']))
for name in ['_author.json','_edition.json','_publication.json','_publication-v2.json']:
 x=json.loads((e/('bilara-'+name)).read_text());print(name,str(x)[:700])
