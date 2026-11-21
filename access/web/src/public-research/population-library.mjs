// The space reads the Population; the bonsai remains the owner of what it says about each member.
// A member is a figure; a research node (a batch of the registers) and a table of the research are traditions of the
// research's own structure. A node contains every member whose register rows it holds: a fact about rows, not about
// who taught whom.
const bi=(ru,en)=>({ru,en});
export const memberMaterial=id=>`population:member:${id}`;
export const nodeMaterial=batch=>`population:node:${batch}`;
export const tableMaterial=table=>`population:table:${table}`;
export const batchOfRow=row=>String(row).split(':')[0];
export const tableOfBatch=batch=>batch.startsWith('A')?'I':batch.startsWith('T2')?'II':'III';
// The names below are written as {ru, en} so that the space's catalogs are checked to hold them in every language.
export const TABLES=Object.freeze({
  I:{ru:'Таблица I: древность',en:'Table I: antiquity'},
  II:{ru:'Таблица II: 600–1500',en:'Table II: 600–1500'},
  III:{ru:'Таблица III: Новое время',en:'Table III: modern times'},
});
export const NODE_OF_TABLE={ru:'Узел таблицы',en:'A node of the table'};
const ROWS_OF_NODE={ru:'Строки узла',en:'Rows of the node'};
const TREE_LENS={id:'tree',icon:'✧',title:{ru:'Древо',en:'The tree'},description:{ru:'Таблицы исследования, его узлы и авторы, чьи строки они держат.',en:'The research tables, their nodes and the authors whose rows they hold.'}};
const byBatch=(a,b)=>{const order=batch=>[tableOfBatch(batch),Number(batch.replace(/^\D+/,''))];const [ta,na]=order(a),[tb,nb]=order(b);return ta===tb?na-nb:ta<tb?-1:1;};

/** The member's home node: the batch of the row its ID was founded on, else of its first row. */
export function homeOf(member){return batchOfRow(member.founded??member.grew_from[0]);}

/** The constructor library of the Population space: tables, research nodes and members, and what contains what. */
export function makePopulation({population}){
  const nodes=[],edges=[];
  const add=(id,kind,title,extra={})=>nodes.push({id,kind,title,parentId:null,demo:true,body:title,position:[0,0,0],...extra});
  for(const [table,title] of Object.entries(TABLES))add(tableMaterial(table),'tradition',title,{table});
  const batches=Object.keys(population.batches).sort(byBatch);
  for(const batch of batches){
    add(nodeMaterial(batch),'tradition',bi(batch,batch),{batch,table:tableOfBatch(batch)});
    edges.push({id:`population:table-node:${batch}`,from:tableMaterial(tableOfBatch(batch)),to:nodeMaterial(batch),kind:'contains',label:NODE_OF_TABLE});
  }
  for(const [id,member] of Object.entries(population.members)){
    const own=[...new Set(member.grew_from.map(batchOfRow))].sort(byBatch);
    add(memberMaterial(id),'figure',bi(member.name.ru,member.name.en),{legacyId:id,home:homeOf(member),batches:own,memberKind:member.kind});
    for(const batch of own)edges.push({id:`population:node-member:${batch}:${id}`,from:nodeMaterial(batch),to:memberMaterial(id),kind:'contains',label:ROWS_OF_NODE});
  }
  // The space opens on the research's own structure; no member stands out at rest.
  const defaultIds=Object.keys(TABLES).map(tableMaterial);
  return {schema:'tos_constructor_library_v1',rootId:defaultIds[0],fingerprint:'0'.repeat(64),nodes,
    atlas:{defaultIds,edges,clusters:[],assemblies:[],extensions:[],routes:[]},
    space:{profile:'population',title:bi('Популяция','Population'),posture:population.posture,collection:nodes.map(node=>({...node,materialId:node.id})),routes:[],
      lenses:[TREE_LENS],
      relationKinds:['contains']}};
}
