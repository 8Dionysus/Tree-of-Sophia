// Explicit presentation lenses over the prepared mockup. They never change graph authority.
export const LENSES = [
 {id:'tree', icon:'✧',title:{ru:'Древо',en:'The tree'},description:{ru:'Источники, образы и идеи в одном пространстве',en:'Sources, images and ideas in one space'}},
 {id:'meaning',icon:'◈',title:{ru:'Смыслы',en:'Meanings'},description:{ru:'Как понятия поддерживают, развивают и меняют друг друга',en:'How concepts support, develop and transform one another'}},
 {id:'sources',icon:'⌘',title:{ru:'Источники',en:'Sources'},description:{ru:'От книги и фрагмента — к наблюдению и толкованию',en:'From a work and a passage to an observation and interpretation'}},
 {id:'tensions',icon:'⋈',title:{ru:'Противоречия',en:'Tensions'},description:{ru:'Несовпадающие прочтения остаются видимыми',en:'Competing readings remain visible together'}},
 {id:'images',icon:'❋',title:{ru:'Образы',en:'Images'},description:{ru:'Символы связывают текст, опыт и мысль',en:'Symbols connect text, experience and thought'}},
 {id:'history',icon:'⌁',title:{ru:'Переклички',en:'Resonances'},description:{ru:'Сопоставления между мыслителями, без мнимой генеалогии',en:'Comparisons across thinkers, without an invented genealogy'}}
];
export const RELATIONS={
 contains:{ru:'Содержит',en:'Contains',color:'#829dbb'},supports:{ru:'Поддерживает',en:'Supports',color:'#b7c991'},interprets:{ru:'Толкует',en:'Interprets',color:'#bea7e1'},
 questions:{ru:'Ставит вопрос',en:'Questions',color:'#e5a18f'},contrasts:{ru:'Противостоит',en:'Contrasts',color:'#e09591'},echoes:{ru:'Перекликается',en:'Echoes',color:'#8ecbc4'},
 develops:{ru:'Развивает',en:'Develops',color:'#e0c48c'},compares:{ru:'Сопоставляет',en:'Compares',color:'#96b7e4'},translates:{ru:'Переводит',en:'Translates',color:'#8ebdc9'},relates:{ru:'Связано с',en:'Relates to',color:'#a5b7cf'}
};
export function makeLensView(state,library,{lens='tree',assembly=null,enabled=null,neighborhood=null,path=null}={}){
 const materials=new Map(library.nodes.map(n=>[n.id,n])),byId=new Map(state.nodes.map(n=>[n.id,n]));
 let ids=new Set(state.nodes.map(n=>n.id));
 const kind=n=>n.kind,mat=n=>materials.get(n.materialId);
 const focusKinds={meaning:['concept','interpretation','question','dossier'],sources:['work','part','chapter','fragment','dossier'],tensions:['question','interpretation'],images:['symbol'],history:['figure','tradition']};
 const lensEdges={meaning:['supports','develops','interprets','contrasts','questions'],sources:['contains','supports','interprets','translates'],tensions:['contrasts','questions'],images:['echoes','interprets','develops','compares'],history:['echoes','compares','develops']};
 const focused=new Set(state.nodes.filter(n=>focusKinds[lens]?.includes(kind(n))&&(lens!=='history'||mat(n)?.cluster==='echoes')).map(n=>n.id));
 if(lens!=='tree'){
   ids=new Set(focused);
   for(const e of state.edges)if(lensEdges[lens]?.includes(e.kind)&&(focused.has(e.from)||focused.has(e.to))){ids.add(e.from);ids.add(e.to);}
 }
 if(assembly){const selected=new Set(assembly.nodeIds);ids=new Set([...ids].filter(id=>selected.has(byId.get(id)?.materialId)||!byId.get(id)?.materialId));}
 if(neighborhood){const near=new Set([neighborhood]);for(const e of state.edges)if(e.from===neighborhood||e.to===neighborhood){near.add(e.from);near.add(e.to);}ids=new Set([...ids].filter(id=>near.has(id)));}
 if(path)ids=new Set(path.nodeIds);
 let nodes=state.nodes.filter(n=>ids.has(n.id));
 const edges=state.edges.filter(e=>ids.has(e.from)&&ids.has(e.to)&&(!path?.restrictEdges||path.edgeIds.includes(e.id))&&(!enabled||enabled.has(e.kind))&&(path||lens==='tree'||lensEdges[lens]?.includes(e.kind)));
 // A lens changes arrangement, while authored offsets remain available in every lens.
 const clusters=library.atlas.clusters,groups=new Map();
 for(const n of nodes){const group=lens==='sources'?({work:0,part:0,chapter:1,fragment:2,dossier:3,concept:3,interpretation:4}[n.kind]??4):lens==='tensions'?(n.kind==='question'?1:n.kind==='interpretation'?0:2):lens==='history'?(n.kind==='figure'||n.kind==='tradition'?0:1):null;if(group!==null){const bucket=groups.get(group)??[];bucket.push(n);groups.set(group,bucket);}}
 nodes=nodes.map(n=>{
  const m=mat(n);let position=[...n.position];
  if(groups.size&&m){for(const [g,bucket]of groups){const index=bucket.findIndex(item=>item.id===n.id);if(index<0)continue;const total=groups.size;
    const base=m?.position??n.position,offset=n.position.map((v,i)=>v-(base[i]??v));
    const x=lens==='sources'?-330+g*165:lens==='tensions'?(g-1)*265:((index%7)-3)*108;
    const y=lens==='history'?(g===0?-90:110)+Math.floor(index/7)*70:((index-(bucket.length-1)/2)*Math.min(62,360/Math.max(1,bucket.length-1)));
    position=[x+offset[0],y+offset[1],((index%3)-1)*22+offset[2]];
  }}
  return {...n,position,prominent:!!m?.prominent,color:clusters.find(c=>c.id===m?.cluster)?.color,major:['work','concept','figure','dossier'].includes(n.kind)};
 });
 return {nodes,edges:edges.map(e=>({...e,color:RELATIONS[e.kind]?.color,highlight:path?.edgeIds?.includes(e.id)})),clusters:lens==='tree'&&!assembly&&!neighborhood&&!path?clusters:[]};
}
export function shortestPath(state,from,to,enabled=null){
 if(!state.nodes.some(n=>n.id===from)||!state.nodes.some(n=>n.id===to))return null;
 const queue=[from],seen=new Map([[from,null]]);
 for(let i=0;i<queue.length;i++){
  const id=queue[i];if(id===to)break;
  for(const e of state.edges){if(enabled&&!enabled.has(e.kind))continue;const next=e.from===id?e.to:e.to===id?e.from:null;if(next&&!seen.has(next)){seen.set(next,{from:id,edge:e.id});queue.push(next);}}
 }
 if(!seen.has(to))return null;
 const nodeIds=[to],edgeIds=[];let cursor=to;
 while(cursor!==from){const step=seen.get(cursor);edgeIds.unshift(step.edge);cursor=step.from;nodeIds.unshift(cursor);}
 return {nodeIds,edgeIds};
}
