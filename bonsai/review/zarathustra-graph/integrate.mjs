// Rebuilds the Zarathustra stand's graph on the strict plan Dionysus chose: on the map,
// the links the book itself makes at a named place, and the questions those places raise
// with their answers; everything else is kept, whole, in the held registry
// (access/web/constructor/atlas-held.mjs) to be grown together later.
//
//   node integrate.mjs
//
// Reads out/plan.json and Terra's texts out/write-{A,B,C,D}.json; rewrites atlas-data.mjs
// (node and link rows, sources, clusters, assemblies, extensions, prominent stars) and the
// authored modules (atlas-content, the inquiry layers, relation-inquiry, atlas-routes).
import fs from 'node:fs';

const HERE=new URL('./',import.meta.url);
const ROOT=new URL('../../../',HERE).pathname;
const C=ROOT+'access/web/constructor/';
const read=name=>JSON.parse(fs.readFileSync(new URL(name,HERE),'utf8'));
const plan=read('out/plan.json');
const parts=['A','B','C'].map(x=>read(`out/write-${x}.json`));
const routing=read('out/write-D.json');
// A source is named once among a reading's grounds; its places are gathered under it.
const gather=grounds=>{
  if(!Array.isArray(grounds))return grounds;
  const byRef=new Map();
  for(const ground of grounds){
    const kept=byRef.get(ground.ref);
    if(!kept){byRef.set(ground.ref,{...ground,cite:[...(ground.cite??[])]});continue;}
    for(const cite of ground.cite??[])if(!kept.cite.some(other=>JSON.stringify(other)===JSON.stringify(cite)))kept.cite.push(cite);
    if(ground.focus)kept.focus=kept.focus?{ru:kept.focus.ru+' '+ground.focus.ru,en:kept.focus.en+' '+ground.focus.en}:ground.focus;
  }
  return [...byRef.values()];
};
for(const route of routing.routes){route.grounds=gather(route.grounds);for(const step of route.steps)step.grounds=gather(step.grounds);}
for(const part of parts)for(const layer of [part.inquiry,part.relations])for(const value of Object.values(layer??{}))value.grounds=gather(value.grounds);

const {NODE_CONTENT}=await import(C+'atlas-content.mjs');
const {INQUIRY_SOURCES_CONCEPTS}=await import(C+'inquiry-sources-concepts.mjs');
const {INQUIRY_READINGS_ECHOES}=await import(C+'inquiry-readings-echoes.mjs');
const {RELATION_INQUIRY}=await import(C+'relation-inquiry.mjs');
const {RESEARCH_ROUTES}=await import(C+'atlas-routes.mjs');

const SOURCES=['work','chapter-p3.r2','moment','chapter-p3.r13','all-things','same-life','dossier'];
const idOf=slug=>SOURCES.includes(slug)?slug:'demo:'+slug;
const edgeId=([from,to,kind])=>`atlas:${from}:${kind}:${to}`;

// The rows of a function's `const rows = [ ... ];` in atlas-data.mjs: one JSON array per line.
let data=fs.readFileSync(C+'atlas-data.mjs','utf8');
function rowsBlock(fn){
  const at=data.indexOf(`function ${fn}(`),open=data.indexOf('const rows = [',at)+'const rows = ['.length,close=data.indexOf('\n  ];',open);
  const rows=data.slice(open,close).split('\n').map(line=>line.trim()).filter(line=>line.startsWith('[')).map(line=>JSON.parse(line.replace(/,\s*$/,'')));
  return {open,close,rows};
}
const putRows=(fn,rows)=>{const {open,close}=rowsBlock(fn);data=data.slice(0,open)+'\n'+rows.map(row=>'    '+JSON.stringify(row)+',').join('\n')+data.slice(close);};

const oldNodes=rowsBlock('makeNodes').rows,oldEdges=rowsBlock('makeEdges').rows;
const newNodes=parts.flatMap(part=>part.nodes).filter(row=>!SOURCES.includes(row[0]));
const newEdges=parts.flatMap(part=>part.edges);
const keptSlugs=new Set([...newNodes.map(row=>row[0]),...SOURCES.filter(slug=>slug!=='dossier')]);
const keptEdgeIds=new Set(newEdges.map(edgeId));

