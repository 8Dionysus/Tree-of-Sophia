// Continuing on another device (G2): a link that opens the same place of the work (subject, mode, lens, layer and route
// step) as text and as a QR code a phone camera reads, the complete export of the person's work for a device that
// should carry all of it, and the act journal as JSON lines. Nothing leaves the device unless the person copies it,
// shows the code or saves the file.
import './act-desk.css';
import {localeText,resolveLocale} from './locale.mjs';
import {qrCode,qrSvg} from './qr-code.mjs';

const labels={
  title:['Продолжить на другом устройстве','Continue on another device'],
  lead:['Ссылка и код открывают то же место: предмет, режим, линзу, слой и шаг маршрута. Ваша работа в этом браузере по ссылке не переходит: её переносит полный экспорт.','The link and the code open the same place: subject, mode, lens, layer and route step. Your work in this browser does not travel with the link; the complete export carries it.'],
  link:['Ссылка на это место','Link to this place'],
  copy:['Копировать ссылку','Copy the link'],
  code:['Код для камеры телефона','A code for a phone camera'],
  tooLong:['Ссылка слишком длинна для кода; скопируйте её','The link is too long for a code; copy it'],
  exportAll:['Полный экспорт работы','Complete export of the work'],
  exportActs:['Журнал действий (JSONL)','Act journal (JSONL)'],
  exportLead:['На другом устройстве полный экспорт открывается импортом в «Моём исследовании». Журнал действий — архив и материал для агента.','On another device the complete export opens through import in “My research”. The act journal is an archive and material for an agent.'],
  close:['Закрыть','Close'],
};

/** The act journal as JSON lines, oldest first: every record whole, with its digest. */
export async function actLines(acts,{page=500}={}){
  const rows=[];let before=Number.MAX_SAFE_INTEGER;
  for(;;){const batch=await acts.recent({before,limit:page});if(!batch.length)break;rows.push(...batch);before=batch.at(-1).seq;if(batch.length<page)break;}
  return rows.sort((a,b)=>a.seq-b.seq).map(row=>JSON.stringify(row.record)).join('\n')+(rows.length?'\n':'');
}

export function mountDeviceHandoff({shell,actionLayer,acts=null,exportWork=null,locale=()=> 'ru'}){
  const ru=()=>resolveLocale(locale())==='ru';
  const t=key=>ru()?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument,view=doc.defaultView;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const dialogs=[];
  function save(name,text,type){const url=view.URL.createObjectURL(new view.Blob([text],{type}));const link=el('a');link.href=url;link.download=name;link.click();view.setTimeout(()=>view.URL.revokeObjectURL(url),1000);}
  function open(target=actionLayer?.currentTarget?.()??{type:'none'}){
    const href=actionLayer?.linkTo?.(target)??view.location.href;
    const dialog=el('dialog','ad-sheet dh-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    head.append(el('h2','',t('title')),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);
    const field=el('input','ad-input dh-link');field.readOnly=true;field.value=href;field.setAttribute('aria-label',t('link'));
    const copy=button(t('copy'),'primary');copy.addEventListener('click',()=>void actionLayer?.copy?.(href));
    body.append(el('p','ad-muted',t('lead')),el('h3','ad-minor',t('link')),field,copy,el('h3','ad-minor',t('code')));
    // The page admits images from itself and data: addresses only; the code is drawn as an SVG image of its own.
    try{
      const code=qrCode(href,{level:'M'}),image=el('img','dh-code');
      image.src=`data:image/svg+xml;charset=utf-8,${encodeURIComponent(qrSvg(code,{label:t('link')}))}`;image.alt=`${t('code')}: ${href}`;image.width=image.height=Math.min(320,(code.size+8)*6);
      body.append(image);
    }catch{body.append(el('p','ad-muted',t('tooLong')));}
    const row=el('div','ad-row');
    if(exportWork){const all=button(t('exportAll'),'text-button');all.addEventListener('click',()=>void Promise.resolve(exportWork()).catch(error=>actionLayer?.say?.(String(error?.message??error))));row.append(all);}
    if(acts){const lines=button(t('exportActs'),'text-button');lines.addEventListener('click',async()=>{try{save(`tos-act-journal-${new Date().toISOString().slice(0,10)}.jsonl`,await actLines(acts),'application/x-ndjson');}catch(error){actionLayer?.say?.(String(error?.message??error));}});row.append(lines);}
    if(row.childElementCount)body.append(row,el('p','ad-muted',t('exportLead')));
    dialog.showModal?.();field.select?.();return {dialog,href};
  }
  const remove=actionLayer?.addActions?.([{id:'continue-elsewhere',text:()=>t('title'),for:['any'],run:target=>open(target)}]);
  return {open,destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
