// Translate explicitly selected native candidates into the source-owner wire
// grammar. This module issues no grants, guesses no Tree identities and never
// turns a stand assessment into an admission.

const SCHEMA='tos_local_source_command_v1';
const clone=value=>structuredClone(value);
const lexicalContext=cite=>{
  if(typeof cite?.c!=='string'||!cite.c.trim()||typeof cite?.w!=='string'||!cite.w.trim())
    throw new TypeError('A cited chapter and work are required for a lexical use.');
  return `Chapter ${cite.c} of ${cite.w}.`;
};

/** Build a source candidate for one stand use after the caller has selected
 * its exact native unit binding. Binding bytes and rights are checked again
 * by the native owner; this function cannot establish either one. */
export function unitOccurrenceSourceCandidate(subject,{binding,sourceRefs,formId,visibility='local_only'}={}){
  const {recordId,unitId,label,account,context,uses}=subject??{};
  if(!/^tos\.occurrence\.[a-z0-9.-]+$/.test(recordId??'')||!unitId||!label||!account||!context||!Array.isArray(uses)||!uses.length)
    throw new TypeError('An identified unit occurrence with its retained stand uses is required.');
  if(binding?.schema_version!=='tos_native_text_unit_binding_v1'||binding.unit_id!==unitId||
     !binding.packet_ref||!binding.packet_sha256||!binding.packet_id||!binding.packet_version||
     !binding.segmentation_id||!binding.segmentation_version||!binding.unit_version||
     !Array.isArray(binding.ordered_anchor_refs)||!binding.ordered_anchor_refs.length||
     !binding.text_layer?.record_ref||!binding.text_layer?.record_sha256||!binding.text_layer?.layer_id||!binding.text_layer?.layer_version||
     !['work','expression','edition','item'].every(key=>binding.source_record_refs?.[key]))
    throw new TypeError('An exact native binding for the cited unit is required.');
  if(!Array.isArray(sourceRefs)||!sourceRefs.length||!formId||!['local_only','public_metadata_only'].includes(visibility))
    throw new TypeError('Source references, an authorized form ID and a visibility are required.');
  return {kind:'record',profile_type_id:'tos.entity.occurrence',record:{
    schema_version:'tos_occurrence_description_record_v1',record_type:'occurrence',record_id:recordId,record_version:1,
    preferred_label:label,field_languages:{preferred_label:{language:'en',script:'Latn'},notes:{language:'en',script:'Latn'}},
    variant_labels:[],identity_status:'provisional',source_refs:clone(sourceRefs),external_identifiers:[],same_as_posture:'no_equivalence_claim',visibility,
    notes:'Stand-proposed descriptions within the selected text unit. Review, rights and admission remain separate.',
    semantic_scope:{scope_note:'The bound text unit and the explicitly retained stand uses; no individual token-position claim.',identity_criterion:'The explicitly identified occurrence at this selected unit; a change of referent requires a separate occurrence.',language:'en',script:'Latn'},
    semantic_content:{occurrence_account:account,context_account:context,language:'en',script:'Latn'},
    native_text_binding:clone(binding),extensions:{bonsai_uses:clone(uses)},
  },forms:[{form_id:formId,field_id:'metadata.preferred-name'}]};
}

/** Lexical uses are grouped by unit and form, never by a guessed token offset. */
export function occurrenceSourceCandidate(use,options={}){
  const unit=use?.cite?.u;
  if(!use?.id||!use?.form||!Array.isArray(unit)||unit.length!==2||unit[0]!==unit[1])
    throw new TypeError('A single-unit stand lexical use is required.');
  return unitOccurrenceSourceCandidate({recordId:use?.native_candidate?.record_id,unitId:unit[0],
    label:`${use.form} — ${use.cite.c}`,account:`The stand records the form ${use.form} within the bound unit.`,
    context:lexicalContext(use.cite),uses:[use]},options);
}

/** Collect explicit candidate IDs, grouping voice and place observations of
 * the same unit. Unresolved voice identities remain visible as deferrals. */
