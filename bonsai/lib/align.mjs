// Alignment of the witnesses of one work: paragraphs first, then sentences
// inside each aligned group of paragraphs.
//
// Paragraphs of two languages are joined by length (Gale & Church 1993) with
// small costs for what a translation keeps — questions, exclamations, an
// opening quotation mark or dash, a name the work repeats, verse — and then,
// in a second pass, by words (Moore 2002): an IBM Model 1 is trained on the
// confident one-to-one pairs of the first pass and every candidate group pays
// for the words the model cannot account for. Each group keeps its shape
// (1:1, 1:2, 2:1, 2:2, 1:3, 3:1, a paragraph without counterpart 1:0 or 0:1,
// and up to 1:6 for verse), so where translations part ways the alignment
// says so instead of forcing a match. Inside each group, sentences are
// aligned the same way. A third language checks the two pairings against each
// other (the triangle).
//
// Everything here is deterministic: the same units give the same alignment,
// which is how the stored alignment of a bonsai is checked against its texts.
// The parameters are kept in `ALIGNMENT` so a bonsai can try others.

export const ALIGNMENT=Object.freeze({
  // Prior costs of the group shapes.
  beads:[[1,1,0],[1,0,4.6],[0,1,4.6],[2,1,2.3],[1,2,2.3],[2,2,3.9],[3,1,3.6],[1,3,3.6]],
  // A verse group may stand against up to six lines or paragraphs.
  verse:[[1,4],[1,5],[1,6]],
  band:18,
  stem:{de:6,en:6,ru:5,fr:6,es:6,la:6,grc:5,da:6},
  cues:{question:.45,exclamation:.2,name:.35,opening:.5,verse:1},
  names:/Zarathustra|Заратустр/g,
  model:{iterations:10,prune:1e-4,link:.05,confident:2,beta:.15},
  // A unit of the source that the target leaves out right after another left-out unit costs this
  // instead of its shape's prior: a translation drops whole passages more often than it adds them.
  gapExtend:3,
  sentenceVariance:1.6,
  // A 2:2 group splits into two 1:1 when each half stands on its own lexically.
  split:.6,
});

const ROMAN=Object.fromEntries('I II III IV V VI VII VIII IX X XI XII XIII XIV XV XVI XVII XVIII XIX XX XXI XXII XXIII XXIV XXV XXVI XXVII XXVIII XXIX XXX'.split(' ').map((value,index)=>[value,String(index+1)]));
const OPENING=new Set([...'„»“"«‚›‘—–-']);
const chars=text=>Array.from(text);
const collapse=text=>text.replace(/\s+/g,' ').trim();

/** The word stems a unit is compared by: letters only, lower case, cut to the stem length of its language. */
export function tokens(text,language){
  const n=ALIGNMENT.stem[language]??6;
  return (text.toLowerCase().match(/\p{L}+/gu)??[]).map(word=>chars(word).slice(0,n).join(''));
}

/** What a translation tends to keep: questions, exclamations, an opening mark, a repeated name, verse. */
export function cues(text){
  const value=text.trim();
  return [chars(value).filter(c=>c==='?').length,chars(value).filter(c=>c==='!').length,OPENING.has(chars(value)[0]),(value.match(ALIGNMENT.names)??[]).length,value.includes(' / ')];
}

/**
 * A unit as the aligner sees it. `text` is the unit's reading text (verse
 * lines joined with " / "), `section` its numbered section in the chapter.
 */
export function prepare(units,language){
  return units.map(unit=>({id:unit.id,len:chars(unit.text).length,cue:cues(unit.text),tok:tokens(unit.text,language),section:ROMAN[unit.section]??unit.section??null,text:unit.text}));
}

