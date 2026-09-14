// Synthetic delivery examples for UI contracts. These are not ToS sources,
// source assessments, or a backend adapter.
export const revision='a'.repeat(64),content='b'.repeat(64);
export const roles=['name','caption','hover','statement','grounds','history','technical'];
export const ref=(id='tos.form.fixture',letter='c')=>({id,version:1,digest:'sha256:'+letter.repeat(64)});
export function formNode(id='fixture:forms',language='ru'){
  const subject=ref('tos.fixture.subject','d');
  const raw={id,entity_id:subject.id,kind_id:'claim',content_revision:content,source_refs:['UI fixture; not a ToS source'],
    display:{title:{ru:'Проверочный материал',default:'Проверочный материал',en:null,original:null},kind_label:{ru:'Проверочный Claim'},
      summary:{ru:'Legacy preview'},summary_state:'source-derived'},semantics:{test:true},epistemic:{review_posture:'unreviewed',confidence:null}};
  const selection={schema_version:'tos_human_form_selection_v1',content_revision:content,requested_language:language,source_ref:'fixture:forms',
    state:'available',roles:{},candidates:[],issues:[],performs_translation:false,performs_assessment:false};
  for(const role of roles){
    const form=ref('tos.form.fixture.'+role),packet={schema_version:'tos_human_form_materialization_v1',form,subject,state:'ready',role,
      language,script:language==='ru'?'Cyrl':'Latn',derivation:'source-copy',standalone_reading:false,
      display_text:'Текст формы '+role+'. '+('Полная оговорка сохраняется. '.repeat(7))+'Это НЕ подтверждение.',
      context:[{slot:'qualification',binding:{record:subject,pointer:'/context'},value:{uncertain:true,negation:'НЕ доказано',unknown:{zero:0,false:false,null:null,empty:''}}}],
      dependencies:[subject],issues:[],admission:null,performs_semantic_assessment:false};
    selection.roles[role]={state:'ready',reason:'exact-language',form,packet};
    selection.candidates.push({form,role,language,state:'ready',source_pointer:'/attributes/human_forms/'+selection.candidates.length});
  }
  raw.human_form_selection=selection;
  // Full material reads retain the source-owned packets in attributes. The
  // compact selection above remains the bounded delivery surface.
  raw.attributes={human_forms:roles.map(role=>structuredClone(selection.roles[role].packet)),human_forms_source_ref:'fixture:forms',
    source_record:{record_id:subject.id,record_version:subject.version},source_sha256:subject.digest.slice('sha256:'.length)};
  return raw;
}
export const formLens=(nodes=[formNode()],relations=[])=>({schema:'tos_lens_result_v1',source_revision:revision,
  authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},nodes,relations,focus:{node_id:nodes[0].id}});

