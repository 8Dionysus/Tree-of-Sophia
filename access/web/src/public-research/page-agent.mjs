import {createPageCommandRegistry} from '../page-commands.ts';
import {createWebMCPAdapter} from '../webmcp.ts';
import {runResearchOperation} from './research-operations.mjs';
import {localeValue} from './locale.mjs';
import {createReference,validateReference,referenceDocumentId,referenceVersionId} from '../corpus-reader/model.mjs';

const SOURCE_CURSOR='tos-bonsai-source-1.';

/** Read the cited selection without substituting another work or losing its end. */
export async function readResearchSource({index,provider,fragments},input,{signal}={}){
  const limit=input.limit??8;
  if(!Number.isSafeInteger(limit)||limit<1||limit>20)throw new TypeError('Choose a source window limit from 1 to 20.');
  if(Boolean(input.reference)===Boolean(input.item_id))throw new TypeError('Choose one exact passage ID or native reference.');
  if(input.reference){
    const reference=validateReference(input.reference),resolved=await provider.resolve({reference,signal});
    if(resolved.status!=='exact')return resolved;
    return provider.window({documentId:referenceDocumentId(reference),versionId:referenceVersionId(reference),reference,
      ...(reference.target?.textLayerSha256?{revision:reference.target.textLayerSha256}:{}),
      ...(input.cursor?{cursor:input.cursor}:{}),limit,signal});
  }
  const item=index.get(input.item_id);
  if(item?.kind!=='passage')throw new TypeError('Choose an exact passage ID from research-item dependencies.');
  const range=await fragments.sourceSelection(item.record,{signal});
  const binding={item_id:item.id,index_revision:index.revision,documentId:range.documentId,versionId:range.versionId,revision:range.revision};
  let offset=0;
  if(input.cursor!==undefined){
    try{
      if(typeof input.cursor!=='string'||input.cursor.length>4096||!input.cursor.startsWith(SOURCE_CURSOR))throw new Error();
      const saved=JSON.parse(decodeURIComponent(input.cursor.slice(SOURCE_CURSOR.length)));
      if(JSON.stringify(saved.binding)!==JSON.stringify(binding)||!Number.isSafeInteger(saved.offset)||saved.offset<0||saved.offset>=range.selections.length)throw new Error();
      offset=saved.offset;
    }catch{throw new TypeError('The source cursor belongs to another citation or revision.');}
  }
  const units=[];
  for(const selected of range.selections.slice(offset,offset+limit)){
    const page=await provider.window({documentId:range.documentId,versionId:range.versionId,revision:range.revision,unitId:selected.unitId,limit:1,signal});
    const unit=page.units?.[0];
    if(page.documentId!==range.documentId||page.versionId!==range.versionId||page.revision!==range.revision||unit?.id!==selected.unitId
      ||unit.text!==selected.raw)throw new TypeError('The cited witness words changed; this selection cannot be reproduced.');
    const chars=Array.from(unit.text);
    if(selected.start<0||selected.end<selected.start||selected.end>chars.length)throw new TypeError('The citation range is outside its exact source.');
    units.push({...unit,selection:{start:selected.start,end:selected.end,text:chars.slice(selected.start,selected.end).join('')},
      exactReference:createReference({source:range.source,versionId:range.versionId,unitId:unit.id,start:selected.start,end:selected.end})});
  }
  const next=offset+units.length,complete=next===range.selections.length;
  return {...binding,units,total:range.selections.length,rangeComplete:complete,
    nextCursor:complete?null:SOURCE_CURSOR+encodeURIComponent(JSON.stringify({binding,offset:next}))};
}

