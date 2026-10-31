import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {NODE_CONTENT} from '../../constructor/atlas-content.mjs';
import {RESEARCH_ROUTES} from '../../constructor/atlas-routes.mjs';
import {RELATION_INQUIRY} from '../../constructor/relation-inquiry.mjs';
import {INQUIRY_SOURCES_CONCEPTS} from '../../constructor/inquiry-sources-concepts.mjs';
import {INQUIRY_READINGS_ECHOES} from '../../constructor/inquiry-readings-echoes.mjs';
import {INQUIRY_FOUNDATION,LENS_INQUIRY} from '../../constructor/inquiry-foundation.mjs';
import {LINK_KINDS,objectsIssues,semanticRegistry} from '../../../../bonsai/lib/objects.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const module=JSON.parse(readFileSync(join(root,'bonsai','zarathustra','objects.json'),'utf8'));
const content={NODE_CONTENT,RESEARCH_ROUTES,RELATION_INQUIRY,INQUIRY_SOURCES_CONCEPTS,INQUIRY_READINGS_ECHOES,INQUIRY_FOUNDATION,LENS_INQUIRY};

// A form path names a module and then keys and indices; a key may itself hold dots (chapter-p3.r2), so the longest key that fits wins.
function resolve(path){
  const head=/^[A-Z_]+/.exec(path);let value=content[head[0]],rest=path.slice(head[0].length);
  while(rest&&value!==undefined){
    if(rest[0]==='['){const end=rest.indexOf(']');value=value[Number(rest.slice(1,end))];rest=rest.slice(end+1);continue;}
    rest=rest.slice(1);
    const key=Object.keys(value??{}).filter(item=>rest.startsWith(item)&&(rest.length===item.length||'.['.includes(rest[item.length]))).sort((a,b)=>b.length-a.length)[0];
    if(key===undefined)return undefined;
    value=value[key];rest=rest.slice(key.length);
  }
  return value;
}
// A form holds words or cited places, itself or in its parts (a perspective's body, an experiment's setup).
const said=value=>!!value&&typeof value==='object'&&!Array.isArray(value)&&(typeof value.ru==='string'||Array.isArray(value.cite)||Object.values(value).some(said));
const formsOf=type=>module.objects.filter(object=>object.type===type).map(object=>object.form);

