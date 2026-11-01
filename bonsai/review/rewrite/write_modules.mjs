// Writes the constructor's authored content modules from a rewritten snapshot (JSON).
//   node write_modules.mjs <snapshot.json>
import {readFileSync,writeFileSync} from 'node:fs';
const DIR=new URL('../../../access/web/constructor/',import.meta.url).pathname;
const data=JSON.parse(readFileSync(process.argv[2],'utf8'));
// Readable and compact: a value that fits on a line stays on one line; cites always do.
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
const RULE=` * Own words are kept only for what a quotation cannot give (a reading, a
 * reconstructed argument, a distinction, a connection, a question, a method,
 * context). What a work itself says is never retold: a form cites the place
 * instead ({ru, en, cite}), and the stand shows the work's own text there —
 * the original and, through the alignment of the witnesses, the reader's
 * language (bonsai/zarathustra/witnesses.json, fragments.json).`;
const write=(name,text)=>writeFileSync(DIR+name,text);
write('atlas-content.mjs',`/** Authored introductions of the atlas materials.
${RULE}
 * Read with the passage-specific grounds in the inquiry modules and
 * source-references.mjs; SEMANTIC_REVIEW.md records scope and review. */
export const NODE_CONTENT = ${json(data.NODE_CONTENT)};
`);
write('atlas-routes.mjs',`/** Authored source-grounded reading routes; transitions retain atlas edge identity.
${RULE} */
export const RESEARCH_ROUTES = ${json(data.RESEARCH_ROUTES)};
`);
write('relation-inquiry.mjs',`/** Source-grounded reading connections for the atlas.
 * Grounds identify passages to read; they do not turn a connection into proof.
${RULE} */
export const RELATION_INQUIRY = ${json(data.RELATION_INQUIRY)};
`);
write('inquiry-sources-concepts.mjs',`/** Authored source-grounded inquiry of the sources and concepts; source bytes remain separate.
${RULE} */
export const INQUIRY_SOURCES_CONCEPTS = ${json(data.INQUIRY_SOURCES_CONCEPTS)};
`);
write('inquiry-readings-echoes.mjs',`/** Authored close inquiry of the readings and echoes; grounds name exact places.
 * Comparisons are argued here, without changing source or canon status.
${RULE} */
export const INQUIRY_READINGS_ECHOES = ${json(data.INQUIRY_READINGS_ECHOES)};
`);
const lens=JSON.parse(JSON.stringify(data.LENS_INQUIRY));
const sharesFoundation=JSON.stringify(lens.tree?.grounds)===JSON.stringify(data.INQUIRY_FOUNDATION.grounds);
if(sharesFoundation)delete lens.tree.grounds;
write('inquiry-foundation.mjs',`// Authored questions of the stand, separate from the source corpus.
/*
${RULE} */
export const INQUIRY_FOUNDATION = ${json(data.INQUIRY_FOUNDATION)};

export const LENS_INQUIRY = ${json(lens)};
${sharesFoundation?'\nLENS_INQUIRY.tree.grounds = INQUIRY_FOUNDATION.grounds;\n':''}`);
write('close-reading-guides.mjs',`// Reading prompts of the local demonstration's reader. A move names its place
// in the original (\`paragraphs\` by the original language of the supplied catalog,
// \`cite\` in the bonsai witnesses for Zarathustra); no translation gets
// positions chosen by hand, a translation's place follows from the alignment.
/*
${RULE} */
export const GUIDE_CATALOG_SHA256 = ${JSON.stringify(data.GUIDE_CATALOG_SHA256)};
export const CLOSE_READING_GUIDES = ${json(data.CLOSE_READING_GUIDES)};

/** A changed catalog gets no old positional guidance. Reading itself stays available. */
export function bindCloseReadingGuides(catalog,sha256){
 if(sha256!==GUIDE_CATALOG_SHA256)return new Map();
 const documents=new Map(catalog.passages.map(item=>[item.id,item])),bound=new Map();
 for(const [id,guide]of Object.entries(CLOSE_READING_GUIDES)){
  const doc=documents.get(id);if(doc?.status!=='available')throw Error('Reading guide requires its available text: '+id);
  for(const step of guide.moves)for(const [code,index]of Object.entries(step.paragraphs??{}))if(!Number.isSafeInteger(index)||index<0||index>=doc.versions?.[code]?.paragraphs.length||!doc.versions?.[code])throw Error('Reading guide paragraph is outside its version: '+id+' '+code);
  bound.set(id,guide);
 }
 return bound;
}
`);
console.log('written');