// The complementary error function, to double precision (W. J. Cody's rational approximations).
function erfc(x){
  const z=Math.abs(x);
  let r;
  if(z<0.5){
    const t=z*z;
    const p=[3.16112374387056560e00,1.13864154151050156e02,3.77485237685302021e02,3.20937758913846947e03,1.85777706184603153e-1];
    const q=[2.36012909523441209e01,2.44024637934444173e02,1.28261652607737228e03,2.84423683343917062e03];
    const num=(((p[4]*t+p[0])*t+p[1])*t+p[2])*t+p[3],den=(((t+q[0])*t+q[1])*t+q[2])*t+q[3];
    r=1-z*num/den;
  }else if(z<4){
    const p=[5.64188496988670089e-1,8.88314979438837594e00,6.61191906371416295e01,2.98635138197400131e02,8.81952221241769090e02,1.71204761263407058e03,2.05107837782607147e03,1.23033935479799725e03,2.15311535474403846e-8];
    const q=[1.57449261107098347e01,1.17693950891312499e02,5.37181101862009858e02,1.62138957456669019e03,3.29079923573345963e03,4.36261909014324716e03,3.43936767414372164e03,1.23033935480374942e03];
    let num=p[8]*z,den=z;
    for(let i=0;i<7;i++){num=(num+p[i])*z;den=(den+q[i])*z;}
    r=(num+p[7])/(den+q[7]);
    const zz=Math.trunc(z*16)/16,del=(z-zz)*(z+zz);
    r*=Math.exp(-zz*zz)*Math.exp(-del);
  }else{
    const p=[3.05326634961232344e-1,3.60344899949804439e-1,1.25781726111229246e-1,1.60837851487422766e-2,6.58749161529837803e-4,1.63153871373020978e-2];
    const q=[2.56852019228982242e00,1.87295284992346725e00,5.27905102951428412e-1,6.05183413124413191e-2,2.33520497626869185e-3];
    const t=1/(z*z);
    let num=p[5]*t,den=t;
    for(let i=0;i<4;i++){num=(num+p[i])*t;den=(den+q[i])*t;}
    r=t*(num+p[4])/(den+q[4]);
    r=(1/Math.sqrt(Math.PI)-r)/z;
    const zz=Math.trunc(z*16)/16,del=(z-zz)*(z+zz);
    r*=Math.exp(-zz*zz)*Math.exp(-del);
  }
  return x<0?2-r:r;
}

const sum=(items,pick)=>items.reduce((total,item)=>total+pick(item),0);
const prior=(di,dj)=>ALIGNMENT.beads.find(([a,b])=>a===di&&b===dj)?.[2]??3+.6*(Math.max(di,dj)-3);

/** The length and cue cost of one group: how unlikely its lengths are for the two languages, and what the translation lost. */
export function lengthCost(source,target,ratio,variance){
  const ls=sum(source,unit=>unit.len),lt=sum(target,unit=>unit.len);
  if(!source.length||!target.length)return prior(source.length,target.length)+.002*(ls+lt);
  const delta=(lt-ls*ratio)/Math.sqrt(Math.max(1,(ls+lt/ratio)/2*variance));
  const weights=ALIGNMENT.cues;
  let cost=-Math.log(Math.max(1e-12,erfc(Math.abs(delta)/Math.SQRT2)))+prior(source.length,target.length);
  cost+=weights.question*Math.abs(sum(source,unit=>unit.cue[0])-sum(target,unit=>unit.cue[0]))
    +weights.exclamation*Math.abs(sum(source,unit=>unit.cue[1])-sum(target,unit=>unit.cue[1]))
    +weights.name*Math.abs(sum(source,unit=>unit.cue[3])-sum(target,unit=>unit.cue[3]));
  if(source[0].cue[2]!==target[0].cue[2])cost+=weights.opening;
  if(source.some(unit=>unit.cue[4])!==target.some(unit=>unit.cue[4]))cost+=weights.verse;
  return cost;
}