export function collectSourceOccurrences(packet){
  if(packet?.schema!=='tos.bonsai.philosophy-export.v1')throw new TypeError('A philosophy export is required.');
  const subjects=new Map(),deferred=[];
  const add=(recordId,unitId,use,label,account,context)=>{
    if(!recordId||!unitId){deferred.push(use);return;}
    const previous=subjects.get(recordId);
    if(previous&&previous.unitId!==unitId)throw new Error(`Occurrence ${recordId} names multiple units`);
    if(previous)previous.uses.push(clone(use));
    else subjects.set(recordId,{recordId,unitId,label,account,context,uses:[clone(use)]});
  };
  for(const term of packet.semantic_uses?.terms??[])for(const use of term.occurrences??[]){
    const context=lexicalContext(use.cite);
    const units=use.cite?.u;
    if(!Array.isArray(units)||units.length!==2||units[0]!==units[1]){deferred.push({kind:'lexical-use',use:clone(use)});continue;}
    add(use.native_candidate?.record_id,units[0],{kind:'lexical-use',use},`${use.form} — ${use.cite.c}`,
      `The stand records the form ${use.form} within the bound unit.`,context);
  }
  for(const relation of packet.place_relations?.relations??[])for(const side of ['from','to']){
    const endpoint=relation[side],units=endpoint?.cite?.u,use={kind:'place-relation-endpoint',relation:clone(relation),side};
    if(!Array.isArray(units)||units.length!==2||units[0]!==units[1]){deferred.push(use);continue;}
    add(endpoint.occurrence_candidate,units[0],use,`Text place — ${endpoint.cite.c}`,
      'A stand-proposed place in a textual comparison.','The whole relation and its assessment remain in the retained stand uses.');
  }
  for(const [chapter,segments] of Object.entries(packet.voices??{}))for(const segment of segments){
    const attribution=segment.attribution,use={kind:'voice-segment',chapter,segment:clone(segment)};
    if(!attribution?.occurrence_candidate||attribution.scope!=='first-unit-of-segment'||segment.u?.[0]!==attribution.anchor_unit){deferred.push(use);continue;}
    add(attribution.occurrence_candidate,attribution.anchor_unit,use,`Voice anchor — ${chapter}`,
      'The first source unit of a stand-proposed voice segment.','The voice spans the declared segment; this occurrence addresses only its first unit.');
  }
  return {subjects:[...subjects.values()],deferred};
}

/** A single ordered source-owner preparation. Bindings and forms must already
 * be selected by the caller against the destination's source and grant. */
export function prepareSourceRequest(candidate,{owner='public'}={}){
  if(!['public','private'].includes(owner))throw new TypeError('Select the public or private source owner.');
  if(candidate?.kind==='record'){
    if(!candidate.record?.record_id||!Array.isArray(candidate.forms)||!candidate.forms.length)throw new TypeError('Record identity and source-copy forms are required.');
    return {schema_version:SCHEMA,operation:'prepare-create',record:clone(candidate.record),forms:clone(candidate.forms)};
  }
  if(candidate?.kind==='claims'){
    if(!Array.isArray(candidate.claims)||!candidate.claims.length||candidate.claims.length>32)throw new TypeError('A batch of 1–32 Claims is required.');
    if(candidate.claims.some(item=>item.review_status!=='unreviewed'))throw new TypeError('Initial Claims must be unreviewed.');
    const request={schema_version:SCHEMA,operation:'prepare-create',claims:clone(candidate.claims)};
    if(owner==='private'){
      const ids=candidate.claims.map(claim=>claim.claim_id);
      if(ids.some(id=>typeof id!=='string'||!/^tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*$/.test(id))||new Set(ids).size!==ids.length)
        throw new TypeError('Private Claims require distinct exact claim IDs.');
      if(!Array.isArray(candidate.forms)||!candidate.forms.length||candidate.forms.length>32)
        throw new TypeError('Private Claims require 1–32 selected source-copy forms.');
      const formIds=new Set(),covered=new Set();
      for(const form of candidate.forms){
        if(!form||typeof form!=='object'||Array.isArray(form)||Object.keys(form).sort().join(',')!=='claim_id,field_id,form_id'||
           !ids.includes(form.claim_id)||typeof form.form_id!=='string'||!/^tos\.form\.[a-z0-9]+(?:[.-][a-z0-9]+)*$/.test(form.form_id)||
           form.field_id!=='claim.statement'||formIds.has(form.form_id))
          throw new TypeError('Each private form must uniquely select a Claim and its complete statement.');
        formIds.add(form.form_id);covered.add(form.claim_id);
      }
      if(covered.size!==ids.length)throw new TypeError('Every private Claim requires a selected statement form.');
      request.forms=clone(candidate.forms);
    }
    return request;
  }
  throw new TypeError('Select record or claims.');
}

/** Complete the exact write request from the native owner's own preview. Save
 * this complete request for exact retry; never manufacture preview digests. */
export function commitSourceRequest(candidate,preview,commandId,options={}){
  if(typeof commandId!=='string'||!commandId)throw new TypeError('A stable command ID is required.');
  const result=preview?.result??preview;
  if(!result?.owner_configuration||!result?.expected_dependencies)throw new TypeError('The native preview lacks required owner bindings.');
  const request=prepareSourceRequest(candidate,options);
  Object.assign(request,{operation:candidate.kind==='record'?'source.create':'claims.create',command_id:commandId,
    expected_configuration:result.owner_configuration,expected_dependencies:result.expected_dependencies,expected_revision:null});
  if(candidate.kind==='record')request.expected_source=null;
  else {
    if(!result.source_bindings)throw new TypeError('The native Claim preview lacks source bindings.');
    if(options.owner==='private')request.expected_source=null;
    request.expected_inputs=clone(result.source_bindings);
  }
  return request;
}