/** Public stands expose their existing owners through the shared page registry. */
export function connectResearchPage({index,space,experience,session,api,workspace,locale,targetDocument=document}){
  const summary=()=>workspace.summary();
  const selection=()=>{
    const item=index.get(experience.getState().id??space.selection()?.id);if(!item)return null;
    return {id:item.id,kind:item.kind==='relation'?'edge':['statement','passage'].includes(item.kind)?'item':'node',semantic_kind:item.kind,label:localeValue(item.title,locale()),source_refs:item.dependencies.filter(id=>index.get(id)?.kind==='passage'),
      from_id:item.record?.from,to_id:item.record?.to,predicate_id:item.record?.relation,authority_posture:item.owner,review_posture:item.record?.assessment?.verdict??item.record?.reading?.review_status??'source-visible-proposal',canon_status:'stand-scope',path_available:false};
  };
  const context=()=>({mode:'philosophy',view_id:`bonsai:${api.profile}`,graph_mode:'nodes',selected:selection(),path_start_node_id:null,active_layers:[experience.getState().mode],active_predicates:[],deep_link:location.href,research_workspace:summary()});
  const readSource=(input,options)=>readResearchSource({index,provider:api.provider,fragments:space.fragments},input,options);
  let registry;
  const handlers={
    'tos.page.select':input=>{if(!index.get(input.item_id??input.id))throw new TypeError('Unknown item ID');experience.open(input.item_id??input.id);return selection();},
    'tos.page.open-view':input=>{experience.setMode(input.view_id??input.view??'reference');if(input.focus_id??input.focus)experience.open(input.focus_id??input.focus);return context();},
    'tos.page.search':input=>{const found=index.search(input.query??'',{limit:input.limit??30});return {query:input.query,result_count:found.length,results:found.map(item=>({id:item.id,kind:item.kind,label:localeValue(item.title,locale()),summary:item.owner,review_posture:item.record?.assessment?.verdict??'proposal'}))};},
    'tos.page.inspect-selection':()=>index.inspect(selection()?.id),
    'tos.page.research-item':input=>{const result=index.inspect(input.id);if(!result)throw new TypeError('Unknown research item');return result;},
    'tos.page.research-operation':input=>runResearchOperation(input.request),
    'tos.page.read-source':readSource,
    'tos.page.cognitive-next':(input,{signal})=>session.dispatch('next-event',input,signal),
    'tos.page.cognitive-action':(input,{signal})=>session.dispatch(input.action,input,signal),
    'tos.page.research-workspace':()=>JSON.parse(workspace.exportPacket()),
    'tos.page.workspace-export':()=>JSON.parse(workspace.exportPacket()),
    'tos.page.add-research-note':input=>workspace.addNote({id:input.id??`agent-note-${crypto.randomUUID()}`,body:input.text??input.body,targetId:input.target_id??selection()?.id}),
    'tos.page.add-session-hypothesis':input=>workspace.addHypothesis({id:input.id??`agent-hypothesis-${crypto.randomUUID()}`,title:input.title,body:input.text??input.body,targetId:input.target_id??selection()?.id,kind:'thesis'}),
    'tos.page.stage-proposal':input=>workspace.stageProposal({...input,id:input.id??`agent-proposal-${crypto.randomUUID()}`,parentHypothesisId:input.parent_hypothesis_id,targetId:input.target_id??selection()?.id,sourceRefs:input.source_refs??[],evidenceRefs:input.evidence_refs??[],confidencePosture:input.confidence_posture??'source-grounded proposal',actorOrigin:'model',basePageRevision:registry.context().revision,baseWorkspaceRevision:workspace.summary().revision,dataFingerprint:index.revision}),
    'tos.page.clear-focus':()=>{experience.open(index.items.keys().next().value);return context();},
  };
  registry=createPageCommandRegistry(context,handlers);
  const schema=(properties,required=[])=>({type:'object',properties,required,additionalProperties:false});
  const str={type:'string'},integer={type:'integer',minimum:0},idSchema=schema({id:str},['id']);
  const tool=(name,commandId,description,inputSchema,bind=input=>input,readOnlyHint=false)=>({name,description,inputSchema,annotations:{readOnlyHint,untrustedContentHint:true},execute:async(input,options={})=>({content:[{type:'text',text:JSON.stringify(await registry.invoke(commandId,bind(input),{signal:options.signal}))}]})});
  const extra=[
    tool('tos.bonsai.research-item','tos.page.research-item','Read an exact subject, statement, relation or passage with its current version, dependencies and backlinks. Pass a search result ID.',idSchema,undefined,true),
    tool('tos.bonsai.research-operation','tos.page.research-operation','Run the same explicit, versioned research operation as the calculations panel. Read the input records first; the result retains its request and explanation.',schema({request:{type:'object'}},['request']),undefined,true),
    tool('tos.bonsai.read-source','tos.page.read-source','Read exact public witness words. Provide one passage item_id from research-item dependencies or its native reference. Passage results preserve selected ranges in Unicode code points. Repeat with the same item_id and nextCursor as cursor until rangeComplete is true.',schema({item_id:str,reference:{type:'object'},cursor:{type:'string',maxLength:8192},limit:{type:'integer',minimum:1,maximum:20}}),undefined,true),
    tool('tos.bonsai.session-context','tos.page.cognitive-next','Read the question, assistance level and selected subject without changing connection state.',schema({}),()=>({peek:true,timeout_ms:0}),true),
    tool('tos.bonsai.session-join','tos.page.cognitive-action','Join or resume this notebook session in one active host task. Continue reading session-next and respond to the human actions until paused.',schema({session_id:str},['session_id']),input=>({...input,action:'join'})),
    tool('tos.bonsai.session-next','tos.page.cognitive-next','Wait for the next human action (up to 20 seconds). Empty waiting result means repeat in the same active host task. After responding continue waiting. Every event binds exact question and index revisions.',schema({session_id:str,after_event_id:str,timeout_ms:{type:'integer',minimum:0,maximum:20000}}),undefined,true),
    tool('tos.bonsai.session-respond','tos.page.cognitive-action','Return an attributed question, hint, finding or feedback into the learning panel. Bind all revisions from the event; source_ids must resolve in the same research index. Repeated identical delivery is idempotent.',schema({session_id:str,event_id:str,question_revision:integer,index_revision:str,actor:str,kind:{type:'string',enum:['question','hint','finding','feedback']},body:{type:'string',maxLength:12000},source_ids:{type:'array',items:str,maxItems:32},next_step:{type:'string',maxLength:2000}},['event_id','question_revision','index_revision','body']),input=>({...input,action:'respond'})),
    tool('tos.bonsai.session-pause','tos.page.cognitive-action','Pause processing while retaining every action and response.',schema({session_id:str}),input=>({...input,action:'pause'})),
  ];
  const allowed=new Set(['tos.page.context','tos.page.cancel',...Object.keys(handlers)]);
  const adapter=createWebMCPAdapter(registry,targetDocument,allowed,extra);
  void adapter.start();
  return {registry,adapter,refresh:()=>registry.notifyStateChange(),destroy(){adapter.stop();registry.cancel();}};
}
