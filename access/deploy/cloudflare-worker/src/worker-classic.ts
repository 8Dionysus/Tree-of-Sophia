import {stringValue,stringArray,type Item} from "./common.ts";
// Compiled D1 compatibility rules. Opaque IDs and physical source objects stay
// in the host; Rust receives numeric identity observations, never graph JSON.
// Actual callers use existing HTTP integer limits and plain decoded D1 JSON.
// Arbitrary-number direct exports and accessor/method overrides are not claimed.
export interface NeighborhoodRules {
  active(): boolean; frontier(): Uint32Array; candidates(): Uint32Array;
  observe_edge(id:number,from:number,to:number,row:number):void;
  finish_level(allowed:Uint32Array):void; selected():Uint32Array;
  neighbors():Uint32Array; edges():Uint32Array; enclosed_needed():boolean;
  observe_enclosed(id:number,row:number):void; free():void;
}
export interface PathRules {
  active():boolean; current_ids():Uint32Array;
  observe_edge(id:number,from:number,to:number):void; finish_level():void;
  path_count():number; path_nodes(index:number):Uint32Array;
  path_edges(index:number):Uint32Array; path_directions(index:number):Uint32Array;
  explored():number; enqueued():number; max_frontier():number; truncated():boolean; free():void;
}
interface SetRules {add(id:number,eligible:boolean):boolean;has(id:number):boolean;remove(id:number):void;values():Uint32Array;free():void;}
interface GraphRules {observe_node(id:number):void;observe_edge(from:number,to:number,row:number):boolean;edge_capacity():boolean;node_capacity():boolean;fill_node(id:number,nonempty:boolean):void;selected(id:number):boolean;edge_rows():Uint32Array;free():void;}
interface EpistemicRules {observe_relation(id:number,row:number,challenge:boolean):void;finish_relations():void;challenge_rows():Uint32Array;context_rows():Uint32Array;available():number;challenge_state():string;observe_endpoint(id:number,nonempty:boolean):void;neighbor(id:number,selected_node:boolean,selection:number):boolean;corpus_neighbor(id:number,selected_node:boolean,selection:number):boolean;free():void;}
interface EvidenceRules {route(scene:number,mode:string,items:Uint32Array):void;scene():number;curated():boolean;count_route(current:number):number;free():void;}
interface ClusterRules {node(id:number):boolean;edge(id:number):boolean;free():void;}
interface EndpointsRules {endpoint(id:number,nonempty:boolean):void;reference(id:number,ref:number,nonempty:boolean):void;ids():Uint32Array;contains(id:number):boolean;references(id:number):Uint32Array;free():void;}
interface SearchRules {remaining():number;observed(count:number):void;free():void;}
interface Runtime {
  WorkerProfile:{priority(left_selected:boolean,right_selected:boolean):number;direction(value:string):number;same(left:number,right:number):boolean;different(left:number,right:number):boolean;outside_view(row:number,view:number):boolean;endpoints_missing(count:number,same:boolean):boolean;node_fetch_limit(limit:number):number;edge_fetch_limit(limit:number):number;supported_corpus_evidence(view:string):boolean;missing(count:number):boolean;challenge_predicates():string;search_collections():string};
  WorkerEndpoints:{new():EndpointsRules;graph_mode(view:string):number};
  WorkerSearch:{new(limit:number):SearchRules;needle_order():Uint8Array};
  WorkerHealth:{expected(is_number:boolean):boolean;schema(schema:string):boolean;coverage(expected:number,actual:number,is_number:boolean):boolean};
  WorkerSet:new()=>SetRules;
  WorkerGraph:new(limit:number)=>GraphRules;
  WorkerEpistemic:{new(limit:number,context:boolean):EpistemicRules;challenge(predicate:string):boolean;corpus_candidate(id:number,selection:number,from:number,to:number,nodes:Uint32Array):boolean};
  WorkerEvidence:{new(mode:number,selection:number):EvidenceRules;field_fallback(length:number):boolean;conclusion_record(present:boolean,is_object:boolean):boolean;canon_layer(layer:string):boolean;coverage_posture():string;fallback(canon:boolean):string};
  WorkerClusters:{new(nodes:Uint32Array,edges:Uint32Array):ClusterRules;retain(nodes:number,edges:number):boolean};
  WorkerNeighborhood: new(root:number,depth:number,limit:number)=>NeighborhoodRules;
  WorkerPath: new(from:number,to:number,depth:number,direction:number,alternatives:number,excluded:Uint32Array)=>PathRules;
}
let installed:Runtime|undefined;
export function installWorkerClassicRules(runtime:Runtime):void {installed=runtime;}
export function workerClassicRules():Runtime {if(!installed)throw new Error("Worker classic Rust rules are not installed");return installed;}
export class PhysicalKeys {
  private readonly keys=new Map<string,number>();
  private readonly values:string[]=[];
  key(value:string):number {let key=this.keys.get(value);if(key===undefined){key=this.values.length;this.keys.set(value,key);this.values.push(value);}return key;}
  observe(values:string[]):Uint32Array{return Uint32Array.from(values.map(value=>this.key(value)));}
  strings(keys:Uint32Array):string[]{return Array.from(keys,key=>this.values[key]!);}
}

