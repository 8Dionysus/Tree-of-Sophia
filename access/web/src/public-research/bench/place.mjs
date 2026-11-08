// Places of the book on the bench: a selection made in a witness becomes a citation of the original, and a citation
// finds everything the Tree keeps at the same place. A unit's raw text is the source's own; offsets count its
// characters, and what a reader sees (displayText) maps back to them exactly.

import {displayText,fragmentOf} from '../../../../../bonsai/lib/witnesses.mjs';

export const PIVOT='de',WORK='also-sprach-zarathustra';
const WORD=/[\p{L}\p{M}\p{N}'’\-]/u;

/** The raw offset at which a shown offset of a unit begins. */
export function rawOffset(unit,shown){
  const view=displayText(unit.raw,unit.kind);let low=0,high=unit.raw.length;
  while(low<high){const mid=(low+high)>>1;if(view.at(mid)<shown)low=mid+1;else high=mid;}
  return low;
}
/** The selection widened to whole words, inside one shown text. */
export function snapToWords(text,start,end){
  let s=Math.max(0,Math.min(start,end)),e=Math.min(text.length,Math.max(start,end));
  while(s<e&&!WORD.test(text[s]))s++;
  while(e>s&&!WORD.test(text[e-1]))e--;
  while(s>0&&WORD.test(text[s-1]))s--;
  while(e<text.length&&WORD.test(text[e]))e++;
  return {start:s,end:e};
}
/** The selection widened to whole sentences of the shown text. */
export function snapToSentence(text,start,end){
  let s=start,e=end;
  while(s>0&&!/[.!?…;]\s/.test(text.slice(s-2,s)))s--;
  while(s<e&&/\s/.test(text[s]))s++;
  while(e<text.length&&!/[.!?…;]/.test(text[e-1]))e++;
  while(e<text.length&&/[»«“”"')\]]/.test(text[e]))e++;
  return {start:s,end:e};
}
/** Is this a single word (no space inside)? */
export const isWord=value=>!!value&&!/\s/.test(value.trim())&&WORD.test(value);

/** The first and last unit of a citation in a chapter, with the raw offsets it starts and ends at. */
export function rangeOf(cite,chapter,language=PIVOT){
  const index=chapter?.index?.[language],list=chapter?.units?.[language];if(!index||!list)return null;
  const a=index.get(cite.u[0]),b=index.get(cite.u.at(-1));if(a===undefined||b===undefined)return null;
  return {a:Math.min(a,b),b:Math.max(a,b),s:Number.isInteger(cite.s)?cite.s:0,e:Number.isInteger(cite.e)?cite.e:list[Math.max(a,b)].raw.length};
}
/** How near two places of one chapter lie: the same words, the same paragraph, or only the same chapter. */
export function nearness(x,y){
  if(!x||!y)return null;
  const before=(ua,oa,ub,ob)=>ua<ub||(ua===ub&&oa<=ob);
  const overlap=before(x.a,x.s,y.b,y.e)&&before(y.a,y.s,x.b,x.e)&&!(x.b===y.a&&x.e===y.s)&&!(y.b===x.a&&y.e===x.s);
  if(overlap)return 'exact';
  if(x.a<=y.b&&y.a<=x.b)return 'paragraph';
  return 'chapter';
}

/**
 * A selection in a witness as a citation of the original. `from` and `to` are {index, offset} in raw characters of
 * the witness's units. In the original the citation is the selection itself; in a translation it is the original's
 * part that the row's sentence pairs join to the selected sentences, or the row's whole original where a row has no
 * sentence pairs.
 */
export function citeOfSelection(chapter,language,from,to){
  const list=chapter.units[language],a=Math.min(from.index,to.index),b=Math.max(from.index,to.index);
  const s=from.index<=to.index?from.offset:to.offset,e=from.index<=to.index?to.offset:from.offset;
  if(!list?.[a]||!list[b])return null;
  const witness={language,u:[list[a].id,list[b].id],s,e};
  if(language===PIVOT)return {cite:{w:WORK,c:chapter.key,u:[list[a].id,list[b].id],s,e},witness};
  const pivot=chapter.units[PIVOT],inside=(i,ss,ee)=>i>=a&&i<=b&&!(i===a&&ee<=s)&&!(i===b&&ss>=e);
  let low=null,high=null;
  const take=(i,ss,ee)=>{if(!low||i<low.i||(i===low.i&&ss<low.o))low={i,o:ss};if(!high||i>high.i||(i===high.i&&ee>high.o))high={i,o:ee};};
  for(const row of chapter.rows){
    const targets=row[language]??[];if(!targets.some(i=>i>=a&&i<=b))continue;
    const pairs=(row.sentences?.[language]??[]).filter(([,target])=>target.some(([i,ss,ee])=>inside(i,ss,ee)));
    if(pairs.length)for(const [source] of pairs)for(const [i,ss,ee] of source)take(i,ss,ee);
    else for(const i of row[PIVOT]??[])take(i,0,pivot[i].raw.length);
  }
  if(!low)return null;
  return {cite:{w:WORK,c:chapter.key,u:[pivot[low.i].id,pivot[high.i].id],s:low.o,e:high.o},witness};
}

/** The words of a citation in one witness, joined as a reader sees them. */
export function wordsOf(cite,chapter,language=PIVOT){
  const part=fragmentOf(cite,chapter,language);
  if(!part||part.omitted)return '';
  return part.units.map(unit=>unit.text.slice(unit.from,unit.to)).join(' ').replace(/\s+/g,' ').trim();
}
/** The words of the witness the selection was made in, from its own offsets. */
export function witnessWords(fragment,chapter){
  const w=fragment.witness;
  if(!w?.u||w.language===PIVOT)return wordsOf(fragment.cite,chapter,PIVOT);
  const list=chapter.units[w.language],index=chapter.index[w.language];if(!list)return '';
  const a=index.get(w.u[0]),b=index.get(w.u.at(-1));if(a===undefined||b===undefined)return '';
  const parts=[];
  for(let i=a;i<=b;i++){const view=displayText(list[i].raw,list[i].kind);parts.push(view.text.slice(i===a?view.at(w.s??0):0,i===b?view.at(w.e??list[i].raw.length):view.text.length));}
  return parts.join(' ').replace(/\s+/g,' ').trim();
}
/** A short handle for a fragment: its first words. */
export function labelOf(words,limit=8){
  const list=String(words).split(/\s+/).filter(Boolean);
  return list.length>limit?`${list.slice(0,limit).join(' ')}…`:list.join(' ');
}
/** A fingerprint of a fragment's words, to notice when the source under it changes. */
export function fingerprintOf(words){
  let h=2166136261>>>0;for(const char of String(words)){h^=char.codePointAt(0);h=Math.imul(h,16777619)>>>0;}
  return `fnv1a:${h.toString(16)}`;
}

/**
 * Everything the bonsai keeps at a fragment's place, found by meeting of places: the voices that speak there, the
 * stars and links whose grounds fall there, the place relations that start or end there, and the senses of the
 * words used there. Each finding says how near it lies (the same words, the same paragraph, the same chapter).
 */
export function whatTreeKnows({fragment,chapter,index,core}){
  const place=rangeOf(fragment.cite,chapter);if(!place)return null;
  const rank={exact:0,paragraph:1,chapter:2};
  const found=new Map(),relations=new Map();
  const note=(map,key,value)=>{const old=map.get(key);if(!old||rank[value.near]<rank[old.near])map.set(key,value);};
  for(const item of index?.items?.values?.()??[]){
    if(item.kind!=='passage'||item.record?.c!==fragment.cite.c)continue;
    const near=nearness(place,rangeOf(item.record,chapter));if(!near)continue;
    for(const owner of index.references(item.id)){
      const subject=owner.kind==='statement'?index.get(owner.parentId):owner;if(!subject)continue;
      if(subject.kind==='passage-relation')note(relations,subject.id,{item:subject,near,cite:item.record});
      else note(found,subject.id,{item:subject,near,cite:item.record});
    }
  }
  const voices=[],seen=new Set();
  for(const segment of core?.voices?.[fragment.cite.c]??[]){
    const near=nearness(place,rangeOf({u:segment.u},chapter));if(near!=='exact')continue;
    const key=`${segment.speaker}:${segment.attribution?.addressee??''}:${segment.mode}:${segment.layer}`;if(seen.has(key))continue;seen.add(key);
    voices.push({speaker:segment.speaker,addressee:segment.attribution?.addressee??segment.addressee??null,mode:segment.mode,layer:segment.layer,via:segment.attribution?.carrier??null});
  }
  const uses=[];
  for(const term of core?.semanticUses?.terms??[])for(const group of term.groups??[]){
    const near=(group.grounds??[]).map(cite=>cite.c===fragment.cite.c?nearness(place,rangeOf(cite,chapter)):null).filter(Boolean).sort((x,y)=>rank[x]-rank[y])[0];
    if(near==='exact')uses.push({term:term.lemma,label:group.label,meaning:group.meaning});
  }
  const byNear=(x,y)=>rank[x.near]-rank[y.near];
  const items=[...found.values()].sort(byNear);
  return {
    nodes:items.filter(entry=>entry.item.kind!=='relation'&&entry.item.kind!=='chain'),
    links:items.filter(entry=>entry.item.kind==='relation'||entry.item.kind==='chain'),
    places:[...relations.values()].sort(byNear),voices,uses,
  };
}

/** Every place of a chapter where the needle stands, in the shown text of one witness. */
export function findInChapter(chapter,language,needle,{whole=true}={}){
  const list=chapter.units[language];if(!list||!needle)return [];
  const flags='giu',escaped=needle.trim().replace(/[.*+?^${}()|[\]\\]/g,'\\$&').replace(/\s+/g,'\\s+');
  const pattern=new RegExp(whole?`(?<![\\p{L}\\p{M}\\p{N}])${escaped}(?![\\p{L}\\p{M}\\p{N}])`:escaped,flags);
  const out=[];
  list.forEach((unit,i)=>{
    const view=displayText(unit.raw,unit.kind);
    for(const match of view.text.matchAll(pattern)){
      const from=Math.max(0,match.index-60),to=Math.min(view.text.length,match.index+match[0].length+60);
      out.push({chapter:chapter.key,index:i,id:unit.id,start:match.index,end:match.index+match[0].length,
        before:(from>0?'…':'')+view.text.slice(from,match.index),hit:match[0],after:view.text.slice(match.index+match[0].length,to)+(to<view.text.length?'…':'')});
    }
  });
  return out;
}
