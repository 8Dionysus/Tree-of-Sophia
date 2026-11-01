import {createPageCommandRegistry} from '../page-commands.ts';
import {createWebMCPAdapter} from '../webmcp.ts';
import {runResearchOperation} from './research-operations.mjs';
import {localeValue} from './locale.mjs';

/** Public stands expose their existing owners through the shared page registry. */
export function connectResearchPage({index,space,experience,session,api,workspace,locale,targetDocument=document}){
  const summary=()=>workspace.summary();
  const selection=()=>{
    const item=index.get(experience.getState().id??space.selection()?.id);if(!item)return null;
    return {id:item.id,kind:item.kind==='relation'?'edge':['statement','passage'].includes(item.kind)?'item':'node',semantic_kind:item.kind,label:localeValue(item.title,locale()),source_refs:item.dependencies.filter(id=>index.get(id)?.kind==='passage'),
      from_id:item.record?.from,to_id:item.record?.to,predicate_id:item.record?.relation,authority_posture:item.owner,review_posture:item.record?.assessment?.verdict??item.record?.reading?.review_status??'source-visible-proposal',canon_status:'stand-scope',path_available:false};
  };
  const context=()=>({mode:'philosophy',view_id:`bonsai:${api.profile}`,graph_mode:'nodes',selected:selection(),path_start_node_id:null,active_layers:[experience.getState().mode],active_predicates:[],deep_link:location.href,research_workspace:summary()});
  const readSource=async(input,{signal})=>{
    if(input.reference){const resolved=await api.provider.resolve({reference:input.reference,signal});if(resolved.status!=='exact')return resolved;return api.provider.window({documentId:input.reference.target.workId,versionId:input.reference.versionId,reference:input.reference,limit:Math.min(20,input.limit??8),signal});}
    const item=index.get(input.item_id);if(item?.kind!=='passage')throw new TypeError('Choose an exact passage ID from research-item dependencies');
    const cite=item.record;
    return api.provider.window({documentId:'tos.work.friedrich-nietzsche.also-sprach-zarathustra',versionId:'dta-first-editions-de',unitId:cite.u?.[0]??undefined,limit:Math.min(20,input.limit??Math.max(4,cite.u?.length??4)),signal});
  };
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
    tool('tos.bonsai.read-source','tos.page.read-source','Read exact public witness words. Provide a passage item_id from research-item dependencies, or its native reference. Returns bounded original TextUnits and revision.',schema({item_id:str,reference:{type:'object'},limit:{type:'integer',minimum:1,maximum:20}}),undefined,true),
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