// Every new link joins kept stars and is written once; every kept star has its texts.
for(const row of newEdges)for(const end of row.slice(0,2))if(!keptSlugs.has(end))throw new Error(`Link ${edgeId(row)} reaches ${end}, which is not kept`);
if(keptEdgeIds.size!==newEdges.length)throw new Error('A link is written twice');
const content=Object.assign({},...parts.map(part=>part.content));
const inquiry=Object.assign({},...parts.map(part=>part.inquiry));
const relations=Object.assign({},...parts.map(part=>part.relations));
for(const slug of keptSlugs){const id=idOf(slug);if(!content[id])throw new Error('No text for '+id);if(!inquiry[id])throw new Error('No inquiry for '+id);}
for(const id of keptEdgeIds)if(!relations[id])throw new Error('No relation inquiry for '+id);

// The held registry: what leaves the map, whole, with the audit's reasons.
const audit=new Map(plan.held_edges.map(item=>[`${item.from}|${item.to}|${item.kind}`,item]));
const nodeReason=new Map(plan.held_nodes.map(item=>[item.slug,item.reason]));
const held={
  nodes:oldNodes.filter(row=>!keptSlugs.has(row[0])).map(row=>({row,reason:nodeReason.get(row[0])??null,content:NODE_CONTENT[idOf(row[0])]??null,inquiry:INQUIRY_SOURCES_CONCEPTS[idOf(row[0])]??INQUIRY_READINGS_ECHOES[idOf(row[0])]??null})),
  sources:['dossier'].map(slug=>({id:slug,reason:nodeReason.get(slug)??null,content:NODE_CONTENT[slug]??null,inquiry:INQUIRY_SOURCES_CONCEPTS[slug]??INQUIRY_READINGS_ECHOES[slug]??null})),
  edges:oldEdges.filter(row=>!keptEdgeIds.has(edgeId(row))).map(row=>{const item=audit.get(`${row[0]}|${row[1]}|${row[2]}`);return {row,verdict:item?.verdict??null,reason:item?.reason??null,places:item?.places??[],relation:RELATION_INQUIRY[edgeId(row)]??null};}),
  // The earlier texts of the stars and links that stay, for the record of how they read before.
  earlier:{
    content:Object.fromEntries([...keptSlugs].map(idOf).filter(id=>NODE_CONTENT[id]).map(id=>[id,NODE_CONTENT[id]])),
    inquiry:Object.fromEntries([...keptSlugs].map(idOf).map(id=>[id,INQUIRY_SOURCES_CONCEPTS[id]??INQUIRY_READINGS_ECHOES[id]]).filter(([,value])=>value)),
    edges:oldEdges.filter(row=>keptEdgeIds.has(edgeId(row))),
    routes:RESEARCH_ROUTES,
  },
  routes:plan.routes.filter(route=>route.action!=='сократить'&&route.action!=='keep'&&route.action!=='оставить').map(route=>({id:route.id,reason:route.reason})),
};

// atlas-data.mjs: the rows, the sources, the clusters, the assemblies, the extensions and the prominent stars.
const oldAssemblies=data.slice(data.indexOf('const definitions = [',data.indexOf('function makeAssemblies')),data.indexOf('\n  ];',data.indexOf('function makeAssemblies'))+5);
const oldExtensions=data.slice(data.indexOf('const extensionDefinitions = ['),data.indexOf('\n];',data.indexOf('const extensionDefinitions = ['))+3);
held.assemblies=oldAssemblies;held.extensions=oldExtensions;
putRows('makeNodes',newNodes);
putRows('makeEdges',newEdges);
const replaceOnce=(before,after)=>{if(data.split(before).length!==2)throw new Error('Not found once: '+before.slice(0,60));data=data.replace(before,after);};
replaceOnce("const sourceIds = ['work', 'chapter-p3.r2', 'moment', 'chapter-p3.r13', 'all-things', 'same-life', 'dossier'];","const sourceIds = ['work', 'chapter-p3.r2', 'moment', 'chapter-p3.r13', 'all-things', 'same-life'];");
for(const cluster of ['readings','echoes']){const line=data.split('\n').find(text=>text.trim().startsWith(`['${cluster}',`));if(!line)throw new Error('No cluster '+cluster);data=data.replace(line+'\n','');}
replaceOnce(oldAssemblies,'const definitions = [\n'+routing.assemblies.map(row=>'    '+JSON.stringify(row)+',').join('\n')+'\n  ];');
replaceOnce(oldExtensions,"// Branches to grow are held until they are grown together; see atlas-held.mjs.\nconst extensionDefinitions = [];");
const prominent=data.match(/const prominent = new Set\(\[[^\]]*\]\);/)[0];
replaceOnce(prominent,`const prominent = new Set(${JSON.stringify(routing.prominent)});`);
fs.writeFileSync(C+'atlas-data.mjs',data);

