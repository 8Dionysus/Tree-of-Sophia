// The space reads the Population; the bonsai remains the owner of what it says about each member.
// A member is a figure. The research's own structure stands as traditions: the regions the stand groups the research
// nodes under, and the nodes themselves (a node is a batch of the registers, one row of the master tables). A region
// contains its nodes; a node contains every member whose register rows it holds: a fact about rows, not about who
// taught whom.
export const memberMaterial=id=>`population:member:${id}`;
export const nodeMaterial=batch=>`population:node:${batch}`;
export const regionMaterial=region=>`population:region:${region}`;
export const batchOfRow=row=>String(row).split(':')[0];
export const tableOfBatch=batch=>batch.startsWith('A')?'I':batch.startsWith('T2')?'II':'III';
// The regions in the order the grove stands them round its ground, as the old world runs and then the new.
export const REGION_ORDER=Object.freeze(['europe','west-asia','north-africa','africa','south-asia','central-asia','east-asia','southeast-asia','oceania','americas','caucasus','transnational']);
// The names below are written as {ru, en} so that the space's catalogs are checked to hold them in every language.
// The regions' names are the bonsai's (population.json regions), said here again for the five languages of the site.
export const REGION_NAMES=Object.freeze({
  europe:{ru:'Европа',en:'Europe'},
  'west-asia':{ru:'Западная Азия',en:'West Asia'},
  'north-africa':{ru:'Северная Африка',en:'North Africa'},
  africa:{ru:'Африка к югу от Сахары',en:'Sub-Saharan Africa'},
  'south-asia':{ru:'Южная Азия',en:'South Asia'},
  'central-asia':{ru:'Центральная Азия',en:'Central Asia'},
  'east-asia':{ru:'Восточная Азия',en:'East Asia'},
  'southeast-asia':{ru:'Юго-Восточная Азия',en:'Southeast Asia'},
  caucasus:{ru:'Кавказ',en:'The Caucasus'},
  americas:{ru:'Америка',en:'The Americas'},
  oceania:{ru:'Океания',en:'Oceania'},
  transnational:{ru:'Транснациональные поля',en:'Transnational fields'},
});
export const NODE_OF_REGION={ru:'Узел региона',en:'A node of the region'};
const ROWS_OF_NODE={ru:'Строки узла',en:'Rows of the node'};
const TREE_LENS={id:'tree',icon:'✧',title:{ru:'Древо',en:'The tree'},description:{ru:'Регионы, узлы исследования и авторы, чьи строки они держат.',en:'The regions, the research nodes and the authors whose rows they hold.'}};
const order=(a,b)=>{const key=batch=>[tableOfBatch(batch),Number(batch.replace(/^\D+/,''))];const [ta,na]=key(a),[tb,nb]=key(b);return ta===tb?na-nb:ta<tb?-1:1;};

/** The member's home node: the batch of the row its ID was founded on. */
export function homeOf(member){return batchOfRow(member.founded??member.grew_from[0]);}

/** The constructor library of the Population space: regions, research nodes and members, and what contains what. */
export function makePopulation({population}){
  const nodes=[],edges=[];
  const add=(id,kind,title,extra={})=>nodes.push({id,kind,title,parentId:null,demo:true,body:title,position:[0,0,0],...extra});
  const regions=REGION_ORDER.filter(key=>population.regions?.[key]);
  for(const key of regions)add(regionMaterial(key),'tradition',REGION_NAMES[key]??population.regions[key],{region:key});
  for(const batch of Object.keys(population.batches).sort(order)){
    const node=population.batches[batch].node;
    add(nodeMaterial(batch),'tradition',node.title,{batch,table:node.table,region:node.region});
    edges.push({id:`population:region-node:${batch}`,from:regionMaterial(node.region),to:nodeMaterial(batch),kind:'contains',label:NODE_OF_REGION});
  }
  for(const [id,member] of Object.entries(population.members)){
    const own=[...new Set(member.grew_from.map(batchOfRow))].sort(order);
    add(memberMaterial(id),'figure',{ru:member.name.ru,en:member.name.en},{legacyId:id,home:homeOf(member),batches:own,memberKind:member.kind});
    for(const batch of own)edges.push({id:`population:node-member:${batch}:${id}`,from:nodeMaterial(batch),to:memberMaterial(id),kind:'contains',label:ROWS_OF_NODE});
  }
  // The space opens on the regions alone: no member and no node stands out at rest, and no two of them are joined.
  const defaultIds=regions.map(regionMaterial);
  return {schema:'tos_constructor_library_v1',rootId:defaultIds[0],fingerprint:'0'.repeat(64),nodes,
    atlas:{defaultIds,edges,clusters:[],assemblies:[],extensions:[],routes:[]},
    space:{profile:'population',title:{ru:'Популяция',en:'Population'},posture:population.posture,collection:nodes.map(node=>({...node,materialId:node.id})),routes:[],
      lenses:[TREE_LENS],relationKinds:['contains']}};
}
