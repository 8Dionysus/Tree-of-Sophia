import {KnowledgeClient,ContractError,RevisionError,RequestSlots,checkRevision,claimMaterialReference,materialVersions,isSourceDossierRef,SOURCE_DOSSIER_LIMIT} from './knowledge-client.mjs';
import {ExplorationSceneCache} from './exploration-cache.mjs';
import {validateHumanForms,claimPathFor} from './human-forms.mjs';
import {readingSnapshot} from './reader-model.mjs';
import {nativeStrip,nativeLower} from '../../../shared/native-unicode.ts';
import {chooseKnowledgeSearchMode} from '../knowledge-search.ts';
import {readExactSource} from './exact-source-read.mjs';

const hash=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const strings=value=>Array.isArray(value)&&value.every(item=>typeof item==='string'&&item.length>0);
const readonly=value=>value?.writes_to_tree===false&&value?.is_source===false&&value?.is_canon===false;
const searchSchemas=Object.freeze({indexed:'tos_knowledge_search_indexed_v2',compressed:'tos_knowledge_search_compressed_v3'});
function immutable(value){
  if(value&&typeof value==='object'&&!Object.isFrozen(value)){
    Object.values(value).forEach(immutable);Object.freeze(value);
  }return value;
}
function requireContract(value){if(!value)throw new ContractError('The backend did not provide the required exploration contract.');}
function selectSearchMode(search,query){
  try{return chooseKnowledgeSearchMode(search,undefined,query);}
  catch(error){throw new ContractError(error?.message||'The backend did not provide a usable knowledge search engine.');}
}
function searchDescriptor(search,mode){
  const descriptor=search?.modes?.[mode];
  requireContract(object(descriptor)&&descriptor.available===true&&descriptor.schema===searchSchemas[mode]);
  return descriptor;
}
function selectedSearch(search,sourceRevision,query){
  const mode=selectSearchMode(search,query);
  const descriptor=searchDescriptor(search,mode);
  if(sourceRevision!==undefined&&descriptor.source_revision!==undefined)checkRevision(descriptor,sourceRevision);
  return {mode,descriptor};
}

export function bindExplorationDiscovery(catalog,exploration,search){
  checkRevision(catalog);
  requireContract(catalog.schema==='tos_knowledge_catalog_v1'&&readonly(catalog.authority_boundary)
    &&strings(catalog.capabilities?.sources)&&catalog.capabilities.sources.length>0
    &&Array.isArray(catalog.capabilities.neighborhood_profiles)
    &&catalog.capabilities.neighborhood_profiles.every(item=>object(item)&&typeof item.profile==='string'&&typeof item.definition==='string')
    &&Array.isArray(catalog.predicates)&&catalog.predicates.every(item=>object(item)&&typeof item.predicate_id==='string')
    &&exploration?.schema==='tos_exploration_capabilities_v1'&&exploration.available===true
    &&exploration.writes_to_tree===false&&exploration.request_versions?.includes('tos_exploration_request_v2')
    &&exploration.result_versions?.includes('tos_exploration_result_v2')
    &&['node','relation'].every(kind=>exploration.v2_origin_kinds?.includes(kind))
    &&search?.schema==='tos_knowledge_search_capabilities_v1'&&search.writes_to_tree===false);
  const {mode:searchMode,descriptor}=selectedSearch(search,catalog.source_revision),schema=descriptor.schema;
  if(exploration.source_revision!==undefined)checkRevision(exploration,catalog.source_revision);
  return immutable(structuredClone({catalog,exploration,search,searchMode,searchSchema:schema}));
}

