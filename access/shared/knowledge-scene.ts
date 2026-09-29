/** Packet-local Rust presentation rules; source objects stay in the host. */
import type {KnowledgeSceneSession} from '../deploy/cloudflare-worker/generated/tos_web_rules.js';
type Item = Record<string, unknown>;
type Runtime = {KnowledgeSceneSession: typeof KnowledgeSceneSession};
const ABSENT=0xffffffff;
let installed: Runtime | undefined;
let priority: Record<string, number>;
export function installKnowledgeSceneRules(runtime: Runtime) {
  if(typeof runtime?.KnowledgeSceneSession!=='function')throw new TypeError('Generated scene Rust rule export is incomplete');
  installed=runtime;
  priority={};
  runtime.KnowledgeSceneSession.priority_keys().split('\n').forEach((key,index)=>{
    Object.defineProperty(priority,key,{value:index,writable:true,enumerable:true,configurable:true});
  });
}
function rules() {if(!installed)throw new Error('Scene Rust rules are not installed');return installed.KnowledgeSceneSession;}
const units=(value:string)=>Uint16Array.from({length:value.length},(_,index)=>value.charCodeAt(index));
export function compareSceneIds(a:string,b:string) {return rules().compare_ids(units(a),units(b));}
function record(value: unknown): Item {
  return value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Item : {};
}
function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string' && item.length > 0) : [];
}
export function knowledgeScene(nodes:Item[],relations:Item[],focusNodeId:string|null=null,focusRelationId:string|null=null) {
  const Rule=rules(),session=new Rule();
  const keyTexts:string[]=[];
  const key=(value:string)=>{const id=session.intern(units(value));keyTexts[id]=value;return id;};
  const string=(id:number)=>keyTexts[id]!;
  const vertex=(group:number)=>string(session.group_key(group));
  // Handles refer to the exact encountered objects, including repeated objects.
  const nodeHandles:Item[]=[],relationHandles:Item[]=[],entityHandles:unknown[]=[];
  try {
    for(const node of nodes) {
      const entity=typeof node.entity_id==='string'&&node.entity_id.startsWith(Rule.entity_prefix())?node.entity_id:null;
      const id=Rule.vertex_prefix(Boolean(entity))+(entity?entity:String(node.id));
      // Preserve the second, fresh ID coercion used for the packet index.
      const nodeId=key(String(node.id));
      const handle=nodeHandles.push(node)-1;
      // `id` above owns the actual vertex spelling even if a getter mutated.
      // The original carrier key used the earlier coercion. The observation is
      // supplied separately so that physical source reads keep their old order.
      session.node_key(handle,nodeId,key(id),entityHandles.push(entity)-1);
    }
    const presentNodes=new Set(Array.from(session.node_ids(),string));
    const groups=Array.from({length:session.group_count()},(_,i)=>i)
      .sort((a,b)=>compareSceneIds(vertex(a),vertex(b)));
    const vertices=groups.map(group=>{
      const groupNodes=Array.from(session.group_nodes(group),handle=>nodeHandles[handle]!);
      const ordered=groupNodes.slice().sort((a,b)=>{
        const delta=(priority[String(a.source_graph)]??99)-(priority[String(b.source_graph)]??99);
        if(Rule.priority_delta(typeof delta==='number'?delta:Number(Boolean(delta))))return delta;
        return compareSceneIds(String(a.id),String(b.id));
      });
      const nodeIds=groupNodes.map(n=>String(n.id)).sort(compareSceneIds);
      session.group_ids(group,Uint32Array.from(nodeIds,key));
      return {id:vertex(group),entity_id:entityHandles[session.group_entity(group)],
        node_ids:nodeIds,representative_node_id:String(ordered[0]!.id)};
    });
    for(const relation of relations.slice().sort((a,b)=>compareSceneIds(String(a.id),String(b.id)))) {
      const left=session.vertex_for(key(String(relation.from_id))),right=session.vertex_for(key(String(relation.to_id)));
      // Do not read type/id on the missing endpoint refusal path.
      try{session.endpoints(left,right);}catch(error){throw new Error(String(error));}
      const projects=Rule.same_vertex(left,right)&&relation.relation_type_id===Rule.collapse_type();
      const selected=projects&&relation.id===focusRelationId;
      session.arc(key(String(relation.id)),left,right,projects,selected);
    }
    const arcs=Array.from({length:session.arc_count()},(_,i)=>({relation_id:string(session.arc_relation(i)),
      from_id:vertex(session.arc_from(i)),to_id:vertex(session.arc_to(i))}));
    // The host invokes the original collection callbacks in their original order.
    const relationRecords=new Map(relations.map(r=>[String(r.id),r] as const));
    for(const [id,relation] of relationRecords)session.relation_record(key(id),relationHandles.push(relation)-1);
    for(const relation of relations) {
      const handle=relationHandles.push(relation)-1;
      session.outgoing_record(key(String(relation.from_id)),handle);
    }
    session.index_incident();
    const claimRecords=new Map(nodes.filter(n=>n.type_id===Rule.claim_entity()||strings(record(n.semantics).type_ancestors).includes(Rule.claim_entity()))
      .map(n=>[String(n.id),n] as const));
    for(const [id,node] of claimRecords)session.claim(key(id),nodeHandles.push(node)-1);
    const claimIds=Array.from(session.claim_ids()).sort((a,b)=>compareSceneIds(string(a),string(b)));
    const candidates=new Map<number,{node:Item;claim:Item;legs:string[]}>();
    const outgoing=(id:number)=>Array.from(session.outgoing(id),handle=>relationHandles[handle]!);
    for(const id of claimIds) {
      const node=nodeHandles[session.claim_handle(id)]!;
      const claim=record(record(node.semantics).claim),subject=claim.subject_node_id,object=claim.object_node_id;
      const subjectString=typeof subject==='string',objectString=typeof object==='string';
      if(!session.claim_contract(id,subjectString,objectString,subjectString?key(subject):ABSENT,objectString?key(object):ABSENT))continue;
      const mapped=claim.predicate_mapping_status==='mapped';
      if(!session.claim_mapping(id,mapped,mapped&&Boolean(claim.relation_type_id)))continue;
      if(!session.claim_identity(id,key(subject as string),key(object as string)))continue;
      const legs=Rule.leg_types().split('\n').map(kind=>outgoing(id).filter(r=>r.relation_type_id===kind));
      // Fresh endpoint reads are conditional on the two cardinalities.
      const countValid=!legs.some(leg=>leg.length!==1);
      const subjectMatches=countValid&&legs[0]![0]!.to_id===subject;
      const objectMatches=subjectMatches&&legs[1]![0]!.to_id===object;
      if(!session.claim_legs(id,legs[0]!.length,legs[1]!.length,subjectMatches,objectMatches))continue;
      const members=claim.value_member_node_ids;
      const memberEdges=outgoing(id).filter(r=>r.relation_type_id===Rule.member_type());
      if(Object.hasOwn(claim,'value_member_node_ids')||memberEdges.length) {
        const targets=new Set(memberEdges.map(r=>String(r.to_id)));
        // Native Set/some preserve physical identity, sparse callbacks and refusal order.
        const array=Array.isArray(members);
        if(!session.member_shape(id,array,array&&Boolean(members.length)))continue;
        if(!session.member_strings(id,(members as unknown[]).some(member=>typeof member!=='string')))continue;
        if(!session.member_unique(id,new Set(members as unknown[]).size===(members as unknown[]).length))continue;
        if(!session.member_presence(id,(members as unknown[]).some(member=>!presentNodes.has(member as string)||!targets.has(member as string))))continue;
        if(!session.member_edges(id,memberEdges.length===(members as unknown[]).length))continue;
        if(!session.member_targets(id,targets.size===(members as unknown[]).length))continue;
      }
      const legIds=[String(legs[0]![0]!.id),String(legs[1]![0]!.id)];
      // Opaque source-owned claim fields are retained without JSON or clone.
      candidates.set(id,{node,claim,legs:legIds});
      session.candidate(id,Uint32Array.from(legIds,key));
    }
    session.focus(focusNodeId!==null,focusNodeId===null?ABSENT:key(focusNodeId));
    const paths:(Item&{id:string;from_id:string;to_id:string})[]=[];
    for(let index=0;index<vertices.length;index++) {
      const group=groups[index]!;
      if(!session.local_claims(group).length)continue;
      const incident=Array.from(session.incident(group));
      let reason=session.vertex_reason(group,incident.some(i=>arcs[i]!.relation_id===focusRelationId));
      if(!reason)session.index_vertex_legs(group);
      if(!reason)for(const i of incident) {
        const arc=arcs[i]!;
        if(session.is_leg(group,key(arc.relation_id)))continue;
        const relation=relationHandles[session.relation_handle(key(arc.relation_id))]!;
        const from=key(String(relation.from_id));
        // Preserve original short-circuit before the relation type coercion.
        const member=session.group_contains(group,from);
        const detail=member&&Rule.detail_type(units(String(relation.relation_type_id)));
        reason=session.incident_reason(group,from,detail,session.arc_to(i));
        if(reason)break;
      }
      if(!session.accept_vertex(group,reason))continue;
      for(const id of session.candidate_ids(group)) {
        const {node,claim,legs}=candidates.get(id)!;
        const details=outgoing(id).filter(r=>Rule.detail_type(units(String(r.relation_type_id))))
          .map(r=>String(r.id)).sort(compareSceneIds);
        session.consume(id,Uint32Array.from(details,key));
        for(const relationId of details) {
          const relation=relationHandles[session.relation_handle(key(relationId))]!;
          session.detail_vertex(key(String(relation.to_id)));
        }
        let wording:string|null=null,wordingMode=Rule.wording_mode(false);
        const roles=record(record(node.human_form_selection).roles);
        for(const role of Rule.wording_roles().split('\n'))if(record(roles[role]).state==='ready') {
          const shared=record(node.human_form_selection).schema_version===Rule.shared_schema();
          wording=Rule.wording_pointer(role,shared);
          wordingMode=Rule.wording_mode(shared);break;
        }
        if(wording===null) {
          const fields=record(record(node.display_selection).fields);
          for(const field of Rule.wording_fields().split('\n'))if(record(fields[field]).content_available===true){wording=Rule.wording_field_pointer(field);break;}
        }
        const from=session.vertex_for(key(String(claim.subject_node_id))),to=session.vertex_for(key(String(claim.object_node_id)));
        const path={id:'tos-scene:claim-path:'+string(id),from_id:from===ABSENT?undefined:vertex(from),to_id:to===ABSENT?undefined:vertex(to),
          claim_node_id:string(id),relation_type_id:claim.relation_type_id,node_ids:[claim.subject_node_id,string(id),claim.object_node_id],
          relation_ids:legs,detail_relation_ids:details,reading:{mode:wordingMode,node_id:string(id),content_revision:node.content_revision,
            wording_pointer:wording,wording_state:wording?'available':'missing',context_pointers:['/semantics','/epistemic'],relation_context_ids:[...legs,...details],standalone:false}};
        paths.push(path as Item&{id:string;from_id:string;to_id:string});
        session.path_endpoint(from);session.path_endpoint(to);
      }
    }
    session.finish_fold();
    const retainedArcs=arcs.filter((_,i)=>session.retained(i));
    const folded=Array.from(session.folded_groups(),group=>group===ABSENT?undefined:vertex(group)).sort(compareSceneIds as (a:string|undefined,b:string|undefined)=>number);
    const reasons=Array.from(session.reason_ids()).sort((a,b)=>compareSceneIds(string(a),string(b)))
      .map(id=>({node_id:string(id),reason:session.reason(id)}));
    const compact={rule:'explicit-claim-paths-v1',vertex_ids:vertices.filter((_,i)=>!session.folded(groups[i]!)).map(v=>v.id),
      relation_ids:retainedArcs.map(a=>a.relation_id),claim_paths:paths.sort((a,b)=>compareSceneIds(a.id,b.id)),folded_vertex_ids:folded,
      retained_claims:reasons,authority:'presentation-only-no-new-assertion'};
    const focus=focusNodeId===null?ABSENT:session.vertex_for(key(focusNodeId));
    return {schema_version:'tos_knowledge_scene_v1',vertices,arcs,collapsed_relation_ids:Array.from(session.collapsed(),string),compact,
      focus_vertex_id:focus===ABSENT?null:vertex(focus),scope:'returned-packet-only',identity_rule:'declared-tos-entity-id',authority:'presentation-mapping-not-semantic-admission'};
  } finally {session.free();}
}