/** IBM Model 1 over word stems, trained by expectation maximisation on pairs of token lists. */
export class LexicalModel{
  constructor(pairs,{iterations=ALIGNMENT.model.iterations,prune=ALIGNMENT.model.prune}={}){
    // Words become numbers so the table is one flat map; the arithmetic and its order stay those of the model.
    const words=new Map(),id=word=>{let value=words.get(word);if(value===undefined){value=words.size;words.set(word,value);}return value;};
    const NULL=id('\u0000');
    const numbered=pairs.map(([source,target])=>[[...source.map(id),NULL],target.map(id)]);
    const targetWords=new Set();for(const [,target] of numbered)for(const word of target)targetWords.add(word);
    const V=words.size,uniform=1/Math.max(1,targetWords.size);
    let table=new Map();
    for(let iteration=0;iteration<iterations;iteration++){
      const counts=new Map(),totals=new Float64Array(V);
      for(const [from,target] of numbered){
        const probabilities=new Float64Array(from.length);
        for(const word of target){
          let z=0;
          for(let k=0;k<from.length;k++){const p=iteration?table.get(from[k]*V+word)??1e-7:uniform;probabilities[k]=p;z+=p;}
          for(let k=0;k<from.length;k++){const c=probabilities[k]/z,key=from[k]*V+word;counts.set(key,(counts.get(key)??0)+c);totals[from[k]]+=c;}
        }
      }
      table=new Map();
      for(const [key,c] of counts){const p=c/totals[Math.floor(key/V)];if(p>prune)table.set(key,p);}
    }
    const names=[...words.keys()];
    // A word links to the words it translates with some weight; the reverse serves the other side's cover.
    this.linked=new Map();this.reverse=new Map();
    for(const [key,p] of table){
      const x=Math.floor(key/V),word=key-x*V;
      if(x===NULL||p<ALIGNMENT.model.link)continue;
      const a=names[x],b=names[word];
      if(!this.linked.has(a))this.linked.set(a,new Set());this.linked.get(a).add(b);
      if(!this.reverse.has(b))this.reverse.set(b,new Set());this.reverse.get(b).add(a);
    }
  }
  /** The target words a source group accounts for: its own words and the words they link to. */
  sourceCover(source){
    const cover=new Set(source);
    for(const x of source)for(const word of this.linked.get(x)??[])cover.add(word);
    return cover;
  }
  /** The source words a target group accounts for: its own words and the words that link to them. */
  targetCover(target){
    const cover=new Set(target);
    for(const word of target)for(const x of this.reverse.get(word)??[])cover.add(x);
    return cover;
  }
  /** Words on either side that nothing on the other side accounts for. */
  uncovered(source,target,sourceCover=this.sourceCover(source),targetCover=this.targetCover(target)){
    let count=0;
    for(const word of target)if(!sourceCover.has(word))count++;
    for(const x of source)if(!targetCover.has(x))count++;
    return count;
  }
}

/** The cheapest sequence of groups joining two runs of units, searched in a band around the diagonal. */
export function alignRun(source,target,ratio,variance,{model=null,beta=ALIGNMENT.model.beta,band=ALIGNMENT.band}={}){
  const n=source.length,m=target.length;
  // Three states per cell: after a group with both sides, after a unit the target leaves out, after
  // one the target adds. A run of left-out source units opens once and then extends more cheaply, so a
  // long omission (a passage a censored edition drops) reads as one omission, not as false matches.
  const key=(i,j,state)=>(i*(m+1)+j)*3+state;
  const cost=new Map([[key(0,0,0),0]]),back=new Map();
  // A group's words and what they account for are worked out once per group, not once per candidate.
  const groupsOf=(units,size)=>{const cache=new Map();return (from,to,cover)=>{const k=from*(size+1)+to;let value=cache.get(k);if(!value){const tokens=units.slice(from,to).flatMap(unit=>unit.tok);value={tokens,cover:model?cover(tokens):null};cache.set(k,value);}return value;};};
  const sourceGroup=groupsOf(source,n),targetGroup=groupsOf(target,m);
  for(let i=0;i<=n;i++){
    const centre=Math.trunc(i*m/Math.max(1,n)),lo=Math.max(0,centre-band),hi=Math.min(m,centre+band);
    for(let j=lo;j<=hi;j++){
      for(let state=0;state<3;state++){
        const base=cost.get(key(i,j,state));
        if(base===undefined)continue;
        const moves=ALIGNMENT.beads.map(([a,b])=>[a,b]);
        if(i<n&&source[i].cue[4])moves.push(...ALIGNMENT.verse);
        if(j<m&&target[j].cue[4])moves.push(...ALIGNMENT.verse.map(([a,b])=>[b,a]));
        for(const [di,dj] of moves){
          const ni=i+di,nj=j+dj;
          if(ni>n||nj>m)continue;
          const s=source.slice(i,ni),t=target.slice(j,nj);
          const next=dj===0?1:di===0?2:0;
          let k=lengthCost(s,t,ratio,variance);
          if(next===1&&state===1)k+=ALIGNMENT.gapExtend-prior(di,dj);
          let lex=0;
          if(model){const a=sourceGroup(i,ni,tokens=>model.sourceCover(tokens)),b=targetGroup(j,nj,tokens=>model.targetCover(tokens));lex=beta*model.uncovered(a.tokens,b.tokens,a.cover,b.cover);}
          const value=base+k+lex,at=key(ni,nj,next);
          if(value<(cost.get(at)??Infinity)){cost.set(at,value);back.set(at,[i,j,state,Math.round(k*1000)/1000]);}
        }
      }
    }
  }
  const end=[0,1,2].map(state=>[state,cost.get(key(n,m,state))]).filter(([,value])=>value!==undefined).sort((a,b)=>a[1]-b[1]||a[0]-b[0])[0];
  if(!end)return alignRun(source,target,ratio,variance,{model,beta,band:Math.max(n,m)+1});
  const groups=[];
  for(let i=n,j=m,state=end[0];i||j;){
    const [pi,pj,ps,k]=back.get(key(i,j,state));
    groups.push({s:source.slice(pi,i).map(unit=>unit.id),t:target.slice(pj,j).map(unit=>unit.id),shape:`${i-pi}:${j-pj}`,cost:k});
    i=pi;j=pj;state=ps;
  }
  return groups.reverse();
}

