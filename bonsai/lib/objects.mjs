// What a stand's own words are, in the Tree's object types (Node only).
//
// `objects.json` (tos.bonsai.objects.v1) types the authored forms of a stand in
// the Tree's entity registry — a problem and the question that articulates it,
// a concept and its aspects, a conception, a position, an argument and the
// objection to it, a distinction, a method or an image of thought, a thought
// experiment — names the form that holds the words and the places of text the
// object rests on, and joins objects by relations of the Tree's registry only
// where the stand's own structure says so. Links between places of texts (one
// place repeats, varies, develops, contrasts with or echoes another) have no
// relation in the registry yet and are kept as the stand's proposal. Nothing
// here is admitted.

import {readFileSync} from 'node:fs';
import {join} from 'node:path';

export const OBJECTS_SCHEMA='tos.bonsai.objects.v1';
export const LINK_KINDS=Object.freeze(['repeats','varies','develops','contrasts','echoes','answers','recalls','prepares']);

/** The Tree's entity and relation registries in this checkout: type lineage through parents and the ends of each relation. */
export function semanticRegistry(root){
  const dir=join(root,'ToS','doctrine','semantic-interchange'),read=name=>JSON.parse(readFileSync(join(dir,name),'utf8'));
  const types=read('entity-types.v1.json').types??[],relations=read('relation-types.v1.json').relations??[];
  const parents=new Map(types.map(type=>[type.type_id,type.parent_type_ids??[]])),byId=new Map(relations.map(relation=>[relation.relation_type_id,relation]));
  const lineage=typeId=>{const seen=new Set(),stack=[typeId];while(stack.length){const id=stack.pop();if(id&&!seen.has(id)){seen.add(id);stack.push(...(parents.get(id)??[]));}}return seen;};
  return {
    types,relations,lineage,
    hasType:typeId=>parents.has(typeId),
    relation:relationId=>byId.get(relationId)??null,
    /** Whether a type falls, through its parents, within one of the allowed types. */
    within:(typeId,allowed)=>{if(!parents.has(typeId))return false;const line=lineage(typeId);return (allowed??[]).some(id=>line.has(id));},
  };
}

/**
 * Problems of a stand's typed objects against the Tree's registries. `placeIssues`, when given, checks a cited
 * place (a ground of an object or an end of a link) against the stand's witnesses and places of other works.
 */
export function objectsIssues(module,registry,placeIssues=null){
  const problems=[],byId=new Map(),places=(label,list)=>{if(placeIssues)for(const cite of list??[])for(const issue of placeIssues(cite))problems.push(`${label}: ${issue}`);};
  if(module?.schema!==OBJECTS_SCHEMA)problems.push(`schema is not ${OBJECTS_SCHEMA}`);
  for(const object of module?.objects??[]){
    if(byId.has(object.id))problems.push(`duplicate object ${object.id}`);
    byId.set(object.id,object);
    if(!registry.hasType(object.type))problems.push(`${object.id}: ${object.type} is not in the Tree's entity registry`);
    if(typeof object.form!=='string'||!object.form)problems.push(`${object.id}: names no form`);
    places(object.id,object.grounds);
  }
  for(const relation of module?.relations??[]){
    const type=registry.relation(relation.type),from=byId.get(relation.from),to=byId.get(relation.to);
    if(!type){problems.push(`${relation.type} is not in the Tree's relation registry`);continue;}
    if(type.abstract)problems.push(`${relation.type} is abstract`);
    if(!from||!to){problems.push(`${relation.type}: unknown object ${from?relation.to:relation.from}`);continue;}
    if(!registry.within(from.type,type.domain_type_ids))problems.push(`${relation.type} ${relation.from}: ${from.type} is outside its domain`);
    if(!registry.within(to.type,type.range_type_ids))problems.push(`${relation.type} ${relation.to}: ${to.type} is outside its range`);
    if(!relation.basis)problems.push(`${relation.type} ${relation.from} → ${relation.to} names no basis in the stand`);
  }
  for(const link of module?.links??[]){
    if(!LINK_KINDS.includes(link.kind))problems.push(`link in ${link.form}: unknown kind ${link.kind}`);
    if(!link.from||!link.to){problems.push(`link in ${link.form}: an end is missing`);continue;}
    places(`link in ${link.form}`,[link.from,link.to]);
  }
  return problems;
}
