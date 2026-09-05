#!/usr/bin/env python3
"""Opt-in real UI-client/HTTP compatibility check, without editing either tree."""
import argparse
import hashlib
import json
import subprocess
import sys
import threading
import time
from http.server import ThreadingHTTPServer
from pathlib import Path
from urllib.request import urlopen

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))
from tos_access.core import ToSAccessCore
from tos_access.http_server import build_handler
from tos_access.knowledge import search_knowledge_graph

CLIENT_CHECK = r'''
const {KnowledgeClient,focusSpec,relationSpec,DEFAULT_FOCUS}=await import(process.argv[1]);
const base=process.argv[2];
const client=new KnowledgeClient({base:base+'/api/knowledge'});
const first=await client.compile(focusSpec(DEFAULT_FOCUS));
if(!first.nodes.length||!first.relations.length)throw Error('empty initial focus');
const search=await client.search('Заратустра');
if(!search.nodes.length)throw Error('empty search');
await client.inspect('node',first.nodes[0].id,undefined,first.source_revision,first.nodes[0].content_revision);
const relation=first.relations[0];
await client.inspect('relation',relation.id,undefined,first.source_revision,relation.content_revision);
const pair=await client.compile(relationSpec(relation),undefined,first.source_revision);
if(pair.relations.length!==1||pair.relations[0].id!==relation.id)throw Error('relation selection drift');
const next=await client.compile(focusSpec(relation.to_id),undefined,first.source_revision);
const {explorationQuery,loadPaths}=await import(new URL('./navigation-model.mjs',process.argv[1]));
const {loadEvidence}=await import(new URL('./evidence-model.mjs',process.argv[1]));
const {createToSQueryOperations}=await import(new URL('../query-operations.ts',process.argv[1]));
const queries=createToSQueryOperations(async(path,options)=>{
 const response=await fetch(base+path,options);
 if(!response.ok){const error=new Error('HTTP '+response.status);error.status=response.status;throw error;}
 return response.json();
});
// The UI opens exploration from the selected opaque knowledge ID, not its alias.
let page=await client.explore(explorationQuery(first.focus.node_id),undefined,first.source_revision);
const pages=[page.page.number];
while(page.page.next_cursor&&pages.length<4){
 page=await client.explore({cursor:page.page.next_cursor},undefined,first.source_revision,page);
 pages.push(page.page.number);
}
// A source-owned contested example exercises native/knowledge identity binding.
const evidenceArea=await client.compile({schema_version:'tos_lens_spec_v1',lens_id:'integration-evidence',sources:['philosophy'],
 detail:'compact',node_query:{enabled:false},relation_query:{filters:[{field:'native_id',op:'eq',
 value:'edge:candidate-relation:table-i-a01-relation-027'}]},traversal:{depth:0,profile:'all'},
 composition:{endpoint_policy:'independent'},limits:{nodes:2,relations:1,groups:1}},undefined,first.source_revision);
if(evidenceArea.relations.length!==1)throw Error('missing contested source fixture');
const subject=evidenceArea.relations[0];
const evidence=await loadEvidence(subject,'relation',first.source_revision,{client,queries});
if(evidence.availability!=='available'||evidence.packet.conclusion.can_conclude!==false)
 throw Error('contested evidence boundary drift');
const start=evidenceArea.nodes.find(n=>n.id===subject.from_id),end=evidenceArea.nodes.find(n=>n.id===subject.to_id);
const paths=await loadPaths(start,end,first.source_revision,{client,queries});
if(!paths.found||!paths.paths.some(path=>path.edge_ids.includes(subject.id)))throw Error('missing bound direct path');
const excluded=await loadPaths(start,end,first.source_revision,{client,queries,excluded:[subject]});
if(excluded.paths.some(path=>path.edge_ids.includes(subject.id)))throw Error('excluded edge returned');
console.log(JSON.stringify({consumer:'observatory KnowledgeClient',source_revision:first.source_revision,
 focus:[first.nodes.length,first.relations.length],pair:[pair.nodes.length,pair.relations.length],
 next:[next.nodes.length,next.relations.length],search:[search.nodes.length,search.relations.length],
 exploration_pages:pages,evidence:evidence.availability,path_count:paths.path_count,
 excluded_path_count:excluded.path_count}));
'''


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[2])
    parser.add_argument('--web-root',type=Path,required=True)
    parser.add_argument('--client-module',type=Path,required=True)
    parser.add_argument('--report',type=Path,help='new JSONL report outside the source repository')
    args=parser.parse_args()
    if args.report:
        args.report=args.report.resolve()
        if args.report.exists() or args.report.is_relative_to(args.root.resolve()):
            parser.error('use a new report outside the source repository')
        args.report.parent.mkdir(parents=True,exist_ok=True)
    def emit(packet):
        line=json.dumps(packet)
        if args.report:
            with args.report.open('a',encoding='utf-8') as stream:
                stream.write(line+'\n')
        print(line,flush=True)
    emit({'client_sha256':hashlib.sha256(args.client_module.read_bytes()).hexdigest(),
          'html_sha256':hashlib.sha256((args.web_root/'index.html').read_bytes()).hexdigest()})
    core=ToSAccessCore.discover(args.root)
    for name,operation in [('catalog-cold',core.knowledge_catalog),('catalog-warm',core.knowledge_catalog),
                           ('search-first',lambda:core.knowledge_search('Заратустра',limit=6)),
                           ('search-warm',lambda:core.knowledge_search('Заратустра',limit=6)),
                           ('search-alternative',lambda:core.knowledge_search('Ницше',limit=6)),
                           ('focus',lambda:core.knowledge_focus('tos.work.friedrich-nietzsche.also-sprach-zarathustra',node_limit=40,relation_limit=80))]:
        started=time.monotonic();packet=operation();seconds=time.monotonic()-started
        emit({'query':name,'seconds':seconds,'bytes':len(json.dumps(packet,ensure_ascii=False).encode())})
    expected=search_knowledge_graph(core.knowledge_graph(),'Заратустра',limit=6)
    assert core.knowledge_search('Заратустра',limit=6)==expected
    selected=packet['relations'][0]
    for name,operation in [('inspect-node',lambda:core.knowledge_node(selected['from_id'],relation_limit=0)),
                           ('inspect-relation',lambda:core.knowledge_relation(selected['id'])),
                           ('explore',lambda:core.knowledge_explore({'focus_node_id':selected['from_id'],'max_depth':2,'page_nodes':10,'page_relations':10}))]:
        started=time.monotonic();result=operation();seconds=time.monotonic()-started
        emit({'query':name,'seconds':seconds,'bytes':len(json.dumps(result,ensure_ascii=False).encode())})
    cursor=result['page']['next_cursor']
    if cursor:
        started=time.monotonic();result=core.knowledge_explore({'cursor':cursor})
        emit({'query':'explore-resume','seconds':time.monotonic()-started,
              'bytes':len(json.dumps(result,ensure_ascii=False).encode())})
    server=ThreadingHTTPServer(('127.0.0.1',0),build_handler(core,args.web_root))
    worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
    base=f'http://127.0.0.1:{server.server_port}'
    try:
        with urlopen(base,timeout=30) as response:
            assert response.status==200
            assert 'script-src' in response.headers['Content-Security-Policy']
            assert response.read(), 'empty frontend HTML'
        result=subprocess.run(['node','--experimental-strip-types','--input-type=module','-e',CLIENT_CHECK,args.client_module.resolve().as_uri(),base],timeout=240,capture_output=True,text=True)
        if result.returncode:
            raise RuntimeError(f'UI consumer check failed:\n{result.stderr}')
        emit(json.loads(result.stdout))
    finally:
        server.shutdown();server.server_close();worker.join(timeout=5)
    emit({'ok':True,'scope':'local real-client HTTP contract, not browser rendering or deployment'})


if __name__=='__main__':
    main()
