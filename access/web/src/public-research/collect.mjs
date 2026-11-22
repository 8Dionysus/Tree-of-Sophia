// The phone as a tool of collection (G1): a touch marks what is in front of the reader and a sentence, typed or
// dictated, says what was noticed; each is a note of the person's own act journal, marked as collected. On a large
// screen the collected notes are sorted: each becomes a ticket (which keeps the note as its parent) or is let go.
// Notes gathered on another device arrive as its act journal in JSON lines, imported record by record; a record
// already present is left as it is. Nothing here changes the stand, its bonsai or the Tree.
import './act-desk.css';
import './collect.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';

const SPEECH_LANGUAGES=new Map([['ru','ru-RU'],['en','en-US'],['es','es-ES'],['de','de-DE'],['zh-Hans','zh-CN']]);
export const COLLECT_TAGS=Object.freeze(['translation','relation','question','quote']);
const TAG_CATEGORY={translation:'translation',relation:'relation',question:'reading',quote:'text'};

const labels={
  collect:['Собрать','Collect'],
  collectTitle:['Собрать наблюдение','Collect an observation'],
  noSubject:['Без предмета: стенд целиком','No subject: the whole stand'],
  observation:['Что вы заметили','What you noticed'],
  dictate:['Диктовать','Dictate'],
  listening:['Слушаю…','Listening…'],
  noVoice:['Голосовой ввод здесь недоступен','Voice input is unavailable here'],
  save:['Сохранить','Save'],
  saved:['Собрано','Collected'],
  inbox:['Собранное','Collected'],
  inboxLead:['Наблюдения, собранные на ходу: каждое становится тикетом или отпускается. Собранное на другом устройстве приходит его журналом действий.','Observations collected on the go: each becomes a ticket or is let go. What was collected on another device arrives as its act journal.'],
  toTicket:['В тикет','To a ticket'],
  letGo:['Отпустить','Let go'],
  nothing:['Разобрано всё','Everything is sorted'],
  importActs:['Импорт журнала (JSONL)','Import a journal (JSONL)'],
  imported:['Импорт: добавлено {0}, уже было {1}, отказано {2}','Import: added {0}, already present {1}, refused {2}'],
  close:['Закрыть','Close'],
  'tag-translation':['перевод','translation'],
  'tag-relation':['связь','link'],
  'tag-question':['вопрос','question'],
  'tag-quote':['цитата','quotation'],
};

/** The collected notes not yet sorted: neither a parent of a ticket nor withdrawn, newest first. */
export function unsorted(records){
  const sorted=new Set();
  for(const record of records){
    if(record.kind==='ticket')for(const parent of record.parents??[])sorted.add(parent);
    if(record.kind==='withdrawal')for(const target of record.targets??[])if(target.startsWith('act:'))sorted.add(target.slice(4));
  }
  return records.filter(record=>record.kind==='note'&&record.body?.collected===true&&!sorted.has(record.id)).sort((a,b)=>String(b.at).localeCompare(String(a.at)));
}

/** Records of another device's journal, each appended unless the same act is already here. */
export async function importActLines(acts,text){
  const result={added:0,present:0,refused:0,errors:[]};
  for(const line of String(text).split(/\r?\n/)){
    if(!line.trim())continue;
    let record;try{record=JSON.parse(line);}catch{result.refused++;result.errors.push('json');continue;}
    if(record?.id&&await acts.record(record.id)){result.present++;continue;}
    try{await acts.append(record);result.added++;}catch(error){result.refused++;result.errors.push(error?.code??error?.message??String(error));}
  }
  return result;
}

