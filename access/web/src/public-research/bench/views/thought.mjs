// The reader's own thought on the bench: a question, a thesis in the reader's words and the places of the text that
// ground it. Fragments come into it from the tray, by dragging, or from a selection in the text. Saved, it joins
// "My research" as a personal thought with its exact grounds.

import {putThought,subjectKey,updateThought} from '../model.mjs';
import {measureOf} from '../breathing.mjs';
import {btn,el,section,sign} from './dom.mjs';

const DRAG='application/x-tos-bench';

export function thoughtView({root,windowId,ctx}){
  const {t}=ctx;
  const body=el('div','bn-thought');root.append(body);
  let win=null,mark='',timer=null;
  const record=()=>ctx.state().thoughts[win?.subject.id]??null;
  const change=(fn,history=false)=>ctx.commit(s=>updateThought(s,win.subject.id,fn),{history});
  const addGround=id=>change(thought=>({grounds:[...thought.grounds,id],saved:null}),true);

  function render(){
    const thought=record();body.replaceChildren();if(!thought)return;
    mark=JSON.stringify([thought.grounds,thought.saved,thought.grounds.map(id=>ctx.state().fragments[id]?.label),ctx.bench().tray.map(item=>subjectKey(item.subject))]);
    const question=el('input','bn-input bn-thought-question');question.type='text';question.maxLength=4000;question.placeholder=t('thoughtQuestion');question.setAttribute('aria-label',t('thoughtQuestion'));question.value=thought.question;
    const thesis=el('textarea','bn-input bn-thought-thesis');thesis.rows=3;thesis.maxLength=4000;thesis.placeholder=t('thesisPlaceholder');thesis.setAttribute('aria-label',t('thoughtThesis'));thesis.value=thought.thesis;
    const save=()=>{clearTimeout(timer);timer=setTimeout(()=>change(()=>({question:question.value,thesis:thesis.value,saved:null})),400);};
    question.addEventListener('input',save);thesis.addEventListener('input',save);
    body.append(question,thesis);
    const grounds=section(t('grounds'),'bn-grounds');
    for(const id of thought.grounds){
      const fragment=ctx.state().fragments[id];if(!fragment)continue;
      const row=el('div','bn-ground'),open=btn('','bn-ground-open');open.append(sign('passage'),el('span','bn-ground-where',fragment.cite.c),el('span','bn-ground-label',fragment.label));
      open.addEventListener('click',()=>ctx.open({kind:'fragment',id}));
      const remove=btn('×','bn-note-remove',t('removeGround'));remove.addEventListener('click',()=>change(item=>({grounds:item.grounds.filter(value=>value!==id),saved:null}),true));
      row.append(open,remove);grounds.append(row);
    }
    const dropZone=el('div','bn-drop-zone',t('groundsDrop'));
    dropZone.addEventListener('dragover',event=>{if(event.dataTransfer.types.includes(DRAG)){event.preventDefault();dropZone.dataset.over='true';}});
    dropZone.addEventListener('dragleave',()=>{delete dropZone.dataset.over;});
    dropZone.addEventListener('drop',event=>{delete dropZone.dataset.over;let data;try{data=JSON.parse(event.dataTransfer.getData(DRAG));}catch{return;}if(data?.subject?.kind==='fragment'){event.preventDefault();event.stopPropagation();addGround(data.subject.id);}});
    grounds.append(dropZone);
    const fromTray=ctx.bench().tray.filter(item=>item.subject.kind==='fragment'&&!thought.grounds.includes(item.subject.id));
    if(fromTray.length){
      const pick=el('select','bn-select');pick.setAttribute('aria-label',t('addGround'));
      const none=el('option','',`+ ${t('addGround')}`);none.value='';pick.append(none);
      for(const item of fromTray){const option=el('option','',item.title||item.subject.id);option.value=item.subject.id;pick.append(option);}
      pick.addEventListener('change',()=>{if(pick.value)addGround(pick.value);});grounds.append(pick);
    }
    body.append(grounds);
    const keep=btn(thought.saved?`✓ ${t('savedResearch')}`:t('saveResearch'),'bn-action bn-primary');keep.disabled=!!thought.saved;
    const need=el('p','bn-muted');
    keep.addEventListener('click',()=>{
      const current=record(),fragments=current.grounds.map(id=>ctx.state().fragments[id]).filter(Boolean);
      if(!fragments.length){need.textContent=t('groundsNeed');return;}
      const id=ctx.saveThought?.({...current,question:question.value,thesis:thesis.value},fragments);
      if(id)change(()=>({question:question.value,thesis:thesis.value,saved:id}),true);
    });
    body.append(keep,need);
  }
  return {
    update(next){
      win=next;
      if(!next.subject.id||!ctx.state().thoughts[next.subject.id]){
        // A thought window opened on nothing begins a fresh thought under the bench's question.
        let id=null;ctx.commit(s=>{const r=putThought(s,{question:ctx.bench().title});id=r.id;return r.state;},{history:false});
        if(id)ctx.commit(s=>({...s,benches:s.benches.map(bench=>bench.id!==s.active?bench:{...bench,windows:{...bench.windows,[windowId]:{...bench.windows[windowId],subject:{kind:'thought',id}}}})}),{history:false});
        return;
      }
      render();
    },
    stateChanged(){const thought=record();if(!thought)return;const next=JSON.stringify([thought.grounds,thought.saved,thought.grounds.map(id=>ctx.state().fragments[id]?.label),ctx.bench().tray.map(item=>subjectKey(item.subject))]);if(next!==mark&&!body.contains(document.activeElement))render();else if(next!==mark)mark=next;},
    title:w=>{const thought=ctx.state().thoughts[w.subject.id];const line=(thought?.question||thought?.thesis||'').trim();return line?(line.length>40?`${line.slice(0,39)}…`:line):t('view_thought');},
    sign:()=>'personal',
    measure:ch=>measureOf('thought',{ch}),
    dispose(){clearTimeout(timer);},
  };
}
