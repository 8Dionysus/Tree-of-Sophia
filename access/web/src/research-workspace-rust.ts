/** Browser storage and event adapter for the shared Rust research machine. */
import {type ResearchWorkspaceOptions, type ResearchWorkspaceState,
  type ResearchWorkspaceSummary, type WorkspaceListener, type WorkspaceSelection,
  type ResearchProposalInput, type ResearchProposal, type ResearchHypothesis,
  type RouteSnapshot} from './research-workspace.ts';

type Machine = {
  import_packet(packet:string):void;
  apply(command:Uint8Array):Uint8Array;
  state_packet():Uint8Array;
  summary_packet():Uint8Array;
  export_packet():Uint8Array;
  comparable_routes_ready():boolean;
  can_undo():boolean;
  can_redo():boolean;
  undo():boolean;
  redo():boolean;
  clear_history():void;
  free():void;
};
type MachineConstructor = new(sessionId:string,historyLimit:number)=>Machine;
let browserMachine:MachineConstructor|undefined;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const parse=<T>(bytes:Uint8Array):T=>JSON.parse(decoder.decode(bytes)) as T;
const packet=(value:unknown):Uint8Array=>encoder.encode(JSON.stringify(value));
const trim=(value:string|undefined)=>value?.trim()||undefined;
const optional=(name:string,value:string|undefined)=>value?{[name]:value}:{};

/** Installed by the browser entry after the exact generated WASM has initialized. */
export function installBrowserWorkspaceMachine(machine:MachineConstructor):void {browserMachine=machine;}

function fromPacket(value:Record<string,any>):ResearchWorkspaceState {
  return {
    schema:value.schema,version:value.version,sessionId:value.session_id,revision:value.revision,
    selectedLens:value.selected_lens,excludedEdgeIds:value.excluded_edge_ids,
    routeSnapshots:value.route_snapshots.map((item:any)=>({id:item.id,label:item.label,fromId:item.from_id,toId:item.to_id,nodeIds:item.node_ids,edgeIds:item.edge_ids})),
    hypotheses:value.hypotheses.map((item:any)=>({id:item.id,title:item.title,body:item.body,
      ...optional('targetId',item.target_id),...optional('fromId',item.from_id),...optional('toId',item.to_id),
      ...optional('predicateLabel',item.predicate_label),posture:item.posture})),
    proposals:value.proposals.map(proposalFromPacket),
    notes:value.notes.map((item:any)=>({id:item.id,body:item.body,...optional('targetId',item.target_id)})),
    journal:value.journal.map((item:any)=>({sequence:item.sequence,action:item.action,
      ...optional('targetId',item.target_id),...(item.detail?{detail:item.detail}:{})})),
  };
}
function proposalFromPacket(item:any):ResearchProposal {
  return {id:item.id,kind:item.kind,parentHypothesisId:item.parent_hypothesis_id,
    ...optional('targetId',item.target_id),...optional('fromId',item.from_id),...optional('toId',item.to_id),
    statement:item.statement,sourceRefs:item.source_refs,evidenceRefs:item.evidence_refs,
    confidencePosture:item.confidence_posture,actorOrigin:item.actor_origin,
    basePageRevision:item.base_page_revision,baseWorkspaceRevision:item.base_workspace_revision,
    dataFingerprint:item.data_fingerprint,createdAt:item.created_at,digest:item.digest,
    localOnly:true,reviewStatus:item.review_status,
    ...(item.review_requirement?{reviewRequirement:item.review_requirement}:{}),canon:false};
}