describe('the typed objects of the Zarathustra stand',()=>{
  const registry=semanticRegistry(root);
  it('use only the Tree\'s entity types and relations, each relation within its domain and range',()=>{
    expect(objectsIssues(module,registry)).toEqual([]);
    expect(module.relations.length).toBeGreaterThan(0);
    for(const relation of module.relations)expect(registry.relation(relation.type)?.abstract,relation.type).toBe(false);
  });
  it('name a form of the stand that holds each object\'s words, and every relation names its basis in the stand',()=>{
    for(const object of module.objects)expect(said(resolve(object.form)),object.form).toBe(true);
    const forms=new Set(module.objects.map(object=>object.form));
    for(const relation of module.relations)expect(forms.has(relation.basis)||relation.basis in RELATION_INQUIRY,relation.basis).toBe(true);
  });
  it('type every argument, objection, experiment, question, perspective, distinction and method the stand authors',()=>{
    const expected=[];
    for(const [id,node] of Object.entries(NODE_CONTENT)){
      (node.perspectives??[]).forEach((_,i)=>expected.push(`NODE_CONTENT.${id}.perspectives[${i}]`));
      if(node.question)expected.push(`NODE_CONTENT.${id}.question`);
    }
    for(const [name,inquiries] of [['INQUIRY_SOURCES_CONCEPTS',INQUIRY_SOURCES_CONCEPTS],['INQUIRY_READINGS_ECHOES',INQUIRY_READINGS_ECHOES]])
      for(const [id,inquiry] of Object.entries(inquiries))for(const [field,form] of [['argument','argument'],['counterReading','counterReading'],['experiment','experiment'],['question','question']])if(inquiry[field])expected.push(`${name}.${id}.${form}`);
    for(const [id,inquiry] of Object.entries(RELATION_INQUIRY))if(inquiry.question)expected.push(`RELATION_INQUIRY.${id}.question`);
    RESEARCH_ROUTES.forEach((route,i)=>{if(route.question)expected.push(`RESEARCH_ROUTES[${i}].question`);});
    (INQUIRY_FOUNDATION.distinctions??[]).forEach((_,i)=>expected.push(`INQUIRY_FOUNDATION.distinctions[${i}]`));
    expected.push('INQUIRY_FOUNDATION.question','INQUIRY_FOUNDATION.practice');
    for(const [lens,value] of Object.entries(LENS_INQUIRY)){if(value.question)expected.push(`LENS_INQUIRY.${lens}.question`);if(value.method)expected.push(`LENS_INQUIRY.${lens}.method`);}
    const typed=module.objects.map(object=>object.form);
    for(const form of expected)expect(typed,form).toContain(form);
    // One object per form: a material's own object is named by its body.
    expect(new Set(typed).size).toBe(typed.length);
    expect(formsOf('tos.entity.argument')).toHaveLength(Object.values(INQUIRY_SOURCES_CONCEPTS).concat(Object.values(INQUIRY_READINGS_ECHOES)).filter(inquiry=>inquiry.argument).length);
  });
  it('type a tension as a problem articulated by its question, and its perspectives as positions that address it',()=>{
    const problem=module.objects.find(object=>object.id==='za:demo:fatalism');
    expect(problem.type).toBe('tos.entity.problem');
    expect(module.relations).toContainEqual({type:'tos.relation.problem-has-question',from:'za:demo:fatalism',to:'za:demo:fatalism/question',basis:'NODE_CONTENT.demo:fatalism.question'});
    expect(module.relations.filter(relation=>relation.to==='za:demo:fatalism'&&relation.type==='tos.relation.position-addresses-problem')).toHaveLength(NODE_CONTENT['demo:fatalism'].perspectives.length);
  });
  it('keep every link between two places of the text with both places shown beside its words',()=>{
    expect(module.links.length).toBeGreaterThan(0);
    const same=(a,b)=>a.w===b.w&&a.c===b.c&&(a.loc??null)===(b.loc??null);
    for(const link of module.links){
      expect(LINK_KINDS,link.form).toContain(link.kind);
      const form=resolve(link.form),cites=(form?.cite??[]).concat(link.form.endsWith('.focus')?resolve(link.form.replace(/\.focus$/,''))?.cite??[]:[]);
      // Each end is shown by a citation of the same place, or one that covers it.
      for(const end of [link.from,link.to])expect(cites.some(cite=>same(cite,end)),`${link.form} → ${JSON.stringify(end)}`).toBe(true);
    }
  });
});

describe('the typed objects of the Antiquity stand',()=>{
  const registry=semanticRegistry(root);
  const objects=JSON.parse(readFileSync(join(root,'bonsai','antiquity','objects.json'),'utf8'));
  const collection=JSON.parse(readFileSync(join(root,'bonsai','antiquity','collection.json'),'utf8'));
  const at=path=>{let value={collection};for(const part of path.match(/[^.[\]]+/g))value=value?.[/^\d+$/.test(part)?Number(part):part];return value;};
  it('use only the Tree\'s entity types and relations, each relation within its domain and range',()=>{
    expect(objectsIssues(objects,registry)).toEqual([]);
  });
  it('name a form of the collection that holds each object\'s words',()=>{
    for(const object of objects.objects)expect(at(object.form),object.form).toBeTruthy();
    const typed=objects.objects.map(object=>object.form);
    expect(new Set(typed).size).toBe(typed.length);
  });
  it('set out every argument with its theses, its inference steps from premises to conclusion and the objections to its theses',()=>{
    const arguments_=collection.passages.filter(passage=>passage.argument);
    expect(objects.objects.filter(object=>object.type==='tos.entity.argument')).toHaveLength(arguments_.length);
    const theses=new Set(objects.objects.filter(object=>object.type==='tos.entity.thesis').map(object=>object.id));
    for(const relation of objects.relations.filter(item=>item.type==='tos.relation.step-has-premise'||item.type==='tos.relation.step-has-conclusion'))expect(theses.has(relation.to),relation.to).toBe(true);
    // A conclusion is what its argument is for.
    for(const object of objects.objects.filter(item=>item.role==='conclusion'))expect(objects.relations.some(relation=>relation.type==='tos.relation.argument-for-thesis'&&relation.to===object.id),object.id).toBe(true);
    for(const question of collection.questions)expect(objects.objects.some(object=>object.id===`an:${question.id}`&&object.type==='tos.entity.problem'),question.id).toBe(true);
  });
});
