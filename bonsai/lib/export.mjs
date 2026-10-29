// What a bonsai hands to the Tree: candidates in the Tree's own contracts.
//
// The export never writes into the Tree and admits nothing. It turns a
// bonsai's chronology and places into the records and claims the Tree's
// source-owner routes take in — historical processes and places as provisional
// identities, datings as `tos_source_temporal_claim_v1`, the work and place of
// a process as `tos_source_relation_claim_v1`, the person of a life phase as
// `tos_historical_context_claim_v1` — and lists every identity it refers to,
// marked resolved (it exists in the Tree the bonsai was grafted from) or
// to-mint (a proposal the Tree's own route would have to create). Identities
// pass through the bonsai's graft map, so a Tree that renames or mints
// differently changes one map, not the bonsai's modules.

import {readChronology} from './chronology.mjs';

export const EXPORT_SCHEMA='tos.bonsai.export.v1';
const MAKER=Object.freeze({maker_type:'model',agent_ref:'agent:claude-tos-bonsai'});
const SCRIPT={ru:'Cyrl',en:'Latn'};

const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const slug=id=>String(id).replace(/^tos\./,'');
const pointer=id=>String(id).replaceAll('~','~0').replaceAll('/','~1');

/**
 * The identity a bonsai reference stands for in the Tree, through the graft map:
 * `{tree, status}` with status `resolved` or `to-mint`.
 */
export function graftedIdentity(identities,kind,localId){
  const entry=identities?.[kind]?.[localId];
  if(object(entry)&&typeof entry.tree==='string')return {tree:entry.tree,status:entry.status==='resolved'?'resolved':'to-mint'};
  // Situations keep the Tree identity the bonsai proposed for them until the Tree mints another.
  if(kind==='situations')return {tree:localId,status:'to-mint'};
  return null;
}

