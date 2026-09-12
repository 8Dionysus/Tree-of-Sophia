import './reader.css';
import {readerAnchorKey,readerVersionKey} from './notebook.mjs';
import {findInParagraphs} from './search.mjs';

const wording={
 ru:{reader:'Чтение',library:'Доступные фрагменты',filter:'Найти фрагмент',close:'Вернуться к Древу',sidebar:'Библиотека',research:'Исследование',single:'Один текст',parallel:'Рядом',settings:'Вид текста',search:'Найти в тексте',previous:'Предыдущее совпадение',next:'Следующее совпадение',emptySearch:'Совпадений нет',searchHint:'Поиск в открытой версии',context:'Связи',notes:'Блокнот',source:'Источник',bookmark:'Закладка',removeBookmark:'Снять закладку',note:'Мысль на полях',noteHint:'Что здесь открылось? Вопрос, возражение, связь…',saved:'Сохранено на этом устройстве',session:'Сохранение недоступно. Скачайте блокнот, чтобы не потерять записи.',conflict:'Блокнот изменён в другом окне. Ваши новые записи доступны для скачивания.',invalid:'Сохранённый блокнот не удалось прочитать. Исходная запись сохранена; новые записи можно скачать.',copy:'Скопировать цитату',copied:'Цитата и источник скопированы',copyManual:'Скопируйте текст из поля',delete:'Удалить заметку',toTree:'Развить в Древе',export:'Скачать блокнот · JSON',markdown:'Скачать заметки · Markdown',import:'Импортировать и заменить блокнот',imported:'Блокнот загружен',allNotes:'Все записи',noNotes:'Выберите номер абзаца, чтобы оставить мысль или закладку.',select:'Выберите абзац в тексте',paragraph:'Абзац',end:'Конец фрагмента',of:'из',boundary:'Границы текста',provenance:'Подготовка текста',rights:'Перевод и права',original:'Открыть источник',credit:'Атрибуция для записи видео',parallelHint:'Колонки прокручиваются независимо; абзацы переводов не выровнены.',night:'Ночь',paper:'Бумага',size:'Размер букв',line:'Межстрочный интервал',width:'Ширина строки',comfortable:'Книжная',wide:'Широкая',related:'Рядом в Древе',contextEmpty:'У этого текста пока нет связей в открытом Древе.',unavailable:'Полный текст пока недоступен',stale:'Версия текста изменилась или недоступна. Запись сохранена в блокноте.',limit:'Достигнут предел блокнота или записи.',error:'Не удалось выполнить действие',local:'Личный блокнот · хранится в этом браузере',go:'Перейти к абзацу',added:'Мысль добавлена в Древо',unsaved:'Сохранение…',bookmarkList:'Закладки',noteList:'Заметки',version:'Версия текста'},
 en:{reader:'Reading',library:'Available passages',filter:'Find a passage',close:'Return to the tree',sidebar:'Library',research:'Research',single:'One text',parallel:'Side by side',settings:'Text appearance',search:'Find in the text',previous:'Previous match',next:'Next match',emptySearch:'No matches',searchHint:'Search the open version',context:'Relations',notes:'Notebook',source:'Source',bookmark:'Bookmark',removeBookmark:'Remove bookmark',note:'A thought in the margin',noteHint:'What opened here? A question, objection, connection…',saved:'Saved on this device',session:'Saving is unavailable. Download the notebook to keep your writing.',conflict:'The notebook changed in another window. Download this window’s new writing to keep it.',invalid:'The stored notebook could not be read. The original is preserved; download new writing to keep it.',copy:'Copy quotation',copied:'Quotation and source copied',copyManual:'Copy the text from this field',delete:'Delete note',toTree:'Develop in the tree',export:'Download notebook · JSON',markdown:'Download notes · Markdown',import:'Import and replace notebook',imported:'Notebook imported',allNotes:'All entries',noNotes:'Choose a paragraph number to leave a thought or bookmark.',select:'Choose a paragraph in the text',paragraph:'Paragraph',end:'End of passage',of:'of',boundary:'Text boundaries',provenance:'Text preparation',rights:'Translation and rights',original:'Open the source',credit:'Attribution for a video',parallelHint:'Columns scroll independently; translation paragraphs are not aligned.',night:'Night',paper:'Paper',size:'Type size',line:'Line spacing',width:'Line width',comfortable:'Book',wide:'Wide',related:'Nearby in the tree',contextEmpty:'This text has no relations in the open tree yet.',unavailable:'Full text is not yet available',stale:'This text version has changed or is unavailable. The entry is preserved in your notebook.',limit:'The notebook or entry limit has been reached.',error:'Could not complete the action',local:'Personal notebook · saved in this browser',go:'Go to paragraph',added:'Thought added to the tree',unsaved:'Saving…',bookmarkList:'Bookmarks',noteList:'Notes',version:'Text version'}
};
const element=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
const button=(text,action,cls='')=>{const b=element('button',cls,text);b.type='button';b.onclick=action;return b;};
function link(text,url){const a=element('a','rr-link',text);const parsed=new URL(url);if(!['https:','http:'].includes(parsed.protocol))throw Error('Invalid source URL');a.href=url;a.target='_blank';a.rel='noopener noreferrer';return a;}
const sameAnchor=(a,b)=>!!a&&!!b&&readerAnchorKey(a)===readerAnchorKey(b);
function download(name,text,type){const a=element('a');const url=URL.createObjectURL(new Blob([text],{type}));a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}

