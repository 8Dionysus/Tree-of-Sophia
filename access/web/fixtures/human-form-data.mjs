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
  raw.human_form_selection=selection;return raw;
}
export const formLens=(nodes=[formNode()],relations=[])=>({schema:'tos_lens_result_v1',source_revision:revision,
  authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},nodes,relations,focus:{node_id:nodes[0].id}});
