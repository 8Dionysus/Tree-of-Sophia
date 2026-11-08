// The book's text on the bench: one chapter in one witness, or the original and a translation row by row. A
// selection takes whole words; its menu lays the place down as a fragment, puts it in the tray, widens it to its
// sentence, opens the word's origin or makes the place a ground of the reader's thought. The margin shows where the
// Tree rests on the text and where the reader's own fragments lie.

import {displayText,fragmentOf} from '../../../../../../bonsai/lib/witnesses.mjs';
import {putFragment,updateThought} from '../model.mjs';
import {PIVOT,citeOfSelection,fingerprintOf,isWord,labelOf,rangeOf,rawOffset,snapToSentence,snapToWords,witnessWords} from '../place.mjs';
import {chapterKeys,loadChapter,witnessManifest} from '../witnesses.mjs';
import {measureOf} from '../breathing.mjs';
import {btn,chapterName,el,localized,offsetIn,pointAt} from './dom.mjs';

const LANGUAGES=['de','en','ru'];
const densities=new Map();

export function textView({root,windowId,ctx}){
  const {t}=ctx,frame=root.parentElement;
  const head=el('div','bn-text-head'),body=el('div','bn-text');root.append(head,body);
  const menu=el('div','bn-select-menu');menu.hidden=true;menu.setAttribute('role','toolbar');frame.append(menu);
  let win=null,chapter=null,manifest=null,token=0,selection=null,lastFlash=null,marked='';
  const settings=()=>({witness:win?.subject.language??PIVOT,numbers:true,margins:true,parallel:false,...ctx.effectiveSettings(windowId)});
  const language=()=>LANGUAGES.includes(settings().witness)?settings().witness:PIVOT;
  const pair=()=>{const lang=language();return lang===PIVOT?(ctx.readingLanguage?.()!=='de'&&LANGUAGES.includes(ctx.readingLanguage?.())?ctx.readingLanguage():'ru'):lang;};

  function density(key){
    if(densities.has(key))return densities.get(key);
    const map=new Map(),index=ctx.index;
    for(const item of index?.items?.values?.()??[]){
      if(item.kind!=='passage'||item.record?.c!==key)continue;
      const range=rangeOf(item.record,chapter);if(!range)continue;
      const owners=index.references(item.id).map(owner=>owner.kind==='statement'?owner.parentId:owner.id);
      for(let i=range.a;i<=range.b;i++){if(!map.has(i))map.set(i,new Set());for(const id of owners)map.get(i).add(id);}
    }
    densities.set(key,map);return map;
  }
  /** Where the reader's fragments lie in a unit of a witness, in shown offsets. */
  function mineIn(lang){
    const marks=new Map(),put=(i,from,to,id)=>{if(to<=from)return;if(!marks.has(i))marks.set(i,[]);marks.get(i).push([from,to,id]);};
    for(const fragment of Object.values(ctx.state().fragments)){
      if(fragment.cite.c!==chapter.key)continue;
      const w=fragment.witness;
      // In the witness it was marked in, a fragment shows exactly as it was selected; elsewhere, as the alignment carries it.
      if(w?.language===lang&&w.u&&lang!==PIVOT){
        const a=chapter.index[lang].get(w.u[0]),b=chapter.index[lang].get(w.u.at(-1));if(a===undefined||b===undefined)continue;
        for(let i=a;i<=b;i++){const unit=chapter.units[lang][i],view=displayText(unit.raw,unit.kind);put(i,i===a?view.at(w.s??0):0,i===b?view.at(w.e??unit.raw.length):view.text.length,fragment.id);}
        continue;
      }
      const part=fragmentOf(fragment.cite,chapter,lang);if(!part||part.omitted)continue;
      for(const unit of part.units)put(unit.index,unit.from,unit.to,fragment.id);
    }
    return marks;
  }
  function unitElement(lang,i,marks,tree){
    const unit=chapter.units[lang][i],view=displayText(unit.raw,unit.kind),p=el('div','bn-unit');
    p.dataset.lang=lang;p.dataset.i=String(i);p.dataset.kind=unit.kind;p.lang=lang;
    const s=settings();
    if(s.numbers){const n=el('span','bn-unit-n',String(i+1));n.setAttribute('aria-hidden','true');p.append(n);}
    if(s.margins){
      const pivotUnits=lang===PIVOT?[i]:chapter.rows.filter(row=>(row[lang]??[]).includes(i)).flatMap(row=>row[PIVOT]??[]);
      const ids=new Set(pivotUnits.flatMap(k=>[...(tree.get(k)??[])]));
      if(ids.size){const dot=el('span','bn-unit-tree');dot.dataset.count=String(Math.min(3,ids.size));dot.title=t('treeHere',[ids.size]);p.append(dot);}
    }
    const text=el('span','bn-unit-text');
    const ranges=(marks.get(i)??[]).sort((x,y)=>x[0]-y[0]);let at=0;
    for(const [from,to,id] of ranges){
      if(from<at)continue;
      if(from>at)text.append(view.text.slice(at,from));
      const mark=el('mark','bn-mine',view.text.slice(from,to));mark.dataset.fragment=id;mark.title=t('mineHere');text.append(mark);at=to;
    }
    if(at<view.text.length)text.append(view.text.slice(at));
    p.append(text);return p;
  }
  function render(){
    const lang=language(),s=settings();
    marked=JSON.stringify(Object.values(ctx.state().fragments).filter(item=>item.cite.c===chapter.key).map(item=>[item.id,item.cite]));
    head.replaceChildren();body.replaceChildren();
    const select=el('select','bn-chapter');select.setAttribute('aria-label',t('chapter'));
    for(const key of chapterKeys()){const info=manifest?.chapters?.find(item=>item.key===key);const absent=lang!==PIVOT&&!info?.titles?.[lang]&&(info?.omitted??[]).includes(lang);const option=el('option','',`${key} · ${chapterName(info?.titles,lang)}${absent?` — ${t('notInWitness')}`:''}`);option.value=key;option.selected=key===chapter.key;select.append(option);}
    select.addEventListener('change',()=>ctx.replace(windowId,{kind:'chapter',chapter:select.value,language:win.subject.language}));
    const keys=chapterKeys(),at=keys.indexOf(chapter.key);
    const prev=btn('‹','bn-step',t('prevChapter')),next=btn('›','bn-step',t('nextChapter'));prev.disabled=at<=0;next.disabled=at<0||at>=keys.length-1;
    prev.addEventListener('click',()=>ctx.replace(windowId,{kind:'chapter',chapter:keys[at-1],language:win.subject.language}));
    next.addEventListener('click',()=>ctx.replace(windowId,{kind:'chapter',chapter:keys[at+1],language:win.subject.language}));
    const witnesses=el('div','bn-witnesses');witnesses.setAttribute('role','group');witnesses.setAttribute('aria-label',t('witness'));
    for(const item of LANGUAGES){
      const info=manifest?.witnesses?.[item],label=localized(info?.short,ctx.locale)||item.toUpperCase();
      const choice=btn(label,'bn-witness');choice.lang=item;choice.setAttribute('aria-pressed',String(item===lang));choice.title=info?.label?.[String((typeof ctx.locale==='function'?ctx.locale():ctx.locale)||'ru')]??(localized(info?.short,ctx.locale)||label);
      choice.addEventListener('click',()=>ctx.settings(windowId,{witness:item}));witnesses.append(choice);
    }
    head.append(prev,select,next,witnesses);
    const tree=density(chapter.key);
    if(s.parallel){
      const other=pair(),left=lang===PIVOT?PIVOT:PIVOT,right=lang===PIVOT?other:lang,marksL=mineIn(left),marksR=mineIn(right);
      body.classList.add('bn-parallel');
      for(const row of chapter.rows){
        const line=el('div','bn-row'),a=el('div','bn-col'),b=el('div','bn-col');
        for(const i of row[left]??[])a.append(unitElement(left,i,marksL,tree));
        for(const i of row[right]??[])b.append(unitElement(right,i,marksR,tree));
        if(!(row[right]??[]).length)b.append(el('p','bn-omitted',t('omitted')));
        line.append(a,b);body.append(line);
      }
    }else{
      body.classList.remove('bn-parallel');
      const marks=mineIn(lang),list=chapter.units[lang]??[];
      if(!list.length||chapter.omitted?.[lang])body.append(el('p','bn-omitted',t('omitted')));
      list.forEach((unit,i)=>body.append(unitElement(lang,i,marks,tree)));
    }
    ctx.markTerms?.(body);
    reveal();
  }
  function reveal(){
    const unit=win?.subject.unit;if(!unit||lastFlash===`${chapter.key}:${unit}:${language()}`)return;
    lastFlash=`${chapter.key}:${unit}:${language()}`;
    const pivotIndex=chapter.index[PIVOT].get(unit),lang=language();
    let target=null;
    if(lang===PIVOT||settings().parallel)target=body.querySelector(`.bn-unit[data-lang="${PIVOT}"][data-i="${pivotIndex}"]`);
    else{const row=chapter.rows.find(item=>(item[PIVOT]??[]).includes(pivotIndex));const i=row?.[lang]?.[0];if(i!==undefined)target=body.querySelector(`.bn-unit[data-lang="${lang}"][data-i="${i}"]`);}
    if(target)requestAnimationFrame(()=>{target.scrollIntoView({block:'center'});target.classList.add('bn-flash');setTimeout(()=>target.classList.remove('bn-flash'),2400);});
  }
  async function load(){
    const mine=++token;
    if(!chapter||chapter.key!==win.subject.chapter)body.replaceChildren(el('p','bn-wait',t('loading')));
    try{
      manifest??=await witnessManifest();
      const next=await loadChapter(win.subject.chapter);if(mine!==token)return;
      chapter=next;render();ctx.changed();
    }catch{if(mine===token)body.replaceChildren(el('p','bn-failed',t('unavailable')));}
  }

  // ------------------------------------------------------------ selection
  const unitOf=node=>(node?.nodeType===1?node:node?.parentElement)?.closest?.('.bn-unit');
  function hideMenu(){menu.hidden=true;selection=null;}
  function readSelection(){
    const sel=document.getSelection();if(!sel||sel.rangeCount===0||sel.isCollapsed)return null;
    const range=sel.getRangeAt(0),first=unitOf(range.startContainer),last=unitOf(range.endContainer);
    if(!first||!last||!body.contains(first)||!body.contains(last)||first.dataset.lang!==last.dataset.lang)return null;
    const lang=first.dataset.lang,a=Number(first.dataset.i),b=Number(last.dataset.i);
    const textA=first.querySelector('.bn-unit-text'),textB=last.querySelector('.bn-unit-text');
    let s=textA.contains(range.startContainer)||textA===range.startContainer?offsetIn(textA,range.startContainer,range.startOffset):0;
    let e=textB.contains(range.endContainer)||textB===range.endContainer?offsetIn(textB,range.endContainer,range.endOffset):textB.textContent.length;
    if(s===null||e===null)return null;
    return {lang,a,b,s,e,textA,textB};
  }
  function applySelection(pick,{sentence=false}={}){
    const {lang,a,b,textA,textB}=pick;let {s,e}=pick;
    const fullA=textA.textContent,fullB=textB.textContent;
    if(a===b){const snapped=sentence?snapToSentence(fullA,s,e):snapToWords(fullA,s,e);s=snapped.start;e=snapped.end;}
    else{s=(sentence?snapToSentence(fullA,s,fullA.length):snapToWords(fullA,s,fullA.length)).start;e=(sentence?snapToSentence(fullB,0,e):snapToWords(fullB,0,e)).end;}
    if(a===b&&e<=s)return null;
    const start=pointAt(textA,s),end=pointAt(textB,e),range=document.createRange();
    try{range.setStart(start.node,start.offset);range.setEnd(end.node,end.offset);}catch{return null;}
    const sel=document.getSelection();sel.removeAllRanges();sel.addRange(range);
    const list=chapter.units[lang];
    return {lang,a,b,s,e,words:range.toString().replace(/\s+/g,' ').trim(),from:{index:a,offset:rawOffset(list[a],s)},to:{index:b,offset:rawOffset(list[b],e)},rect:range.getBoundingClientRect()};
  }
  function showMenu(picked){
    selection=picked;menu.replaceChildren();
    const action=(label,run,cls='')=>{const b=btn(label,`bn-select-action ${cls}`.trim());b.addEventListener('pointerdown',event=>event.preventDefault());b.addEventListener('click',()=>{run();});menu.append(b);};
    action(t('menuFragment'),async()=>{const id=await lay();if(id){ctx.open({kind:'fragment',id});ctx.say(t('savedFragment'));}hideMenu();},'bn-strong');
    action(t('menuTray'),async()=>{const id=await lay();if(id)ctx.tray({kind:'fragment',id},ctx.state().fragments[id]?.label);hideMenu();});
    if(selection.words.split(/\s+/).length>1||!isWord(selection.words))action(t('menuSentence'),()=>{const again=readSelection();if(again){const widened=applySelection(again,{sentence:true});if(widened)showMenu(widened);}});
    if(isWord(selection.words))action(t('menuOrigin'),()=>{const subject={kind:'word',form:selection.words,language:selection.lang,chapter:chapter.key,unit:chapter.units[selection.lang][selection.a].id,at:selection.from.offset};if(!ctx.broadcast(subject))ctx.open(subject,{binding:'follow'});else{const [first]=ctx.windowsOf('word');if(first)ctx.open(first.win.subject);}hideMenu();});
    const thoughts=ctx.windowsOf('thought');
    if(thoughts.length)action(t('menuGround'),async()=>{const id=await lay();const target=thoughts.at(-1);if(id&&target?.win.subject.id){ctx.commit(s=>updateThought(s,target.win.subject.id,thought=>({grounds:[...thought.grounds,id]})));ctx.say(t('addedGround'));}hideMenu();});
    menu.hidden=false;
    const r=frame.getBoundingClientRect(),box=selection.rect;
    const top=box.top-r.top-menu.offsetHeight-8,below=box.bottom-r.top+8;
    menu.style.top=`${top>4?top:below}px`;menu.style.left=`${Math.max(6,Math.min(r.width-menu.offsetWidth-6,box.left-r.left+box.width/2-menu.offsetWidth/2))}px`;
  }
  /** The selection laid down as a fragment; the place is cited in the original whatever witness it was made in. */
  async function lay(){
    if(!selection)return null;
    const made=citeOfSelection(chapter,selection.lang,selection.from,selection.to);if(!made)return null;
    const words=witnessWords(made,chapter)||selection.words;
    let id=null;await ctx.commit(s=>{const r=putFragment(s,{...made,label:labelOf(words),fingerprint:fingerprintOf(words)});id=r.id;return r.state;});
    return id;
  }
  function onSelect(){
    requestAnimationFrame(()=>{
      const pick=readSelection();if(!pick){hideMenu();return;}
      const snapped=applySelection(pick);if(!snapped){hideMenu();return;}
      // A single word reaches the word windows that follow the selection.
      if(isWord(snapped.words))ctx.broadcast({kind:'word',form:snapped.words,language:snapped.lang,chapter:chapter.key,unit:chapter.units[snapped.lang][snapped.a].id,at:snapped.from.offset});
      showMenu(snapped);
    });
  }
  body.addEventListener('mouseup',onSelect);
  body.addEventListener('keyup',event=>{if(event.shiftKey||event.key==='Shift')onSelect();});
  body.addEventListener('click',event=>{const mark=event.target.closest?.('.bn-mine');if(mark&&document.getSelection()?.isCollapsed)ctx.open({kind:'fragment',id:mark.dataset.fragment});});
  // The window body scrolls; a menu left behind would point at a line that has moved away.
  root.addEventListener('scroll',()=>{if(!menu.hidden)hideMenu();},{passive:true});
  const outside=event=>{if(!menu.hidden&&!menu.contains(event.target)&&!body.contains(event.target))hideMenu();};
  document.addEventListener('pointerdown',outside,true);

  return {
    update(next){
      const changed=!win||win.subject.chapter!==next.subject.chapter;win=next;
      if(next.subject.unit&&lastFlash&&!lastFlash.includes(next.subject.unit))lastFlash=null;
      if(changed||!chapter)void load();else render();
    },
    title(w){const info=manifest?.chapters?.find(item=>item.key===w.subject.chapter);const lang=LANGUAGES.includes(ctx.effectiveSettings(windowId).witness)?ctx.effectiveSettings(windowId).witness:w.subject.language;return info?`${w.subject.chapter} · ${chapterName(info.titles,lang)}`:w.subject.chapter;},
    sign:()=>'chapter',
    measure:ch=>measureOf('text',{ch,parallel:!!settings().parallel}),
    settings:(w,values)=>[
      {key:'witness',label:t('witness'),type:'choice',options:LANGUAGES.map(item=>({value:item,label:localized(manifest?.witnesses?.[item]?.short,ctx.locale)||item.toUpperCase()}))},
      {key:'parallel',label:t('parallel'),type:'toggle'},
      {key:'numbers',label:t('numbers'),type:'toggle'},
      {key:'margins',label:t('margins'),type:'toggle'},
    ],
    values:()=>settings(),
    // A fragment laid down or taken away in this chapter shows at once in the text.
    stateChanged(state){if(!chapter)return;const mark=JSON.stringify(Object.values(state.fragments).filter(item=>item.cite.c===chapter.key).map(item=>[item.id,item.cite]));if(mark!==marked){marked=mark;render();}},
    refresh(){if(chapter)render();},
    dispose(){document.removeEventListener('pointerdown',outside,true);menu.remove();},
  };
}