// Mirrors the current scene.compact envelope using visibly synthetic records.
export function compactFormLens(language='ru'){
  const claim=formNode('fixture:claim',language==='original'?'ru':language);
  claim.type_id='tos.entity.claim';claim.display.title.ru='Проверочный Claim-путь';
  claim.semantics={claim:{subject_node_id:'fixture:subject',object_node_id:'fixture:object',
    relation_type_id:'tos.relation.fixture',predicate_mapping_status:'mapped'},assertion_contexts:[{qualifiers:{unknown:false}}]};
  for(const [role,selected]of Object.entries(claim.human_form_selection.roles)){
    selected.form.id+='.'+selected.packet.language;
    selected.packet.display_text='Fixture '+selected.packet.language.toUpperCase()+' '+role+'. '+selected.packet.display_text;
  }
  claim.human_form_selection.requested_language=language;
  if(language==='original')for(const role of roles)claim.human_form_selection.roles[role]={state:'unavailable',reason:'original-role-not-declared',form:null,packet:null};
  claim.display_selection={fields:{title:{content_available:true,value:'Проверочный Claim-путь',language:null}}};
  const endpoints=['subject','object','evidence'].map(kind=>{
    const node=formNode('fixture:'+kind);delete node.human_form_selection;
    node.entity_id='tos.fixture.'+kind;node.kind_id='fixture';node.display.title.ru='Проверочный '+kind;return node;
  });
  const nodes=[endpoints[0],claim,endpoints[1],endpoints[2]],relations=['subject','object','evidence'].map((kind,index)=>({
    id:'fixture:claim-'+kind,from_id:claim.id,to_id:'fixture:'+kind,content_revision:content,source_refs:['UI fixture; not a ToS source'],
    relation_type_id:['tos.relation.has-subject','tos.relation.has-object','tos.relation.claim-supported-by'][index],
    display:{label:{ru:'Проверочная связь '+kind}},qualifiers:{unknown:false}}));
  const vertex=id=>'tos-scene:entity:'+nodes.find(node=>node.id===id).entity_id;
  const path={id:'tos-scene:claim-path:'+claim.id,claim_node_id:claim.id,from_id:vertex('fixture:subject'),to_id:vertex('fixture:object'),
    relation_type_id:'tos.relation.fixture',node_ids:['fixture:subject',claim.id,'fixture:object'],
    relation_ids:relations.slice(0,2).map(r=>r.id),detail_relation_ids:[relations[2].id],
    reading:{mode:'claim-with-mandatory-context',node_id:claim.id,content_revision:content,
      wording_pointer:language==='original'?'/display_selection/fields/title':'/human_form_selection/roles/caption/packet',
      wording_state:'available',context_pointers:['/semantics','/epistemic'],relation_context_ids:relations.map(r=>r.id),standalone:false}};
  return {...formLens(nodes,relations),scene:{schema_version:'tos_knowledge_scene_v1',
    vertices:nodes.map(n=>({id:vertex(n.id),entity_id:n.entity_id,node_ids:[n.id],representative_node_id:n.id})),
    arcs:relations.map(r=>({relation_id:r.id,from_id:vertex(r.from_id),to_id:vertex(r.to_id)})),
    collapsed_relation_ids:[],focus_vertex_id:vertex('fixture:subject'),scope:'returned-packet-only',identity_rule:'declared-tos-entity-id',
    authority:'presentation-mapping-not-semantic-admission',compact:{rule:'explicit-claim-paths-v1',authority:'presentation-only-no-new-assertion',
      vertex_ids:[vertex('fixture:subject'),vertex('fixture:object')],relation_ids:[],claim_paths:[path],
      folded_vertex_ids:[vertex(claim.id),vertex('fixture:evidence')],retained_claims:[]}}};
}

// Three exact synthetic TextUnit references; structural edges carry context,
// not accepted membership or a Sign judgment.
export function memberFormLens(language='ru'){
  const packet=compactFormLens(language),path=packet.scene.compact.claim_paths[0];
  const claim=packet.nodes.find(node=>node.id===path.claim_node_id);
  const members=[1,2,3].map(index=>{
    const node=formNode('fixture:text-unit:'+index);delete node.human_form_selection;
    node.entity_id='tos.fixture.text-unit.'+index;node.type_id='tos.entity.text-unit';node.kind_id='text-unit';
    node.display.title.ru='Проверочный TextUnit '+index;
    node.source_refs=['UI fixture exact span '+index+'; not a ToS source'];
    node.attributes={span:{start:(index-1)*10,end:index*10},fixture_only:true};return node;
  });
  claim.semantics.claim.value_member_node_ids=members.map(node=>node.id);
  const vertex=node=>'tos-scene:entity:'+node.entity_id;
  for(const member of members){
    const relation={id:'fixture:claim-member:'+member.id,from_id:claim.id,to_id:member.id,
      relation_type_id:'tos.relation.claim-value-member',content_revision:content,
      source_refs:['UI fixture; not a ToS source'],display:{label:{ru:'Объявленный участник значения'}},
      epistemic:{review_posture:'unreviewed',confidence:null}};
    packet.nodes.push(member);packet.relations.push(relation);
    packet.scene.vertices.push({id:vertex(member),entity_id:member.entity_id,node_ids:[member.id],representative_node_id:member.id});
    packet.scene.arcs.push({relation_id:relation.id,from_id:vertex(claim),to_id:vertex(member)});
    packet.scene.compact.folded_vertex_ids.push(vertex(member));
    path.detail_relation_ids.push(relation.id);path.reading.relation_context_ids.push(relation.id);
  }
  for(const selected of Object.values(claim.human_form_selection.roles))if(selected.packet){
    selected.packet.context.push({slot:'member-context',binding:{record:selected.packet.subject,pointer:'/claim/value_member_node_ids'},
      value:{declared_node_ids:[...claim.semantics.claim.value_member_node_ids],accepted_membership:false,sign_judgment:null}});
  }
  return packet;
}
