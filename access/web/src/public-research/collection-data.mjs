import collection from './antiquity-collection-data.json' with {type:'json'};
import {CLOSE_READING_GUIDES} from '../../constructor/close-reading-guides.mjs';

const bi=(ru,en)=>({ru,en});
const digest=async value=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(JSON.stringify(value)))),x=>x.toString(16).padStart(2,'0')).join('');
const publicIds={apology:'tos.work.plato.apology',meno:'tos.work.plato.meno',ethics:'tos.work.aristotle.nicomachean-ethics'};
const periods={classical:bi('Классическая Греция','Classical Greece'),hellenistic:bi('Эллинистический мир','Hellenistic world'),roman:bi('Римский мир','Roman world'),late:bi('Поздняя Античность','Late Antiquity')};
const modelKinds={author:'figure',school:'tradition',era:'dossier',translator:'figure',translation:'work',original:'work',edition:'work',passage:'fragment'};
const refs=entry=>(entry.sourceRefs??[]).map(ref=>({...ref,label:ref.label??entry.title}));
function sectionEntry(document,version,structure,locator){
  if(!locator||!version)return null;
  let label=null;
  if([publicIds.apology,publicIds.meno].includes(document.id))label=/^(\d+)[a-e](?:\b|[–—-])/.exec(locator)?.[1]??null;
  if(document.id===publicIds.ethics){const match=/^([IVX]+)\.(\d+)\b/.exec(locator),books={I:1,II:2,III:3,IV:4,V:5,VI:6,VII:7,VIII:8,IX:9,X:10};if(match&&books[match[1]])label=`${books[match[1]]}.${match[2]}.1`;}
  const matches=structure?.filter(item=>item.label===label)??[];
  return matches.length===1?{documentId:document.id,versionId:version.id,revision:version.revision,unitId:matches[0].unitId,precision:'section-entry'}:null;
}
const materialId=entry=>entry.kind==='passage'?`antiquity:reading:${entry.id}`:entry.kind==='work'&&publicIds[entry.id]?`antiquity:work:${publicIds[entry.id]}`:`antiquity:catalog:${entry.id}`;
const entries=()=>[...collection.entities,...collection.eras,...collection.passages,...collection.questions,
  ...collection.terms.map(term=>({...term,id:`lexical:${term.id}`,lexical:true,meta:term.lemma,relatedIds:term.related.map(id=>`lexical:${id}`),sourceRefs:[{label:bi(term.source.title,term.source.title.replace('греческий словарь','Greek dictionary').replace('латинский словарь','Latin dictionary')),url:term.source.url}],readingSteps:term.uses.map(use=>({...use,parts:[use.part],title:bi(`${use.form} · ${use.citation}`,`${use.form} · ${use.citation}`)}))}))
].map(entry=>({...entry,legacyId:entry.id,materialId:materialId(entry),period:periods[entry.period]??entry.period,
  relatedIds:[...(entry.relatedIds??[]),...(entry.works??[]),...(entry.about??[]),...(entry.positions??[]).map(x=>x.passage),...(entry.kind==='era'?collection.entities.filter(x=>x.period===entry.period).map(x=>x.id):[]),...(entry.kind==='school'?collection.entities.filter(x=>x.school===entry.id).map(x=>x.id):[])].filter(Boolean),sourceRefs:refs(entry)}));

