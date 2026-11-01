// Types the Zarathustra stand's own words again after a rebuild: bonsai/zarathustra/objects.json.
//
//   node objects.mjs            # write the module
//   node objects.mjs --check    # report what would change
//
// The rules are those of the module itself (bonsai/README.md, «Objects of thought»): a material's
// body is its object, typed by the material's kind; a concept's perspectives are its aspects, a
// conception's perspectives are aspects it has, and a tension's perspectives are positions that
// address the problem its question articulates; an inquiry has its argument and, where it has one,
// the objection to it; every question of a material, a relation, a route or a lens is a question.
// Relations come from the stand's structure only. Links between two places of the text are the
// authored ones of the earlier module whose words still stand and still show both places.
import fs from 'node:fs';

const ROOT=new URL('../../../',import.meta.url).pathname;
const C=ROOT+'access/web/constructor/';
const FILE=ROOT+'bonsai/zarathustra/objects.json';
const {NODE_CONTENT}=await import(C+'atlas-content.mjs');
const {RESEARCH_ROUTES}=await import(C+'atlas-routes.mjs');
const {RELATION_INQUIRY}=await import(C+'relation-inquiry.mjs');
const {INQUIRY_SOURCES_CONCEPTS}=await import(C+'inquiry-sources-concepts.mjs');
const {INQUIRY_READINGS_ECHOES}=await import(C+'inquiry-readings-echoes.mjs');
const {INQUIRY_FOUNDATION,LENS_INQUIRY}=await import(C+'inquiry-foundation.mjs');
const content={NODE_CONTENT,RESEARCH_ROUTES,RELATION_INQUIRY,INQUIRY_SOURCES_CONCEPTS,INQUIRY_READINGS_ECHOES,INQUIRY_FOUNDATION,LENS_INQUIRY};
const earlier=JSON.parse(fs.readFileSync(FILE,'utf8'));

// The kind of each prepared material, from the rows of atlas-data.mjs; the sources are fragments of the work.
const data=fs.readFileSync(C+'atlas-data.mjs','utf8');
const at=data.indexOf('function makeNodes('),open=data.indexOf('const rows = [',at)+'const rows = ['.length,close=data.indexOf('\n  ];',open);
const kindOf=new Map(data.slice(open,close).split('\n').map(line=>line.trim()).filter(line=>line.startsWith('[')).map(line=>JSON.parse(line.replace(/,\s*$/,''))).map(row=>['demo:'+row[0],row[1]]));
const TYPES={concept:'tos.entity.crosscutting-concept',interpretation:'tos.entity.conception',question:'tos.entity.problem',symbol:'tos.entity.thought-image',character:'tos.entity.thought-image',figure:'tos.entity.figure',tradition:'tos.entity.intellectual-tradition',note:'tos.entity.thought-method',excerpt:'tos.entity.textual-fragment'};
const typeOf=id=>TYPES[kindOf.get(id)]??'tos.entity.textual-fragment';

// The places a form rests on: every citation inside it, once.
function placesIn(value){
  const out=[],seen=new Set();
  const walk=item=>{
    if(Array.isArray(item)){item.forEach(walk);return;}
    if(!item||typeof item!=='object')return;
    if(typeof item.w==='string'&&(item.u||item.loc)){const key=JSON.stringify(item);if(!seen.has(key)){seen.add(key);out.push(item);}return;}
    Object.values(item).forEach(walk);
  };
  walk(value);return out;
}
const objects=[],relations=[];
function add(id,type,form,value,material){
  const object={id,type,form};
  if(material)object.material=material;
  const grounds=placesIn(value);if(grounds.length)object.grounds=grounds;
  objects.push(object);return object;
}
const relate=(type,from,to,basis)=>relations.push({type,from,to,basis});

