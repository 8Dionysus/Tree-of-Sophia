#!/usr/bin/env python3
"""Verify the real UI client against one explicitly selected native backend."""
import argparse
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import urlopen

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))
from tos_access.core import ReferenceToSAccessCore
from tos_access.native_access_core import NativeAccessCore
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
// Exercise the actual material adapter too: a successful inspect does not
// establish full human-form delivery or language-aware relation restoration.
const materialId=process.argv[3]||first.focus.node_id,language=process.argv[4];
const material=await client.readMaterial('node',materialId,undefined,first.source_revision,undefined,{language});
if(material.match.id!==materialId||material.packet.nodes.length!==1||material.packet.relations.length!==0)
 throw Error('material identity or isolated-read boundary drift');
const knownRelation=await client.readMaterial('relation',relation.id,undefined,first.source_revision,
 relation.content_revision,{language,relation});
const restoredRelation=await client.readMaterial('relation',relation.id,undefined,first.source_revision,
 relation.content_revision,{language});
if(JSON.stringify(knownRelation.packet)!==JSON.stringify(restoredRelation.packet))
 throw Error('known and restored relation material differ at the same exact revision');
const selection=material.match.human_form_selection;
if(process.argv[3]&&(!selection||selection.requested_language!==language))
 throw Error('explicit material has no language-bound human-form selection');
const {createHash}=await import('node:crypto');
const packetDigest=value=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
const roles=selection?Object.fromEntries(Object.entries(selection.roles).map(([role,value])=>[role,
 {state:value.state,reason:value.reason,form:value.form,language:value.packet?.language??null,
 context_slots:value.packet?.context?.map(item=>item.slot)??[],
 packet_sha256:value.packet?packetDigest(value.packet):null}])):null;
console.log(JSON.stringify({consumer:'observatory KnowledgeClient',source_revision:first.source_revision,
 focus:[first.nodes.length,first.relations.length],pair:[pair.nodes.length,pair.relations.length],
 next:[next.nodes.length,next.relations.length],search:[search.nodes.length,search.relations.length],
 exploration_pages:pages,evidence:evidence.availability,path_count:paths.path_count,
 excluded_path_count:excluded.path_count,
 material:{id:material.match.id,content_revision:material.match.content_revision,requested_language:language,
 selection_state:selection?.state??'not-delivered',roles},
 relation_material:{id:restoredRelation.match.id,nodes:restoredRelation.packet.nodes.length,
 relations:restoredRelation.packet.relations.length,restoration_equal:true}}));
'''


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[2])
    parser.add_argument('--web-root',type=Path,required=True)
    parser.add_argument('--client-module',type=Path,required=True)
    parser.add_argument('--native-prefix',type=Path,required=True,
                        help='explicit installed native software prefix')
    parser.add_argument('--release-root',type=Path,required=True,
                        help='explicit managed release selected by both native query and HTTP routes')
    parser.add_argument('--native-state-root',type=Path,required=True,
                        help='existing private state parent selected for NativeAccessCore')
    parser.add_argument('--http-base',required=True,
                        help='loopback base URL of the already-running native Rust HTTP server')
    parser.add_argument('--material-id',help='exact public knowledge node ID with human forms for the material canary')
    parser.add_argument('--language',default='ru',help='content-language request for the actual UI material reader')
    parser.add_argument('--report',type=Path,help='new JSONL report outside the source repository')
    args=parser.parse_args()
    parsed=urlsplit(args.http_base)
    if (parsed.scheme!='http' or parsed.hostname not in {'127.0.0.1','localhost','::1'}
            or parsed.username is not None or parsed.password is not None
            or parsed.query or parsed.fragment or parsed.path not in ('','/')):
        parser.error('--http-base must be an http loopback origin without path, query, or credentials')
    base_url=args.http_base.rstrip('/')
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
    core=NativeAccessCore.discover(
        tos_root=args.root,
        native_prefix=args.native_prefix,
        release_root=args.release_root,
        native_state_root=args.native_state_root,
    )
    direct_source_revision=None
    try:
        for name,operation in [('catalog-cold',core.knowledge_catalog),('catalog-warm',core.knowledge_catalog),
                               ('search-first',lambda:core.knowledge_search('Заратустра',limit=6)),
                               ('search-warm',lambda:core.knowledge_search('Заратустра',limit=6)),
                               ('search-alternative',lambda:core.knowledge_search('Ницше',limit=6)),
                               ('focus',lambda:core.knowledge_focus('tos.work.friedrich-nietzsche.also-sprach-zarathustra',node_limit=40,relation_limit=80))]:
            started=time.monotonic();packet=operation();seconds=time.monotonic()-started
            emit({'query':name,'seconds':seconds,'bytes':len(json.dumps(packet,ensure_ascii=False).encode())})
            if name=='search-first':
                direct_source_revision=packet.get('source_revision')
        if not isinstance(direct_source_revision,str) or not direct_source_revision:
            raise RuntimeError('selected native Core search omitted its source revision')
        # Search semantics are compared with the source-owned reference graph;
        # the native query route never builds that graph in Python.
        reference=ReferenceToSAccessCore.discover(tos_root=args.root)
        expected=search_knowledge_graph(reference.knowledge_graph(),'Заратустра',limit=6)
        actual=core.knowledge_search('Заратустра',limit=6)
        if expected.get('source_revision')!=actual.get('source_revision'):
            raise RuntimeError('independent source search oracle does not match the selected release revision')
        if actual!=expected:
            raise RuntimeError('native search differs from the independent source search oracle')
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
    except BaseException:
        core.close()
        raise
    base=base_url
    try:
        with urlopen(base,timeout=30) as response:
            assert response.status==200
            assert 'script-src' in response.headers['Content-Security-Policy']
            html=response.read()
            assert html, 'empty frontend HTML'
        expected_html=(args.web_root/'index.html').read_bytes()
        if hashlib.sha256(html).digest()!=hashlib.sha256(expected_html).digest():
            raise RuntimeError('native HTTP server HTML differs from the selected --web-root asset')
        with urlopen(base+'/api/knowledge/catalog',timeout=30) as response:
            http_catalog=json.loads(response.read())
        direct_catalog=core.knowledge_catalog()
        if http_catalog!=direct_catalog:
            raise RuntimeError('native HTTP and imported native Core selected different catalogs')
        if http_catalog.get('source_revision')!=direct_source_revision:
            raise RuntimeError('native HTTP, imported Core and independent source oracle revisions differ')
        result=subprocess.run(['node','--experimental-strip-types','--input-type=module','-e',CLIENT_CHECK,
                               args.client_module.resolve().as_uri(),base,args.material_id or '',args.language],
                              timeout=240,capture_output=True,text=True)
        if result.returncode:
            raise RuntimeError(f'UI consumer check failed:\n{result.stderr}')
        client_report=json.loads(result.stdout)
        if client_report.get('source_revision')!=direct_source_revision:
            raise RuntimeError('UI HTTP client and imported native Core selected different source revisions')
        emit(client_report)
    finally:
        core.close()
    emit({'ok':True,'scope':'selected native Rust HTTP and imported Core contracts, not browser rendering or deployment'})


if __name__=='__main__':
    main()