// These are transport/query defaults, never a subject-specific taxonomy. The
// available sources, predicates and profiles come from the backend's catalog.
export function explorationRequest(discovery,target,options={}){
  requireContract(discovery&&object(target)&&['node','relation'].includes(target.kind)
    &&typeof target.id==='string'&&target.id.length>0&&target.id.length<=1024&&hash(target.content_revision)
    &&object(options)&&Object.keys(options).every(key=>['profile','direction','max_depth','page_nodes','page_relations','sources','predicate_ids'].includes(key)));
  const query={schema_version:'tos_exploration_request_v2',source_revision:discovery.catalog.source_revision,
    origin:{kind:target.kind,id:target.id,content_revision:target.content_revision},
    profile:'overview',direction:'either',max_depth:2,page_nodes:12,page_relations:18,
    sources:[...discovery.catalog.capabilities.sources],predicate_ids:[],...structuredClone(options)};
  requireContract(discovery.catalog.capabilities.neighborhood_profiles.some(item=>item.profile===query.profile)
    &&['either','incoming','outgoing'].includes(query.direction));
  for(const [name,minimum,maximum] of [['max_depth',0,10],['page_nodes',1,40],['page_relations',1,80]]){
    requireContract(Number.isSafeInteger(query[name])&&query[name]>=minimum&&query[name]<=maximum
      &&query[name]<=discovery.exploration.limits?.[name==='max_depth'?'depth':name]);
  }
  for(const [name,allowed] of [['sources',discovery.catalog.capabilities.sources],['predicate_ids',discovery.catalog.predicates.map(item=>item.predicate_id)]]){
    requireContract(strings(query[name])&&(name!=='sources'||query[name].length>0)&&query[name].length<=100
      &&new Set(query[name]).size===query[name].length&&query[name].every(id=>allowed.includes(id)));
  }
  return query;
}

export function validateExplorationSearch(packet,discovery,query,cursor=null){
  const {mode:searchMode,descriptor}=selectedSearch(discovery.search,discovery.catalog.source_revision,query);
  checkRevision(packet,discovery.catalog.source_revision);
  requireContract(packet.schema===descriptor.schema&&packet.query===(searchMode==='compressed'?nativeStrip(query):query)
    &&readonly(packet.authority_boundary)&&packet.page?.cursor===cursor
    &&packet.page.limit_per_kind===6&&typeof packet.page.has_more==='boolean'
    &&(packet.page.has_more?typeof packet.page.next_cursor==='string'&&packet.page.next_cursor.length>0:packet.page.next_cursor===null));
  for(const kind of ['nodes','relations']){
    const rows=packet[kind];requireContract(Array.isArray(rows)&&rows.length<=6);
    const ids=new Set();
    for(const row of rows){
      requireContract(object(row)&&typeof row.id==='string'&&row.id.length>0&&!ids.has(row.id)&&hash(row.content_revision)
        &&object(row.display)&&strings(row.source_refs)&&row.source_refs.length>0);
      ids.add(row.id);validateHumanForms(row);
    }
  }
  return packet;
}

