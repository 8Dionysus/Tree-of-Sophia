import {knowledgeScene} from '../../../shared/knowledge-scene.ts';
import {validateExploration,ContractError,RevisionError,sameJson,explorationRequestMatches} from './knowledge-client.mjs';
import {validateHumanForms} from './human-forms.mjs';

// A bounded browser reading session, not corpus membership or a new LensResult.
// Removing a page from a response never means deleting a ToS object.
export const EXPLORATION_CACHE_LIMITS=Object.freeze({nodes:200,relations:600,contexts:32,pagesPerContext:128,bytes:4*1024*1024});
export class SceneCapacityError extends ContractError {
  constructor(){super('The reading space reached its bounded capacity. Keep it or explicitly open a new space.');this.name='SceneCapacityError';}
}
const revision=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const bytes=value=>new TextEncoder().encode(JSON.stringify(value)).byteLength;
function freeze(value){
  if(value&&typeof value==='object'&&!Object.isFrozen(value)){
    for(const item of Object.values(value))freeze(item);Object.freeze(value);
  }
  return value;
}
function originTarget(origin){return {kind:origin.kind,id:origin.id};}
function focusFor(selection,origin,nodes,relations){
  const target=selection??originTarget(origin);
  if(target.kind==='relation'){
    const relation=relations.get(target.id);
    if(!relation)throw new ContractError('The selected relation is absent from the reading space.');
    return {node:relation.from_id,relation:relation.id};
  }
  const id=target.kind==='claim-path'?target.claimId:target.id;
  if(!nodes.has(id))throw new ContractError('The selected object is absent from the reading space.');
  return {node:id,relation:null};
}
function mergeRows(oldRows,newRows){
  const merged=new Map(oldRows);
  for(const row of newRows){
    const prior=merged.get(row.id);
    // Within an exact publication, even an unchanged advertised digest cannot
    // authorize different bytes, wording or source-return provenance.
    if(prior&&!sameJson(prior,row))throw new RevisionError();
    if(!prior)merged.set(row.id,row);
  }
  return merged;
}
function mergeReasons(previous,current,page){
  const result=new Map(Object.entries(previous??{}));
  for(const [id,reason] of Object.entries(current)){
    const old=result.get(id)??[];
    // A later context-endpoint must not erase the earlier discovery path.
    // Preserve distinct execution reasons with their first observed page.
    result.set(id,old.some(item=>sameJson(item.reason,reason))?old:[...old,{page,reason}]);
  }
  return Object.fromEntries(result);
}