function bySection(units){
  const runs=[];
  for(const unit of units){if(!runs.length||runs.at(-1)[0]!==unit.section)runs.push([unit.section,[]]);runs.at(-1)[1].push(unit);}
  return runs;
}
/** A chapter aligns section by section when both witnesses number the same sections, else as one run. */
export function alignChapter(source,target,ratio,variance,options){
  const a=bySection(source),b=bySection(target);
  if(a.length>1&&a.length===b.length&&a.every(([key],index)=>key===b[index][0]))return a.flatMap(([,units],index)=>alignRun(units,b[index][1],ratio,variance,options));
  return alignRun(source,target,ratio,variance,options);
}

/** The length ratio of the two languages and its variance, from the chapters whose paragraphs pair off one to one. */
export function estimate(sourceChapters,targetChapters){
  const pairs=[],short=[];
  for(const [key,source] of Object.entries(sourceChapters)){
    const target=targetChapters[key];
    if(!target||target.length!==source.length)continue;
    source.forEach((unit,index)=>{(unit.len>40?pairs:short).push([unit.len,target[index].len]);});
  }
  // Too little text to estimate from: short pairs, and failing those Gale and Church's own constants.
  if(!pairs.length)pairs.push(...short.filter(([s,t])=>s&&t));
  if(!pairs.length)return {ratio:1,variance:6.8};
  const ratio=sum(pairs,([,t])=>t)/sum(pairs,([s])=>s);
  const values=pairs.map(([s,t])=>((t-s*ratio)**2)/((s+t/ratio)/2)).sort((x,y)=>x-y);
  const middle=values.length%2?values[(values.length-1)/2]:(values[values.length/2-1]+values[values.length/2])/2;
  return {ratio,variance:Math.max(1,middle*2.198)};
}

