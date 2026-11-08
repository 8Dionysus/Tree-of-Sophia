// Where a place of one edition stands in another edition of the same work, as finely as the Tree knows it.
//   - The unit, through a bonsai's alignment: Zarathustra's four editions through the first editions (the
//     sentence alignment with Antonovsky and Common, the unit map onto Gutenberg 7205), and an ancient work's
//     cited units through its original (lib/corpus-align.mjs).
//   - Else the section both editions label alike: a Stephanus page, a Bekker section, a chapter whose title the
//     Zarathustra witnesses give in each language.
// A place found is a navigation aid, as the alignment itself is: the reader opens there and claims nothing more.
import witness from '../../../../bonsai/zarathustra/witnesses.json';

const chapterFiles=import.meta.glob('../../../../bonsai/zarathustra/witnesses/*.json',{import:'default'});
const alignmentFiles=import.meta.glob('../../../../bonsai/*/alignment.json',{import:'default'});

/** A label as it compares across editions: case, apostrophes, punctuation and spacing aside. */
export const normalLabel=label=>String(label??'').normalize('NFKC').toLocaleLowerCase()
  .replace(/[’'`´ʼ]/gu,'').replace(/[^\p{L}\p{N}]+/gu,' ').trim();

const orders=new WeakMap();
function orderOf(version){
  if(!orders.has(version))orders.set(version,new Map((version?.units??[]).map((unit,index)=>[unit.id??unit.unitId,index])));
  return orders.get(version);
}

/** The structure entries of `version` at or before `unitId`, the nearest first. */
export function sectionsBefore(version,unitId){
  const order=orderOf(version),position=order.get(unitId);
  if(position===undefined)return [];
  return (version.structure??[]).filter(entry=>(order.get(entry.unitId)??Infinity)<=position)
    .sort((a,b)=>order.get(b.unitId)-order.get(a.unitId));
}

// Each Zarathustra chapter title, in every language, names its chapter.
const bridge=new Map((witness.chapters??[]).flatMap(chapter=>Object.values(chapter.titles??{}).map(title=>[normalLabel(title),chapter.key])));
const titlesOf=key=>new Set(Object.values(witness.chapters?.find(chapter=>chapter.key===key)?.titles??{}).map(normalLabel));
const keyOf=entry=>entry.id?.match(/^bonsai\.[a-z]+\.(.+)$/)?.[1]??bridge.get(normalLabel(entry.label))??null;

/** The Zarathustra chapter holding `unitId` of `version`, by its structure. */
export function zarathustraChapter(version,unitId){
  for(const entry of sectionsBefore(version,unitId)){const key=keyOf(entry);if(key)return key;}
  return null;
}

// The roles of Zarathustra's editions in the witness chapters: the first editions (dta) are the pivot.
const roleOf=versionId=>Object.entries(witness.corpus??{}).find(([,entry])=>entry?.versionId===versionId)?.[0]??null;

/** The row of a chapter's alignment that holds `unitId` of the edition in `role`, or -1. */
export function rowOf(data,role,unitId){
  if(role==='dta'||role==='de'){
    const index=data.units.de.findIndex(unit=>role==='dta'?unit[0]===unitId:(unit[4]??[]).includes(unitId));
    return index<0?-1:data.rows.findIndex(row=>row.de?.includes(index));
  }
  const index=(data.units[role]??[]).findIndex(unit=>unit[0]===unitId);
  return index<0?-1:data.rows.findIndex(row=>row[role]?.includes(index));
}

/** The first unit of the edition in `role` at row `at`, else at the nearest row before it, else after it. */
export function unitAtRow(data,role,at){
  const lang=role==='dta'||role==='de'?'de':role;
  const pick=row=>{
    const index=row?.[lang]?.[0];if(index===undefined)return null;
    const unit=data.units[lang]?.[index];if(!unit)return null;
    return role==='de'?unit[4]?.[0]??null:unit[0];
  };
  for(let i=at;i>=0;i--){const found=pick(data.rows[i]);if(found)return found;}
  for(let i=at+1;i<data.rows.length;i++){const found=pick(data.rows[i]);if(found)return found;}
  return null;
}

async function zarathustraPlace(document,from,to,unitId){
  if(document?.id!==witness.corpus?.documentId)return null;
  const fromRole=roleOf(from.id),toRole=roleOf(to.id);
  if(!fromRole||!toRole)return null;
  const key=zarathustraChapter(from,unitId);
  const path=key&&Object.keys(chapterFiles).find(value=>value.endsWith(`/${key}.json`));
  if(!path)return null;
  const data=await chapterFiles[path](),at=rowOf(data,fromRole,unitId);
  return at<0?null:unitAtRow(data,toRole,at);
}

// A corpus alignment maps each cited unit of an original onto units of its translations.
let alignments=null;
async function loadAlignments(){
  // A file that fails to load fails the lookup, which a later lookup tries again; only a lookup that read them answers none.
  alignments??=Promise.all(Object.values(alignmentFiles).map(load=>load())).then(list=>list.filter(Boolean).map(data=>{
    const back=new Map();
    for(const [original,entry] of Object.entries(data.units??{}))for(const [versionId,target] of Object.entries(entry.t??{}))
      for(const unit of target.u??[])back.set(`${versionId}\u0000${unit}`,original);
    const originals=new Set(Object.values(data.works??{}).map(work=>work.original?.versionId).filter(Boolean));
    return {units:data.units??{},back,originals};
  })).catch(error=>{alignments=null;throw error;});
  return alignments;
}

async function alignedPlace(from,to,unitId){
  for(const data of await loadAlignments()){
    const original=data.originals.has(from.id)?unitId:data.back.get(`${from.id}\u0000${unitId}`);
    if(!original||!data.units[original])continue;
    if(data.originals.has(to.id))return original;
    const target=data.units[original].t?.[to.id]?.u?.[0];
    if(target)return target;
  }
  return null;
}

/** The unit opening the section of `to` labelled like the nearest labelled section of `from` before `unitId`. */
export function sectionPlace(from,to,unitId){
  const labels=new Map((to.structure??[]).map(entry=>[normalLabel(entry.label),entry.unitId]));
  for(const entry of sectionsBefore(from,unitId)){
    const same=labels.get(normalLabel(entry.label));
    if(same)return same;
    const key=keyOf(entry);
    if(key){
      const titles=titlesOf(key);
      const target=(to.structure??[]).find(item=>item.id?.match(/^bonsai\.[a-z]+\.(.+)$/)?.[1]===key||titles.has(normalLabel(item.label)));
      if(target)return target.unitId;
    }
  }
  return null;
}

/**
 * Where the place `unitId` of edition `from` stands in edition `to` of `document`: {unitId, by:'unit'|'section'},
 * or null where the Tree knows no such place. The unit found is always one of `to`'s own units. A witness or
 * alignment file that fails to load rejects the lookup: a failed lookup says nothing about the counterpart.
 */
export async function placeInEdition({document,from,to,unitId}){
  if(!document||!from||!to||!unitId||from.id===to.id)return null;
  const own=id=>id&&orderOf(to).has(id)?id:null;
  const unit=own(await zarathustraPlace(document,from,to,unitId))??own(await alignedPlace(from,to,unitId));
  if(unit)return {unitId:unit,by:'unit'};
  const section=own(sectionPlace(from,to,unitId));
  return section?{unitId:section,by:'section'}:null;
}
