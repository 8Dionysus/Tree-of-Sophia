import {knowledgeScene} from '../../../shared/knowledge-scene.ts';
// Synthetic transport fixtures, never historical or source-admitted material.
export const R='a'.repeat(64),S='b'.repeat(64),C='c'.repeat(64);
export function node(id,entity='tos.agent.fixture'){
  return {id,entity_id:entity,source_graph:'source-claims',content_revision:C,
    source_refs:['ToS/fixture/agent.json'],display:{title:{ru:'Фиктивный участник'},kind_label:{ru:'Участник'}}};
}
export function pageFixture(kind='node'){
  const nodes=[node('source-claims:one'),node('source-claims:two','tos.agent.other')];
  const relations=[{id:'opaque:relation',from_id:nodes[0].id,to_id:nodes[1].id,
    relation_type_id:'tos.relation.related-to',content_revision:C,source_refs:['ToS/fixture/relation.json'],display:{label:{ru:'Учебное отношение'}}}];
  const origin={kind,id:(kind==='node'?nodes:relations)[0].id,content_revision:C};
  const query={schema_version:'tos_exploration_request_v2',source_revision:R,origin:structuredClone(origin),
    direction:'either',profile:'overview',max_depth:2,page_nodes:8,page_relations:8,predicate_ids:[],sources:['source-claims']};
  if(kind==='relation')origin.endpoints=Object.fromEntries(['from','to'].map((side,index)=>
    [side,{node_id:nodes[index].id,entity_id:nodes[index].entity_id,content_revision:C}]));
  return {schema:'tos_exploration_result_v2',execution_version:'tos-exploration-execution-v6',
    source_revision:R,snapshot_revision:S,origin,query,nodes,relations,status:'paused',limit_reason:null,
    page:{number:1,primary_node_ids:kind==='node'?[nodes[1].id]:[],context_node_ids:kind==='node'?[nodes[0].id]:nodes.map(n=>n.id),
      primary_relation_ids:kind==='node'?[relations[0].id]:[],context_relation_ids:kind==='node'?[]:[relations[0].id],
      next_cursor:'d'.repeat(64),returned_nodes:2,returned_relations:1,scope:'resumable-neighborhood',work_units:3},
    counts:{scope:'cumulative-discovered-not-global-total',discovered_nodes:2,emitted_relations:1},
    inclusion:{authority:'query-execution-not-semantic-proof',nodes:Object.fromEntries(nodes.map((n,index)=>
      [n.id,{kind:kind==='relation'?'origin-endpoint':index===0?'origin':'context-endpoint'}])),
      relations:{[relations[0].id]:{kind:kind==='relation'?'origin':'incident'}}},
    scene:knowledgeScene(nodes,relations,kind==='node'?nodes[0].id:null,kind==='relation'?relations[0].id:null),
    authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},writes_to_tree:false};
}

export function secondPage(first){
  const next=structuredClone(first);
  next.nodes[1]=node('source-claims:third','tos.agent.third');
  next.relations[0].id='opaque:second-relation';next.relations[0].to_id=next.nodes[1].id;
  next.page.number=2;next.page.next_cursor='e'.repeat(64);next.page.primary_node_ids=[next.nodes[1].id];next.page.primary_relation_ids=[next.relations[0].id];
  next.inclusion.nodes={[next.nodes[0].id]:{kind:'origin'},[next.nodes[1].id]:{kind:'context-endpoint'}};
  next.inclusion.relations={[next.relations[0].id]:{kind:'incident'}};
  next.counts.discovered_nodes=3;next.counts.emitted_relations=2;
  next.scene=knowledgeScene(next.nodes,next.relations,next.origin.id);
  return next;
}
