// Drafts of act records (access/contracts/act-record.v1.schema.json). The journal's Rust rules check the draft,
// give it its digest and refuse what breaks an invariant; these helpers only fill what the host knows.
import registry from '../../../contracts/act-kinds.v1.json' with {type:'json'};

export const ACT_SCHEMA='tos.act.v1';
export const ACT_POSTURE=Object.freeze({local_only:true,source:false,reviewed_by_tree:false,canon:false});
export {registry as ACT_KINDS};

const random=()=>{
  const bytes=new Uint8Array(8);globalThis.crypto.getRandomValues(bytes);
  return [...bytes].map(value=>value.toString(36).padStart(2,'0')).join('').slice(0,12);
};

/** A fresh act ID: `<prefix>-<time>-<random>`, within the record's ID grammar. */
export function actId(prefix='act'){
  const clean=String(prefix).replace(/[^A-Za-z0-9._:-]/g,'-').replace(/^[^A-Za-z0-9]+/,'').slice(0,48)||'act';
  return `${clean}-${Date.now().toString(36)}-${random()}`;
}

/** The person at this browser. Its ID stays local; a public name is a separate choice. */
export const LOCAL_PERSON=Object.freeze({type:'human',id:'local-person'});

/**
 * A draft act. `author` is {type,id,model?,role?,squad?,session?}; targets and grounds are addresses; intent is one
 * line. `at` defaults to now.
 */
export function draftAct({id,kind,author=LOCAL_PERSON,targets,grounds=[],intent,body={},parents=[],replaces,at=new Date()}){
  const line=String(intent??'').replace(/\s+/g,' ').trim();
  const record={schema:ACT_SCHEMA,id:id??actId(kind),kind,author:{...author},targets:[...new Set(targets??[])],
    grounds:[...new Set(grounds)],intent:line.length>280?`${line.slice(0,279)}…`:line,body,parents:[...new Set(parents)],
    at:(at instanceof Date?at:new Date(at)).toISOString(),posture:{...ACT_POSTURE}};
  if(replaces)record.replaces=replaces;
  return record;
}

/** The address of an act, as other acts name it. */
export const actAddress=id=>`act:${id}`;

/** Merge a stand's categories (from its bonsai) into the base registry. A later category refines an earlier one. */
export function registryWith(categories=[]){
  if(!categories.length)return registry;
  return {...registry,categories:[...registry.categories,...categories]};
}