export class ExplorationSceneCache {
  #limits;#generation=0;#pending=null;#accepted=null;#state=null;#nodes=new Map();#relations=new Map();
  constructor({limits={}}={}){
    if(!limits||typeof limits!=='object'||Array.isArray(limits)
      ||Object.keys(limits).some(key=>!Object.hasOwn(EXPLORATION_CACHE_LIMITS,key)))throw new TypeError('Unknown scene cache limit.');
    this.#limits={...EXPLORATION_CACHE_LIMITS,...limits};
    for(const [key,value] of Object.entries(this.#limits))if(!Number.isSafeInteger(value)||value<1||value>EXPLORATION_CACHE_LIMITS[key])throw new TypeError('Scene cache limits can only narrow the owner bounds.');
  }
  snapshot(){return this.#state;}
  // A failed/late request leaves the last good state intact. A new query may
  // reuse same-publication carriers; a different publication needs explicit
  // replacement, accepted atomically with its first valid page.
  begin(request,{replace=false,selectOrigin=true}={}){
    if(request?.schema_version!=='tos_exploration_request_v2'||!revision(request.source_revision)
      ||!['node','relation'].includes(request.origin?.kind)||!revision(request.origin?.content_revision)
      ||typeof request.origin.id!=='string'||!request.origin.id||Object.hasOwn(request,'cursor')
      ||typeof replace!=='boolean'||typeof selectOrigin!=='boolean')throw new ContractError('Invalid exact exploration request.');
    const owned=structuredClone(request);
    if(bytes(owned)>16384)throw new SceneCapacityError();
    if(this.#state&&!replace&&owned.source_revision!==this.#state.source_revision)throw new RevisionError();
    const ticket=Object.freeze({generation:++this.#generation});
    this.#pending={ticket,contextId:ticket.generation,request:freeze(owned),replace,selectOrigin,previous:null,context:null};
    return ticket;
  }
  cancel(ticket){if(this.#pending?.ticket===ticket)this.#pending=null;}
  current(ticket){return this.#pending?.ticket===ticket;}
  // Failed replacement preserves both the visible last-good space and its
  // accepted continuation. A fresh ticket prevents an old ignored-abort
  // response from becoming current again when the user resumes that space.
  resume(){
    if(!this.#accepted||this.#accepted.previous?.status!=='paused')throw new ContractError('No accepted continuation remains.');
    const ticket=Object.freeze({generation:++this.#generation});
    this.#pending={...this.#accepted,ticket};return ticket;
  }
  nextRequest(ticket){
    if(!this.current(ticket))throw new RevisionError();
    const cursor=this.#pending.previous?.page.next_cursor;
    return cursor?{cursor}:null;
  }
  accept(ticket,input){
    if(!this.current(ticket))throw new RevisionError();
    const pending=this.#pending;
    if(bytes(input)>this.#limits.bytes)throw new SceneCapacityError();
    const packet=structuredClone(input);
    if(pending.previous&&sameJson(packet,pending.previous))return this.#state;
    validateExploration(packet,pending.request.source_revision,pending.previous);
    if(packet.schema!=='tos_exploration_result_v2')throw new ContractError('Only exact-origin pages enter this reading space.');
    if(!pending.previous&&(packet.page.number!==1||!explorationRequestMatches(packet.query,pending.request)))throw new RevisionError();
    const retaining=this.#state&&(!pending.replace||pending.previous!==null);
    if(retaining&&(packet.source_revision!==this.#state.source_revision||packet.snapshot_revision!==this.#state.snapshot_revision
      ||packet.execution_version!==this.#state.execution_version))throw new RevisionError();
    for(const item of [...packet.nodes,...packet.relations])validateHumanForms(item);
    const nodes=mergeRows(retaining?this.#nodes:[],packet.nodes),relations=mergeRows(retaining?this.#relations:[],packet.relations);
    if(nodes.size>this.#limits.nodes||relations.size>this.#limits.relations)throw new SceneCapacityError();
    const contexts=retaining?[...this.#state.contexts]:[];
    const oldContext=pending.context;
    const context={id:pending.contextId,query:packet.query,origin:packet.origin,
      pages:(oldContext?.pages??0)+1,status:packet.status,limit_reason:packet.limit_reason,
      // Reasons belong to this query, not to the object or to semantic truth.
      inclusion:{nodes:mergeReasons(oldContext?.inclusion.nodes,packet.inclusion.nodes,packet.page.number),
        relations:mergeReasons(oldContext?.inclusion.relations,packet.inclusion.relations,packet.page.number)}};
    if(oldContext)contexts[contexts.findIndex(item=>item.id===context.id)]=context;else contexts.push(context);
    if(contexts.length>this.#limits.contexts||context.pages>this.#limits.pagesPerContext)throw new SceneCapacityError();
    let selection=retaining?this.#state.selection:null;
    if(!pending.previous&&(pending.selectOrigin||!selection))selection=originTarget(packet.origin);
    const focus=focusFor(selection,packet.origin,nodes,relations);
    const scene=knowledgeScene([...nodes.values()],[...relations.values()],focus.node,focus.relation);
    if(selection?.kind==='claim-path'&&!scene.compact.claim_paths.some(path=>path.id===selection.id&&path.claim_node_id===selection.claimId))throw new ContractError('The selected Claim path changed; inspect its exact Claim before replacing the view.');
    const next={schema:'tos_browser_exploration_view_v1',source_revision:packet.source_revision,
      snapshot_revision:packet.snapshot_revision,execution_version:packet.execution_version,
      nodes:[...nodes.values()],relations:[...relations.values()],scene,selection,origin:packet.origin,contexts,
      continuation:{generation:ticket.generation,page:packet.page.number,status:packet.status,next_cursor:packet.page.next_cursor},
      scope:'bounded-retained-exploration-carriers',writes_to_tree:false,
      authority_boundary:packet.authority_boundary};
    // Include the replay/continuation packet as retained work, not just the
    // visible node counts. No eviction or partial acceptance on overflow.
    if(bytes({view:next,replay:packet})>this.#limits.bytes)throw new SceneCapacityError();
    freeze(packet);freeze(next);
    this.#nodes=nodes;this.#relations=relations;this.#state=next;
    pending.previous=packet;pending.context=context;this.#accepted={...pending};
    return next;
  }
  select(target){
    if(!this.#state||!target||!['node','relation','claim-path'].includes(target.kind))throw new ContractError('No exact scene target.');
    let selection;
    if(target.kind==='claim-path'){
      const path=this.#state.scene.compact.claim_paths.find(path=>path.id===target.id);
      if(!path)throw new ContractError('Unknown Claim path.');
      selection={kind:'claim-path',id:path.id,claimId:path.claim_node_id};
    }else{
      if(!(target.kind==='node'?this.#nodes:this.#relations).has(target.id))throw new ContractError('Unknown exact scene target.');
      selection={kind:target.kind,id:target.id};
    }
    const focus=focusFor(selection,this.#state.origin,this.#nodes,this.#relations);
    const scene=knowledgeScene(this.#state.nodes,this.#state.relations,focus.node,focus.relation);
    const next={...this.#state,selection,scene};
    if(bytes({view:next,replay:this.#accepted?.previous})>this.#limits.bytes)throw new SceneCapacityError();
    this.#state=freeze(next);return next;
  }
}
