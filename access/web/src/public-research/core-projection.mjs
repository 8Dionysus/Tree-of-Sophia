// Display-only projection. The bonsai owns every reading and admission;
// existing atlas identities and the saved graph are never rewritten here.
import {citedPlaces} from './research-index.mjs';
export function attachCore(library,core){
  const materials=new Map(library.nodes.map(node=>[node.id,node]));
  const materialIds={},records={},edges=[],routes=[];
  const entries=[...core.concepts,...core.figures,...core.proposed];
  for(const chain of core.chains)for(const state of chain.states??[])entries.push({...state,chainId:chain.id});
  for(const [index,record] of entries.entries()){
    if(records[record.id])continue;
    const id=materials.has(`demo:${record.id}`)?`demo:${record.id}`:materials.has(record.id)?record.id:`core:${record.id}`;
    materialIds[record.id]=id;records[record.id]=record;
    let material=materials.get(id);
    if(!material){
      const angle=index*2.399963229728653,radius=180+22*Math.sqrt(index);
      material={id,title:record.title,body:record.conception?.thesis??record.essence?.who??(record.cite?{cite:[record.cite]}:{ru:'',en:''}),kind:record.existence?'symbol':record.chainId?'fragment':'concept',parentId:'work',demo:true,
        position:[Math.cos(angle)*radius,Math.sin(angle)*radius,((index%7)-3)*50],
        sourceAccess:'proposed-interpretation',sourcePosture:'bonsai reading; no Tree canon admission'};
      library.nodes.push(material);materials.set(id,material);
    }
    material.coreId=record.id;material.coreCitations=citedPlaces(record);
  }
  const edge=(record,id)=>({id,from:materialIds[record.from],to:materialIds[record.to],kind:'relates',label:record.verb,coreRecord:record,
    body:{...record.sense,cite:[...new Map([...(record.cite?[record.cite]:[]),...(record.grounds??[])].map(cite=>[JSON.stringify(cite),cite])).values()]}});
  for(const link of core.links)if(link.admission==='map')edges.push(edge(link,`core-link:${link.id}`));
  const featured=new Set(['disciples-freedom','bestowing-virtue','shepherd-echo','zarathustra-giving']);
  for(const chain of core.chains){
    if(chain.admission!=='map'||!chain.steps.length)continue;
    const sequence=chain.structure==='comparison-branching'
      ? [...new Set(chain.steps.flatMap(step=>[step.from,step.to]))]
      : [chain.steps[0].from,...chain.steps.map(step=>step.to)];
    const transitions=chain.steps.map((step,index)=>{
      const link=edge(step,`core-chain:${chain.id}:${index}`);edges.push(link);
      return {edgeId:link.id,fromNodeId:link.from,toNodeId:link.to,label:step.verb,body:link.body};
    });
    routes.push({id:`core:${chain.id}`,coreChainId:chain.id,featured:featured.has(chain.id),title:chain.title,
      question:chain.question,subtitle:chain.sense,structure:chain.structure,transitions,
      steps:sequence.map((id,index)=>({nodeId:materialIds[id],title:records[id]?.title,
        body:{...chain.states?.find(state=>state.id===id)?.reading,
          cite:[chain.states?.find(state=>state.id===id)?.cite??chain.steps[Math.max(0,index-1)]?.cite].filter(Boolean)}}))});
  }
  library.space.core={...core,materialIds,records,edges};
  library.space.routes=[...(library.space.routes??[]),...routes];
  return library;
}

/** Context is temporary; rejected links never become map edges. */
export function coreContext(core,{materialId,edgeId,chainId}={}){
  if(!core)return {materialIds:[],edges:[]};
  let edges=[];
  if(chainId)edges=core.edges.filter(edge=>edge.id.startsWith(`core-chain:${chainId}:`));
  else if(edgeId)edges=core.edges.filter(edge=>`atlas:${edge.id}`===edgeId||edge.id===edgeId);
  else if(materialId)edges=core.edges.filter(edge=>edge.id.startsWith('core-link:')&&(edge.from===materialId||edge.to===materialId));
  const own=Object.values(core.materialIds).includes(materialId)?materialId:null;
  return {materialIds:[...new Set([own,...edges.flatMap(edge=>[edge.from,edge.to])].filter(Boolean))],edges};
}