export function createBrowserResearchWorkspace(options:ResearchWorkspaceOptions={}) {
  if(!browserMachine)throw new Error('research workspace WASM session is unavailable');
  const machine=new browserMachine(options.sessionId??'local',options.historyLimit??50);
  const persistence=options.persistence===false?null:options.persistence;
  const listeners=new Set<WorkspaceListener>();let persistenceError:string|null=null,disposed=false;
  // Cache only the last Rust-emitted projection. Classic rendering asks for the
  // same state repeatedly; it must not re-serialize the WASM machine each time.
  let currentPacket=decoder.decode(machine.export_packet());
  let currentState=fromPacket(JSON.parse(currentPacket));
  const refresh=()=>{const nextPacket=decoder.decode(machine.export_packet());const nextState=fromPacket(JSON.parse(nextPacket));
    currentPacket=nextPacket;currentState=nextState;};
  const exportPacket=()=>currentPacket;
  const getState=():ResearchWorkspaceState=>structuredClone(currentState);
  const summary=():ResearchWorkspaceSummary=>parse(machine.summary_packet());
  const persist=(text:string)=>{if(!persistence)return;try{persistence.save(text);persistenceError=null;}
    catch(error){persistenceError=error instanceof Error?error.message:String(error);}};
  const changed=()=>{refresh();persist(currentPacket);
    // Maintained API returns a separate snapshot from the one sent to listeners.
    const result=getState(),notified=listeners.size?getState():null;
    if(notified)for(const listener of listeners)listener(notified);return result;};
  const apply=(kind:string,argument:string,value:unknown)=>{
    const result=parse<{changed:boolean;value:any}>(machine.apply(packet({kind,[argument]:value})));
    return {...result,state:result.changed?changed():getState()};
  };
  if(persistence){try{const saved=persistence.load();if(saved){machine.import_packet(saved);refresh();}}
    catch(error){persistenceError=error instanceof Error?error.message:String(error);}}
  return {
    getState,summary,exportPacket,
    persistenceEnabled:()=>persistence!==null,persistenceError:()=>persistenceError,
    subscribe:(listener:WorkspaceListener)=>{listeners.add(listener);return ()=>listeners.delete(listener);},
    importPacket:(value:string)=>{machine.import_packet(value);changed();return true;},
    selectLens:(value:WorkspaceSelection|null)=>apply('lens.select','selection',value&&{id:value.id.trim(),kind:value.kind,
      ...optional('label',trim(value.label))}).state,
    excludeEdge:(id:string)=>apply('edge.exclude','edge_id',id.trim()).state,
    includeEdge:(id:string)=>apply('edge.include','edge_id',id.trim()).state,
    addHypothesis:(input:Omit<ResearchHypothesis,'posture'>):ResearchHypothesis=>{
      const value=apply('hypothesis.add','hypothesis',{id:input.id.trim(),title:input.title.trim(),body:input.body.trim(),
        ...optional('target_id',trim(input.targetId)),...optional('from_id',trim(input.fromId)),
        ...optional('to_id',trim(input.toId)),...optional('predicate_label',trim(input.predicateLabel)),
        posture:{session_hypothesis:true,source:false,reviewed:false,canon:false}}).value;
      return fromPacket({schema:'tos_research_workspace_session_v1',version:1,session_id:'',revision:0,
        selected_lens:null,excluded_edge_ids:[],route_snapshots:[],hypotheses:[value],proposals:[],notes:[],journal:[]}).hypotheses[0];
    },
    removeHypothesis:(id:string)=>apply('hypothesis.remove','id',id.trim()).state,
    saveRouteSnapshot:(route:RouteSnapshot)=>apply('route.snapshot','route',{id:route.id.trim(),label:route.label.trim(),
      from_id:route.fromId.trim(),to_id:route.toId.trim(),node_ids:route.nodeIds.map(id=>id.trim()),
      edge_ids:route.edgeIds.map(id=>id.trim())}).state,
    removeRouteSnapshot:(id:string)=>apply('route.remove','id',id.trim()).state,
    addNote:(input:{id:string;body:string;targetId?:string})=>apply('note.add','note',{id:input.id.trim(),body:input.body.trim(),
      ...optional('target_id',trim(input.targetId))}).state,
    updateNote:(input:{id:string;body:string;targetId?:string})=>apply('note.update','note',{id:input.id.trim(),body:input.body.trim(),
      ...optional('target_id',trim(input.targetId))}).state,
    removeNote:(id:string)=>apply('note.remove','id',id.trim()).state,
    stageProposal:(input:ResearchProposalInput):ResearchProposal=>{
      const value=apply('proposal.stage','proposal',{id:input.id.trim(),kind:input.kind,parent_hypothesis_id:input.parentHypothesisId.trim(),
        ...optional('target_id',trim(input.targetId)),...optional('from_id',trim(input.fromId)),...optional('to_id',trim(input.toId)),
        statement:input.statement.trim(),source_refs:input.sourceRefs.map(ref=>ref.trim()),
        evidence_refs:input.evidenceRefs.map(ref=>ref.trim()),confidence_posture:input.confidencePosture,
        actor_origin:input.actorOrigin,base_page_revision:input.basePageRevision,
        base_workspace_revision:input.baseWorkspaceRevision,data_fingerprint:input.dataFingerprint.trim(),
        created_at:input.createdAt??new Date().toISOString(),local_only:true,review_status:'pending_review',
        review_requirement:'human_or_authorized_agent',canon:false}).value;
      return proposalFromPacket(value);
    },
    canUndo:()=>machine.can_undo(),canRedo:()=>machine.can_redo(),
    undo:()=>{const result=machine.undo();if(result)changed();return result;},
    redo:()=>{const result=machine.redo();if(result)changed();return result;},
    clearHistory:()=>machine.clear_history(),
    comparableRoutesReady:()=>machine.comparable_routes_ready(),
    dispose:()=>{if(disposed)return;disposed=true;listeners.clear();machine.free();},
  };
}
