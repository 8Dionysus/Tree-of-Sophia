// How a bonsai stands to the Tree it was grafted from (Node only).
//
// The Tree grows on its own. A bonsai records the Tree revision and the
// digests of the Tree contracts it was shaped by; this module reads a bonsai
// from disk, checks it and its export against the contracts in the current
// checkout, and reports what has moved since the graft: changed contracts,
// identities the bonsai calls resolved that the Tree no longer has, and
// identities it would mint that the Tree now holds.

import {createHash} from 'node:crypto';
import {existsSync,readFileSync,readdirSync,statSync} from 'node:fs';
import {join} from 'node:path';
import {chronologyIssues} from './chronology.mjs';
import {contractOf,exportBonsai} from './export.mjs';
import {placesIssues} from './places.mjs';
import {createValidator} from './schema.mjs';

export const MANIFEST_SCHEMA='tos.bonsai.v1';
const json=path=>JSON.parse(readFileSync(path,'utf8'));
const sha256=path=>createHash('sha256').update(readFileSync(path)).digest('hex');

/** A bonsai's manifest and its modules by id, read from `<root>/bonsai/<id>/`. */
export function loadBonsai(root,id){
  const home=join(root,'bonsai',id),manifest=json(join(home,'bonsai.json')),modules={};
  for(const module of manifest.modules??[])if(module.path&&!module.borrowed)modules[module.id]=json(join(home,module.path));
  return {home,manifest,modules};
}

/** Every bonsai in the repository. */
export function bonsaiIds(root){
  const dir=join(root,'bonsai');
  return readdirSync(dir).filter(name=>existsSync(join(dir,name,'bonsai.json'))).sort();
}

/** A validator over the Tree contracts in this checkout. */
export function treeContracts(root){
  const dir=join(root,'ToS','contracts');
  return createValidator(readdirSync(dir).filter(name=>name.endsWith('.schema.json')).map(name=>json(join(dir,name))));
}

/** Record identities the Tree holds in this checkout, or null where its source witnesses are not checked out. */
export function treeIdentities(root){
  const base=join(root,'ToS','source-witnesses');
  if(!existsSync(join(base,'works')))return null;
  const ids=new Set();
  const walk=(dir,file,depth)=>{
    if(!existsSync(dir))return;
    for(const name of readdirSync(dir)){
      const path=join(dir,name);
      if(name===file){try{const id=json(path).record_id;if(typeof id==='string')ids.add(id);}catch{}}
      else if(depth>0&&statSync(path).isDirectory())walk(path,file,depth-1);
    }
  };
  walk(join(base,'works'),'work.json',2);walk(join(base,'agents'),'agent.json',1);walk(join(base,'places'),'place.json',1);
  return ids;
}

/** Problems that make a bonsai or its export unfit for the Tree, and what has moved since the graft. */
export function checkBonsai(root,id,{validator=treeContracts(root),identities=treeIdentities(root)}={}){
  const {manifest,modules}=loadBonsai(root,id),problems=[],drift=[];
  if(manifest.schema!==MANIFEST_SCHEMA)problems.push('manifest schema');
  if(manifest.id!==id)problems.push('manifest id');
  if(modules.chronology)problems.push(...chronologyIssues(modules.chronology).map(issue=>`chronology: ${issue}`));
  if(modules.places)problems.push(...placesIssues(modules.places,modules.chronology??null).map(issue=>`places: ${issue}`));
  for(const module of manifest.modules??[])if(module.borrowed){for(const path of [].concat(module.borrowed))if(!existsSync(join(root,path)))problems.push(`borrowed module ${module.id}: ${path} is missing`);}
  let packet=null;
  if(modules.chronology){
    packet=exportBonsai({manifest,chronology:modules.chronology,places:modules.places??null,identities:modules.identities??{}});
    for(const item of [...packet.records,...packet.claims]){
      const contract=contractOf(item);if(!contract){problems.push(`export: no Tree contract for ${item.schema_version}`);continue;}
      for(const error of validator.validate(contract,item))problems.push(`export ${item.claim_id??item.record_id}${error.path}: ${error.message}`);
    }
  }
  for(const [name,entry] of Object.entries(manifest.graft?.contracts??{})){
    const path=join(root,entry.path);
    if(!existsSync(path))drift.push(`contract ${name}: ${entry.path} is gone`);
    else if(sha256(path)!==entry.sha256)drift.push(`contract ${name}: ${entry.path} changed since the graft`);
  }
  if(identities&&packet)for(const item of packet.identities){
    if(!/^(work|agent|place)$/.test(item.type))continue;
    if(item.status==='resolved'&&!identities.has(item.tree))drift.push(`identity ${item.tree}: resolved in the bonsai, absent from the Tree`);
    if(item.status==='to-mint'&&identities.has(item.tree))drift.push(`identity ${item.tree}: the Tree now holds it; mark it resolved`);
  }
  return {id,problems,drift,packet,checkedIdentities:!!identities};
}

/** The digest the manifest records for a Tree contract. */
export const contractDigest=(root,path)=>sha256(join(root,path));
