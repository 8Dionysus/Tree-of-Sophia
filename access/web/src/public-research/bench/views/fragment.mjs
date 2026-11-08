// A fragment on the bench, in three layers. The place itself: its words in the witness it was marked in, the same
// place in the other witnesses, the voice that speaks there. What the Tree already knows: every star, link, place
// relation and sense whose place meets this one, nearest first. The reader's own work: observations, questions and
// readings, links to stars and to other places by verbs, and the fragment kept in "My research".

import {fragmentOf} from '../../../../../../bonsai/lib/witnesses.mjs';
import {PLACE_VERBS,makeId,subjectKey,updateFragment} from '../model.mjs';
import {PIVOT,fingerprintOf,whatTreeKnows,witnessWords,wordsOf} from '../place.mjs';
import {findInBook,loadChapter,witnessManifest} from '../witnesses.mjs';
import {measureOf} from '../breathing.mjs';
import {btn,chapterName,el,localized,section,sign} from './dom.mjs';

const LANGUAGES=['de','en','ru'],KINDS=['observation','question','reading'];

export function fragmentView({root,windowId,ctx}){
  const {t}=ctx;
  const body=el('div','bn-fragment');root.append(body);
  let win=null,chapter=null,manifest=null,token=0,mark='',kind='observation',draft='';
  const lang=()=>String((typeof ctx.locale==='function'?ctx.locale():ctx.locale)||'ru');
  const record=()=>ctx.state().fragments[win?.subject.id]??null;
  const change=(fn,history=true)=>ctx.commit(s=>updateFragment(s,win.subject.id,fn),{history});
  const nameOf=id=>localized(ctx.index?.get?.(id)?.title,lang())||id;
  const witnessLabel=language=>localized(manifest?.witnesses?.[language]?.short,lang())||language.toUpperCase();

  function placeLayer(fragment){
    const box=section(t('place'),'bn-layer bn-layer-place');
    const language=fragment.witness?.language??PIVOT,words=witnessWords(fragment,chapter);
    const where=el('p','bn-where',`${chapter.key} · ${chapterName(chapter.titles,language)} · ${witnessLabel(language)}`);where.dataset.terms='off';
    const quote=el('blockquote','bn-quote bn-quote-main',words);quote.lang=language;
    box.append(where,quote);
    if(fragment.fingerprint&&fragment.fingerprint!==fingerprintOf(words))box.append(el('p','bn-warning',t('sourceChanged')));
    const tools=el('div','bn-row-actions');
    const read=btn(`↗ ${t('openInText')}`,'bn-action');read.addEventListener('click',()=>ctx.open({kind:'chapter',chapter:chapter.key,language,unit:fragment.cite.u[0]}));
    const keep=btn(t('menuTray'),'bn-action');keep.addEventListener('click',()=>ctx.tray({kind:'fragment',id:fragment.id},fragment.label));
    tools.append(read,keep);box.append(tools);
    const others=LANGUAGES.filter(item=>item!==language);
    const list=el('div','bn-witness-list');
    for(const other of others){
      const part=fragmentOf(fragment.cite,chapter,other),row=el('div','bn-witness-row');
      row.append(el('span','bn-label',witnessLabel(other)));
      if(!part||part.omitted){row.append(el('p','bn-muted',t('omitted')));}
      else{const q=el('p','bn-quote',other===PIVOT?wordsOf(fragment.cite,chapter,PIVOT):part.units.map(unit=>unit.text.slice(unit.from,unit.to)).join(' '));q.lang=other;row.append(q);}
      list.append(row);
    }
    box.append(el('span','bn-label',t('otherWitnesses')),list);
    return box;
  }
  function knownLayer(fragment){
    const box=section(t('treeKnows'),'bn-layer bn-layer-tree');
    const known=whatTreeKnows({fragment,chapter,index:ctx.index,core:ctx.core});
    if(known?.voices?.length){
      const line=el('p','bn-voices');line.append(el('span','bn-label',t('voice')));
      // Who speaks to whom: an arrow, so a name keeps the form the Tree gives it in any language.
      for(const voice of known.voices){const v=el('span','bn-voice',[nameOf(voice.speaker),voice.addressee?`→ ${nameOf(voice.addressee)}`:'',t(`mode_${voice.mode}`)!==`mode_${voice.mode}`?`· ${t(`mode_${voice.mode}`)}`:''].filter(Boolean).join(' '));line.append(v);}
      box.append(line);
    }
    const group=(title,entries)=>{
      if(!entries?.length)return;
      // One row for one star: records that the stand names alike stand once, at their nearest.
      const seen=new Set();entries=entries.filter(entry=>{const key=`${ctx.kindOf?.(ctx.toSubject?.(entry.item.id))??entry.item.kind}|${localized(entry.item.title,lang())||entry.item.id}`;if(seen.has(key))return false;seen.add(key);return true;});
      const part=el('div','bn-known');part.append(el('h4','bn-known-title',title));
      for(const entry of entries.slice(0,24)){
        const subject=ctx.toSubject?.(entry.item.id);
        const row=btn('','bn-known-item');row.dataset.near=entry.near;
        row.append(sign(ctx.kindOf?.(subject)??(entry.item.kind==='relation'?'step':'concept')),el('span','bn-known-name',localized(entry.item.title,lang())||entry.item.id),el('span','bn-near',t(`near_${entry.near}`)));
        if(subject)row.addEventListener('click',event=>ctx.open(subject,event.shiftKey?{place:'tab'}:{}));else row.disabled=true;
        part.append(row);
      }
      if(entries.length>24)part.append(el('p','bn-muted',`+${entries.length-24}`));
      box.append(part);
    };
    group(t('nodes'),known?.nodes);group(t('links'),known?.links);group(t('places'),known?.places);
    if(known?.uses?.length){const part=el('div','bn-known');part.append(el('h4','bn-known-title',t('usesHere')));for(const use of known.uses){const row=el('p','bn-use-line');row.append(el('strong','',use.term),` · ${localized(use.label,lang())}`);part.append(row);}box.append(part);}
    const words=witnessWords(fragment,chapter),language=fragment.witness?.language??PIVOT;
    const terms=ctx.termsIn?.(words,language)??[];
    if(terms.length){
      const part=el('div','bn-known');part.append(el('h4','bn-known-title',t('termsHere')));
      for(const term of terms){const chip=btn('','bn-term-chip');chip.lang=language;chip.append(sign('term'),el('span','',term.form));chip.title=t('menuOrigin');chip.addEventListener('click',()=>ctx.open({kind:'word',form:term.form,language,chapter:chapter.key,unit:fragment.cite.u[0]},{binding:'pinned'}));part.append(chip);}
      box.append(part);
    }
    if(!known?.nodes?.length&&!known?.links?.length&&!known?.places?.length)box.append(el('p','bn-white',t('whiteSpot')));
    const repeats=el('div','bn-repeats'),find=btn(t('findRepeats'),'bn-action');
    find.addEventListener('click',async()=>{
      find.disabled=true;repeats.replaceChildren(el('p','bn-wait',t('searching')));
      const needle=words.split(/\s+/).slice(0,8).join(' ');
      try{
        const {hits}=await findInBook(language,needle,{limit:80});
        repeats.replaceChildren(el('p','bn-count',t('found',[hits.length])));
        for(const hit of hits){const row=btn('','bn-hit');row.lang=language;row.append(el('span','bn-hit-where',`${hit.chapter} `),el('span','',hit.before),el('strong','',hit.hit),el('span','',hit.after));row.addEventListener('click',()=>ctx.open({kind:'chapter',chapter:hit.chapter,language,unit:language===PIVOT?hit.id:undefined}));repeats.append(row);}
      }catch{repeats.replaceChildren(el('p','bn-failed',t('unavailable')));}
      find.disabled=false;
    });
    box.append(el('h4','bn-known-title',t('repeats')),find,repeats);
    return box;
  }
  function workLayer(fragment){
    const box=section(t('myWork'),'bn-layer bn-layer-mine');
    for(const note of fragment.notes){
      const row=el('div','bn-note');row.dataset.kind=note.kind;
      row.append(el('span','bn-label',t(`note_${note.kind}`)),el('p','bn-note-body',note.body));
      const remove=btn('×','bn-note-remove',t('removeNote'));remove.addEventListener('click',()=>change(f=>({notes:f.notes.filter(item=>item.id!==note.id)})));row.append(remove);
      box.append(row);
    }
    const form=el('div','bn-note-form'),kinds=el('div','bn-choices');
    for(const item of KINDS){const choice=btn(t(`note_${item}`),'bn-choice');choice.setAttribute('aria-pressed',String(item===kind));choice.addEventListener('click',()=>{kind=item;render();});kinds.append(choice);}
    const input=el('textarea','bn-input');input.rows=3;input.maxLength=4000;input.placeholder=t(`notePlaceholder_${kind}`);input.value=draft;
    input.addEventListener('input',()=>{draft=input.value;});
    const write=btn(t('write'),'bn-action bn-primary');write.addEventListener('click',()=>{const text=input.value.trim();if(!text)return;draft='';change(f=>({notes:[...f.notes,{id:makeId('note'),kind,body:text,at:new Date().toISOString()}]}));});
    form.append(kinds,input,write);box.append(form);
    for(const link of fragment.links){
      const row=el('div','bn-link-row'),open=btn('','bn-link-target');
      open.append(el('em','',link.verb),' → ',el('span','',link.title||ctx.titleOf?.(link.target)||subjectKey(link.target)));
      open.addEventListener('click',()=>ctx.open(link.target));
      const remove=btn('×','bn-note-remove',t('removeNote'));remove.addEventListener('click',()=>change(f=>({links:f.links.filter(item=>item.id!==link.id)})));
      row.append(open,remove);box.append(row);
    }
    const candidates=ctx.bench().tray.filter(item=>['node','edge','fragment'].includes(item.subject.kind)&&!(item.subject.kind==='fragment'&&item.subject.id===fragment.id));
    const linker=el('div','bn-linker');
    if(!candidates.length)linker.append(el('p','bn-muted',t('linkHint')));
    else{
      const target=el('select','bn-select');target.setAttribute('aria-label',t('linkTarget'));
      for(const item of candidates){const option=el('option','',item.title||ctx.titleOf?.(item.subject)||subjectKey(item.subject));option.value=subjectKey(item.subject);target.append(option);}
      const verb=el('input','bn-input bn-verb');verb.placeholder=t('linkVerb');verb.maxLength=120;verb.setAttribute('list',`${windowId}-verbs`);
      const verbs=el('datalist');verbs.id=`${windowId}-verbs`;for(const item of PLACE_VERBS){const option=el('option');option.value=t(`verb_${item}`);verbs.append(option);}
      const add=btn(t('link'),'bn-action');add.addEventListener('click',()=>{const chosen=candidates.find(item=>subjectKey(item.subject)===target.value);const said=verb.value.trim();if(!chosen||!said)return;change(f=>({links:[...f.links,{id:makeId('link'),verb:said,target:chosen.subject,title:chosen.title,at:new Date().toISOString()}]}));});
      linker.append(target,verb,verbs,add);
    }
    box.append(el('h4','bn-known-title',t('link')),linker);
    const save=btn(fragment.saved?`✓ ${t('savedResearch')}`:t('saveResearch'),'bn-action');save.disabled=!!fragment.saved;
    save.addEventListener('click',()=>{const words=witnessWords(fragment,chapter);const id=ctx.saveExcerpt?.(fragment,words);if(id)change(()=>({saved:id}));});
    box.append(save);
    return box;
  }
  function render(){
    const fragment=record();body.replaceChildren();
    if(!fragment||!chapter){body.append(el('p','bn-failed',t('unavailable')));return;}
    mark=JSON.stringify([fragment,ctx.bench().tray.map(item=>subjectKey(item.subject))]);
    body.append(placeLayer(fragment),knownLayer(fragment),workLayer(fragment));
    ctx.markTerms?.(body.querySelector('.bn-layer-place'));
  }
  async function load(){
    const mine=++token,fragment=record();if(!fragment){body.replaceChildren(el('p','bn-failed',t('unavailable')));return;}
    body.replaceChildren(el('p','bn-wait',t('loading')));
    try{manifest??=await witnessManifest();const next=await loadChapter(fragment.cite.c);if(mine!==token)return;chapter=next;render();ctx.changed();}
    catch{if(mine===token)body.replaceChildren(el('p','bn-failed',t('unavailable')));}
  }
  return {
    update(next){win=next;kind='observation';draft='';void load();},
    stateChanged(){const fragment=record();if(!fragment||!chapter)return;const next=JSON.stringify([fragment,ctx.bench().tray.map(item=>subjectKey(item.subject))]);if(next!==mark){const scroll=root.scrollTop;render();root.scrollTop=scroll;}},
    title:w=>ctx.state().fragments[w.subject.id]?.label||t('view_fragment'),
    sign:()=>'passage',
    measure:ch=>measureOf('fragment',{ch}),
    dispose(){},
  };
}
