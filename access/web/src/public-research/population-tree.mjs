import {memberMaterial,NODE_OF_REGION,nodeMaterial,regionMaterial,REGION_ORDER} from './population-library.mjs';

const ALSO_IN_NODE={ru:'Есть и в строках узла',en:'Also in the rows of the node'};
// A banyan's aerial roots reach the ground and become trunks, so that one tree stands as a grove. The grove's trunks are
// the regions, round one ground; on each, the research nodes of its region branch in three tiers, the tables of the
// research (antiquity lowest, the modern times highest); each member is a leaf at the node of its founding row. A
// member whose rows stand in other nodes too hangs a thread to each of them: the rows say so; it is no lineage of ideas.
const GROUND=820,TIER={I:-330,II:-540,III:-750};

export function populationTree({library}){
  const positions=new Map(),roles=new Map(),sides=new Map(),edges=[],lines=[],names=[],notes=new Map();
  const set=(id,role,p)=>{positions.set(id,p);roles.set(id,role);};
  const regionIds=new Set(library.nodes.filter(node=>node.region&&!node.batch).map(node=>node.region));
  const regions=REGION_ORDER.filter(key=>regionIds.has(key));
  const nodesOf=new Map(),leavesOf=new Map();
  for(const node of library.nodes){
    if(node.batch){const key=`${node.region}|${node.table}`;if(!nodesOf.has(key))nodesOf.set(key,[]);nodesOf.get(key).push(node);}
    else if(node.kind==='figure'){if(!leavesOf.has(node.home))leavesOf.set(node.home,[]);leavesOf.get(node.home).push(node);}
  }
  for(const [k,region] of regions.entries()){
    const angle=2*Math.PI*k/regions.length,out=[Math.sin(angle),Math.cos(angle)],along=[Math.cos(angle),-Math.sin(angle)];
    const at=(r,y,t=0,u=0)=>[out[0]*(r+u)+along[0]*t,y,out[1]*(r+u)+along[1]*t];
    set(regionMaterial(region),'era',at(GROUND,40));
    // The trunk: strands from the ground to the crown.
    for(const dt of [-34,-11,11,34])lines.push({tone:'trunk',points:[at(GROUND,300,dt),at(GROUND,40,dt*.8),at(GROUND,-240,dt*.6)]});
    for(const table of ['I','II','III']){
      const own=nodesOf.get(`${region}|${table}`)??[];
      for(const [j,node] of own.entries()){
        // A tier's nodes stand in a disc round the trunk, across and outward, so that none hides another at rest.
        const a=j*2.399963,r=j?34*Math.sqrt(j):0,t=Math.cos(a)*r,u=Math.sin(a)*r*.6,p=at(GROUND,TIER[table]-(j%2)*18,t,u);
        set(node.id,'school',p);
        edges.push({id:`population:tree:region-node:${node.batch}`,from:regionMaterial(region),to:node.id,kind:'contains',label:NODE_OF_REGION});
        // Its leaves fan out round it, across the trunk and up and down.
        for(const [i,leaf] of (leavesOf.get(node.batch)??[]).entries()){
          const b=i*2.399963,s=10+5.5*Math.sqrt(i);
          set(leaf.id,'passage',[p[0]+along[0]*Math.cos(b)*s+out[0]*(i%3)*4,p[1]-Math.sin(b)*s*.75,p[2]+along[1]*Math.cos(b)*s+out[1]*(i%3)*4]);
        }
      }
    }
  }
  // Threads to the other nodes whose rows hold a member.
  for(const node of library.nodes)if(node.kind==='figure')for(const batch of node.batches)if(batch!==node.home&&positions.has(nodeMaterial(batch)))
    edges.push({id:`population:tree:also:${node.legacyId}:${batch}`,from:memberMaterial(node.legacyId),to:nodeMaterial(batch),kind:'contains',label:ALSO_IN_NODE});
  const reach=GROUND+260;
  return {positions,roles,sides,edges,lines,names,notes,bounds:{x:[-reach,reach],y:[-980,340],z:[-reach,reach]},rootRadius:GROUND,
    regionOf:id=>positions.has(id)?(positions.get(id)[1]>200?'roots':positions.get(id)[1]>-240?'trunk':'crown'):null,
    angleOf:id=>{const p=positions.get(id);return p?Math.atan2(p[0],p[2]):null;}};
}