export function classicDistinct(values:string[]):string[]{
  const keys=new PhysicalKeys(),rules=new (workerClassicRules().WorkerSet)();
  try{for(const value of values)rules.add(keys.key(value),true);return keys.strings(rules.values());}finally{rules.free();}
}
export function classicSourceRefs(items:Item[]):string[]{
  const keys=new PhysicalKeys(),rules=new (workerClassicRules().WorkerSet)();
  try{for(const item of items){const ref=stringValue(item.source_ref);rules.add(keys.key(ref),Boolean(ref));for(const value of stringArray(item.source_refs))rules.add(keys.key(value),true);}return keys.strings(rules.values()).sort();}finally{rules.free();}
}
export function classicUniqueValues(items:Item[],key:string):string[]{
  const keys=new PhysicalKeys(),rules=new (workerClassicRules().WorkerSet)();
  try{for(const item of items){const value=stringValue(item[key]);rules.add(keys.key(value),Boolean(value));}return keys.strings(rules.values()).sort();}finally{rules.free();}
}
export function classicBoundedGraph(nodes:Item[],edges:Item[],limit:number):{nodes:Item[];edges:Item[]}{
  const keys=new PhysicalKeys(),rules=new (workerClassicRules().WorkerGraph)(limit);
  try{
    for(const node of nodes)rules.observe_node(keys.key(stringValue(node.node_id)));
    for(let index=0;index<edges.length;index+=1){if(!rules.edge_capacity())break;const edge=edges[index]!;rules.observe_edge(keys.key(stringValue(edge.from_id)),keys.key(stringValue(edge.to_id)),index);}
    for(const node of nodes){if(!rules.node_capacity())break;const id=stringValue(node.node_id);rules.fill_node(keys.key(id),Boolean(id));}
    return {nodes:nodes.filter(node=>rules.selected(keys.key(stringValue(node.node_id)))),edges:Array.from(rules.edge_rows(),index=>edges[index]!)};
  }finally{rules.free();}
}
export function classicBoundedClusters(clusters:Item[],nodes:Item[],edges:Item[]):Item[]{
  const keys=new PhysicalKeys(),runtime=workerClassicRules();
  const rules=new runtime.WorkerClusters(keys.observe(nodes.map(node=>stringValue(node.node_id))),keys.observe(edges.map(edge=>stringValue(edge.edge_id))));
  try{
    const result:Item[]=[];
    for(const cluster of clusters){
      const originalNodes=stringArray(cluster.member_node_ids),originalEdges=stringArray(cluster.member_edge_ids);
      const memberNodes=originalNodes.filter(id=>rules.node(keys.key(id))),memberEdges=originalEdges.filter(id=>rules.edge(keys.key(id)));
      if(!runtime.WorkerClusters.retain(memberNodes.length,memberEdges.length))continue;
      const properties=cluster.properties&&typeof cluster.properties==="object"&&!Array.isArray(cluster.properties)?{...cluster.properties as Item}:{};
      if("member_count" in properties)properties.member_count=memberNodes.length;
      if("edge_count" in properties)properties.edge_count=memberEdges.length;
      result.push({...cluster,member_node_ids:memberNodes,member_edge_ids:memberEdges,available_member_node_count:originalNodes.length,available_member_edge_count:originalEdges.length,properties});
    }return result;
  }finally{rules.free();}
}

// Rust requests the existing ECMAScript intrinsics. SQL search_text/instr remains
// the physical producer/consumer seam; Python Unicode normalization differs.
export function classicNeedle(query:string):string{
  let needle=query;
  for(const intrinsic of workerClassicRules().WorkerSearch.needle_order()){
    if(intrinsic===0)needle=needle.trim();
    else if(intrinsic===1)needle=needle.toLowerCase();
  }
  return needle;
}