/** The export packet of one bonsai: identities, candidate records and claims. */
export function exportBonsai({manifest,chronology,places=null,identities={}}){
  const id=manifest?.id;if(!id)throw new TypeError('A bonsai manifest with an id is required.');
  const reader=readChronology(chronology),situations=chronology?.situations??{},home=`bonsai/${id}`;
  const event=`tos.event.bonsai-${id}-export-${String(chronology?.checked??'undated').replaceAll('-','')}`;
  const used=new Map();
  const identity=(kind,localId,detail={})=>{
    const found=graftedIdentity(identities,kind,localId);if(!found)throw new Error(`No Tree identity for ${kind} ${localId}`);
    const key=found.tree;if(!used.has(key))used.set(key,{tree:found.tree,status:found.status,type:detail.type??kind.replace(/s$/,''),bonsai:localId,...(detail.label?{label:detail.label}:{}),...(detail.external?{external_identifiers:detail.external}:{})});
    return found.tree;
  };
  const sourceUrls=(ids,sources=chronology?.sources)=>[...new Set((ids??[]).map(key=>sources?.[key]?.url).filter(Boolean))];
  const attribution=(ids,sources)=>(ids??[]).map(key=>sources?.[key]?.short).filter(Boolean).join('; ');
  const statement=(text,extra={})=>({statement:text.ru,statement_language:'ru',statement_script:SCRIPT.ru,parallel_statement:{text:text.en,language:'en',script:SCRIPT.en,posture:'authored parallel wording, not a translation claim'},...extra});
  const claim=(schema,suffix,subject,predicate,object,{evidence,epistemic='reported',qualifiers})=>({
    schema_version:schema,claim_id:`tos.claim.bonsai-${id}.${slug(subject)}.${suffix}`,claim_type:'relation',assertion_layer:'scholarly_report',
    subject_ref:subject,predicate,object,evidence_refs:evidence,maker:{...MAKER},provenance_event_ref:event,epistemic_status:epistemic,
    review_status:'unreviewed',claim_version:1,visibility:'public_metadata_only',qualifiers,
  });
  const records=[],claims=[];
  for(const key of Object.keys(situations).sort()){
    const item=situations[key],subject=identity('situations',key,{type:item.type,label:item.label});
    const own=`${home}/chronology.json#/situations/${pointer(key)}`;
    // A relative dating names its anchor as the Tree will know it.
    const dating=structuredClone(item.dating);
    if(dating.kind==='relative-order')dating.relative.anchor_ref=identity('situations',dating.relative.anchor_ref,{type:situations[dating.relative.anchor_ref]?.type,label:situations[dating.relative.anchor_ref]?.label});
    claims.push(claim('tos_source_temporal_claim_v1','dating',subject,'historical_dating',dating,{
      evidence:[...sourceUrls(item.sources),own],epistemic:item.epistemic,
      qualifiers:statement(item.statement,{attribution:attribution(item.sources,chronology.sources)||'bonsai chronology',source_footing:item.footing,
        calendar_posture:'Years as the sources give them, converted from BCE/CE to astronomical numbering (1 BCE = 0); no calendar is asserted or converted.',
        ...(item.note?{note:item.note.ru,parallel_note:item.note.en}:{})}),
    }));
    if(item.type==='historical-process'){
      records.push({schema_version:'tos_historical_record_v1',record_type:'historical-process',record_id:subject,preferred_label:item.label.ru,
        field_languages:{preferred_label:{language:'ru',script:SCRIPT.ru}},
        variant_labels:[{value:item.label.en,language:'en',script:SCRIPT.en,source_ref:`${home}/chronology.json`,status:'unverified'}],
        identity_status:'provisional',source_refs:[...sourceUrls(item.sources),`${home}/chronology.json`],external_identifiers:[],same_as_posture:'no_equivalence_claim',record_version:1,visibility:'public_metadata_only'});
      if(item.work){
        const work=identity('works',item.work,{type:'work'});
        claims.push(claim('tos_source_relation_claim_v1','work',subject,'historical_work',work,{evidence:[own],
          qualifiers:statement({ru:`Процесс «${item.label.ru}» относится к этому сочинению.`,en:`The process “${item.label.en}” concerns this work.`},{association_scope:'The writing of the work as the bonsai dates it; not an Edition, Expression or Item identity.'})}));
      }
    }
    if(item.type==='biographical-phase'&&item.subject){
      const agent=identity('agents',item.subject,{type:'agent'});
      claims.push(claim('tos_historical_context_claim_v1','subject',subject,'biographical_subject',agent,{evidence:[own],
        qualifiers:statement({ru:`«${item.label.ru}» — фаза жизни этого человека.`,en:`“${item.label.en}” is a phase of this person's life.`},{
          relation_basis:'The bonsai defines the phase as part of this person’s life; its boundaries come from the phase’s own dating claim.',
          context_scope:'Biographical phase as a span of life or writing; no claim about every activity within it.',
          time_scope_note:'See the separate historical_dating claim of this phase.'})}));
    }
  }
  if(places){
    for(const [key,place] of Object.entries(places.places??{}).sort(([a],[b])=>a.localeCompare(b))){
      const gazetteer=place.gazetteer??[],external=gazetteer.map(entry=>({scheme:entry.scheme,value:entry.id,source_ref:entry.url,status:'unverified'}));
      const tree=identity('places',key,{type:'place',label:place.label,external});
      if(graftedIdentity(identities,'places',key)?.status==='resolved')continue;
      records.push({schema_version:'tos_corpus_record_v1',record_type:'place',record_id:tree,preferred_label:place.label.en,
        field_languages:{preferred_label:{language:'en',script:SCRIPT.en}},
        variant_labels:[{value:place.label.ru,language:'ru',script:SCRIPT.ru,source_ref:`${home}/places.json`,status:'unverified'}],
        identity_status:'provisional',source_refs:gazetteer.map(entry=>entry.url),external_identifiers:external,same_as_posture:'candidate_equivalence',record_version:1});
    }
    // A place the sources leave open is no claim: the stand shows what they say, the export proposes nothing.
    const BASIS={composition:'The source states where the work was written.',event:'The source states where the event took place.',activity:'The source states where the author lived and worked in the dated span; the place of writing is inferred from it.'};
    for(const [key,at] of Object.entries(places.situations??{}).sort(([a],[b])=>a.localeCompare(b))){
      if(!situations[key]||!BASIS[at.basis]||!places.places?.[at.place])continue;
      const subject=identity('situations',key),place=identity('places',at.place);
      claims.push(claim('tos_source_relation_claim_v1','place',subject,'historical_place',place,{
        evidence:[...sourceUrls(at.sources,places.sources),`${home}/places.json#/situations/${pointer(key)}`],epistemic:at.basis==='activity'?'inferred':'reported',
        qualifiers:statement(at.statement,{place_basis:BASIS[at.basis],attribution:attribution(at.sources,places.sources),source_footing:at.footing})}));
    }
  }
  const order=['work','agent','place','biographical-phase','historical-process','historical-event'];
  return {
    schema:EXPORT_SCHEMA,bonsai:id,checked:chronology?.checked??null,graft:manifest.graft??null,
    posture:'Candidate records and claims for the Tree’s own source-owner routes. Nothing here is reviewed, admitted or canonical; identities marked to-mint do not exist in the Tree the bonsai was grafted from.',
    identities:[...used.values()].sort((a,b)=>order.indexOf(a.type)-order.indexOf(b.type)||a.tree.localeCompare(b.tree)),
    records,claims,
  };
}

/** The Tree contract each exported item must satisfy. */
export function contractOf(item){
  const base='https://tree-of-sophia.local/ToS/contracts/';
  return {
    tos_source_temporal_claim_v1:`${base}source-temporal-claim.schema.json`,
    tos_source_relation_claim_v1:`${base}source-relation-claim.schema.json`,
    tos_historical_context_claim_v1:`${base}historical-context-claim.schema.json`,
    tos_historical_record_v1:`${base}historical-record.schema.json`,
    tos_corpus_record_v1:`${base}corpus-record.schema.json`,
  }[item?.schema_version]??null;
}
