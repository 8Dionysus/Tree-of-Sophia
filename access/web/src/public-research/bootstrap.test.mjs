import {readFileSync} from 'node:fs';
import {expect,it} from 'vitest';
import * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {installPublicReaderRules} from './bootstrap.mjs';
import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {createCorpusNotebook} from '../corpus-reader/notebook.mjs';
import {createResearchShelfStore} from '../research-shelf/storage.mjs';
import {PUBLIC_WORKSPACE_SCHEMA,PUBLIC_WORKSPACE_VERSION,validateAggregate} from './workspace-view.mjs';

it('installs the full public runtime needed for a saved lens and an ordinary backup',async()=>{
  const bytes=readFileSync(new URL('../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url));
  rules.initSync({module:new WebAssembly.Module(bytes)});
  installPublicReaderRules(rules);
  const workspace=createBrowserResearchWorkspace({persistence:false});
  const notebook=createCorpusNotebook({adapter:'memory'}),shelf=createResearchShelfStore({adapter:'memory'});
  try {
    const base={schema:PUBLIC_WORKSPACE_SCHEMA,version:PUBLIC_WORKSPACE_VERSION,exportedAt:new Date().toISOString(),
      workspace:JSON.parse(workspace.exportPacket()),notebook:await notebook.exportPacket(),shelf:await shelf.exportPacket()};
    expect((await validateAggregate(base)).shelf.records).toHaveLength(0);
    await shelf.save({id:'public-lens',title:'Public lens',type:'lens',target:{draft:{v:2,name:'Public lens',scope:'all',
      sources:['knowledge'],nodeIds:[],focusId:null,query:'',kinds:[],predicates:[],depth:0,direction:'either',profile:'overview',
      limit:1,relations:false,conditions:{nodes:[],relations:[]},paths:[]}},collectionIds:[]});
    const packet={...base,shelf:await shelf.exportPacket()};
    expect((await validateAggregate(packet)).shelf.records[0]).toEqual(packet.shelf.records[0]);
    const hash='a'.repeat(64),material={kind:'node',id:'tos.node.shelf-fixture',sourceRevision:hash,contentRevision:'b'.repeat(64)};
    const claimReference={claimId:material.id,pathId:'tos.path.shelf-fixture',relationType:'tos.relation.supports',
      nodeIds:['tos.node.subject',material.id,'tos.node.object'],relationIds:['tos.relation.subject','tos.relation.object'],
      detailRelationIds:[],closureNodeIds:['tos.node.subject',material.id,'tos.node.object']};
    await shelf.save({id:'public-form',title:'Public form',type:'form',target:{material,role:'statement',
      form:{id:'tos.form.shelf-fixture',version:1,digest:`sha256:${hash}`}},collectionIds:[]});
    await shelf.save({id:'public-claim',title:'Public claim',type:'material',target:{...material,claimReference},collectionIds:[]});
    const extended={...base,shelf:await shelf.exportPacket()};
    expect((await validateAggregate(extended)).shelf.records).toEqual(extended.shelf.records);
  } finally {workspace.dispose();await notebook.close();await shelf.close();}
});
