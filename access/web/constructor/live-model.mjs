import {relationLabel} from '../src/observatory/human-presentation.mjs';
import {displayTitleForm,localized} from '../src/observatory/knowledge-client.mjs';
import {validateHumanForms,resolveClaimReading} from '../src/observatory/human-forms.mjs';

// Labels remain supplied material. A missing wording stays a visible gap; no
// meaning, historical relation or confidence is inferred from opaque IDs.
export function liveLabel(raw,language='ru'){
  if(raw?.predicate_id)return {text:relationLabel(raw,language),lang:language,role:'navigation'};
  const navigation=displayTitleForm(raw,language);
  if(navigation?.navigationOnly||raw?.display?.title?.[language]||raw?.display?.label?.[language])return {...navigation,role:'navigation'};
  const selected=validateHumanForms(raw)?.roles.name;
  if(selected?.state==='ready')return {text:selected.packet.display_text,lang:selected.packet.language,role:'name'};
  const fallback=displayTitleForm(raw,language);
  return fallback?{...fallback,role:'navigation'}:{text:language==='ru'?'Название не предоставлено':'Name not supplied',lang:null,role:'missing'};
}
// Catalog predicates carry the language map directly in display, unlike
// graph records. Consume that contract without deriving words from the ID.
export function livePredicateLabel(predicate,language='ru'){
  return relationLabel(predicate,language);
}
const TYPE_COLORS=['#bdd5ed','#dbc69a','#b6a5dc','#96c9bb','#d8acae','#aabfdd','#c4ca9b','#d0b7d4'];
export function liveTypeColor(raw){
  const id=raw?.type_id??raw?.kind_id;if(typeof id!=='string')return '#bdd5ed';
  let hash=0;for(const point of id)hash=(Math.imul(hash,31)+point.codePointAt(0))>>>0;
  return TYPE_COLORS[hash%TYPE_COLORS.length];
}
export function liveHover(raw,language='ru'){
  const form=validateHumanForms(raw)?.roles.hover;
  return form?.state==='ready'&&form.packet.derivation!=='source-copy'?form.packet.display_text:liveLabel(raw,language).text;
}
export function liveEdgeLabel(view,edge,language='ru'){
  const missing=()=>({text:language==='ru'?'Формулировка не предоставлена':'Wording not supplied',lang:null,role:'missing'});
  if(edge.kind==='claim-path'){
    const reading=resolveClaimReading(view,edge.path.reading),wording=reading.wording;
    if(wording?.display_text)return {text:wording.display_text,lang:wording.language,role:'claim-path'};
    if(wording?.content_available&&wording.text)return {text:wording.text,lang:wording.actual_language,role:'claim-path-navigation'};
    return missing();
  }
  const raw=view.relations.find(item=>item.id===edge.rawId),selected=validateHumanForms(raw)?.roles.caption;
  if(selected?.state==='ready')return {text:selected.packet.display_text,lang:selected.packet.language,role:'caption'};
  return raw?liveLabel(raw,language):missing();
}

// Layout is a bounded presentation state, separate from packet authority. It
// never changes old coordinates when pages arrive or projection mode changes.
// Explicit new-space reset is the only way to discard retained positions.
export class StableExplorationLayout {
  #positions=new Map();#serial=0;
  reset(){this.#positions.clear();this.#serial=0;}
  capture(){return {v:1,serial:this.#serial,positions:[...this.#positions].map(([id,position])=>[id,[...position]])};}
  restore(value){
    if(value?.v!==1||!Number.isSafeInteger(value.serial)||value.serial<0||value.serial>400||!Array.isArray(value.positions)||value.positions.length>400)throw new TypeError('Invalid saved layout.');
    const positions=new Map();
    for(const row of value.positions){if(!Array.isArray(row)||row.length!==2||typeof row[0]!=='string'||!row[0]||row[0].length>2048||positions.has(row[0])
      ||!Array.isArray(row[1])||row[1].length!==3||row[1].some(n=>!Number.isFinite(n)||Math.abs(n)>100000))throw new TypeError('Invalid saved layout.');
      positions.set(row[0],[...row[1]]);}
    if(value.serial<positions.size)throw new TypeError('Invalid saved layout.');
    this.#positions=positions;this.#serial=value.serial;
  }
  position(id){const value=this.#positions.get(id);return value?[...value]:null;}
  move(id,position){
    if(!this.#positions.has(id)||!Array.isArray(position)||position.length!==3
      ||position.some(value=>!Number.isFinite(value)||Math.abs(value)>100000))throw new RangeError('Invalid bounded scene position.');
    this.#positions.set(id,[...position]);
  }
  project(view,model,{language='ru',selection=view.selection}={}){
    const absent=model.vertices.filter(vertex=>!this.#positions.has(vertex.id));
    // At most one grouped and one raw identity per retained raw node, plus
    // small compatibility headroom. Hidden compact Claims keep their places.
    if(this.#positions.size+absent.length>400)throw new RangeError('The layout reached its bounded capacity.');
    const selected=selection,focusRaw=selected?.kind==='relation'?model.rawRelationsById.get(selected.id)?.from_id:
      selected?.kind==='claim-path'?selected.claimId:selected?.id;
    const focus=model.carrierToVertex.get(focusRaw);
    for(const vertex of absent){
      const n=this.#serial++,angle=n*2.399963229728653,radius=n===0?0:90*Math.sqrt(n),
        position=[Math.cos(angle)*radius,Math.sin(angle)*radius*.62,Math.sin(n*1.73)*85];
      this.#positions.set(vertex.id,position);
    }
    const labels={},nodes=model.vertices.map(vertex=>{
      const raw=model.rawNodesById.get(vertex.representativeId),label=liveLabel(raw,language);labels[vertex.id]=label.text;
      return {id:vertex.id,position:this.position(vertex.id),title:label.text,kind:'knowledge',hover:liveHover(raw,language),
        ownerType:raw?.type_id??raw?.kind_id??null,
        prominent:vertex.id===focus,major:vertex.id===focus,accessibilityLabel:label.text,
        // Categorical identity only: the legend names the declared type.
        color:liveTypeColor(raw)};
    });
    const edges=model.edges.map(edge=>({id:edge.id,from:edge.fromId,to:edge.toId,kind:'relates',
      label:liveEdgeLabel(view,edge,language).text,color:'#a5b7cf'}));
    return {nodes,edges,clusters:[],labels,selectedNodeId:selected?.kind==='node'?focus:null,
      selectedEdgeId:selected?.kind==='relation'||selected?.kind==='claim-path'?selected.id:null};
  }
}