/** Host-independent view over already verified, read-only text documents.
 * See README.md for the adapter contract and the limits of local anchors. */
export function createResearchReader({host=document.body,documents,notebook,locale=()=> 'ru',related=()=>[],context=()=>'',guide=()=>null,onReveal,onDevelop}={}){
 const docs=new Map(documents.map(item=>[item.id,item]));
 const versionCodes=item=>Object.keys(item.versions??{}).sort((a,b)=>a===b?0:a==='ru'?-1:b==='ru'?1:a.localeCompare(b));
 const versionText=(value,code)=>typeof value==='string'?value:value?.[code]??local(value);
 const dialog=element('dialog','tos-reader');host.append(dialog);
 dialog.setAttribute('aria-label','Tree of Sophia · Reading');
 let ui='ru',doc=null,version='ru',contextId=null,selected=null,tab='context',query='',matchIndex=-1,matches=[],searchOpen=false,settingsOpen=false,libraryQuery='';
 let panels=[],editor=null,editorAnchor=null,editorDirty=false,noteTimer=null,scrollTimer=null,opener=null,renderGeneration=0;
 const openGuideSteps=new Set();
 let searchBox,searchCount,message,saveState;
 const t=key=>wording[ui][key]??key,local=value=>typeof value==='string'?value:value?.[ui]??value?.ru??value?.en??'';
 const identity=(code=version,item=doc)=>({documentId:item.id,version:code,sourceRevision:item.versions[code].sourceRevision,textSha256:item.versions[code].textSha256});
 const anchor=(paragraph,code=version)=>({...identity(code),paragraph});
 function say(text){if(message){message.textContent=text;message.hidden=!text;}}
 function status(){if(!saveState)return;const s=notebook.status();saveState.textContent=t(s.writable?'saved':s.error==='conflict'?'conflict':s.error==='invalid-storage'?'invalid':'session');saveState.dataset.warning=String(!s.writable);}
 function act(fn){try{const result=fn();status();return result;}catch(error){say(t(error.message==='limit'?'limit':'error')+(error.message==='limit'?'':': '+error.message));return false;}}
 function flushNote(){
  clearTimeout(noteTimer);if(!editorDirty||!editorAnchor||!editor)return true;
  const ok=act(()=>{if(editor.value.trim())notebook.saveNote(editorAnchor,editor.value);else notebook.deleteNote(editorAnchor);return true;});
  if(ok){editorDirty=false;updateMarkers();refreshEntries();}return ok;
 }
 function capturePositions(){
  clearTimeout(scrollTimer);
  for(const pane of panels){
   const top=pane.getBoundingClientRect().top+20,rows=[...pane.querySelectorAll('.rr-row')];
   const row=rows.find(row=>row.getBoundingClientRect().bottom>top)??rows.at(-1);if(!row)continue;
   const rect=row.getBoundingClientRect(),fraction=pane.scrollTop<5?0:Math.max(0,Math.min(1,(top-rect.top)/rect.height));
   act(()=>notebook.savePosition(anchor(Number(row.dataset.ordinal),pane.dataset.version),fraction));
  }
 }
 function restorePositions(){
  for(const pane of panels){const pos=notebook.positionFor(identity(pane.dataset.version));if(!pos)continue;
   const row=pane.querySelector(`[data-ordinal="${pos.anchor.paragraph}"]`);if(!row)continue;
   if(pos.anchor.paragraph===0&&pos.fraction===0){pane.scrollTop=0;continue;}
   const rect=row.getBoundingClientRect();pane.scrollTop+=rect.top-pane.getBoundingClientRect().top-20+rect.height*pos.fraction;
  }
 }
 function preferences(patch){if(!flushNote())return;capturePositions();act(()=>notebook.setPreferences(patch));render();}
 function switchDocument(id,code=null,keepContext=false){
  if(!docs.has(id)||!flushNote())return;capturePositions();if(!keepContext&&doc.id!==id)contextId=null;doc=docs.get(id);
  const codes=versionCodes(doc),active=notebook.getState().active;
  version=codes.includes(code)?code:active?.documentId===id&&codes.includes(active.version)?active.version:codes.includes(ui)?ui:codes[0]??ui;
  act(()=>notebook.setActive({documentId:doc.id,version}));selected=null;query='';matchIndex=-1;render();
 }
 function switchVersion(code){if(!flushNote())return;capturePositions();version=code;selected=null;query='';matchIndex=-1;act(()=>notebook.setActive({documentId:doc.id,version}));render();}
 function close(){if(!flushNote())return;capturePositions();dialog.close();}
 function selectParagraph(value,{focus=false}={}){
  if(!flushNote())return;capturePositions();selected=value;version=value.version;tab='notes';act(()=>{notebook.setActive({documentId:doc.id,version});notebook.setPreferences({inspector:true});});render();
  if(focus)editor?.focus();
 }
 function jump(value){
  const target=docs.get(value.documentId),supplied=target?.versions?.[value.version];
  if(!supplied||readerVersionKey({...value})!==readerVersionKey(identity(value.version,target))||value.paragraph>=supplied.paragraphs.length){say(t('stale'));return;}
  if(!flushNote())return;capturePositions();if(doc.id!==target.id)contextId=null;doc=target;version=value.version;selected=value;query='';tab='notes';
  act(()=>{notebook.setActive({documentId:doc.id,version});notebook.setPreferences({inspector:true});notebook.savePosition(value,0);});render();
 }
 function citation(value,{quote=false}={}){
  const item=docs.get(value.documentId),v=item?.versions?.[value.version];
  if(!v||readerVersionKey(value)!==readerVersionKey(identity(value.version,item)))return `${value.documentId} · ${value.version} · ${t('paragraph')} ${value.paragraph+1}\n${value.sourceRevision}\nSHA-256 ${value.textSha256}`;
  return `${quote?'«'+v.paragraphs[value.paragraph]+'»\n\n':''}${local(item.author)} — ${local(item.work)}. ${local(item.locator)}. ${value.version.toUpperCase()} · ${t('paragraph')} ${value.paragraph+1}\n${local(v.translator)}. ${local(v.edition)}\n${v.sourceUrl}\n${local(v.rights.credit)}\n${v.rights.url}\n${local(v.editorialNote)}`;
 }
 async function copyCitation(value){
  const text=citation(value,{quote:true});try{await navigator.clipboard.writeText(text);say(t('copied'));}
  catch{const box=element('textarea','rr-copy');box.readOnly=true;box.value=text;box.setAttribute('aria-label',t('copyManual'));message.replaceChildren(element('p','',t('copyManual')),box);message.hidden=false;box.focus();box.select();}
 }
 function exportMarkdown(){if(!flushNote())return;const data=notebook.getState();let text='# Tree of Sophia — '+t('notes')+'\n\n';
  for(const item of data.notes)text+=`## ${local(docs.get(item.anchor.documentId)?.title)||item.anchor.documentId} · ${item.anchor.version.toUpperCase()} · ${item.anchor.paragraph+1}\n\n${item.text}\n\n${citation(item.anchor)}\n\n---\n\n`;
  for(const item of data.bookmarks)text+=`### ${t('bookmark')}\n\n${citation(item.anchor)}\n\n`;
  download('sophia-notes.md',text,'text/markdown;charset=utf-8');
 }
 function updateMarkers(){const data=notebook.getState();for(const pane of panels)for(const row of pane.querySelectorAll('.rr-row')){
  const value=anchor(Number(row.dataset.ordinal),pane.dataset.version),bookmarked=data.bookmarks.some(item=>sameAnchor(item.anchor,value)),noted=data.notes.some(item=>sameAnchor(item.anchor,value));
  row.dataset.bookmarked=String(bookmarked);row.dataset.noted=String(noted);row.dataset.selected=String(sameAnchor(selected,value));
  const number=row.querySelector('.rr-number');number.title=`${t('paragraph')} ${value.paragraph+1}${bookmarked?' · '+t('bookmark'):''}${noted?' · '+t('note'):''}`;number.setAttribute('aria-label',number.title);
 }}
 function renderLibrary(){const aside=element('aside','rr-library');aside.setAttribute('aria-label',t('library'));
  aside.append(element('h2','rr-eyebrow',t('library')));const input=element('input');input.type='search';input.placeholder=t('filter');input.setAttribute('aria-label',t('filter'));input.value=libraryQuery;aside.append(input);
  const list=element('nav','rr-documents');aside.append(list);
  const fill=()=>{list.replaceChildren();let author='';for(const item of documents){if(libraryQuery&&!`${local(item.author)} ${local(item.title)} ${local(item.work)}`.toLocaleLowerCase().includes(libraryQuery.toLocaleLowerCase()))continue;
    const next=local(item.author);if(author!==next){list.append(element('h3','rr-author-group',next));author=next;}
    const b=button('',()=>switchDocument(item.id),'rr-document');b.append(element('strong','',local(item.title)),element('small','',local(item.locator)+(item.status==='link-only'?' · ↗':'')));b.setAttribute('aria-current',String(item.id===doc.id));list.append(b);
  }};input.oninput=()=>{libraryQuery=input.value;fill();};fill();return aside;
 }
 function renderSettings(){const settings=element('div','rr-settings');settings.setAttribute('aria-label',t('settings'));const p=notebook.getState().preferences;
  const row=(label,children)=>{const r=element('div');r.append(element('span','',label),...children);settings.append(r);};
  row(t('size'),[button('−',()=>preferences({fontSize:Math.max(15,p.fontSize-1)})),element('output','',String(p.fontSize)),button('+',()=>preferences({fontSize:Math.min(24,p.fontSize+1)}))]);
  for(const [key,values,label]of [['lineHeight',[1.6,1.9,2.2],t('line')],['width',['comfortable','wide'],t('width')],['theme',['night','paper'],t('settings')]])row(label,values.map(value=>{const b=button(typeof value==='string'?t(value):String(value),()=>preferences({[key]:value}));b.setAttribute('aria-pressed',String(p[key]===value));return b;}));
  return settings;
 }
 function renderSource(body){
  body.append(element('h2','',t('boundary')),element('p','',local(doc.boundary??doc.reason)));
  for(const [code,v]of Object.entries(doc.versions??{})){
   const section=element('section','rr-source-section');section.append(element('h3','',code.toUpperCase()+' · '+local(v.translator)),element('p','',local(v.edition)),link(t('original'),v.sourceUrl),element('h4','',t('provenance')),element('p','',local(v.editorialNote)),element('h4','',t('rights')),element('p','',local(v.rights.basis)),link(v.rights.label,v.rights.url));
   const credit=element('details');credit.append(element('summary','',t('credit')),element('p','',local(v.rights.credit)));section.append(credit);
   const exact=element('details');exact.append(element('summary','','Source revision · SHA-256'),element('code','',v.sourceRevision+'\n'+v.textSha256));section.append(exact);body.append(section);
  }
 }
 function renderContext(body){
  const explanation=context(contextId,doc.id);if(explanation)body.append(element('p','rr-context-intro',local(explanation)));
  const reading=guide(doc.id);
  if(reading&&doc.status==='available'){
   const section=element('section','rr-reading-guide');section.append(element('h2','rr-eyebrow',ui==='ru'?'Читать внимательнее':'Reading closely'),element('p','',local(reading.orientation)));
   for(const [stepIndex,step]of (reading.moves??[]).entries()){
    const key=doc.id+'\0'+stepIndex,detail=element('details','rr-guide-step');detail.open=openGuideSteps.has(key);detail.ontoggle=()=>{if(detail.open)openGuideSteps.add(key);else openGuideSteps.delete(key);};detail.append(element('summary','',local(step.title)),element('p','',local(step.body)));
    const ordinal=step.paragraphs?.[version];
    if(Number.isSafeInteger(ordinal)&&ordinal>=0&&ordinal<doc.versions[version].paragraphs.length){const go=button(`${t('paragraph')} ${ordinal+1} · ${version.toUpperCase()} ↗`,()=>{
     if(!flushNote())return;capturePositions();const value=anchor(ordinal);if(!act(()=>{notebook.savePosition(value,0);return true;}))return;selected=value;restorePositions();updateMarkers();updateProgress();dialog.querySelector(`.rr-pane[data-version="${version}"]`)?.focus({preventScroll:true});
    },'rr-guide-go');go.dataset.guideParagraph=String(ordinal);detail.append(go);}
    section.append(detail);
   }
   section.append(element('p','rr-guide-question',local(reading.question)),element('p','rr-muted',local(reading.limit)));body.append(section);
  }
  const items=related(doc.id);body.append(element('h2','rr-eyebrow',t('related')));
  if(!items.length)body.append(element('p','rr-muted',t('contextEmpty')));
  for(const item of items){const b=button('',()=>{if(!flushNote())return;const ok=act(()=>{onReveal?.(item.id);return true;});if(ok)close();},'rr-relation');b.append(element('strong','',local(item.title)),element('small','',local(item.context)));body.append(b);}
 }
 function renderNotes(body){
  if(selected){
   const data=notebook.getState(),saved=data.notes.find(item=>sameAnchor(item.anchor,selected)),bookmarked=data.bookmarks.some(item=>sameAnchor(item.anchor,selected));
   body.append(element('h2','',`${t('paragraph')} ${selected.paragraph+1} · ${selected.version.toUpperCase()}`));
   const controls=element('div','rr-note-tools');controls.append(button(bookmarked?'◆ '+t('removeBookmark'):'◇ '+t('bookmark'),()=>{act(()=>notebook.toggleBookmark(selected));renderInspector();updateMarkers();}),button(t('copy'),()=>copyCitation(selected)));body.append(controls);
   const label=element('label','rr-note-label',t('note'));editor=element('textarea','rr-note-editor');editor.maxLength=4000;editor.rows=7;editor.placeholder=t('noteHint');editor.value=saved?.text??'';editor.setAttribute('aria-label',t('note'));editorAnchor={...selected};editorDirty=false;label.append(editor);body.append(label);
   editor.oninput=()=>{editorDirty=true;saveState.textContent=t('unsaved');clearTimeout(noteTimer);noteTimer=setTimeout(flushNote,450);};
   const actions=element('div','rr-note-tools');
   if(onDevelop)actions.append(button('✧ '+t('toTree'),()=>{
    if(!flushNote())return;const item=notebook.getState().notes.find(item=>sameAnchor(item.anchor,selected));if(!item){editor.focus();return;}
    const ok=act(()=>{onDevelop({document:doc,anchor:{...selected},text:item.text,citation:citation(selected),contextId});return true;});if(ok)close();
   },'rr-accent'));
   actions.append(button(t('delete'),()=>{act(()=>notebook.deleteNote(selected));editorDirty=false;renderInspector();updateMarkers();}));body.append(actions);
  }else body.append(element('p','rr-muted',t('noNotes')));
  const all=element('details','rr-all-notes');all.open=true;body.append(all,element('p','rr-muted',t('local')));refreshEntries(all);
  const exports=element('div','rr-export-tools');exports.append(button(t('export'),()=>{if(flushNote())download('sophia-notebook.json',notebook.exportData(),'application/json');}),button(t('markdown'),exportMarkdown));
  const input=element('input');input.type='file';input.accept='.json,application/json';input.hidden=true;
  input.onchange=async()=>{const file=input.files[0];if(!file)return;if(file.size>1_500_000){say(t('limit'));return;}const text=await file.text();if(!flushNote())return;const ok=act(()=>{notebook.importData(text);return true;});if(ok){selected=null;const active=notebook.getState().active;if(active&&docs.has(active.documentId)){doc=docs.get(active.documentId);version=doc.versions?.[active.version]?active.version:versionCodes(doc)[0]??ui;}render();say(t('imported'));}};
  exports.append(button(t('import'),()=>input.click()),input);body.append(exports);
 }
 function refreshEntries(all=dialog.querySelector('.rr-all-notes')){
  if(!all)return;all.replaceChildren(element('summary','',t('allNotes')));const data=notebook.getState();
  for(const [field,label]of [['notes','noteList'],['bookmarks','bookmarkList']]){
   if(!data[field].length)continue;all.append(element('h3','rr-eyebrow',t(label)));
   for(const item of [...data[field]].reverse()){const target=docs.get(item.anchor.documentId),b=button('',()=>jump(item.anchor),'rr-entry');b.append(element('strong','',local(target?.title)||item.anchor.documentId),element('small','',`${item.anchor.version.toUpperCase()} · ${t('paragraph')} ${item.anchor.paragraph+1}`));if(item.text)b.append(element('span','',item.text));all.append(b);}
  }
 }
 function renderInspector(){
  if(!flushNote())return;editor=null;editorAnchor=null;
  const inspector=dialog.querySelector('.rr-inspector');if(!inspector)return;inspector.replaceChildren();
  const nav=element('nav','rr-tabs');for(const name of ['context','notes','source']){const b=button(t(name),()=>{if(!flushNote())return;tab=name;renderInspector();});b.setAttribute('aria-pressed',String(tab===name));nav.append(b);}inspector.append(nav);
  const body=element('div','rr-inspector-body');inspector.append(body);if(tab==='source')renderSource(body);else if(tab==='notes')renderNotes(body);else renderContext(body);
 }
 function paintSearch(){
  matches=[];let truncated=false;
  for(const pane of panels){const code=pane.dataset.version,values=doc.versions[code].paragraphs;
   const result=code===version?findInParagraphs(values,query):{matches:[],truncated:false};truncated ||= result.truncated;
   const byParagraph=new Map();for(const hit of result.matches){if(!byParagraph.has(hit.paragraph))byParagraph.set(hit.paragraph,[]);byParagraph.get(hit.paragraph).push(hit);}
   for(const row of pane.querySelectorAll('.rr-row')){const i=Number(row.dataset.ordinal),p=row.querySelector('.rr-text');p.replaceChildren();let at=0;
    for(const hit of byParagraph.get(i)??[]){p.append(document.createTextNode(values[i].slice(at,hit.index)));const mark=element('mark','',values[i].slice(hit.index,hit.index+hit.length));p.append(mark);matches.push({mark,pane,paragraph:i});at=hit.index+hit.length;}
    p.append(document.createTextNode(values[i].slice(at)));
   }
  }
  if(matchIndex>=matches.length)matchIndex=matches.length-1;
  if(searchCount)searchCount.textContent=query?(matches.length?`${Math.max(0,matchIndex+1)} / ${matches.length}${truncated?'+':''} · ${version.toUpperCase()}`:t('emptySearch')):t('searchHint');
 }
 function nextMatch(direction){if(!matches.length)return;for(const item of matches)item.mark.removeAttribute('data-current');matchIndex=(matchIndex+direction+matches.length)%matches.length;const {mark,pane}=matches[matchIndex];mark.dataset.current='true';pane.scrollTop+=mark.getBoundingClientRect().top-pane.getBoundingClientRect().top-pane.clientHeight*.32;searchCount.textContent=`${matchIndex+1} / ${matches.length} · ${version.toUpperCase()}`;}
 function render(){
  if(!flushNote())return;
  renderGeneration++;const generation=renderGeneration;editor=null;editorAnchor=null;panels=[];const prefs=notebook.getState().preferences;
  dialog.replaceChildren();dialog.dataset.theme=prefs.theme;dialog.dataset.sidebar=String(prefs.sidebar);dialog.dataset.inspector=String(prefs.inspector);dialog.dataset.mode=prefs.mode;dialog.dataset.document=doc.id;dialog.dataset.version=version;
  dialog.style.setProperty('--rr-font',prefs.fontSize+'px');dialog.style.setProperty('--rr-leading',prefs.lineHeight);dialog.style.setProperty('--rr-width',prefs.width==='wide'?'940px':'740px');
  const head=element('header','rr-header');const title=element('div','rr-title');title.append(element('span','rr-eyebrow','✧ '+t('reader')),element('strong','',local(doc.title)),element('small','',local(doc.author)+' · '+local(doc.work)));
  head.append(button('☰',()=>{const library=dialog.querySelector('.rr-library'),visible=library&&getComputedStyle(library).display!=='none';preferences({sidebar:!visible,...(!visible?{inspector:false}:{})});},'rr-icon'),title);head.firstChild.setAttribute('aria-label',t('sidebar'));head.firstChild.setAttribute('aria-expanded',String(prefs.sidebar));
  const back=button('↗ '+t('close'),close,'rr-return');head.append(back);dialog.append(head);
  const toolbar=element('nav','rr-toolbar');toolbar.setAttribute('aria-label',t('version'));
  for(const code of versionCodes(doc)){const b=button(code.toUpperCase(),()=>switchVersion(code));b.setAttribute('aria-pressed',String(version===code));toolbar.append(b);}
  if(versionCodes(doc).length>1){const b=button(prefs.mode==='parallel'?t('single'):t('parallel'),()=>preferences({mode:prefs.mode==='single'?'parallel':'single'}));b.setAttribute('aria-pressed',String(prefs.mode==='parallel'));toolbar.append(b);}
  const spacer=element('span','rr-toolbar-space');toolbar.append(spacer);
  toolbar.append(button('⌕ '+t('search'),()=>{capturePositions();searchOpen=!searchOpen;render();if(searchOpen)searchBox?.focus();}),button('Aa',()=>{capturePositions();settingsOpen=!settingsOpen;render();}),button('✧ '+t('research'),()=>preferences({inspector:!prefs.inspector})));
  toolbar.children[toolbar.children.length-2].setAttribute('aria-label',t('settings'));toolbar.lastChild.setAttribute('aria-expanded',String(prefs.inspector));dialog.append(toolbar);
  if(settingsOpen)dialog.append(renderSettings());
  if(searchOpen){const bar=element('div','rr-searchbar');searchBox=element('input');searchBox.type='search';searchBox.maxLength=160;searchBox.placeholder=t('search');searchBox.setAttribute('aria-label',t('search'));searchBox.value=query;searchCount=element('span','rr-search-count');searchCount.setAttribute('role','status');
   searchBox.oninput=()=>{query=searchBox.value;matchIndex=-1;paintSearch();if(matches.length)nextMatch(1);};searchBox.onkeydown=e=>{if(e.key==='Enter'){e.preventDefault();nextMatch(e.shiftKey?-1:1);}};
   bar.append(searchBox,searchCount,button('↑',()=>nextMatch(-1)),button('↓',()=>nextMatch(1)));bar.children[2].setAttribute('aria-label',t('previous'));bar.children[3].setAttribute('aria-label',t('next'));dialog.append(bar);
  }else{searchBox=null;searchCount=null;}
  const layout=element('div','rr-layout');if(prefs.sidebar)layout.append(renderLibrary());const center=element('main','rr-center');layout.append(center);
  if(doc.status==='available'){
   if(prefs.mode==='parallel')center.append(element('p','rr-parallel-hint',t('parallelHint')));
   const columns=element('div','rr-columns');center.append(columns);const codes=prefs.mode==='parallel'?versionCodes(doc):[version];
   for(const code of codes){
    const v=doc.versions[code],pane=element('section','rr-pane');pane.dataset.version=code;pane.lang=code;pane.tabIndex=0;pane.setAttribute('aria-label',local(doc.title)+' · '+code.toUpperCase());panels.push(pane);
    const page=element('article','rr-page'),heading=element('header','rr-book-heading');heading.append(element('p','rr-eyebrow',code.toUpperCase()+' · '+versionText(v.translator,code)),element('h1','',versionText(doc.title,code)),element('p','rr-locator',versionText(doc.locator,code)));page.append(heading);
    v.paragraphs.forEach((text,index)=>{const row=element('div','rr-row');row.dataset.ordinal=String(index);const number=button(String(index+1),()=>selectParagraph(anchor(index,code),{focus:true}),'rr-number');row.append(number,element('p','rr-text',text));page.append(row);});
    page.append(element('p','rr-end','✧ '+t('end')));pane.append(page);columns.append(pane);
    pane.onscroll=()=>{clearTimeout(scrollTimer);scrollTimer=setTimeout(capturePositions,350);updateProgress();};
   }
  }else{const missing=element('div','rr-unavailable');missing.append(element('h1','',local(doc.title)),element('h2','',t('unavailable')),element('p','',local(doc.reason)));for(const target of doc.links??[])missing.append(link(target.label,target.url));center.append(missing);}
  if(prefs.inspector){const inspector=element('aside','rr-inspector');inspector.setAttribute('aria-label',t('research'));layout.append(inspector);}dialog.append(layout);
  const footer=element('footer','rr-footer');const progress=element('span','rr-progress');saveState=element('span','rr-save');saveState.setAttribute('role','status');footer.append(progress,saveState);dialog.append(footer);
  message=element('div','rr-message');message.setAttribute('role','status');message.hidden=true;dialog.append(message);
  renderInspector();updateMarkers();paintSearch();status();
  requestAnimationFrame(()=>{if(generation!==renderGeneration)return;restorePositions();updateProgress();const library=dialog.querySelector('.rr-library');head.firstChild.setAttribute('aria-expanded',String(!!library&&getComputedStyle(library).display!=='none'));});
 }
 function updateProgress(){const text=panels.map(pane=>{const top=pane.getBoundingClientRect().top+20,rows=[...pane.querySelectorAll('.rr-row')],first=rows.find(row=>row.getBoundingClientRect().bottom>top)??rows.at(-1);return `${pane.dataset.version.toUpperCase()} · ${t('paragraph')} ${Number(first?.dataset.ordinal??0)+1} ${t('of')} ${rows.length}`;}).join(' · ');const progress=dialog.querySelector('.rr-progress');if(progress)progress.textContent=text;}
 dialog.addEventListener('cancel',event=>{event.preventDefault();close();});
 dialog.addEventListener('close',()=>{clearTimeout(noteTimer);clearTimeout(scrollTimer);opener?.focus();});
 dialog.addEventListener('keydown',event=>{if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='f'){event.preventDefault();event.stopPropagation();capturePositions();searchOpen=true;render();searchBox.focus();searchBox.select();}});
 const unload=()=>{if(dialog.open){flushNote();capturePositions();}};window.addEventListener('pagehide',unload);const visibility=()=>{if(document.visibilityState==='hidden')unload();};document.addEventListener('visibilitychange',visibility);
 return {open({documentId,contextId:entry=null}={}){
  if(!documents.length)throw Error('No reader documents available');ui=locale()==='en'?'en':'ru';opener=document.activeElement;contextId=entry;
  const remembered=notebook.getState().active;doc=docs.get(documentId)??docs.get(remembered?.documentId)??documents.find(item=>item.status==='available')??documents[0];
  version=doc.versions?.[remembered?.version]&&remembered?.documentId===doc.id?remembered.version:doc.versions?.[ui]?ui:versionCodes(doc)[0]??ui;
  selected=null;query='';libraryQuery='';matchIndex=-1;tab=entry||guide(doc.id)?'context':'notes';act(()=>notebook.setActive({documentId:doc.id,version}));if(!dialog.open)dialog.showModal();render();dialog.querySelector('.rr-pane')?.focus();
 },close,isOpen:()=>dialog.open,destroy(){unload();dialog.remove();window.removeEventListener('pagehide',unload);document.removeEventListener('visibilitychange',visibility);}};
}