// Sentence ends inside a unit's exact text; whitespace between sentences belongs to none.
const END=/[.!?…]+[»«“”"‚‘’)\]]*(?=\s)/gu;
const NEXT=/\s+(?=[«„»“"‚(\[—–-]|\p{L}(?<=[A-ZÄÖÜА-ЯЁ]))/uy;
// Greek ends a sentence with "." and a question with ";"; its raised dot "·" joins clauses inside a
// sentence (a translation renders it as a colon or semicolon), so it ends none. Greek editions seldom
// capitalise a new sentence, Latin ones not always either: their sentences end on the mark alone.
const END_ANCIENT={grc:/[.;\u037e]+[»”’)\]]*(?=\s)/gu,la:/[.!?]+[»”’)\]]*(?=\s)/gu};
const NEXT_ANY=/\s+(?=\S)/uy;
/** [start, end) of each sentence of a unit, in code points of its exact text. */
export function sentenceSpans(raw,language=null){
  const text=chars(raw),joined=raw;
  const ending=END_ANCIENT[language]??END,next_=END_ANCIENT[language]?NEXT_ANY:NEXT;
  // Work in UTF-16 indexes and convert at the end, so the spans count code points.
  const spans=[];let start=joined.length-joined.trimStart().length;
  ending.lastIndex=0;
  for(let match;(match=ending.exec(joined));){
    const end=match.index+match[0].length;
    next_.lastIndex=end;const next=next_.exec(joined);
    if(!next)continue;
    if(end-start>=2)spans.push([start,end]);
    start=next.index+next[0].length;
  }
  const tail=joined.trimEnd().length;
  if(tail>start)spans.push([start,tail]);
  if(!spans.length)spans.push([0,joined.trimEnd().length]);
  if(text.length===joined.length)return spans;
  const point=index=>chars(joined.slice(0,index)).length;
  return spans.map(([a,b])=>[point(a),point(b)]);
}

/** Sentences of one side of a group, carrying [unit index, start, end]. */
export function sentencesOf(units,language){
  const items=[];
  for(const unit of units){
    const text=chars(unit.raw);
    for(const [s,e] of sentenceSpans(unit.raw,language)){
      const sentence=text.slice(s,e).join('');
      // A Greek question mark reads as one for the cues.
      items.push({id:[unit.index,s,e],len:chars(collapse(sentence)).length,cue:cues(language==='grc'?sentence.replace(/[;\u037e]/g,'?'):sentence),tok:tokens(sentence,language),section:null,text:sentence});
    }
  }
  return items;
}

/** Sentence pairs inside one group of paragraphs: [[source spans], [target spans]] for each sentence group that has both sides. */
export function alignSentences(sourceUnits,targetUnits,sourceLanguage,targetLanguage,ratio,variance,model){
  const S=sentencesOf(sourceUnits,sourceLanguage),T=sentencesOf(targetUnits,targetLanguage);
  if(S.length===1&&T.length===1)return [[[S[0].id],[T[0].id]]];
  return alignRun(S,T,ratio,variance*ALIGNMENT.sentenceVariance,{model}).filter(group=>group.s.length&&group.t.length).map(group=>[group.s,group.t]);
}

/**
 * Align the chapters of two witnesses. `source` and `target` map a chapter key to its
 * units ({id, text, raw, section}); a chapter only the source has gives one group
 * `chapter:0`. Returns the groups per chapter with their shapes, and, with `sentences`,
 * the sentence pairs of each group (unit indexes within the chapter, code point offsets).
 */
export function alignWitnesses(source,target,sourceLanguage,targetLanguage,{sentences=true}={}){
  const S=Object.fromEntries(Object.entries(source).map(([key,units])=>[key,prepare(units,sourceLanguage)]));
  const T=Object.fromEntries(Object.entries(target).map(([key,units])=>[key,prepare(units,targetLanguage)]));
  const {ratio,variance}=estimate(S,T);
  const first=Object.fromEntries(Object.keys(S).filter(key=>T[key]).map(key=>[key,alignChapter(S[key],T[key],ratio,variance)]));
  const tokensOf=new Map();for(const side of [S,T])for(const units of Object.values(side))for(const unit of units)tokensOf.set(unit.id,unit.tok);
  const pairs=[];for(const groups of Object.values(first))for(const group of groups)if(group.shape==='1:1'&&group.cost<ALIGNMENT.model.confident)pairs.push([tokensOf.get(group.s[0]),tokensOf.get(group.t[0])]);
  const model=new LexicalModel(pairs);
  const chapters={};
  for(const key of Object.keys(S)){
    if(!T[key]){chapters[key]={targetMissing:true,groups:[{s:S[key].map(unit=>unit.id),t:[],shape:'chapter:0'}]};continue;}
    const groups=[];
    for(const group of alignChapter(S[key],T[key],ratio,variance,{model})){
      if(group.shape==='2:2'){
        const halves=[0,1].map(index=>[tokensOf.get(group.s[index]),tokensOf.get(group.t[index])]);
        if(halves.every(([s,t])=>s.length&&t.length&&model.uncovered(s,t)/(s.length+t.length)<=ALIGNMENT.split)){
          groups.push({...group,s:[group.s[0]],t:[group.t[0]],shape:'1:1',split:'2:2'},{...group,s:[group.s[1]],t:[group.t[1]],shape:'1:1',split:'2:2'});
          continue;
        }
      }
      groups.push(group);
    }
    if(sentences){
      const sourceIndex=new Map(source[key].map((unit,index)=>[unit.id,{...unit,index}])),targetIndex=new Map(target[key].map((unit,index)=>[unit.id,{...unit,index}]));
      for(const group of groups)if(group.s.length&&group.t.length)group.sentences=alignSentences(group.s.map(id=>sourceIndex.get(id)),group.t.map(id=>targetIndex.get(id)),sourceLanguage,targetLanguage,ratio,variance,model);
    }
    chapters[key]={targetMissing:false,groups};
  }
  const result={source:sourceLanguage,target:targetLanguage,ratio:Math.round(ratio*1e4)/1e4,variance:Math.round(variance*1e3)/1e3,modelPairs:pairs.length,chapters};
  // The trained model stays with the result for review (which groups its words do not account for), out of any JSON.
  Object.defineProperty(result,'model',{value:model,enumerable:false});
  Object.defineProperty(result,'tokens',{value:tokensOf,enumerable:false});
  return result;
}

