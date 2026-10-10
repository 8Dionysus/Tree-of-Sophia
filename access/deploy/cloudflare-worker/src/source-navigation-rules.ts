// Actual consumers use checksum-verified decoded JSON and HTTP integer limits.
import {stringArray,stringValue,type Item} from './common.ts';
import {type PhysicalKeys} from './worker-classic.ts';
export interface SourceWalk {
 need():string;current():number;incoming():boolean;semantic():boolean;sorting_ids():Uint32Array;sorted(ids:Uint32Array):void;
 contains(id:number):boolean;target():number;edge(id:number,row:number,from:number,to:number,kind:string,predicate:string):boolean;
 retained(id:number,row:number):boolean;loaded(exists:boolean,kind:string):void;finish_edges():void;node_ids():Uint32Array;node_depth(id:number):number;
 edge_rows():Uint32Array;truncated():boolean;kind_matches(id:number,kind:string):boolean;decision_ids():Uint32Array;
 prepare_paths(eras:Uint32Array,rows:Uint32Array):void;path_count():number;path_nodes(index:number):Uint32Array;path_edges(index:number):Uint32Array;reference(id:number,nonempty:boolean):void;references():Uint32Array;free():void;
}
export interface SourceRights {
 component(id:number,kind:string):void;
 record(row:number,source:number,nonempty:boolean,scopes:Uint32Array,explicit:boolean,assessment:string,id:Uint16Array,status:string,posture:string,review:string):void;
 membership(item:number,file:number,kind:string,predicate:string,array:boolean,rawLength:number,sources:Uint32Array,present:boolean,arrayContexts:boolean,length:number):number;
 context(index:number,record:boolean,manifest:number,nonempty:boolean,right:number,rightNonempty:boolean,legacyEmpty:boolean):void;
 finish_membership(index:number,present:boolean,array:boolean,length:number):void;
 incoming_owner(file:number,item:number,kind:string,predicate:string):void;
 observe_legacy_file(file:number,kind:string,predicate:string,array:boolean,length:number,valid:number):void;legacy_file_ids():Uint32Array;filtered_rows():Uint32Array;intersects_component(scopes:Uint32Array):boolean;
 status(value:string):void;summary(decision:Uint32Array,links:number):string;free():void;
}
interface Runtime {
 WorkerSourceWalk:{new(root:number,empty:number,kind:string,dossier:boolean,depth:number,limit:number):SourceWalk;supported(kind:string):boolean;chain_kinds():string;semantic_predicates():string;packet_filtered(value:string):boolean};
 WorkerSourceRights:{new():SourceRights;context_ref(record:boolean,nonempty:boolean):boolean};
}
let installed:Runtime|undefined;
export function installSourceNavigationRules(runtime:Runtime):void {installed=runtime;}
export function sourceNavigationRules():Runtime {if(!installed)throw new Error('Source navigation Rust rules are not installed');return installed;}
const units=(value:string)=>Uint16Array.from({length:value.length},(_,index)=>value.charCodeAt(index));
export function observeRights(rules:SourceRights,keys:PhysicalKeys,records:Item[]):void {
 for(const [index,record] of records.entries()){
  const source=stringValue(record.source_ref);
  rules.record(index,keys.key(source),Boolean(source),keys.observe(stringArray(record.scope_refs)),
   Object.prototype.hasOwnProperty.call(record,'assessment_kind'),stringValue(record.assessment_kind),units(stringValue(record.rights_id)),
   stringValue(record.assessment_status),stringValue(record.redistribution_posture),stringValue(record.review_status));
 }
}
export function observeMemberships(rules:SourceRights,keys:PhysicalKeys,edges:Iterable<Item>):void {
 for(const edge of edges){
  const rawRefs=edge.source_refs,refs=stringArray(rawRefs);
  const value=edge.properties,properties=value&&typeof value==='object'&&!Array.isArray(value)?value as Item:{};
  const present=Object.prototype.hasOwnProperty.call(properties,'item_file_contexts'),contexts=properties.item_file_contexts;
  const index=rules.membership(keys.key(stringValue(edge.from_id)),keys.key(stringValue(edge.to_id)),stringValue(edge.edge_kind),stringValue(edge.predicate_id),
   Array.isArray(rawRefs),Array.isArray(rawRefs)?rawRefs.length:0,keys.observe(refs),present,Array.isArray(contexts),Array.isArray(contexts)?contexts.length:0);
  if(index<0)continue;
  if(present&&Array.isArray(contexts))for(const raw of contexts){
   const record=Boolean(raw)&&typeof raw==='object'&&!Array.isArray(raw);
   const context=record?raw as Item:{},manifest=stringValue(context.manifest_ref),rawRight=context.rights_ref,right=stringValue(rawRight);
   rules.context(index,record,keys.key(manifest),Boolean(manifest),keys.key(right),Boolean(right),rawRight===undefined||rawRight===null||rawRight==='');
  }
  rules.finish_membership(index,present,Array.isArray(contexts),Array.isArray(contexts)?contexts.length:0);
 }
}