// A single bounded browser session. Discovery/search/card requests never mutate
// the scene. Only a current, fully validated page enters its immutable cache.
// This adapter has no source command or mutation endpoint.
export class ExplorationSession {
  #client;#cache;#slots=new RequestSlots();#discovery=null;#sceneRequest=null;#ticket=null;#disposed=false;#selectionEpoch=0;
  constructor({client=new KnowledgeClient(),cache=new ExplorationSceneCache()}={}){this.#client=client;this.#cache=cache;}
  discovery(){return this.#discovery;}
  snapshot(){return this.#cache.snapshot();}
  async discover(){
    if(this.#disposed)return null;
    const result=await this.#slots.run('discovery',async signal=>{
      const [catalog,exploration,search]=await Promise.all([
        this.#client.request('/catalog',{signal}),this.#client.request('/explore/capabilities',{signal}),
        this.#client.request('/search/capabilities',{signal})]);
      return bindExplorationDiscovery(catalog,exploration,search);
    });
    if(!result.current||this.#disposed)return null;
    this.#discovery=result.value;return result.value;
  }
  async search(query,{cursor=null}={}){
    if(this.#disposed)return null;
    requireContract(this.#discovery&&typeof query==='string'&&nativeStrip(query).length>0&&Array.from(query).length<=256
      &&Array.from(nativeLower(nativeStrip(query))).length<=256
      &&(cursor===null||typeof cursor==='string'&&cursor.length<=65536));
    const discovery=this.#discovery;
    const result=await this.#slots.run('search',async signal=>{
      const {mode:searchMode}=selectedSearch(discovery.search,discovery.catalog.source_revision,query);
      const params=new URLSearchParams({mode:searchMode,query,limit:'6',...(cursor?{cursor}:{})});
      return validateExplorationSearch(await this.#client.request('/search?'+params,{signal}),discovery,query,cursor);
    });
    return result.current&&!this.#disposed?immutable(structuredClone(result.value)):null;
  }
  cancelSearch(){this.#slots.cancel('search');}
  async sourceDossier(objectId,{limit=SOURCE_DOSSIER_LIMIT}={}){
    if(this.#disposed)return null;
    requireContract(isSourceDossierRef(objectId)&&Number.isSafeInteger(limit)&&limit>=1&&limit<=SOURCE_DOSSIER_LIMIT);
    const result=await this.#slots.run('source-dossier',signal=>this.#client.sourceDossier(objectId,signal,{limit}));
    return result.current&&!this.#disposed?immutable(structuredClone(result.value)):null;
  }
  cancelSourceDossier(){this.#slots.cancel('source-dossier');}
  async sourceRecord(reading,{representation='record'}={}){
    if(this.#disposed)return null;
    const view=this.snapshot(),selectionEpoch=this.#selectionEpoch;
    requireContract(view&&object(reading)&&['node','relation'].includes(reading.kind)&&object(reading.raw));
    if(reading.sourceRevision!==view.source_revision)throw new RevisionError();
    const rows=view[reading.kind==='node'?'nodes':'relations'];
    const raw=rows.find(row=>row.id===reading.raw.id);
    if(!raw||raw.content_revision!==reading.raw.content_revision)throw new RevisionError();
    const selected={kind:reading.kind,id:raw.id,source_revision:view.source_revision,content_revision:raw.content_revision};
    const result=await this.#slots.run('source-record',signal=>readExactSource(this.#client,selected,{signal,representation}));
    if(!result.current||this.#disposed||selectionEpoch!==this.#selectionEpoch||this.snapshot()?.source_revision!==view.source_revision)return null;
    const retained=this.snapshot()[reading.kind==='node'?'nodes':'relations'].find(row=>row.id===raw.id);
    if(retained?.content_revision!==raw.content_revision)return null;
    return immutable(structuredClone(result.value));
  }
  cancelSourceRecord(){this.#slots.cancel('source-record');}
  cancelScene(){
    this.#sceneRequest?.abort();this.#sceneRequest=null;
    if(this.#ticket)this.#cache.cancel(this.#ticket);this.#ticket=null;
  }
  async #scene(work){
    if(this.#disposed)return null;
    this.cancelScene();const controller=new AbortController();this.#sceneRequest=controller;
    const current=()=>this.#sceneRequest===controller&&!controller.signal.aborted&&!this.#disposed;
    try{return await work(controller.signal,current);}
    catch(error){if(!current())return null;throw error;}
    finally{if(this.#sceneRequest===controller)this.#sceneRequest=null;}
  }
  #accept(ticket,packet){
    const previous=this.snapshot(),next=this.#cache.accept(ticket,packet);
    if(JSON.stringify(previous?.selection)!==JSON.stringify(next.selection))this.#changedSelection();
    return next;
  }
  #changedSelection(){this.#selectionEpoch++;this.#slots.cancel('inspect');this.cancelSourceRecord();}
  async open(target,{replace=false,selectOrigin=true,options={}}={}){
    requireContract(this.#discovery&&object(target)&&['node','relation'].includes(target.kind)
      &&typeof target.id==='string'&&target.id.length>0&&typeof replace==='boolean'&&typeof selectOrigin==='boolean');
    const discovery=this.#discovery,owned=structuredClone(target),ownedOptions=structuredClone(options);
    return this.#scene(async(signal,current)=>{
      // URL navigation may carry only an exact API ID, not a guessed entity
      // alias. Inspection resolves its current revision before the v2 request.
      let exact=owned;
      if(exact.content_revision===undefined){
        const result=await this.#client.inspect(exact.kind,exact.id,signal,discovery.catalog.source_revision);
        if(!current())return null;exact={...exact,content_revision:result.match.content_revision};
      }
      const query=explorationRequest(discovery,exact,ownedOptions);
      if(!current())return null;
      const ticket=this.#cache.begin(query,{replace,selectOrigin});this.#ticket=ticket;
      const packet=await this.#client.request('/explore',{signal,body:query});
      return current()?this.#accept(ticket,packet):null;
    });
  }
  async continue(){
    return this.#scene(async(signal,current)=>{
      const ticket=this.#cache.resume();this.#ticket=ticket;
      const query=this.#cache.nextRequest(ticket);requireContract(query!==null);
      const packet=await this.#client.request('/explore',{signal,body:query});
      return current()?this.#accept(ticket,packet):null;
    });
  }
  select(target){
    if(this.#disposed)throw new RevisionError();
    const next=this.#cache.select(target);this.#changedSelection();return next;
  }
  async inspect(target,{language='ru',slot='inspect'}={}){
    if(this.#disposed)return null;
    const view=this.snapshot(),selectionEpoch=this.#selectionEpoch;requireContract(view&&object(target)&&['inspect','compare-left','compare-right'].includes(slot));
    const kind=target.kind==='relation'?'relation':'node';
    const path=target.kind==='claim-path'?view.scene.compact.claim_paths.find(item=>item.id===target.id):
      kind==='node'?claimPathFor(view,target.id):null;
    const id=path?.claim_node_id??target.id,raw=view[kind==='node'?'nodes':'relations'].find(item=>item.id===id);
    requireContract(raw&&['node','relation','claim-path'].includes(target.kind)&&(target.kind!=='claim-path'||path));
    const result=await this.#slots.run(slot,async signal=>{
      const material=path?await this.#client.readClaimReference(claimMaterialReference(view,path),signal,
        {language,expected:view.source_revision,versions:materialVersions(view)}):
        await this.#client.readMaterial(kind,id,signal,view.source_revision,raw.content_revision,{language,relation:kind==='relation'?raw:null});
      // Full inspection has a source revision, not the exploration's snapshot
      // token. Verify every returned carrier against the retained exact closure.
      const versions=materialVersions(view);
      for(const name of ['nodes','relations']){
        requireContract(Array.isArray(material.packet[name]));
        for(const item of material.packet[name])if(item.content_revision!==versions[name][item.id])throw new RevisionError();
      }
      return readingSnapshot(material,kind);
    });
    if(!result.current||this.#disposed||this.snapshot()?.source_revision!==view.source_revision
      ||this.snapshot()?.snapshot_revision!==view.snapshot_revision||(slot==='inspect'&&selectionEpoch!==this.#selectionEpoch))return null;
    // Explicit same-publication replacement can drop a former target without
    // changing R/S. It cannot revive an inspection of an absent closure.
    const current=materialVersions(this.snapshot()),required=path?claimMaterialReference(view,path).closureNodeIds:
      kind==='relation'?[raw.from_id,raw.to_id]:[raw.id];
    if(required.some(nodeId=>current.nodes[nodeId]!==view.nodes.find(item=>item.id===nodeId)?.content_revision)
      ||(kind==='relation'&&current.relations[raw.id]!==raw.content_revision)
      ||(path&&[...path.relation_ids,...path.detail_relation_ids].some(relationId=>current.relations[relationId]!==view.relations.find(item=>item.id===relationId)?.content_revision)))return null;
    return immutable(structuredClone(result.value));
  }
  cancelInspect(slot){
    const slots=['inspect','compare-left','compare-right'];
    requireContract(slot===undefined||slots.includes(slot));
    for(const name of slot===undefined?slots:[slot])this.#slots.cancel(name);
  }
  dispose(){this.#disposed=true;this.cancelScene();this.#slots.cancelAll();}
}
