import {displayTitleForm} from '../src/observatory/knowledge-client.mjs';
import {validateHumanForms,resolveClaimReading} from '../src/observatory/human-forms.mjs';

// Labels remain supplied material. A missing wording stays a visible gap; no
// meaning, historical relation or confidence is inferred from opaque IDs.
export function liveLabel(raw,language='ru'){
  const selected=validateHumanForms(raw)?.roles.name;
  if(selected?.state==='ready')return {text:selected.packet.display_text,lang:selected.packet.language,role:'name'};
  const fallback=displayTitleForm(raw,language);
  return fallback?{...fallback,role:'navigation'}:{text:language==='ru'?'Название не предоставлено':'Name not supplied',lang:null,role:'missing'};
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
  position(id){const value=this.#positions.get(id);return value?[...value]:null;}
  move(id,position){
    if(!this.#positions.has(id)||!Array.isArray(position)||position.length!==3
      ||position.some(value=>!Number.isFinite(value)||Math.abs(value)>100000))throw new RangeError('Invalid bounded scene position.');
    this.#positions.set(id,[...position]);
  }
  project(view,model,{language='ru'}={}){
    const absent=model.vertices.filter(vertex=>!this.#positions.has(vertex.id));
    // At most one grouped and one raw identity per retained raw node, plus
    // small compatibility headroom. Hidden compact Claims keep their places.
    if(this.#positions.size+absent.length>400)throw new RangeError('The layout reached its bounded capacity.');
    const selected=view.selection,focusRaw=selected?.kind==='relation'?model.rawRelationsById.get(selected.id)?.from_id:
      selected?.kind==='claim-path'?selected.claimId:selected?.id;
    const focus=model.carrierToVertex.get(focusRaw);
    for(const vertex of absent){
      const n=this.#serial++,angle=n*2.399963229728653,radius=n===0?0:90*Math.sqrt(n),
        position=[Math.cos(angle)*radius,Math.sin(angle)*radius*.62,Math.sin(n*1.73)*85];
      this.#positions.set(vertex.id,position);
    }
    const labels={},nodes=model.vertices.map(vertex=>{
      const raw=model.rawNodesById.get(vertex.representativeId),label=liveLabel(raw,language);labels[vertex.id]=label.text;
      return {id:vertex.id,position:this.position(vertex.id),title:label.text,kind:'knowledge',
        prominent:vertex.id===focus,major:vertex.id===focus,accessibilityLabel:label.text,
        // The visual style is neutral. A color cannot assert an epistemic state.
        color:'#bdd5ed'};
    });
    const edges=model.edges.map(edge=>({id:edge.id,from:edge.fromId,to:edge.toId,kind:'relates',
      label:liveEdgeLabel(view,edge,language).text,color:'#a5b7cf'}));
    return {nodes,edges,clusters:[],labels,selectedNodeId:selected?.kind==='node'?focus:null,
      selectedEdgeId:selected?.kind==='relation'||selected?.kind==='claim-path'?selected.id:null};
  }
}