export function mountCollect({shell,actionLayer,acts,actDesk=null,space=null,locale=()=> 'ru',profile='zarathustra',media=globalThis.matchMedia?.bind(globalThis)}){
  const ru=()=>resolveLocale(locale())==='ru';
  const pick=pair=>ru()?pair[0]:localeText(pair[1],locale());
  const t=key=>labels[key]?pick(labels[key]):key;
  const doc=shell.ownerDocument,view=doc.defaultView;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const dialogs=[];
  function sheet(title,cls=''){
    const dialog=el('dialog',`ad-sheet ${cls}`.trim()),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    head.append(el('h2','',title),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);dialog.showModal?.();
    return {dialog,body};
  }
  const addressOf=target=>target?.id??(target?.stand?`place:${profile}:${target.stand.language}:${target.stand.chapter}${target.stand.unitId?`:${target.stand.unitId}:${target.stand.start}-${target.stand.end}`:''}`:null)??`stand:${profile}`;

  // ------------------------------------------------------------ collecting (on any device; a button on a phone)
  function collect(target=actionLayer?.currentTarget?.()){
    const subject=target&&target.type!=='none'?target:null;
    const {dialog,body}=sheet(t('collectTitle'),'cl-sheet');
    const tags=el('div','ad-row cl-tags');let tag=null;
    for(const name of COLLECT_TAGS){const chip=button(t(`tag-${name}`),'text-button ad-tab');chip.dataset.tag=name;chip.setAttribute('aria-pressed','false');
      chip.addEventListener('click',()=>{tag=tag===name?null:name;for(const node of tags.children)node.setAttribute('aria-pressed',String(node.dataset.tag===tag));});tags.append(chip);}
    const text=el('textarea','ad-input ad-text cl-text');text.rows=3;text.placeholder=t('observation');text.setAttribute('aria-label',t('observation'));
    const dictate=button(t('dictate'),'text-button'),save=button(t('save'),'primary');
    const Recognition=view.SpeechRecognition??view.webkitSpeechRecognition;if(!Recognition)dictate.hidden=true;
    dictate.addEventListener('click',()=>{try{const recognition=new Recognition();recognition.lang=SPEECH_LANGUAGES.get(resolveLocale(locale()))??'ru-RU';recognition.interimResults=false;dictate.textContent=t('listening');
      recognition.onresult=event=>{const words=[...event.results].map(result=>result[0]?.transcript??'').join(' ').trim();text.value=[text.value.trim(),words].filter(Boolean).join(' ');};
      recognition.onerror=()=>actionLayer?.say?.(t('noVoice'));recognition.onend=()=>{dictate.textContent=t('dictate');};recognition.start();}catch{actionLayer?.say?.(t('noVoice'));}});
    save.addEventListener('click',async()=>{
      const words=text.value.trim();if(!words&&!subject){text.focus();return;}
      save.disabled=true;
      try{
        const device=media?.('(pointer: coarse)')?.matches?'touch':'pointer';
        await acts.act({kind:'note',targets:[addressOf(subject)],grounds:[],intent:(words||subject?.label||t('collect')).split('\n')[0].slice(0,280),
          body:{text:words,collected:true,device,...(tag?{tag}:{}),...(subject?.label?{label:String(subject.label).slice(0,280)}:{})}});
        actionLayer?.say?.(t('saved'));dialog.close();paintButton();
      }catch(error){save.disabled=false;actionLayer?.say?.(String(error?.message??error));}
    });
    const voice=el('div','ad-row');voice.append(dictate);
    body.append(el('p','ad-subject',subject?.label||t('noSubject')),tags,text,voice,save);
    text.focus();return dialog;
  }

  // ------------------------------------------------------------ sorting on a large screen
  async function records(){const rows=[];let before=Number.MAX_SAFE_INTEGER;for(;;){const batch=await acts.recent({before,limit:500});if(!batch.length)break;rows.push(...batch.map(row=>row.record));before=batch.at(-1).seq;if(batch.length<500)break;}return rows;}
  const nameOf=address=>{const item=space?.index?.get?.(address);return item?localeValue(item.title,locale()):null;};
  async function openInbox(){
    const {body}=sheet(t('inbox'),'cl-inbox');
    const list=el('div','ad-groups'),file=el('input');file.type='file';file.accept='.jsonl,.ndjson,application/x-ndjson,text/plain';file.hidden=true;
    const load=button(t('importActs'),'text-button');load.addEventListener('click',()=>file.click());
    file.addEventListener('change',async()=>{const chosen=file.files?.[0];if(!chosen)return;const result=await importActLines(acts,await chosen.text());file.value='';
      actionLayer?.say?.(t('imported').replace('{0}',result.added).replace('{1}',result.present).replace('{2}',result.refused));await paint();});
    async function paint(){
      const notes=unsorted(await records());list.replaceChildren();
      if(!notes.length)list.append(el('p','ad-muted',t('nothing')));
      for(const note of notes){
        const card=el('article','ad-group cl-note'),address=note.targets[0];
        card.append(el('h3','',note.body?.label||nameOf(address)||(address.startsWith('stand:')?t('noSubject'):address)));
        if(note.body?.text)card.append(el('p','',note.body.text));
        card.append(el('p','ad-muted',[note.body?.tag?t(`tag-${note.body.tag}`):'',new Date(note.at).toLocaleString(resolveLocale(locale())==='zh-Hans'?'zh-CN':resolveLocale(locale()))].filter(Boolean).join(' · ')));
        const row=el('div','ad-row');
        // A note about the whole stand names no place for a ticket; the person chooses one first.
        if(actDesk&&!address.startsWith('stand:')){const ticket=button(t('toTicket'),'primary');ticket.addEventListener('click',()=>{
          const item=space?.index?.get?.(address),target=address.startsWith('place:')?{type:'passage',id:null,label:note.body?.label??address,stand:standOf(address)}
            :{type:item?.kind==='relation'?'link':'star',id:address,label:note.body?.label??nameOf(address)??address,item};
          void actDesk.openTicket(target,{category:TAG_CATEGORY[note.body?.tag]??null,observation:note.body?.text??'',parents:[note.id]});
        });row.append(ticket);}
        const go=button(t('letGo'),'text-button');go.addEventListener('click',async()=>{await acts.act({kind:'withdrawal',targets:[`act:${note.id}`],grounds:[],intent:t('letGo'),body:{}});await paint();paintButton();});
        row.append(go);card.append(row);list.append(card);
      }
    }
    const tools=el('div','ad-row');tools.append(load);
    body.append(el('p','ad-muted',t('inboxLead')),tools,list,file);await paint();
    return {paint};
  }
  const standOf=address=>{const [,,language,chapter,unitId,span]=address.split(':');const [start,end]=(span??'').split('-').map(Number);return unitId?{language,chapter,unitId,start,end}:{language,chapter};};

  // A phone gets a button of its own at the edge of the screen; a large screen reaches both from the palette.
  const fab=button('＋','cl-fab');fab.setAttribute('aria-label',t('collectTitle'));fab.addEventListener('click',()=>collect());
  const coarse=media?.('(pointer: coarse)');
  const paintButton=()=>{fab.hidden=!coarse?.matches;fab.setAttribute('aria-label',t('collectTitle'));fab.title=t('collectTitle');};
  coarse?.addEventListener?.('change',paintButton);shell.append(fab);paintButton();
  const remove=actionLayer?.addActions?.([
    {id:'collect',text:()=>t('collectTitle'),for:['any'],run:target=>collect(target)},
    {id:'inbox',text:()=>t('inbox'),for:['any'],run:()=>openInbox()},
  ]);
  return {collect,openInbox,destroy(){remove?.();coarse?.removeEventListener?.('change',paintButton);fab.remove();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