/** Groups with both sides whose words the model mostly cannot account for: where to look first when reviewing. */
export function weakGroups(result,threshold=.7){
  const weak=[];
  for(const [key,chapter] of Object.entries(result.chapters))for(const group of chapter.groups){
    if(!group.s.length||!group.t.length)continue;
    const s=group.s.flatMap(id=>result.tokens.get(id)),t=group.t.flatMap(id=>result.tokens.get(id));
    const share=result.model.uncovered(s,t)/Math.max(1,s.length+t.length);
    if(share>threshold)weak.push({chapter:key,s:group.s,t:group.t,shape:group.shape,uncovered:Math.round(share*100)/100});
  }
  return weak;
}

/**
 * Rows of a chapter: the connected groups of the pivot's units with their counterparts in
 * every other language, in the pivot's order; units a translation adds stand after the row
 * that holds their neighbour. Each row lists unit indexes per language.
 */
export function chapterRows(units,pairings,pivot){
  const index=Object.fromEntries(Object.entries(units).map(([language,list])=>[language,new Map(list.map((unit,i)=>[unit.id,i]))]));
  const parent=new Map(),find=x=>{let root=x;while(parent.get(root)!==root)root=parent.get(root);while(parent.get(x)!==root){const next=parent.get(x);parent.set(x,root);x=next;}return root;};
  const node=(language,id)=>{const k=`${language}\u0001${id}`;if(!parent.has(k))parent.set(k,k);return k;};
  const union=(a,b)=>parent.set(find(a),find(b));
  for(const unit of units[pivot])node(pivot,unit.id);
  for(const [language,groups] of Object.entries(pairings)){
    for(const group of groups){
      const ids=[...group.s.map(id=>node(pivot,id)),...group.t.map(id=>node(language,id))];
      for(const id of ids.slice(1))union(ids[0],id);
    }
  }
  for(const [language,list] of Object.entries(units))if(language!==pivot)for(const unit of list)node(language,unit.id);
  const groups=new Map();
  for(const k of parent.keys()){
    const [language,id]=k.split('\u0001'),root=find(k);
    if(!groups.has(root))groups.set(root,Object.fromEntries(Object.keys(units).map(l=>[l,[]])));
    groups.get(root)[language].push(index[language].get(id));
  }
  const languages=Object.keys(units);
  const rows=[...groups.values()].map(group=>{for(const l of languages)group[l].sort((a,b)=>a-b);return group;})
    .sort((a,b)=>{for(const l of [pivot,...languages.filter(x=>x!==pivot)]){const x=a[l][0]??1e6,y=b[l][0]??1e6;if(x!==y)return x-y;}return 0;})
    .map(group=>Object.fromEntries(languages.filter(l=>group[l].length).map(l=>[l,group[l]])));
  const ordered=rows.filter(row=>row[pivot]);
  for(const row of rows){
    if(row[pivot])continue;
    const language=languages.find(l=>row[l]),first=row[language][0];
    let at=-1;ordered.forEach((other,position)=>{if(other[language]&&other[language].at(-1)<first)at=position;});
    ordered.splice(at+1,0,row);
  }
  return ordered;
}

