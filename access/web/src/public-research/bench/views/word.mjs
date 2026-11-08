// The word on the bench, with its own place apart from the readings: its origin as the philological core keeps it
// (the original's word, the first place, the words of the translations and the Tree's own word), the senses the
// bonsai gives its uses, and every place of the book where it stands.

import {findInBook,witnessManifest} from '../witnesses.mjs';
import {measureOf} from '../breathing.mjs';
import {btn,el,localized,section} from './dom.mjs';
import {renderOrigin} from '../../../corpus-reader/word-origin.mjs';
import {bonsaiWordByForm} from '../../../corpus-reader/bonsai-words.mjs';
import {translationLanguage} from '../../renderings-view.mjs';

export function wordView({root,windowId,ctx}){
  const {t}=ctx;
  const body=el('div','bn-word');root.append(body);
  let win=null,search=null,allForms=false,drawn=0,manifest=null,disposed=false;
  witnessManifest().then(value=>{manifest=value;if(win&&!disposed)render();}).catch(()=>{});
  const lang=()=>String((typeof ctx.locale==='function'?ctx.locale():ctx.locale)||'ru');
  const openPlace=(chapter,unit)=>ctx.open({kind:'chapter',chapter,language:win.subject.language,unit});

  function originOf(found){
    const {term,module}=found,box=section(t('original'),'bn-origin');
    const head=el('p','bn-origin-lemma');head.lang=term.original?.lang??'de';head.textContent=term.original?.lemma??'';box.append(head);
    if(term.original?.forms?.length>1)box.append(el('p','bn-muted',term.original.forms.join(' · ')));
    if(term.first?.q){
      const first=el('div','bn-origin-first');first.append(el('span','bn-label',t('firstPlace')));
      const quote=el('blockquote','bn-quote',term.first.q);quote.lang=term.original?.lang??'de';first.append(quote);
      if(term.first.c&&term.first.u?.[0]){const go=btn(`↗ ${term.first.c}`,'bn-link');go.addEventListener('click',()=>ctx.open({kind:'chapter',chapter:term.first.c,language:'de',unit:term.first.u[0]}));first.append(go);}
      box.append(first);
    }
    const rows=el('dl','bn-renderings');
    // A reader sees the original and their own translation (and the witness the word was read in), never a third tongue.
    const readable=new Set([translationLanguage(lang()),win.subject.language]);
    for(const [language,rendering]of Object.entries(term.renderings??{}).filter(([language])=>readable.has(language))){
      const witnesses=[].concat(rendering?.witness??[]).filter(Boolean);
      for(const witness of witnesses){
        // The edition is one of the stand's witnesses: its short name speaks every language of the stand.
        const edition=module?.editions?.[witness.edition],short=manifest?.witnesses?.[edition?.lang??language]?.short;
        rows.append(el('dt','',`${language.toUpperCase()} · ${localized(short??edition?.short??edition?.label,lang())||witness.edition||''}`));
        const dd=el('dd','',witness.term??'');dd.lang=language;rows.append(dd);
      }
      if(rendering?.tos?.term){rows.append(el('dt','',`${language.toUpperCase()} · ${t('treeWord')}`));const dd=el('dd','bn-tree-word',rendering.tos.term);dd.lang=language;rows.append(dd);if(rendering.tos.basis)rows.append(el('dd','bn-muted',localized(rendering.tos.basis,lang())));}
    }
    if(rows.childElementCount){box.append(el('span','bn-label',t('inTranslations')),rows);}
    const gloss=localized(term.gloss,lang());if(gloss){box.append(el('span','bn-label',t('gloss')),el('p','bn-gloss',gloss));}
    const node=ctx.termNode?.(term);
    if(node){const open=btn(`→ ${t('openNode')}`,'bn-action');open.addEventListener('click',()=>ctx.open(node));box.append(open);}
    return box;
  }
  // The word's descent as the philological engine reads it at its place: the word layer's lemma, then the origin
  // records (the oldest form to the word in the book, its parts, its kin; for a translation, the original's word first).
  async function descentInto(box){
    const mine=++drawn,{form,language,chapter,unit,at}=win.subject,inner=el('div','bn-philo-body');box.append(inner);
    const context=chapter&&unit?await bonsaiWordByForm({chapter,language,unitId:unit,form,near:Number.isFinite(at)?at:null}).catch(()=>null):null;
    if(mine!==drawn)return;
    await renderOrigin(inner,{locale:lang(),form,language,context,
      onOpenPlace:place=>ctx.open({kind:'chapter',chapter:place.chapter,language:place.language,unit:place.unitId})}).catch(()=>inner.append(el('p','bn-failed',t('unavailable'))));
  }
  function usesOf(forms){
    const lower=new Set(forms.map(form=>form.toLocaleLowerCase('de')));
    const terms=(ctx.core?.semanticUses?.terms??[]).filter(term=>term.forms?.some(form=>lower.has(form.toLocaleLowerCase('de')))||lower.has(String(term.lemma).toLocaleLowerCase('de')));
    if(!terms.length)return null;
    const box=section(t('uses'));
    for(const term of terms)for(const group of term.groups??[]){
      const item=el('details','bn-use');const summary=el('summary','',`${localized(group.label,lang())} · ${(group.grounds??[]).length}`);item.append(summary);
      const meaning=localized(group.meaning,lang());if(meaning)item.append(el('p','',meaning));
      const places=el('div','bn-places');
      for(const cite of group.grounds??[]){const go=btn(`${cite.c} · ${cite.q??''}`,'bn-place');go.lang='de';go.addEventListener('click',()=>ctx.open({kind:'chapter',chapter:cite.c,language:'de',unit:cite.u?.[0]}));places.append(go);}
      item.append(places);box.append(item);
    }
    return box;
  }
  async function find(list,forms){
    search?.abort();const controller=new AbortController();search=controller;
    list.replaceChildren(el('p','bn-wait',t('searching')));
    const hits=[];let more=false;
    try{for(const form of forms){const found=await findInBook(win.subject.language,form,{limit:300,signal:controller.signal});hits.push(...found.hits);more||=found.more;}}catch{list.replaceChildren(el('p','bn-failed',t('unavailable')));return;}
    if(controller.signal.aborted)return;
    list.replaceChildren(el('p','bn-count',t('found',[hits.length])));
    if(more)list.append(el('p','bn-muted',t('foundMore',[hits.length])));
    let chapter='';
    for(const hit of hits.slice(0,300)){
      if(hit.chapter!==chapter){chapter=hit.chapter;list.append(el('h4','bn-hit-chapter',chapter));}
      const row=btn('','bn-hit');row.lang=win.subject.language;row.append(el('span','',hit.before),el('strong','',hit.hit),el('span','',hit.after));
      row.addEventListener('click',()=>openPlace(hit.chapter,hit.id));list.append(row);
    }
  }
  function render(){
    search?.abort();body.replaceChildren();
    const form=win.subject.form;
    if(!form){body.append(el('p','bn-placeholder',t('wordEmpty')));return;}
    const head=el('header','bn-word-head');const word=el('h2','bn-word-form',form);word.lang=win.subject.language;head.append(word,el('span','bn-label',win.subject.language.toUpperCase()));body.append(head);
    const descent=section(t('descent'),'bn-philo');body.append(descent);void descentInto(descent);
    const found=ctx.termFor?.(form,win.subject.language);
    if(found)body.append(originOf(found));
    const forms=found&&allForms?[...new Set([form,...(found.term.original?.lang===win.subject.language?found.term.original.forms??[]:[])])]:[form];
    const uses=usesOf(found?[form,...(found.term.original?.forms??[])]:[form]);if(uses)body.append(uses);
    const occ=section(t('occurrences')),list=el('div','bn-hits');
    if(found&&found.term.original?.lang===win.subject.language&&(found.term.original.forms??[]).length>1){
      const toggle=btn(t('allForms'),'bn-choice');toggle.setAttribute('aria-pressed',String(allForms));toggle.addEventListener('click',()=>{allForms=!allForms;render();});occ.append(toggle);
    }
    occ.append(list);body.append(occ);
    void find(list,forms);
  }
  return {
    update(next){const same=win&&win.subject.form===next.subject.form&&win.subject.language===next.subject.language&&win.subject.unit===next.subject.unit;win=next;if(!same){allForms=false;render();root.scrollTop=0;}},
    title:w=>w.subject.form||t('view_word'),
    sign:()=>'term',
    measure:ch=>measureOf('word',{ch}),
    dispose(){disposed=true;search?.abort();},
  };
}
