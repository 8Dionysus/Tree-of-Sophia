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
import {corpusAlignmentIssues,externalPlacesIssues,translationScopesIssues} from './fragments.mjs';
import {objectsIssues,semanticRegistry} from './objects.mjs';
import {coreIssues,loadCore} from './core.mjs';
import {citeIssues,readChapter,witnessesIssues} from './witnesses.mjs';
import {termsIssues} from './terms.mjs';
import {isLayerPath,layerStatus,layersIssues,layersOnDisk,readLayers} from './layers.mjs';
import {renderingsIssues,spellingIssues,termFormGaps,tokenChapterIssues,TOKENS_SCHEMA} from './tokens.mjs';
import {corpusReader} from './corpus-align.mjs';
import {mediaIssues} from './media.mjs';
import {fieldCites,fieldIssues,fieldSubjects,libraryIssues} from './field.mjs';

export const MANIFEST_SCHEMA='tos.bonsai.v1';
const json=path=>JSON.parse(readFileSync(path,'utf8'));
const sha256=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
const textDigest=text=>createHash('sha256').update(text,'utf8').digest('hex');

/** A bonsai's manifest and its modules by id, read from `<root>/bonsai/<id>/`. */
export function loadBonsai(root,id,overrides={}){
  const home=join(root,'bonsai',id),manifest=json(join(home,'bonsai.json')),modules={},layers=readLayers(home);
  for(const module of manifest.modules??[]){
    if(!module.path||module.borrowed)continue;
    // A generated layer kept in R2 (bonsai/lib/layers.mjs) is read where a checkout has fetched it.
    if(isLayerPath(layers,module.path)&&!existsSync(join(home,module.path)))continue;
    modules[module.id]=Object.hasOwn(overrides,module.id)?overrides[module.id]:json(join(home,module.path));
  }
  return {home,manifest,modules,layers};
}

/** The chapter files of a bonsai's witnesses module, by chapter key. */
export function loadWitnessChapters(home,manifest){
  const module=(manifest.modules??[]).find(item=>item.id==='witnesses');
  if(!module?.chapters)return {};
  const dir=join(home,dirname(module.chapters));
  if(!existsSync(dir))return {};
  return Object.fromEntries(readdirSync(dir).filter(name=>name.endsWith('.json')).sort().map(name=>[name.slice(0,-5),json(join(dir,name))]));
}

/** The chapters of a bonsai's word layer, keyed like its witness chapters. */
export function loadTokenChapters(home,manifest){
  const module=(manifest.modules??[]).find(item=>item.id==='tokens');
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
      if(name===file){try{const record=json(path),id=record.record_id??record.artifact_id??record.representation_id??record.file_id;if(typeof id==='string')ids.add(id);}catch{}}
      else if(depth>0&&statSync(path).isDirectory())walk(path,file,depth-1);
    }
  };
  walk(join(base,'works'),'work.json',2);walk(join(base,'works'),'expression.json',4);walk(join(base,'agents'),'agent.json',1);walk(join(base,'places'),'place.json',1);
  walk(join(base,'artifacts'),'artifact-witness.json',3);walk(join(base,'artifacts'),'representation.json',5);
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

/** Whether this checkout holds the public corpus snapshot's files, kept in R2 behind access/public-corpus/layers.json. */
export function corpusOnDisk(root){
  const home=join(root,'access/public-corpus'),layers=readLayers(home);
  return layers?layersOnDisk(home,layers):'present';
}

