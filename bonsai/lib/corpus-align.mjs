// Alignment of the corpus works a stand cites (Node only for reading; the alignment itself is pure).
//
// A stand cites a place of a work in the public corpus by units of its
// original. For each cited unit this finds its counterparts in every
// translation the corpus holds — by the shared reference where both versions
// are divided alike, by the sections inside it where the translation is divided
// more finely, and otherwise by aligning the sentences of the whole work — and
// pairs the sentences inside them with align.mjs. Both steps work in two
// passes (Moore 2002): sentences are paired by length, an IBM Model 1 is
// trained on the confident one-to-one pairs, and everything is paired again
// paying for the words the model cannot account for. An edition's notes at the
// end of a unit are not its text and are left out of the pairing. In each
// language a version whose notes run into its sentences comes after one whose
// notes stand apart, so a stand shows the cleaner text first.
//
// The result holds unit ids and offsets only, never text, with the corpus
// revision of every version and a receipt of how each version was aligned.

import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {ALIGNMENT,LexicalModel,alignRun,sentencesOf} from './align.mjs';
import {CORPUS_ALIGNMENT_SCHEMA} from './fragments.mjs';

const json=path=>JSON.parse(readFileSync(path,'utf8'));
const chars=text=>Array.from(text);
// A reference as numbers ("Книга 1 · §2" and "1.2.3" alike), so a finer division can be found inside a coarser one.
const numbers=reference=>(String(reference??'').match(/\d+/g)??[]).map(Number);
const inside=(inner,outer)=>outer.length>0&&inner.length>outer.length&&outer.every((n,i)=>inner[i]===n);
const VARIANCE=6.8;
// An edition's notes stand at the end of its unit, each line opening with its number in 〔 〕: they are not its text.
const NOTES=/(^|\n)〔\d+〕/u;
/** Where a unit's text ends and its edition's notes begin, in code points (the whole length when it has none). */
export function textEnd(text){const at=text.search(NOTES);return at<0?chars(text).length:chars(text.slice(0,at)).length;}
const continues=text=>/^[\s«“"(—–-]*\p{Ll}/u.test(text);
const unfinished=text=>!/[.!?…»”"\])]+\s*$/u.test(text);
// A version whose notes run into its sentences shows a word glued to the capital of the next ("artsA less well attested").
const GLUED=/[a-z][A-Z][a-z]{2,} [a-z]+/g;
/** How often a version's notes run into its text: glued words per 100,000 characters. */
export function mixedNotes(version){let glued=0,length=0;for(const unit of version.units){glued+=(unit.text.match(GLUED)??[]).length;length+=unit.text.length;}return length?glued*1e5/length:0;}

/** A reader of the public corpus at `dir`: a work's versions with their units in order. */
export function corpusReader(dir){
  const manifest=json(join(dir,'manifest.json')),cache=new Map();
  return documentId=>{
    if(cache.has(documentId))return cache.get(documentId);
    const entry=manifest.documents.find(item=>item.id===documentId);
    let work=null;
    if(entry){
      const doc=json(join(dir,entry.manifest.path));
      work={id:documentId,versions:doc.versions.map(version=>{
        const units=version.pages.flatMap(page=>json(join(dir,page.path)).units).sort((a,b)=>a.ordinal-b.ordinal);
        return {id:version.id,language:version.language,revision:version.revision??null,label:version.label??null,license:version.license?.id??version.license??null,attribution:version.attribution??null,units};
      })};
    }
    cache.set(documentId,work);
    return work;
  };
}

/** How a translation's units stand to the original's: the same references, finer sections inside them, or neither. */
export function counterparts(original,translation){
  // "Unit 59" numbers an edition's chunk of a long chapter, not a place of the work: it names nothing the other version shares.
  const textual=reference=>Boolean(reference)&&!/^Unit \d+$/.test(reference);
  // The same reference names the same place; so do the same numbers under another label ("§1" and "Раздел 1").
  const byReference=new Map(),byNumbers=new Map(),add=(map,key,unit)=>map.set(key,(map.get(key)??[]).concat(unit));
  for(const unit of translation.units)if(textual(unit.reference)){add(byReference,unit.reference,unit);if(numbers(unit.reference).length)add(byNumbers,numbers(unit.reference).join('.'),unit);}
  const whole=unit=>({id:unit.id,s:0,e:chars(unit.text).length});
  const match=unit=>!textual(unit.reference)?null:byReference.get(unit.reference)??(numbers(unit.reference).length?byNumbers.get(numbers(unit.reference).join('.')):null)??null;
  const named=original.units.filter(unit=>textual(unit.reference)).length;
  const same=original.units.filter(unit=>match(unit)).length;
  if(named&&same>=named*.8)return {how:'reference',map:new Map(original.units.filter(unit=>match(unit)).map(unit=>[unit.id,match(unit).map(whole)]))};
  const finer=new Map();
  for(const unit of original.units){if(!textual(unit.reference))continue;const outer=numbers(unit.reference),list=translation.units.filter(item=>textual(item.reference)&&inside(numbers(item.reference),outer));if(list.length)finer.set(unit.id,list.map(whole));}
  if(named&&finer.size>=named*.8)return {how:'reference (finer division)',map:finer};
  // Divided more coarsely: the book of the translation that holds the original's section.
  const coarser=new Map();
  for(const unit of original.units){if(!textual(unit.reference))continue;const inner=numbers(unit.reference),list=translation.units.filter(item=>textual(item.reference)&&inside(inner,numbers(item.reference)));if(list.length)coarser.set(unit.id,list.map(whole));}
  if(named&&coarser.size>=named*.8)return {how:'reference (coarser division)',map:coarser};
  return {how:'sentences of the whole work',map:null};
}

// Sentence pairs of one run, from the groups of alignRun, with the training pairs it offers and its words' cover.
function pairsOf(S,T,groups,model){
  const items=new Map([...S,...T].map(item=>[item.id,item])),pairs=[],training=[];let words=0,uncovered=0;
  for(const group of groups){
    if(!group.s.length||!group.t.length)continue;
    const source=group.s.map(id=>items.get(id)),target=group.t.map(id=>items.get(id));
    pairs.push([group.s,group.t]);
    const a=source.flatMap(item=>item.tok),b=target.flatMap(item=>item.tok);
    if(group.shape==='1:1'&&group.cost<ALIGNMENT.model.confident)training.push([a,b]);
    if(model){words+=a.length+b.length;uncovered+=model.uncovered(a,b);}
  }
  return {pairs,training,words,uncovered};
}

/**
 * Align one translation to the original: for each cited unit of the original, its counterparts in the
 * translation and the sentence pairs inside them. `cited` is a set of unit ids of the original.
 */
// A Greek or Latin period, which ends only on its full stop or question mark, often becomes up to six
// sentences of a modern translation, as a verse line can.
const PERIODS=new Set(['grc','la']),LONG_PERIOD=[[1,4],[1,5],[1,6]];

// Some editions join several short original sentences into one long translation sentence. The scope may
// opt into larger groups for that exact pair of revisions; the aligner still chooses every correspondence.
export function alignVersion(original,translation,cited,{maxSourceSentences=3,handoverUnitIds=[]}={}){
  if(!Number.isInteger(maxSourceSentences)||maxSourceSentences<3||maxSourceSentences>6)throw new Error('Invalid maximum source sentence group');
  const beads=(PERIODS.has(original.language)?LONG_PERIOD:[]).concat(Array.from({length:Math.max(0,maxSourceSentences-3)},(_,i)=>[i+4,1]));
  const {how,map}=counterparts(original,translation),textOf=new Map(translation.units.map(unit=>[unit.id,unit.text]));
  const position=new Map(translation.units.map((unit,index)=>[unit.id,index])),whole=unit=>({id:unit.id,s:0,e:chars(unit.text).length});
  const textOfTarget=target=>chars(textOf.get(target.id)).slice(target.s,Math.min(target.e,textEnd(textOf.get(target.id)))).join('');
  // Where the translation's preceding unit hands over to a run: the original's unit before the run and
  // the run's own units are aligned by length, sentence by sentence, against the preceding unit and the
  // run's counterparts; the run takes the preceding unit from its first sentence paired with the run's
  // original, and ordinarily never more than the two closing sentences the cut can involve. A scoped
  // edition may opt in specific units where complete sentences also cross references, allowing up to eight.
  // With no original unit before the run, only the sentence the reference cuts is offered. Null when the
  // preceding unit gives this run nothing.
  function handover(original,group,previous,priorText){
    const own=sentencesOf([{index:0,raw:priorText}],translation.language);
    if(!own.length)return null;
    const at=original.units.findIndex(unit=>unit.id===group.units[0].id),before=original.units[at-1];
    if(!before)return own.at(-1).id[1];
    const bound=own[Math.max(0,own.length-(handoverUnitIds.includes(group.units[0].id)?8:2))].id[1];
    const S=sentencesOf([before,...group.units].map((unit,index)=>({index,raw:chars(unit.text).slice(0,textEnd(unit.text)).join('')})),original.language);
    // The sentence the reference cuts is one sentence: its continuation in the run's first unit joins the
    // preceding unit's last sentence, so it is weighed whole against the original.
    const [head,...rest]=group.targets.map(textOfTarget),cut=sentencesOf([{index:0,raw:head??''}],translation.language)[0]?.id[2]??0;
    const T=sentencesOf([{index:0,raw:priorText+chars(head??'').slice(0,cut).join('')},{index:1,raw:chars(head??'').slice(cut).join('')},
      ...rest.map((raw,index)=>({index:index+2,raw}))],translation.language).filter(item=>item.len>0);
    const ratio=T.reduce((n,item)=>n+item.len,0)/Math.max(1,S.reduce((n,item)=>n+item.len,0));
    const starts=alignRun(S,T,ratio,VARIANCE,{band:60,beads})
      .filter(pair=>pair.s.some(([unit])=>unit>0)).flatMap(pair=>pair.t).filter(([unit])=>unit===0).map(([,start])=>start);
    return starts.length?Math.max(bound,Math.min(...starts)):null;
  }
  let groups=[];
  if(map){
    // A unit of the original with counterparts begins a run; a unit the translation does not divide (an
    // edition that splits a long chapter into numbered chunks) joins the run before it.
    // Units of the original that share their counterparts (the sections of one book a translation keeps whole) make one run.
    const key=targets=>targets.map(target=>target.id).join(' ');
    const leading=[];let skipContinuation=false;
    for(const unit of original.units){
      const targets=map.get(unit.id);
      // A named place absent from a partial translation is a gap, not the
      // preceding place. Only anonymous numbered chunks continue a run.
      if(!targets?.length&&unit.reference&&!/^Unit \d+$/.test(unit.reference)){skipContinuation=true;continue;}
      if(targets?.length)skipContinuation=false;
      else if(skipContinuation)continue;
      if(targets?.length&&!(groups.length&&groups.at(-1).key===key(targets)))groups.push({units:[unit],targets:[...targets],key:key(targets)});
      else if(groups.length)groups.at(-1).units.push(unit);
      else leading.push(unit);
    }
    // Units before the first that has counterparts open the first run.
    if(groups.length)groups[0].units.unshift(...leading);
    // A unit of the translation no unit of the original names joins the run whose counterparts precede it.
    const used=new Set(groups.flatMap(group=>group.targets.map(target=>target.id)));
    const last=group=>Math.max(...group.targets.map(target=>position.get(target.id)));
    translation.units.forEach((unit,index)=>{
      if(used.has(unit.id))return;
      const before=groups.filter(group=>last(group)<index).sort((a,b)=>last(b)-last(a))[0]??groups[0];
      if(before){before.targets.push(whole(unit));used.add(unit.id);}
    });
    for(const group of groups){
      group.targets.sort((a,b)=>position.get(a.id)-position.get(b.id));
      // A source's printed reference can fall inside a sentence, and the two
      // editions need not place it at the same word. When the translation
      // starts mid-sentence (or the scoped edition cuts at a different sentence), offer this run the part
      // of its preceding unit that belongs to it: the sentences the original's own preceding unit leaves
      // over (handover). A finished sentence that preceding unit accounts for
      // stays with it. Keep offsets in the original units: no source text moves.
      const first=group.targets[0],index=position.get(first?.id),previous=translation.units[index-1];
      const firstText=textOf.get(first?.id)??'',priorText=previous?chars(previous.text).slice(0,textEnd(previous.text)).join(''):'';
      if(previous&&first.s===0&&!group.targets.some(target=>target.id===previous.id)
        &&(continues(firstText)&&unfinished(priorText)||handoverUnitIds.includes(group.units[0].id))){
        const start=handover(original,group,previous,priorText);
        if(start!==null)group.targets.unshift({id:previous.id,s:start,e:textEnd(previous.text)});
      }
    }
  }else groups=[{units:original.units,targets:translation.units.map(whole),whole:true}];
  // Every run: the sentences of its units of the original against those of its counterparts, an edition's notes left out.
  const runs=groups.map(group=>{
    const S=sentencesOf(group.units.map((unit,index)=>({index,raw:chars(unit.text).slice(0,textEnd(unit.text)).join('')})),original.language);
    const T=sentencesOf(group.targets.map((target,index)=>({index,raw:chars(textOf.get(target.id)).slice(target.s,Math.min(target.e,textEnd(textOf.get(target.id)))).join('')})),translation.language);
    const ratio=T.reduce((n,item)=>n+item.len,0)/Math.max(1,S.reduce((n,item)=>n+item.len,0));
    const one=group.units.length===1&&!group.whole;
    return {...group,S,T,ratio,variance:one?VARIANCE*ALIGNMENT.sentenceVariance:VARIANCE,band:one?ALIGNMENT.band:60};
  });
  const align=(run,model)=>run.S.length===1&&run.T.length===1?[{s:[run.S[0].id],t:[run.T[0].id],shape:'1:1',cost:0}]:alignRun(run.S,run.T,run.ratio,run.variance,{model,band:run.band,beads});
  // First pass by length; the confident one-to-one pairs teach the model the words of the two languages.
  const training=runs.flatMap(run=>pairsOf(run.S,run.T,align(run,null),null).training);
  const model=new LexicalModel(training);
  const entries={},shapes={};let words=0,uncovered=0;
  for(const run of runs){
    const aligned=align(run,model),result=pairsOf(run.S,run.T,aligned,model);
    words+=result.words;uncovered+=result.uncovered;
    for(const group of aligned)shapes[group.shape]=(shapes[group.shape]??0)+1;
    // Each cited unit takes the sentences its own sentences were joined with, unit by unit of the translation.
    for(const [s,targets] of result.pairs){
      const t=[...targets];
      // If a selected sentence crosses a reference, retain its continuation
      // in the following unit even when the aligner paired only its first half.
      for(const [j,,end] of targets){
        const current=run.targets[j],next=run.targets[j+1];
        if(!next||position.get(next.id)!==position.get(current.id)+1||next.s!==0)continue;
        const currentText=textOf.get(current.id),nextText=textOf.get(next.id);
        if(current.e!==textEnd(currentText)||chars(currentText).slice(current.s+end,current.e).join('').trim()
          ||!unfinished(currentText)||!continues(nextText))continue;
        const sentence=sentencesOf([{index:j+1,raw:chars(nextText).slice(0,textEnd(nextText)).join('')}],translation.language)[0];
        if(sentence&&!t.some(([k,a,b])=>k===j+1&&a<=sentence.id[1]&&b>=sentence.id[2]))t.push(sentence.id);
      }
      for(const index of new Set(s.map(([unit])=>unit))){
        const unit=run.units[index].id;if(!cited.has(unit))continue;
        const entry=entries[unit]??={u:[],pairs:[]};
        const at=id=>{let i=entry.u.indexOf(id);if(i<0){entry.u.push(id);i=entry.u.length-1;}return i;};
        entry.pairs.push([s.filter(([unitIndex])=>unitIndex===index).map(([,a,b])=>[a,b]),t.map(([j,a,b])=>[at(run.targets[j].id),run.targets[j].s+a,run.targets[j].s+b])]);
      }
    }
    // A unit alone in its run keeps its counterparts even where no sentence of it found a partner.
    if(run.units.length===1&&!run.whole&&cited.has(run.units[0].id)&&!entries[run.units[0].id])entries[run.units[0].id]={u:run.targets.map(target=>target.id),pairs:[]};
  }
  const coverage=words?Math.round((1-uncovered/words)*1000)/1000:null;
  return {entries,receipt:{how,sentences:[runs.reduce((n,run)=>n+run.S.length,0),runs.reduce((n,run)=>n+run.T.length,0)],shapes,trained:training.length,coverage}};
}

/** The places a stand's content cites in corpus works, as cited unit ids per work key: every unit from a citation's first to its last. */
export function citedUnits(content,works,read){
  const ranges=new Map(),walk=value=>{
    if(Array.isArray(value)){for(const item of value)walk(item);return;}
    if(!value||typeof value!=='object')return;
    if(Array.isArray(value.cite))for(const cite of value.cite)if(works[cite?.w]&&Array.isArray(cite.u)&&cite.u.length===2&&!cite.c)(ranges.get(cite.w)??ranges.set(cite.w,[]).get(cite.w)).push(cite.u);
    for(const item of Object.values(value))walk(item);
  };
  walk(content);
  const cited=new Map();
  for(const [key,list] of ranges){
    const work=read(works[key]);if(!work)continue;
    for(const [first,last] of list){
      const original=work.versions.find(version=>version.units.some(unit=>unit.id===first));if(!original)continue;
      const a=original.units.findIndex(unit=>unit.id===first),b=original.units.findIndex(unit=>unit.id===last);
      const entry=cited.get(key)??cited.set(key,{original:original.id,units:new Set()}).get(key);
      for(let i=a;i<=(b<a?a:b);i++)entry.units.add(original.units[i].id);
    }
  }
  return cited;
}

/** The alignment module of a stand: every cited corpus work, its original and translations, and the cited units' counterparts. */
export function alignCorpus(cited,works,read,scopes=[]){
  if(!Array.isArray(scopes))throw new Error('Translation scopes must be an array');
  const seenScopes=new Set();
  for(const scope of scopes){
    const key=`${scope.work}:${scope.translationVersionId}`,work=works[scope.work]&&read(works[scope.work]);
    const original=work?.versions.find(item=>item.id===scope.originalVersionId),translation=work?.versions.find(item=>item.id===scope.translationVersionId);
    const first=original?.units.findIndex(unit=>unit.id===scope.firstUnitId)??-1,last=original?.units.findIndex(unit=>unit.id===scope.lastUnitId)??-1;
    const start=scope.translationStart,firstTarget=translation?.units[0];
    const invalidStart=start!==undefined&&(!firstTarget||start?.unitId!==firstTarget.id||!Number.isInteger(start?.offset)||start.offset<1||start.offset>=chars(firstTarget.text).length);
    const handovers=scope.handoverUnitIds;
    const invalidHandovers=handovers!==undefined&&(!Array.isArray(handovers)||!handovers.length||new Set(handovers).size!==handovers.length||handovers.some(id=>{const i=original?.units.findIndex(unit=>unit.id===id)??-1;return typeof id!=='string'||i<=first||i>last;}));
    if(seenScopes.has(key)||!original||!translation||original===translation||original.language===translation.language||!scope.originalRevision||!scope.translationRevision||scope.originalRevision!==original.revision||scope.translationRevision!==translation.revision||(scope.maxSourceSentences!==undefined&&(!Number.isInteger(scope.maxSourceSentences)||scope.maxSourceSentences<4||scope.maxSourceSentences>6))||invalidStart||invalidHandovers||first<0||last<first||!/^https:\/\//.test(scope.source??''))throw new Error(`${scope.work}: invalid or stale translation scope for ${scope.translationVersionId}`);
    seenScopes.add(key);
  }
  const out={schema:CORPUS_ALIGNMENT_SCHEMA,mechanism:'bonsai/lib/corpus-align.mjs',review:'unreviewed',works:{},units:{},receipt:{}};
  for(const [key,{original:originalId,units}] of [...cited].sort(([a],[b])=>a.localeCompare(b))){
    const work=read(works[key]),original=work.versions.find(version=>version.id===originalId);
    const meta=version=>({versionId:version.id,language:version.language,revision:version.revision,label:version.label,license:version.license,attribution:version.attribution});
    const translations=[];
    for(const version of work.versions.filter(item=>item!==original&&item.language!==original.language)){
      const selected=scopes.filter(scope=>scope.work===key&&scope.translationVersionId===version.id);
      if(selected.length>1)throw new Error(`${key}: duplicate translation scope for ${version.id}`);
      const scope=selected[0];let source=original;
      if(scope){
        const first=original.units.findIndex(unit=>unit.id===scope.firstUnitId),last=original.units.findIndex(unit=>unit.id===scope.lastUnitId);
        if(scope.originalVersionId!==original.id||scope.originalRevision!==original.revision||first<0||last<first||!/^https:\/\//.test(scope.source??''))throw new Error(`${key}: invalid or stale translation scope for ${version.id}`);
        source={...original,units:original.units.slice(first,last+1)};
      }
      // Ignore a declared front matter prefix only while aligning; all returned offsets still address
      // the unmodified, revision-bound corpus unit. Other annotations may remain within the text.
      const start=scope?.translationStart;
      const target=start?{...version,units:version.units.map(unit=>unit.id===start.unitId?{...unit,text:chars(unit.text).slice(start.offset).join('')}:unit)}:version;
      const {entries,receipt}=alignVersion(source,target,units,{maxSourceSentences:scope?.maxSourceSentences??3,handoverUnitIds:scope?.handoverUnitIds??[]});
      if(start)for(const entry of Object.values(entries))for(const [,targets] of entry.pairs)for(const span of targets)if(entry.u[span[0]]===start.unitId){span[1]+=start.offset;span[2]+=start.offset;}
      if(scope)receipt.sourceScope={firstUnitId:scope.firstUnitId,lastUnitId:scope.lastUnitId,originalRevision:scope.originalRevision,translationRevision:scope.translationRevision,source:scope.source,...(scope.maxSourceSentences!==undefined?{maxSourceSentences:scope.maxSourceSentences}:{}),...(start?{translationStart:start}:{}),...(scope.handoverUnitIds?{handoverUnitIds:scope.handoverUnitIds}:{})};
      translations.push({...meta(version),coverage:receipt.coverage,...(mixedNotes(version)>2?{notesInText:true}:{})});
      out.receipt[`${key}:${version.id}`]=receipt;
      for(const [unit,entry] of Object.entries(entries))(out.units[unit]??={w:key,t:{}}).t[version.id]=entry;
    }
    // In each language a version whose notes stand apart from its text comes before one whose notes run into it; the corpus order otherwise.
    const languages=[...new Set(translations.map(item=>item.language))];
    const ordered=languages.flatMap(language=>translations.filter(item=>item.language===language).map((item,index)=>[item,index]).sort(([a,i],[b,j])=>(a.notesInText?1:0)-(b.notesInText?1:0)||i-j).map(([item])=>item));
    out.works[key]={documentId:work.id,original:meta(original),translations:ordered};
  }
  out.units=Object.fromEntries(Object.entries(out.units).sort(([a],[b])=>a.localeCompare(b)));
  return out;
}

/** The module as stored: its works and receipt laid out, one cited unit per line. */
export function formatAlignment(module){
  const lines=['{'],keys=Object.keys(module);
  keys.forEach((key,index)=>{
    const comma=index<keys.length-1?',':'',value=module[key];
    if(key==='units'){
      const entries=Object.entries(value);
      lines.push(`  "units": {`);
      entries.forEach(([unit,entry],i)=>lines.push(`    ${JSON.stringify(unit)}: ${JSON.stringify(entry)}${i<entries.length-1?',':''}`));
      lines.push(`  }${comma}`);
    }else lines.push(`  ${JSON.stringify(key)}: ${JSON.stringify(value,null,2).replace(/\n/g,'\n  ')}${comma}`);
  });
  lines.push('}');
  return `${lines.join('\n')}\n`;
}