// The authored modules, in the readable form the stand's writer uses.
const json=(value,indent='')=>{
  const flat=JSON.stringify(value);
  if(flat===undefined)return 'null';
  if(value===null||typeof value!=='object'||flat.length<=110)return flat;
  const inner=indent+'  ';
  if(Array.isArray(value)){
    if(value.every(item=>item&&typeof item==='object'&&('w' in item)))return `[\n${value.map(item=>inner+JSON.stringify(item)).join(',\n')}\n${indent}]`;
    return `[\n${value.map(item=>inner+json(item,inner)).join(',\n')}\n${indent}]`;
  }
  return `{\n${Object.entries(value).filter(([,item])=>item!==undefined).map(([key,item])=>`${inner}${JSON.stringify(key)}: ${json(item,inner)}`).join(',\n')}\n${indent}}`;
};
const RULE=` * Own words are kept only for what a quotation cannot give: what happens at a place and who
 * speaks, how it joins the next place, the question it raises and the answer the book allows.
 * What a work itself says is never retold: a form cites the place instead ({ru, en, cite}),
 * and the stand shows the work's own text there. The map keeps the links the book itself makes
 * at a named place; everything else waits in atlas-held.mjs to be grown together.`;
const order=ids=>Object.fromEntries(ids.filter(id=>id in inquiry).map(id=>[id,inquiry[id]]));
const questionIds=[...keptSlugs].filter(slug=>newNodes.find(row=>row[0]===slug)?.[1]==='question').map(idOf);
const otherIds=[...SOURCES.filter(slug=>slug!=='dossier'),...newNodes.map(row=>row[0])].map(idOf).filter(id=>!questionIds.includes(id));
const write=(name,text)=>fs.writeFileSync(C+name,text);
write('atlas-content.mjs',`/** Authored introductions of the atlas materials.\n${RULE}\n * Read with the passage-specific grounds in the inquiry modules and\n * source-references.mjs; SEMANTIC_REVIEW.md records scope and review. */\nexport const NODE_CONTENT = ${json(Object.fromEntries([...otherIds,...questionIds].map(id=>[id,content[id]])))};\n`);
write('inquiry-sources-concepts.mjs',`/** Authored source-grounded inquiry of the sources, images and concepts; source bytes remain separate.\n${RULE} */\nexport const INQUIRY_SOURCES_CONCEPTS = ${json(order(otherIds))};\n`);
write('inquiry-readings-echoes.mjs',`/** Authored inquiry of the questions the book's places raise, with the answers the book allows.\n * Readings and comparisons with other thinkers are held in atlas-held.mjs until they are grown together.\n${RULE} */\nexport const INQUIRY_READINGS_ECHOES = ${json(order(questionIds))};\n`);
write('relation-inquiry.mjs',`/** Source-grounded reading connections for the atlas.\n * Grounds identify passages to read; they do not turn a connection into proof.\n${RULE} */\nexport const RELATION_INQUIRY = ${json(Object.fromEntries(newEdges.map(row=>[edgeId(row),relations[edgeId(row)]])))};\n`);
write('atlas-routes.mjs',`/** Authored source-grounded reading routes; transitions retain atlas edge identity.\n${RULE} */\nexport const RESEARCH_ROUTES = ${json(routing.routes)};\n`);
write('atlas-held.mjs',`/** What the strict rebuild of 1 November 2026 took off the Zarathustra map, kept whole.\n *\n * Dionysus asked to begin with links the book itself makes at a named place, and to grow\n * the ambiguous ones together afterwards. Terra's audit of every link against the German text\n * (bonsai/review/zarathustra-graph) found 19 such links among 128; the rest are readings and\n * comparisons, most of them resting on another book or a reader's argument. They are kept\n * here with their stars, texts, verdicts, reasons and places, and with the earlier texts of\n * what stays. Nothing here is shown or bound by the stand. */\nexport const HELD = ${json(held)};\n`);
console.log(`kept ${newNodes.length} prepared stars, ${keptEdgeIds.size} links, ${routing.routes.length} routes; held ${held.nodes.length} stars, ${held.edges.length} links`);