/** Read the exact original span shown by a stand for a corpus or external place; a corpus place is unchecked where the corpus is absent. */
export function termPlaceReader(root,alignment,fragments,{corpus=corpusOnDisk(root)}={}){
  let read;
  return (cite,language)=>{
    if(!cite?.w)return {error:'no work'};
    let units;
    if(cite.loc){
      const work=fragments?.works?.[cite.w],value=fragments?.places?.[cite.w]?.[cite.loc]?.texts?.[language];
      if(work?.original!==language||typeof value!=='string')return {error:`no original ${cite.w} ${cite.loc}`};
      units=[value];
    }else{
      const work=alignment?.works?.[cite.w];
      if(work?.original?.language!==language||!Array.isArray(cite.u)||cite.u.length!==2)return {error:`no original units for ${cite.w}`};
      if(corpus==='absent')return {unchecked:true};
      try{
        read??=corpusReader(join(root,'access/web/public/public-corpus'));
        const version=read(work.documentId)?.versions.find(item=>item.id===work.original.versionId);
        const first=version?.units.findIndex(unit=>unit.id===cite.u[0])??-1,last=version?.units.findIndex(unit=>unit.id===cite.u[1])??-1;
        if(first<0||last<first)return {error:`unknown or unordered units for ${cite.w}`};
        units=version.units.slice(first,last+1).map(unit=>unit.text);
      }catch(error){return {error:`corpus ${cite.w}: ${error.message}`};}
    }
    const start=cite.s??0,end=cite.e??Array.from(units.at(-1)).length;
    if(!Number.isInteger(start)||!Number.isInteger(end)||start<0||end<0||start>=Array.from(units[0]).length||end>Array.from(units.at(-1)).length||(units.length===1&&start>=end))return {error:`invalid offsets for ${cite.w}`};
    const firstChars=Array.from(units[0]),lastChars=Array.from(units.at(-1));
    return {text:units.map((unit,index)=>Array.from(unit).slice(index===0?start:0,index===units.length-1?end:undefined).join('')).join(' '),before:firstChars[start-1]??' ',after:lastChars[end]??' '};
  };
}

