// How a bonsai stands to the Tree it was grafted from (Node only).
//
// The Tree grows on its own. A bonsai records the Tree revision and the
// digests of the Tree contracts and registries it was shaped by; this module
// reads a bonsai from disk, checks it and its export against the contracts and
// the relation registry in the current checkout, and reports what has moved
// since the graft: changed contracts, identities the bonsai calls resolved that
// the Tree no longer has, and identities it would mint that the Tree now holds.

import {createHash} from 'node:crypto';
import {existsSync,readFileSync,readdirSync,statSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {chronologyIssues} from './chronology.mjs';
import {contractOf,exportBonsai} from './export.mjs';
import {placesIssues} from './places.mjs';
import {createValidator} from './schema.mjs';
import {corpusAlignmentIssues,externalPlacesIssues} from './fragments.mjs';
import {objectsIssues,semanticRegistry} from './objects.mjs';
import {citeIssues,readChapter,witnessesIssues} from './witnesses.mjs';

export const MANIFEST_SCHEMA='tos.bonsai.v1';
const json=path=>JSON.parse(readFileSync(path,'utf8'));
const sha256=path=>createHash('sha256').update(readFileSync(path)).digest('hex');

/** A bonsai's manifest and its modules by id, read from `<root>/bonsai/<id>/`. */
export function loadBonsai(root,id){
  const home=join(root,'bonsai',id),manifest=json(join(home,'bonsai.json')),modules={};
  for(const module of manifest.modules??[])if(module.path&&!module.borrowed)modules[module.id]=json(join(home,module.path));
  return {home,manifest,modules};
}

/** The chapter files of a bonsai's witnesses module, by chapter key. */
export function loadWitnessChapters(home,manifest){
  const module=(manifest.modules??[]).find(item=>item.id==='witnesses');
  if(!module?.chapters)return {};
  const dir=join(home,dirname(module.chapters));
  if(!existsSync(dir))return {};
  return Object.fromEntries(readdirSync(dir).filter(name=>name.endsWith('.json')).sort().map(name=>[name.slice(0,-5),json(join(dir,name))]));
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

/**
 * The Tree's relation registry in this checkout, read as a check on exported
 * claims: the relation a claim's predicate maps to must take the claim's
 * schema and assertion layer, and the kinds of the identities it names must
 * fall within its domain and range (through the entity registry's parents).
 */
export function treeRelations(root){
  const registry=semanticRegistry(root),kinds=new Map(),byPredicate=new Map();
  for(const type of registry.types)for(const mapping of type.source_mappings??[])if(mapping.source_graph==='source-claims')kinds.set(mapping.source_kind_id,type.type_id);
  for(const relation of registry.relations)for(const mapping of relation.source_mappings??[])if(mapping.source_graph==='source-claims'&&mapping.scope==='claim-predicate')byPredicate.set(mapping.source_predicate_id,relation);
  const within=(kind,allowed)=>{const typeId=kinds.get(kind);return !!typeId&&registry.within(typeId,allowed);};
  return {
    /** Where a claim departs from the registry; `kindOf` gives the kind of an identity the export lists. */
    issues(claim,kindOf){
      const relation=byPredicate.get(claim?.predicate);if(!relation)return [`predicate ${claim?.predicate} is not in the Tree's relation registry`];
      const profile=relation.source_claim_profile;if(!profile)return [`predicate ${claim.predicate} takes no source claims`];
      const issues=[],schemas=(profile.schemas??[]).map(entry=>entry.schema_version);
      if(!schemas.includes(claim.schema_version))issues.push(`${claim.predicate} takes ${schemas.join(' or ')}, not ${claim.schema_version}`);
      if(!(profile.assertion_layers??[]).includes(claim.assertion_layer))issues.push(`${claim.predicate} takes the layer ${(profile.assertion_layers??[]).join(' or ')}, not ${claim.assertion_layer}`);
      const subject=kindOf(claim.subject_ref);
      if(!within(subject,relation.domain_type_ids))issues.push(`a ${subject??'unlisted identity'} is outside the domain of ${claim.predicate}`);
      if(typeof claim.object==='string'){const object=kindOf(claim.object);if(!within(object,relation.range_type_ids))issues.push(`a ${object??'unlisted identity'} is outside the range of ${claim.predicate}`);}
      return issues;
    },
  };
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
  walk(join(base,'works'),'work.json',2);walk(join(base,'works'),'expression.json',4);walk(join(base,'agents'),'agent.json',1);walk(join(base,'places'),'place.json',1);
  return ids;
}

/**
 * A check of a place a bonsai cites: a place in its witnesses against the chapter it names, a place of another work
 * against the places the bonsai keeps for it. A place in a public corpus work is checked by the stand that reads it.
 */
export function placeChecker(chapters,fragments){
  const read=new Map(),chapterOf=key=>{if(!read.has(key))read.set(key,chapters[key]?readChapter(chapters[key]):null);return read.get(key);};
  return cite=>{
    if(!cite||typeof cite!=='object'||!cite.w)return ['not a citation'];
    if(cite.c){const chapter=chapterOf(cite.c);return chapter?citeIssues(cite,chapter):[`no chapter ${cite.c} in the witnesses`];}
    if(cite.loc)return fragments?.places?.[cite.w]?.[cite.loc]?[]:[`no place ${cite.w} ${cite.loc} among the places of other works`];
    return Array.isArray(cite.u)&&cite.u.length===2?[]:['a corpus place names no units'];
  };
}

/** Problems that make a bonsai or its export unfit for the Tree, and what has moved since the graft. */
export function checkBonsai(root,id,{validator=treeContracts(root),relations,identities=treeIdentities(root)}={}){
  const {home,manifest,modules}=loadBonsai(root,id),problems=[],drift=[];
  // Without a readable relation registry the export's predicates go unchecked, which the check says rather than hides.
  if(relations===undefined){try{relations=treeRelations(root);}catch(error){relations=null;problems.push(`relation registry: ${error.message}`);}}
  if(manifest.schema!==MANIFEST_SCHEMA)problems.push('manifest schema');
  if(manifest.id!==id)problems.push('manifest id');
  if(modules.chronology)problems.push(...chronologyIssues(modules.chronology).map(issue=>`chronology: ${issue}`));
  if(modules.places)problems.push(...placesIssues(modules.places,modules.chronology??null).map(issue=>`places: ${issue}`));
  const chapters=modules.witnesses?loadWitnessChapters(home,manifest):{};
  if(modules.witnesses)problems.push(...witnessesIssues(modules.witnesses,chapters).map(issue=>`witnesses: ${issue}`));
  if(modules.fragments)problems.push(...externalPlacesIssues(modules.fragments).map(issue=>`fragments: ${issue}`));
  if(modules.alignment)problems.push(...corpusAlignmentIssues(modules.alignment).map(issue=>`alignment: ${issue}`));
  if(modules.objects){
    let registry=null;try{registry=semanticRegistry(root);}catch(error){problems.push(`entity registry: ${error.message}`);}
    if(registry)problems.push(...objectsIssues(modules.objects,registry,placeChecker(chapters,modules.fragments??null)).map(issue=>`objects: ${issue}`));
  }
  for(const module of manifest.modules??[])if(module.borrowed){for(const path of [].concat(module.borrowed))if(!existsSync(join(root,path)))problems.push(`borrowed module ${module.id}: ${path} is missing`);}
  let packet=null;
  if(modules.chronology){
    packet=exportBonsai({manifest,chronology:modules.chronology,places:modules.places??null,identities:modules.identities??{}});
    for(const item of [...packet.records,...packet.claims]){
      const contract=contractOf(item);if(!contract){problems.push(`export: no Tree contract for ${item.schema_version}`);continue;}
      for(const error of validator.validate(contract,item))problems.push(`export ${item.claim_id??item.record_id}${error.path}: ${error.message}`);
    }
    const kinds=new Map(packet.identities.map(item=>[item.tree,item.type]));
    if(relations)for(const claim of packet.claims)for(const issue of relations.issues(claim,ref=>kinds.get(ref)))problems.push(`export ${claim.claim_id}: ${issue}`);
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
  // The witnesses' editions are Tree expressions; the graft map says which, and the check sees whether the Tree still holds them.
  if(identities)for(const [key,entry] of Object.entries(modules.identities?.expressions??{})){
    if(entry.status==='resolved'&&!identities.has(entry.tree))drift.push(`expression ${entry.tree} (${key}): resolved in the bonsai, absent from the Tree`);
    if(entry.status==='to-mint'&&identities.has(entry.tree))drift.push(`expression ${entry.tree} (${key}): the Tree now holds it; mark it resolved`);
  }
  return {id,problems,drift,packet,checkedIdentities:!!identities};
}

/** The digest the manifest records for a Tree contract. */
export const contractDigest=(root,path)=>sha256(join(root,path));