for(const [id,node] of Object.entries(NODE_CONTENT)){
  const self=add('za:'+id,typeOf(id),`NODE_CONTENT.${id}.body`,node.body,id);
  (node.perspectives??[]).forEach((perspective,i)=>{
    const form=`NODE_CONTENT.${id}.perspectives[${i}]`,pid=`za:${id}/perspective/${i}`;
    if(self.type==='tos.entity.problem'){add(pid,'tos.entity.position',form,perspective,id);relate('tos.relation.position-addresses-problem',pid,self.id,form);return;}
    add(pid,'tos.entity.aspect',form,perspective,id);
    if(self.type==='tos.entity.crosscutting-concept')relate('tos.relation.aspect-of-concept',pid,self.id,form);
    if(self.type==='tos.entity.conception')relate('tos.relation.conception-has-aspect',self.id,pid,form);
  });
  if(node.question){const form=`NODE_CONTENT.${id}.question`;add(`za:${id}/question`,'tos.entity.question',form,null,id);if(self.type==='tos.entity.problem')relate('tos.relation.problem-has-question',self.id,`za:${id}/question`,form);}
}
for(const [name,layer] of [['INQUIRY_SOURCES_CONCEPTS',INQUIRY_SOURCES_CONCEPTS],['INQUIRY_READINGS_ECHOES',INQUIRY_READINGS_ECHOES]]){
  for(const [id,inquiry] of Object.entries(layer)){
    const base=`${name}.${id}`;
    // An argument rests on its own cited places and on the inquiry's grounds.
    if(inquiry.argument)add(`za:${id}/argument`,'tos.entity.argument',`${base}.argument`,[inquiry.argument,(inquiry.grounds??[]).map(ground=>ground.cite??[])],id);
    if(inquiry.counterReading){add(`za:${id}/objection`,'tos.entity.objection',`${base}.counterReading`,inquiry.counterReading,id);if(inquiry.argument)relate('tos.relation.objection-to-argument',`za:${id}/objection`,`za:${id}/argument`,`${base}.counterReading`);}
    if(inquiry.experiment)add(`za:${id}/experiment`,'tos.entity.thought-experiment',`${base}.experiment`,inquiry.experiment,id);
    if(inquiry.question){add(`za:${id}/inquiry-question`,'tos.entity.question',`${base}.question`,null,id);if(typeOf(id)==='tos.entity.problem')relate('tos.relation.problem-has-question','za:'+id,`za:${id}/inquiry-question`,`${base}.question`);}
  }
}
for(const [edgeId,inquiry] of Object.entries(RELATION_INQUIRY)){
  if(inquiry.question)add(`za:${edgeId}/question`,'tos.entity.question',`RELATION_INQUIRY.${edgeId}.question`,null,edgeId);
  // An interpretation interprets a concept; an image supports or develops a reading it presents.
  const [,from,kind,to]=edgeId.split(':'),ends=[from,to].map(slug=>NODE_CONTENT['demo:'+slug]?'demo:'+slug:slug);
  const [a,b]=ends.map(typeOf);
  if(kind==='interprets'&&a==='tos.entity.conception'&&b==='tos.entity.crosscutting-concept')relate('tos.relation.conception-of','za:'+ends[0],'za:'+ends[1],edgeId);
  if((kind==='supports'||kind==='develops')&&a==='tos.entity.thought-image'&&b==='tos.entity.conception')relate('tos.relation.image-presents-conception','za:'+ends[0],'za:'+ends[1],edgeId);
}
RESEARCH_ROUTES.forEach((route,i)=>{if(route.question)add(`za:route:${route.id}/question`,'tos.entity.question',`RESEARCH_ROUTES[${i}].question`,null,'route:'+route.id);});
add('za:foundation/question','tos.entity.question','INQUIRY_FOUNDATION.question',null);
(INQUIRY_FOUNDATION.distinctions??[]).forEach((item,i)=>add(`za:foundation/distinction/${i}`,'tos.entity.distinction',`INQUIRY_FOUNDATION.distinctions[${i}]`,item));
add('za:foundation/practice','tos.entity.thought-method','INQUIRY_FOUNDATION.practice',INQUIRY_FOUNDATION.practice);
for(const [lens,value] of Object.entries(LENS_INQUIRY)){
  if(value.question)add(`za:lens:${lens}/question`,'tos.entity.question',`LENS_INQUIRY.${lens}.question`,null,'lens:'+lens);
  if(value.method)add(`za:lens:${lens}/method`,'tos.entity.thought-method',`LENS_INQUIRY.${lens}.method`,value.method,'lens:'+lens);
}

// A form path names a module and then keys and indices; a key may itself hold dots, so the longest key that fits wins.
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
const same=(a,b)=>a.w===b.w&&a.c===b.c&&(a.loc??null)===(b.loc??null);
const links=earlier.links.filter(link=>{
  const form=resolve(link.form);if(!form)return false;
  const cites=(form?.cite??[]).concat(link.form.endsWith('.focus')?resolve(link.form.replace(/\.focus$/,''))?.cite??[]:[]);
  return [link.from,link.to].every(end=>cites.some(cite=>same(cite,end)));
});

const module={schema:earlier.schema,posture:earlier.posture,registry:earlier.registry,objects,relations,links};
const lines=list=>list.map(item=>'  '+JSON.stringify(item)).join(',\n');
const text=JSON.stringify({schema:module.schema,posture:module.posture,registry:module.registry},null,1).slice(0,-2)+`,\n "objects": [\n${lines(objects)}\n ],\n "relations": [\n${lines(relations)}\n ],\n "links": [\n${lines(links)}\n ]\n}\n`;
const summary=`objects ${earlier.objects.length} → ${objects.length}, relations ${earlier.relations.length} → ${relations.length}, links ${earlier.links.length} → ${links.length} (kept: ${[...new Set(links.map(link=>link.form))].length} forms)`;
const out=process.argv.includes('--out')?process.argv[process.argv.indexOf('--out')+1]:FILE;
if(!process.argv.includes('--check'))fs.writeFileSync(out,text);
console.log(summary);