/** Additional authored navigation, not a second reader or a copy of witness text. */
export async function extendPublicCollection(prepared,{profile,sources,provider,structures,loadStructure}){
  const {library,bindings,documentBindings}=prepared;
  const items=entries(),byId=new Map(library.nodes.map(node=>[node.id,node]));
  const documents=new Map(sources.documents);
  for(const id of Object.values(publicIds))if(!documents.has(id)&&sources.manifest.documents.some(item=>item.id===id))documents.set(id,{document:await provider.document({documentId:id})});
  const sourceStructures=new Map(structures);
  for(const id of Object.values(publicIds)){const document=documents.get(id)?.document,version=document?.versions.find(v=>v.language==='en')??document?.versions[0];if(document&&version&&!sourceStructures.has(id))sourceStructures.set(id,await loadStructure(document,version));}
  for(const item of items){
    const publicId=publicIds[item.workId??item.id],document=documents.get(publicId)?.document;
    const version=document?.versions.find(v=>v.language==='en')??document?.versions[0];
    if(document&&version){item.publicDocumentId=publicId;item.publicVersionId=version.id;const address=sectionEntry(document,version,sourceStructures.get(publicId),item.meta?.ru);if(address){item.publicAddress=address;bindings.set(item.materialId,address);}}
  }
  if(profile==='zarathustra'){
    const own=library.nodes.map(node=>({...node,materialId:node.id,kind:node.kind==='figure'?'author':node.kind==='tradition'?'school':node.kind,
      readingSteps:(node.inquiry?.grounds??[]).flatMap(ground=>CLOSE_READING_GUIDES[ground.ref]?.moves??[]).map(move=>({title:move.title,body:move.body}))}));
    // Collection metadata does not participate in the constructor identity. The
    // saved Zarathustra graph and its witness addresses remain byte-for-byte.
    library.space.collection=[...own,...items];
    return prepared;
  }
  const predecessor=library.fingerprint;
  const compatibleMaterialIds=library.nodes.map(node=>node.id);
  const routeCluster=library.atlas.clusters.find(cluster=>cluster.id==='routes');if(routeCluster)routeCluster.subtitle=bi('Ветви вопросов Античности','Branches of questions in Antiquity');
  let offset=0;
  for(const item of items){
    if(!byId.has(item.materialId)){
      const node={...item,id:item.materialId,kind:modelKinds[item.kind]??item.kind,parentId:null,demo:true,title:item.title,body:item.body??bi('',''),position:[((offset%8)-3.5)*110,300+Math.floor(offset/8)*80,0],cluster:['author','school','translator'].includes(item.kind)?'echoes':'readings',sourceAccess:'metadata-only'};
      if(item.publicDocumentId){node.sourceAccess='public-document';documentBindings.set(node.id,{documentId:item.publicDocumentId,versionId:item.publicVersionId});}
      library.nodes.push(node);byId.set(node.id,node);offset++;
    }
    const bound=byId.get(item.materialId);
    for(const field of ['readingSteps','argument','workMap','voice'])if(item[field])bound[field]=item[field];
    if(bound.publicDocumentId&&!item.publicDocumentId){item.publicDocumentId=bound.publicDocumentId;item.publicVersionId=bound.publicVersionId;}
  }
  const allRoutes=[];
  for(const [index,branch]of collection.branches.entries()){
    const rootId=`antiquity:route:${branch.id}`,rootPosition=[((index%5)-2)*230,(Math.floor(index/5)-1)*190,0];
    const steps=branch.nodes.map(node=>({nodeId:`antiquity:reading:${node.id}`,sourceNodeId:`antiquity:source:${node.id}`,title:node.title,body:node.reading,question:node.question}));
    for(const node of branch.nodes){
      const id=`antiquity:reading:${node.id}`,material=byId.get(id);if(!material)continue;
      if(!material.question)material.question=node.question;
      const binding=bindings.get(`antiquity:source:${node.id}`);if(binding)bindings.set(id,binding);
    }
    const transitions=branch.edges.map(edge=>({...edge,id:`antiquity:relation:${branch.id}:${edge.from}:${edge.to}`,fromNodeId:`antiquity:reading:${edge.from}`,toNodeId:`antiquity:reading:${edge.to}`}));
    const end=branch.openEnds.map(value=>typeof value==='string'?value:value.ru).map(id=>branch.nodes.find(node=>node.id===id)?.question).filter(Boolean);
    const route={id:branch.id,title:branch.rootTitle,subtitle:branch.summary,question:branch.question,steps,transitions,conclusion:bi(end.map(x=>x.ru).join(' '),end.map(x=>x.en).join(' ')),posture:'proposed editorial route'};
    allRoutes.push(route);
    if(!byId.has(rootId)){
      const node={id:rootId,kind:'question',parentId:null,demo:true,title:branch.rootTitle,body:branch.summary,question:branch.question,position:rootPosition,cluster:'routes',sourceAccess:'proposed-reading',route};
      library.nodes.push(node);byId.set(rootId,node);library.atlas.defaultIds.push(rootId);
    }
    for(const edge of transitions){
      if(library.atlas.edges.some(existing=>existing.id===edge.id))continue;
      library.atlas.edges.push({...edge,from:edge.fromNodeId,to:edge.toNodeId,label:edge.label,body:edge.body});
    }
  }
  library.atlas.routes=allRoutes;library.space.routes=allRoutes;
  library.space.collection=items;
  library.space.intro=bi('Пятнадцать ветвей вопросов, произведения, авторы, школы и подробные прочтения в одном исследовании.','Fifteen branches of questions, works, authors, schools and close readings in one research space.');
  library.space.inquiryFoundation={question:bi('От какого вопроса начнётся ваше исследование?','Which question will begin your inquiry?'),orientation:bi('Выберите ветвь, прочитайте исходное место, затем проследите объяснённые переходы. Вопросы и прочтения остаются предложениями для проверки по источнику.','Choose a branch, read its starting passage and follow the explained transitions. Questions and readings remain proposals to examine against their sources.')};
  library.fingerprint=await digest({...library,fingerprint:undefined});
  // Released collection before the English CE-label correction; material IDs,
  // witness addresses and graph content are unchanged. Prefer its newer saved
  // personal work over the original six-branch namespace.
  prepared.graphPredecessors=[{fingerprint:'fa44cfb83977ee2990adba9725ab3bf93b5801d3dc19379aa123d10a31ac062b',materialIds:library.nodes.map(node=>node.id)},{fingerprint:predecessor,materialIds:compatibleMaterialIds}];
  return prepared;
}