/**
 * Same-language alignment of two electronic texts of one edition by character trigrams,
 * for mapping one text's units onto another's (moves up to 15:1 and 1:15).
 */
export function alignSameLanguage(source,target,{band=25}={}){
  const key=text=>text.toLowerCase().replaceAll('¬','').replace(/[^a-zäöüß]/g,'');
  const grams=text=>{const set=new Set();for(let i=0;i+3<=text.length;i++)set.add(text.slice(i,i+3));return set;};
  const ks=source.map(unit=>key(unit.text)),kt=target.map(unit=>key(unit.text));
  const moves=[[1,1],[1,0],[0,1],[2,2]];for(let k=2;k<16;k++)moves.push([k,1]);for(let k=2;k<16;k++)moves.push([1,k]);
  const n=source.length,m=target.length,id=(i,j)=>i*(m+1)+j;
  const cached=(keys,size)=>{const cache=new Map();return (from,to)=>{const k=from*(size+1)+to;let value=cache.get(k);if(!value){value=grams(keys.slice(from,to).join(''));cache.set(k,value);}return value;};};
  const sourceGrams=cached(ks,source.length),targetGrams=cached(kt,target.length);
  const similarity=(i,ni,j,nj)=>{let a=sourceGrams(i,ni),b=targetGrams(j,nj);if(a.size>b.size)[a,b]=[b,a];let shared=0;for(const g of a)if(b.has(g))shared++;return shared/Math.max(1,a.size+b.size-shared);};
  const run=width=>{
    const cost=new Map([[0,0]]),back=new Map();
    for(let i=0;i<=n;i++){
      const c=Math.trunc(i*m/Math.max(1,n));
      for(let j=Math.max(0,c-width);j<=Math.min(m,c+width);j++){
        const base=cost.get(id(i,j));if(base===undefined)continue;
        for(const [di,dj] of moves){
          const ni=i+di,nj=j+dj;if(ni>n||nj>m)continue;
          const step=di===0||dj===0?1:(1-similarity(i,ni,j,nj))+.02*(di+dj-2);
          if(base+step<(cost.get(id(ni,nj))??Infinity)){cost.set(id(ni,nj),base+step);back.set(id(ni,nj),[i,j,step]);}
        }
      }
    }
    return cost.has(id(n,m))?back:null;
  };
  const back=run(band)??run(Math.max(n,m)+1);
  const groups=[];
  for(let i=n,j=m;i||j;){
    const [pi,pj,step]=back.get(id(i,j));
    groups.push({s:source.slice(pi,i).map(unit=>unit.id),t:target.slice(pj,j).map(unit=>unit.id),shape:`${i-pi}:${j-pj}`,similarity:i>pi&&j>pj?Math.round((1-step)*1000)/1000:0});
    i=pi;j=pj;
  }
  return groups.reverse();
}

/** The triangle: a pivot unit agrees when its counterparts through one language meet its counterparts in the other. */
export function triangle(pivotToA,pivotToB,aToB){
  const forward=groups=>{const map=new Map();for(const group of groups)for(const s of group.s){if(!map.has(s))map.set(s,new Set());for(const t of group.t)map.get(s).add(t);}return map;};
  const ab=forward(aToB),pb=forward(pivotToB);
  let agree=0,disagree=0,skipped=0;const disagreements=[];
  for(const [unit,as] of forward(pivotToA)){
    const bs=pb.get(unit)??new Set();
    if(!as.size||!bs.size){skipped++;continue;}
    let met=false;for(const a of as){for(const b of ab.get(a)??[])if(bs.has(b)){met=true;break;}if(met)break;}
    if(met)agree++;else{disagree++;disagreements.push(unit);}
  }
  return {agree,disagree,skipped,disagreements};
}
