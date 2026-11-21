import {memberMaterial,nodeMaterial,NODE_OF_TABLE,tableMaterial,TABLES} from './population-library.mjs';

const ALSO_IN_NODE={ru:'Есть и в строках узла',en:'Also in the rows of the node'};

// A banyan: the three tables of the research stand as trunks side by side, their research nodes branch above each
// trunk, and each member is a leaf at the node of its founding row. A member whose rows stand in other nodes too
// hangs a thin thread to each of them: the rows say so; it is no lineage of ideas.
const TRUNK_X=Object.freeze({I:-780,II:0,III:780});

export function populationTree({library}){
  const positions=new Map(),roles=new Map(),sides=new Map(),edges=[],lines=[],names=[],notes=new Map();
  const set=(id,role,p)=>{positions.set(id,p);roles.set(id,role);};
  const nodesOf=new Map(),leavesOf=new Map();
  for(const node of library.nodes){
    if(node.batch){if(!nodesOf.has(node.table))nodesOf.set(node.table,[]);nodesOf.get(node.table).push(node);}
    else if(node.kind==='figure'){if(!leavesOf.has(node.home))leavesOf.set(node.home,[]);leavesOf.get(node.home).push(node);}
  }
  for(const table of Object.keys(TABLES)){
    const x=TRUNK_X[table];
    set(tableMaterial(table),'era',[x,-40,0]);
    names.push({id:`population-table-${table}`,title:TABLES[table],center:[x,310,0],kind:'group'});
    // The trunk: strands from the ground to the crown, as a banyan's aerial roots become trunks.
    for(const dx of [-70,-25,25,70])lines.push({tone:'trunk',points:[[x+dx,280,0],[x+dx*.7,0,0],[x+dx*.5,-260,0]]});
    const own=nodesOf.get(table)??[];
    for(const [i,node] of own.entries()){
      const u=own.length>1?i/(own.length-1)*2-1:0;
      const nx=x+u*330,ny=-360-200*(1-u*u)-(i%3)*55,nz=(i%2?1:-1)*(40+(i%4)*35);
      set(node.id,'school',[nx,ny,nz]);
      for(const [j,leaf] of (leavesOf.get(node.batch)??[]).entries()){
        const a=j*2.399963,r=16+8.5*Math.sqrt(j);
        set(leaf.id,'passage',[nx+Math.cos(a)*r,ny-Math.sin(a)*r*.8,nz+20+(j%5)*6]);
      }
      edges.push({id:`population:tree:table-node:${node.batch}`,from:tableMaterial(table),to:node.id,kind:'contains',label:NODE_OF_TABLE});
    }
  }
  // Threads to the other nodes whose rows hold a member.
  for(const node of library.nodes)if(node.kind==='figure')for(const batch of node.batches)if(batch!==node.home&&positions.has(nodeMaterial(batch)))
    edges.push({id:`population:tree:also:${node.legacyId}:${batch}`,from:memberMaterial(node.legacyId),to:nodeMaterial(batch),kind:'contains',label:ALSO_IN_NODE});
  return {positions,roles,sides,edges,lines,names,notes,bounds:{x:[-1250,1250],y:[-900,340],z:[-500,500]},rootRadius:620,
    regionOf:id=>positions.has(id)?(positions.get(id)[1]>200?'roots':positions.get(id)[1]>-260?'trunk':'crown'):null,
    angleOf:id=>{const p=positions.get(id);return p?Math.atan2(p[0],p[2]):null;}};
}