/** Problems that make a bonsai or its export unfit for the Tree, and what has moved since the graft. */
export function checkBonsai(root,id,{validator=treeContracts(root),relations,identities=treeIdentities(root),overrides,mediaLayers}={}){
  const {home,manifest,modules,layers}=loadBonsai(root,id,overrides),problems=[],drift=[];
  let corpus=null;
  // The layers kept in R2: their manifest always; their files, checked like any module, where all are present.
  let layerFiles=null;
  if(layers){
    problems.push(...layersIssues(layers).map(issue=>`layers: ${issue}`));
    const states=layerStatus(home,layers),missing=states.filter(entry=>entry.state==='missing');
    for(const entry of states)if(entry.state==='different')problems.push(`layers: ${entry.path} differs from layers.json; write the manifest again`);
    if(missing.length&&missing.length<states.length)problems.push(`layers: ${missing.length} of ${states.length} files are missing; fetch them`);
    layerFiles=missing.length===states.length?'absent':'present';
  }
  // Without a readable relation registry the export's predicates go unchecked, which the check says rather than hides.
  if(relations===undefined){try{relations=treeRelations(root);}catch(error){relations=null;problems.push(`relation registry: ${error.message}`);}}
  if(manifest.schema!==MANIFEST_SCHEMA)problems.push('manifest schema');
  if(manifest.id!==id)problems.push('manifest id');
  if(modules.chronology)problems.push(...chronologyIssues(modules.chronology).map(issue=>`chronology: ${issue}`));
  if(modules.places)problems.push(...placesIssues(modules.places,modules.chronology??null).map(issue=>`places: ${issue}`));
  const chapters=modules.witnesses?loadWitnessChapters(home,manifest):{};
  if(modules.witnesses)problems.push(...witnessesIssues(modules.witnesses,chapters).map(issue=>`witnesses: ${issue}`));
  // The philological core: each term's first place and the translations' words there (bonsai/lib/terms.mjs).
  if(modules.terms){
    if(!modules.witnesses&&modules.alignment)corpus=corpusOnDisk(root);
    const options=!modules.witnesses&&modules.alignment?{pivot:['grc','la'],place:termPlaceReader(root,modules.alignment,modules.fragments,{corpus})}:{};
    problems.push(...termsIssues(modules.terms,chapters,options).map(issue=>`terms: ${issue}`));
  }
  // The word layer (bonsai/lib/tokens.mjs): every word at its place, bound to the witness text it was made from.
  if(modules.tokens){
    if(modules.tokens.schema!==TOKENS_SCHEMA)problems.push('tokens: schema');
    const tokenChapters=loadTokenChapters(home,manifest),sha256=text=>createHash('sha256').update(text).digest('hex');
    for(const key of Object.keys(chapters)){
      if(!tokenChapters[key]){problems.push(`tokens: no words for chapter ${key}`);continue;}
      problems.push(...tokenChapterIssues(tokenChapters[key],chapters[key],sha256).map(issue=>`tokens: ${issue}`));
    }
    // Forms of a term the layer finds under its lemma and the term does not list: a report, not a failure.
    if(modules.terms)for(const [id,forms] of Object.entries(termFormGaps(modules.terms,tokenChapters,chapters,modules.spelling??null)))
      drift.push(`terms: ${id} occurs also as ${Object.entries(forms).map(([form,count])=>`${form} ×${count}`).join(', ')}`);
  }
  if(modules.spelling)problems.push(...spellingIssues(modules.spelling).map(issue=>`spelling: ${issue}`));
  if(modules.renderings)problems.push(...renderingsIssues(modules.renderings,chapters).map(issue=>`renderings: ${issue}`));
  if(modules.fragments)problems.push(...externalPlacesIssues(modules.fragments).map(issue=>`fragments: ${issue}`));
  if(modules['translation-scopes'])problems.push(...translationScopesIssues(modules['translation-scopes']).map(issue=>`translation scopes: ${issue}`));
  if(modules.alignment)problems.push(...corpusAlignmentIssues(modules.alignment).map(issue=>`alignment: ${issue}`));
  if(modules.media){
    // A medium's subject is a collection entry where the stand keeps a collection, and a core record where it keeps a
    // core: a concept, a figure, a proposed node, a state of a chain, or a link between two of them.
    const coreModule=(manifest.modules??[]).find(module=>module.id==='core'),core=coreModule?loadCore(join(home,dirname(coreModule.path))):null;
    const subjects=Array.isArray(modules.collection?.entities)?new Set(modules.collection.entities.map(entry=>entry.id))
      :core?new Set([...core.concepts,...core.figures,...core.proposed,...core.chains.flatMap(chain=>chain.states??[]),...core.links].map(record=>record.id))
      :modules.field?fieldSubjects(modules.field,modules.library??null):null;
    problems.push(...mediaIssues(modules.media,{subjects,identities:modules.identities??{},digest:textDigest,layers:mediaLayers??layers}).map(issue=>`media: ${issue}`));
  }
  let registry=null;
  if(modules.objects||modules.core||modules.field){try{registry=semanticRegistry(root);}catch(error){problems.push(`entity registry: ${error.message}`);}}
  if(modules.objects&&registry)problems.push(...objectsIssues(modules.objects,registry,placeChecker(chapters,modules.fragments??null)).map(issue=>`objects: ${issue}`));
  // The core of a field (bonsai/CORE.md, "A core across works"): its areas, concepts, conceptions and links.
  if(modules.library)problems.push(...libraryIssues(modules.library).map(issue=>`library: ${issue}`));
  if(modules.field){
    const cited=new Set([...Object.keys(modules.identities?.works??{}),...Object.keys(modules.fragments?.works??{})]);
    problems.push(...fieldIssues(modules.field,{registry,library:modules.library??null,works:cited,placeIssues:placeChecker(chapters,modules.fragments??null)}).map(issue=>`field: ${issue}`));
    // The exact words of every formula are read in the original; a corpus place stays unchecked where the corpus is not fetched.
    corpus??=corpusOnDisk(root);
    const read=termPlaceReader(root,modules.alignment??null,modules.fragments??null,{corpus});
    for(const {conception,cite} of fieldCites(modules.field)){
      const language=cite.loc?modules.fragments?.works?.[cite.w]?.original:modules.alignment?.works?.[cite.w]?.original?.language;
      const place=read(cite,language);
      if(place.error)problems.push(`field: conception ${conception}: ${place.error}`);
    }
  }
  // The philosophical core (bonsai/CORE.md): its places, claims, registry relations and control vectors.
  const coreModule=(manifest.modules??[]).find(module=>module.id==='core');
  if(coreModule)problems.push(...coreIssues(loadCore(join(home,dirname(coreModule.path))),chapters,registry).map(issue=>`core: ${issue}`));
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
  // A medium's work is a Tree Artifact and its image a Tree representation; the check sees whether the Tree holds them yet.
  if(identities)for(const section of ['artifacts','representations'])for(const [key,entry] of Object.entries(modules.identities?.[section]??{})){
    if(entry.status==='resolved'&&!identities.has(entry.tree))drift.push(`${section.slice(0,-1)} ${entry.tree} (${key}): resolved in the bonsai, absent from the Tree`);
    if(entry.status==='to-mint'&&identities.has(entry.tree))drift.push(`${section.slice(0,-1)} ${entry.tree} (${key}): the Tree now holds it; mark it resolved`);
  }
  return {id,problems,drift,packet,checkedIdentities:!!identities,layers:layerFiles,corpus};
}

/** The digest the manifest records for a Tree contract. */
export const contractDigest=(root,path)=>sha256(join(root,path));
