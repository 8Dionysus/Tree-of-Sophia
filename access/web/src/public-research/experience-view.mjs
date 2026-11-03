import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {createCoreView} from './core-view.mjs';
import {citationId,createResearchIndex} from './research-index.mjs';
import {loadResearchVersion} from './research-history.mjs';
import {mountResearchOperations} from './operation-view.mjs';
import './experience.css';

const modes=['original','reference','cognitive'];
const labels={
  researchModes:['Режимы исследования','Research modes'],mode:['Режим','Mode'],more:['Ещё','More'],
  originalNote:['Карта: звёзды, связи и маршруты','The map: stars, links and routes'],referenceNote:['Статья о каждом предмете и оглавление','An article on every subject, and the contents'],cognitiveNote:['Небо, которое открывают ваши связи','A sky your own links open'],
  original:['Оригинальный','Original'],reference:['Справочный','Reference'],cognitive:['Познавательный','Learning'],back:['Назад','Back'],forward:['Вперёд','Forward'],copy:['Скопировать ссылку','Copy link'],pin:['Закрепить','Pin'],unpin:['Убрать','Remove'],operations:['Вычисления','Calculations'],search:['Искать предмет, утверждение или источник','Search items, claims or sources'],empty:['Выберите предмет в пространстве или найдите его здесь.','Choose an item in the space or search here.'],noResults:['Совпадений нет.','No matches.'],question:['Вопрос исследования','Research question'],statements:['Утверждения','Statements'],references:['Используется в','Referenced by'],pins:['Закреплённые предметы','Pinned items'],compare:['Сопоставление','Comparison'],current:['Текущая версия','Current version'],historical:['Сохранённая версия содержания','Retained content version'],historicWitness:['Цитаты открываются через доступный сейчас читатель свидетелей.','Citations use the currently available witness reader.'],unavailable:['Эта версия недоступна','This version is unavailable'],missing:['Предмет в этой версии не найден.','Item not found in this version.'],saveError:['Не удалось сохранить состояние исследования. Оно осталось в открытой вкладке.','Could not save the research state. It remains in this tab.'],conflict:['Состояние изменилось в другой вкладке. Сохраните эту копию перед загрузкой общего состояния.','The state changed in another tab. Save this copy before loading the shared state.'],recover:['Сохранить эту копию и открыть сохранённое','Save this copy and load the saved state'],copied:['Ссылка скопирована.','Link copied.'],copyError:['Не удалось скопировать ссылку.','Could not copy the link.'],history:['История переходов','Navigation history'],contents:['Справочник','Reference'],contentsNote:['Каждый предмет ядра «Заратустры» с первой строкой его прочтения. Статья открывает формулу книги, голос, связи и основания.','Every subject of the Zarathustra core with the first line of its reading. An article opens the book’s formula, the voice, the links and the grounds.'],kindConception:['Трактовка','Interpretation'],kindConceptions:['Трактовки','Interpretations'],kindImage:['Образ','Image'],kindImages:['Образы','Images'],kindFigure:['Фигура','Figure'],kindFigures:['Фигуры','Figures'],kindRoute:['Маршрут','Route'],kindRoutes:['Маршруты','Routes'],kindProposal:['Предложенный предмет','Proposed subject'],kindProposals:['Предложенные предметы','Proposed subjects'],kindBranchs:['Ветви и вопросы','Branches and questions'],kindReadings:['Прочтения','Readings'],kindSources:['Места источников','Sources'],contentsNoteStand:['Каждая ветвь, прочтение и место источника с первой строкой. Статья открывает место в оригинале и в переводе.','Every branch, reading and source passage with its first line. An article opens the passage in the original and in translation.'],kindRelation:['Связь','Link'],kindPassage:['Место книги','Place in the book'],kindItem:['Предмет','Item'],toContents:['Справочник','Reference contents'],noHistory:['Пока нет переходов.','No navigation yet.'],remove:['Убрать из сравнения','Remove from comparison'],onThisPage:['На этой странице','On this page']
};
const make=(tag,cls='',text)=>{const element=document.createElement(tag);if(cls)element.className=cls;if(text!==undefined)element.textContent=text;return element;};
const action=(text,run,cls='experience-button')=>{const element=make('button',cls,text);element.type='button';element.addEventListener('click',run);return element;};
const clone=value=>JSON.parse(JSON.stringify(value));

/** Shared selection and history; the spatial view remains mounted in every mode. */
export function mountResearchExperience({root,header,profile,locale=()=> 'ru',index,space,api,notebook,onCognitive=()=>{}}={}){
  if(!root||!header||!index||!space)throw new TypeError('Research experience requires root, header, index and space');
  const key=`tos.public-research.experience.v1.${profile}`;
  const t=key=>{const pair=labels[key];if(!pair)return key;return resolveLocale(locale())==='ru'?pair[0]:localeText(pair[1],locale());};
  const title=item=>localeValue(item?.title,locale())||item?.id||'';
  const validMode=value=>modes.includes(value)?value:'original';
  const url=new URL(location.href),urlId=url.searchParams.get('item'),urlRevision=url.searchParams.get('revision');
  let state={mode:validMode(url.searchParams.get('experience')),id:urlId||null,revision:urlRevision||index.revision,stack:[],cursor:-1,pins:[],question:api?.getQuestion?.()??''};
  let edits=0;
  let expected,opsVisible=false,touched=!!(urlId||url.searchParams.has('experience')||urlRevision),disposed=false,saveQueue=Promise.resolve(),renderToken=0,suppressSelection=false,ops=null,opsRevision=null,opsToken=0,searchText='';
  // One bar in every mode: the modes stand beside the tree's name in the space's own header, the history and the
  // rest of the tools at its end. "More" opens what is used seldom; on a phone it opens the whole end of the bar.
  const toolbar=make('nav','experience-toolbar'),modeCurrent=action('',()=>toggleModes(),'rb-mode-current'),modeGroup=make('div','experience-modes'),tools=make('div','experience-tools'),menu=make('div','rb-menu'),more=action('⋯',()=>toggleMenu(),'rb-more');
  modeCurrent.setAttribute('aria-expanded','false');more.setAttribute('aria-expanded','false');more.setAttribute('aria-haspopup','true');
  toolbar.setAttribute('aria-label',t('researchModes'));toolbar.append(modeCurrent,modeGroup);
  const wordmark=header.querySelector?.('.pr-wordmark'),bar=header.querySelector?.('.pr-toolbar')??header;
  header.insertBefore(toolbar,wordmark?.nextSibling??null);bar.append(tools);header.append(more);
  const versionNotice=make('p','experience-version-notice');versionNotice.hidden=true;root.insertBefore(versionNotice,space.root);
  const pane=make('section','experience-pane');pane.hidden=true;
  const question=make('p','experience-question'),message=make('p','experience-message'),recover=action('',()=>void recoverState());recover.hidden=true;message.setAttribute('role','status');
  const searchWrap=make('div','experience-search'),search=make('input');search.type='search';const searchResults=make('div','experience-search-results');searchWrap.append(search,searchResults);
  const body=make('div','experience-body'),comparison=make('section','experience-comparison'),history=make('details','experience-history'),historyTitle=make('summary'),historyBody=make('div');history.append(historyTitle,historyBody);
  const cognitiveRoot=make('section','experience-cognitive'),operationDetails=make('details','experience-operations'),operationTitle=make('summary'),operationRoot=make('div');operationDetails.append(operationTitle,operationRoot);
  pane.append(question,message,recover,searchWrap,body,comparison,history,cognitiveRoot,operationDetails);root.append(pane);
  const modeButtons=new Map();for(const mode of modes){const btn=action('',()=>{setModes(false);setMode(mode);});btn.dataset.mode=mode;modeGroup.append(btn);modeButtons.set(mode,btn);}
  const back=action('',()=>navigate(-1),'experience-button rb-step'),forward=action('',()=>navigate(1),'experience-button rb-step'),pin=action('',()=>togglePin()),copy=action('',()=>void copyLink()),calculation=action('',()=>{opsVisible=!opsVisible;operationDetails.open=opsVisible;render();});menu.append(pin,copy,calculation);tools.append(back,forward,menu);
  let menuOpen=false,modesOpen=false,copiedTimer=0;
  function setMenu(open){menuOpen=open;header.dataset.menu=open?'open':'';more.setAttribute('aria-expanded',String(open));}
  function setModes(open){modesOpen=open;header.dataset.modes=open?'open':'';modeCurrent.setAttribute('aria-expanded',String(open));}
  function toggleMenu(){setModes(false);setMenu(!menuOpen);}
  function toggleModes(){setMenu(false);setModes(!modesOpen);}
  // A press outside an open list closes it; a choice inside closes it after it is made; Escape closes it and gives the keyboard back.
  const outside=event=>{const target=event.target;if(menuOpen&&!bar.contains(target)&&!more.contains(target))setMenu(false);if(modesOpen&&!toolbar.contains(target))setModes(false);};
  const chosen=event=>{if(menuOpen&&event.target?.closest?.('button,a')&&!copy.contains(event.target))setMenu(false);};
  const escape=event=>{if(event.key!=='Escape'||!(menuOpen||modesOpen))return;const owner=menuOpen?more:modeCurrent;setMenu(false);setModes(false);owner.focus?.();event.preventDefault();event.stopImmediatePropagation();};
  document.addEventListener?.('pointerdown',outside,true);document.addEventListener?.('keydown',escape,true);bar.addEventListener('click',chosen);
  const cleanEntry=entry=>entry&&typeof entry.id==='string'?{id:entry.id,revision:typeof entry.revision==='string'?entry.revision:index.revision,scroll:Number.isFinite(entry.scroll)?entry.scroll:0}:null;
  const cleanSaved=value=>({mode:validMode(value?.mode),id:typeof value?.id==='string'?value.id:null,revision:typeof value?.revision==='string'?value.revision:index.revision,stack:Array.isArray(value?.stack)?value.stack.map(cleanEntry).filter(Boolean).slice(-80):[],cursor:Number.isInteger(value?.cursor)?value.cursor:-1,pins:Array.isArray(value?.pins)?value.pins.map(cleanEntry).filter(Boolean).slice(0,2):[],question:typeof value?.question==='string'?value.question:''});
  function snapshot(){return {...state,question:api?.getQuestion?.()??state.question};}
  function save(){
    touched=true;edits+=1;const value=clone(snapshot());
    saveQueue=saveQueue.then(async()=>{await initial;await notebook.setPreference(key,value,{expectedValue:expected});expected=value;if(!disposed){message.textContent='';recover.hidden=true;}}).catch(error=>{if(!disposed){message.textContent=t(error?.code==='conflict'?'conflict':'saveError');recover.hidden=error?.code!=='conflict';}});
  }
  async function recoverState(){
    try{await saveQueue;const draft=clone(snapshot());await notebook.setPreference(`${key}.recovery.${crypto.randomUUID()}`,draft);const saved=await notebook.getPreference(key);expected=saved;state=cleanSaved(saved);state.cursor=Math.min(state.cursor,state.stack.length-1);touched=false;recover.hidden=true;message.textContent='';writeUrl(false);render();}
    catch{message.textContent=t('saveError');}
  }
  const initial=(async()=>{try{if(!notebook?.getPreference||!notebook?.setPreference)throw new Error('Notebook unavailable');const saved=await notebook.getPreference(key);expected=saved;if(!edits&&saved){const requested=state;state=cleanSaved(saved);state.cursor=Math.min(state.cursor,state.stack.length-1);if(url.searchParams.has('experience'))state.mode=requested.mode;if(urlId){state.id=urlId;state.revision=urlRevision||index.revision;const at=state.stack.findIndex(entry=>entry.id===state.id&&entry.revision===state.revision);if(at>=0)state.cursor=at;else{state.stack.push({id:state.id,revision:state.revision,scroll:0});state.cursor=state.stack.length-1;}}writeUrl(false);}if(state.id&&!state.stack.length){state.stack=[{id:state.id,revision:state.revision,scroll:0}];state.cursor=0;}if(state.id&&state.mode==='original'&&state.revision===index.revision){suppressSelection=true;try{space.openResearch?.(state.id);}finally{suppressSelection=false;}}render();if(state.mode==='reference'&&state.cursor>=0)requestAnimationFrame(()=>window.scrollTo(0,state.stack[state.cursor]?.scroll??0));}catch{if(!disposed)message.textContent=t('saveError');}})();
  function writeUrl(push){const next=new URL(location.href);next.searchParams.set('experience',state.mode);if(state.id){next.searchParams.set('item',state.id);next.searchParams.set('revision',state.revision??index.revision);}else{next.searchParams.delete('item');next.searchParams.delete('revision');}window.history[push?'pushState':'replaceState'](window.history.state,'',next);}
  function rememberScroll(){if(state.cursor>=0&&state.stack[state.cursor])state.stack[state.cursor].scroll=state.mode==='reference'?window.scrollY:(space.selection?.()?.scroll??0);}
  function setMode(mode,{push=true,persist=true}={}){
    mode=validMode(mode);if(mode===state.mode)return;rememberScroll();state.mode=mode;
    if(mode==='original'&&state.id&&state.revision===index.revision){suppressSelection=true;try{space.openResearch?.(state.id);}finally{suppressSelection=false;}}
    writeUrl(push);render();if(persist)save();onCognitive({action:'mode',mode,id:state.id});
  }
  // History moves record the scroll of the entry they leave before they move the cursor; `remember:false` keeps the destination's own.
  function open(id,{revision=index.revision,push=true,record=true,show=true,remember=true}={}){
    if(!id)return false;if(revision===index.revision)id=index.resolveId?.(id)??id;if(remember)rememberScroll();state.id=id;state.revision=revision||index.revision;
    if(record){state.stack=state.stack.slice(0,state.cursor+1);const previous=state.stack.at(-1);if(!previous||previous.id!==id||previous.revision!==state.revision)state.stack.push({id,revision:state.revision,scroll:0});state.stack=state.stack.slice(-80);state.cursor=state.stack.length-1;}
    if(show&&state.mode==='original'&&state.revision===index.revision){suppressSelection=true;try{space.openResearch?.(id);}finally{suppressSelection=false;}}
    writeUrl(push);render();save();onCognitive({action:'open',id,revision:state.revision});return true;
  }
  function navigate(delta){const next=state.cursor+delta;if(next<0||next>=state.stack.length)return;rememberScroll();state.cursor=next;const entry=state.stack[next];open(entry.id,{revision:entry.revision,record:false,remember:false});requestAnimationFrame(()=>{if(state.mode==='reference')window.scrollTo(0,entry.scroll);});}
  function togglePin(){if(!state.id)return;const at=state.pins.findIndex(item=>item.id===state.id&&item.revision===state.revision);if(at>=0)state.pins.splice(at,1);else{if(state.pins.length===2)state.pins.shift();state.pins.push({id:state.id,revision:state.revision,scroll:0});}render();save();}
  async function copyLink(){try{const target=new URL(location.href);target.searchParams.set('experience',state.mode);if(state.id){target.searchParams.set('item',state.id);target.searchParams.set('revision',state.revision??index.revision);}await navigator.clipboard.writeText(target.href);message.textContent=t('copied');copy.textContent=t('copied');clearTimeout(copiedTimer);copiedTimer=setTimeout(()=>{copy.textContent=t('copy');setMenu(false);},1100);}catch{message.textContent=t('copyError');}}
  const historical=new Map();
  const peekVersionContext=(revision=state.revision)=>revision===index.revision?{index,current:true}:historical.get(revision)?.context??null;
  async function versionContext(revision=state.revision){
    if(revision===index.revision)return {index,current:true};
    if(!historical.has(revision)){
      const entry={context:null};historical.set(revision,entry);
      entry.promise=loadResearchVersion(revision).then(packet=>{
        if(packet.status!=='available')throw new TypeError(`Research version is unavailable: ${revision}`);
        const core=packet.core,records={},materialIds={};for(const item of [...(core.concepts??[]),...(core.figures??[]),...(core.proposed??[]),...(core.chains??[]).flatMap(chain=>chain.states??[])]){records[item.id]=item;materialIds[item.id]=item.id;}
        const historicalCore={...core,records,materialIds},oldIndex=createResearchIndex({nodes:[],space:{core:historicalCore}},{revision});
        const view=createCoreView({core:historicalCore,locale,fragments:space.fragments,onNode:id=>open(id,{revision}),onRelation:id=>open(id,{revision}),onRoute:id=>open(id.replace(/^core:/,''),{revision}),onAction:detail=>handleAction({...detail,revision})});
        return entry.context={index:oldIndex,view,current:false};
      });
    }return historical.get(revision).promise;
  }
  async function renderOperations(){
    const revision=state.revision,token=++opsToken;if(ops&&opsRevision===revision)return;
    ops?.destroy();ops=null;opsRevision=null;operationRoot.replaceChildren();
    try{
      const context=await versionContext(revision);if(disposed||token!==opsToken||!opsVisible)return;
      ops?.destroy();opsRevision=revision;
      ops=mountResearchOperations({root:operationRoot,index:context.index,locale,notebook,getSelection:()=>state.revision===revision?state.id:null,onOpen:id=>open(id,{revision}),timeOf:context.current?space.timeOf:()=>null,placeOf:context.current?space.placeOf:()=>null,current:Boolean(context.current)});
    }catch{
      if(disposed||token!==opsToken||!opsVisible)return;ops?.destroy();ops=null;opsRevision=null;operationRoot.textContent=`${t('unavailable')}: ${revision}`;
    }
  }
  async function renderItem(target,id,revision){
    let context;try{context=await versionContext(revision);}catch{context=null;}if(!target.isConnected||disposed)return;
    if(!context){target.append(make('p','experience-error',`${t('unavailable')}: ${revision}`));return;}
    const item=context.index.get(id);if(!item){target.append(make('p','experience-error',t('missing')));return;}
    if(!context.current){target.append(make('p','experience-version',`${t('historical')} · ${revision}`),make('p','experience-version',t('historicWitness')));
      if(context.view.renderReference(target,{kind:item.kind==='relation'?'relation':'node',id:item.id}))return;
      target.append(make('h2','',title(item)));if(item.kind==='passage')target.append(space.fragments.fragment(item.record));else if(item.kind==='statement')space.fragments.render(target,item.record);else renderStructured(target,item.record);return;
    }
    if(!space.renderResearch?.(target,item.id))target.append(make('p','experience-error',t('missing')));
  }
  function renderStructured(parent,value){
    if(value==null)return;if(Array.isArray(value)){for(const item of value)renderStructured(parent,item);return;}
    if(typeof value!=='object'){parent.append(make('p','',String(value)));return;}
    if(typeof value.ru==='string'||typeof value.en==='string'){space.fragments.render(parent,value);return;}
    for(const [key,item] of Object.entries(value)){if(['id','statement_ids'].includes(key)||item==null)continue;const section=make('section','experience-structured');section.append(make('h3','',key));if(key==='cite'&&item?.w)section.append(space.fragments.fragment(item));else renderStructured(section,item);parent.append(section);}
  }
  // Without a chosen subject the reference opens on its contents: every subject of the core with its first line.
  const kindName=item=>{const k=item?.kind;return t(k==='figure'?'kindFigure':k==='tos.entity.thought-image'?'kindImage':k==='chain'?'kindRoute':k==='relation'?'kindRelation':k==='passage'?'kindPassage':item?.record?.conception?'kindConception':item?.owner==='bonsai/core'?'kindProposal':'kindItem');};
  // The first line follows the name it stands under: a line that opens with that name goes on from it.
  const afterName=(text,name)=>{if(!text||!name)return text;const lower=text.toLocaleLowerCase(),head=name.toLocaleLowerCase();if(!lower.startsWith(head))return text;const rest=text.slice(name.length).replace(/^[\s:;,.—–-]+/u,'');return rest?rest.charAt(0).toLocaleUpperCase()+rest.slice(1):text;};
  function renderContents(target){
    let core=[...index.items.values()].filter(item=>item.owner==='bonsai/core'),stand=false;
    // A stand without a core (Antiquity) opens on its branches, readings and source passages.
    if(!core.length){core=[...index.items.values()].filter(item=>['question','interpretation','fragment'].includes(item.kind));stand=true;}
    if(!core.length)return false;
    const groups=[['kindConception',item=>item.kind==='tos.entity.conception'&&item.record?.conception],['kindImage',item=>item.kind==='tos.entity.thought-image'],['kindFigure',item=>item.kind==='figure'],['kindRoute',item=>item.kind==='chain'&&item.record?.admission==='map'],['kindProposal',item=>!item.record?.conception&&!item.record?.existence&&!['relation','chain','state','statement','passage-relation','figure','tos.entity.thought-image'].includes(item.kind)]];
    if(stand)groups.splice(0,groups.length,['kindBranch',item=>item.kind==='question'],['kindReading',item=>item.kind==='interpretation'],['kindSource',item=>item.kind==='fragment']);
    const line=record=>localeValue(record?.conception?.thesis??record?.essence?.who??record?.sense??record?.reading?.reading??record?.reading??(stand?record?.question??record?.body:null),locale());
    const contents=make('div','experience-contents');contents.append(make('h1','experience-contents-title',t('contents')),make('p','experience-contents-note',t(stand?'contentsNoteStand':'contentsNote')));
    for(const [label,test] of groups){
      const items=core.filter(test).sort((a,b)=>title(a).localeCompare(title(b),resolveLocale(locale())));if(!items.length)continue;
      const section=make('section','experience-contents-group');section.append(make('h2','',`${t(label+'s')} · ${items.length}`));const grid=make('div','experience-contents-grid');
      for(const item of items){const entry=action('',()=>open(item.id),'experience-entry'),name=title(item);entry.append(make('strong','',name));const first=afterName(line(item.record),name);if(first)entry.append(make('span','',first));grid.append(entry);}
      section.append(grid);contents.append(section);
    }
    target.append(contents);return true;
  }
  function renderSearch(){searchResults.replaceChildren();if(!searchText.trim())return;const found=index.search(searchText,{limit:30});if(!found.length){searchResults.append(make('p','',t('noResults')));return;}for(const item of found)searchResults.append(action(`${title(item)} · ${kindName(item)}`,()=>{searchText='';search.value='';renderSearch();open(item.id);},'experience-result'));}
  function renderRefs(target,id){
    const refs=index.references(id),claims=[...index.items.values()].filter(item=>item.kind==='statement'&&item.parentId===id);
    for(const [label,items] of [['statements',claims],['references',refs]])if(items.length){const block=make('details','experience-reference-list'),summary=make('summary','',`${t(label)} · ${items.length}`);block.append(summary);for(const item of items)block.append(action(refLabel(item),()=>open(item.id),'experience-result'));target.append(block);}
  }
  // A claim is named by its own words, anything else by its title and kind; the ids stay in the address, not in the reading.
  function refLabel(item){
    if(item.kind==='statement'){const text=localeValue(item.record,locale());if(typeof text==='string'&&text.trim())return text.length>160?`${text.slice(0,159).trimEnd()}…`:text;}
    return `${title(item)} · ${kindName(item)}`;
  }
  // The contents of a long article stand beside it (under the bar on a narrow screen) and mark the part being read.
  let tocItems=[],tocFrame=0;
  function contentsOf(toc,article){
    const heads=[...(article.querySelectorAll?.('.ps-kernel-section-title')??[])];
    body.classList.toggle('has-toc',heads.length>1);tocItems=[];
    if(heads.length<2){toc.hidden=true;return;}
    const top=article.querySelector('.ps-kernel-title,h1,h2'),still=globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    const entries=[...(top?[[article,top.textContent]]:[]),...heads.map(head=>[head.closest('section')??head,head.textContent])];
    tocItems=entries.map(([target,label])=>[target,action(label,()=>target.scrollIntoView({behavior:still?'auto':'smooth',block:'start'}),'experience-toc-link')]);
    toc.setAttribute('aria-label',t('onThisPage'));toc.replaceChildren(make('p','experience-toc-title',t('onThisPage')),...tocItems.map(([,link])=>link));toc.hidden=false;markContents();
  }
  function markContents(){
    tocFrame=0;if(!tocItems.length)return;const line=(parseFloat(getComputedStyle(root).getPropertyValue('--rb-h'))||64)+120;
    let current=tocItems[0][1];for(const [target,link] of tocItems)if(target.getBoundingClientRect().top<=line)current=link;
    if(current.getAttribute('aria-current')==='true')return;
    for(const [,link] of tocItems)link.setAttribute('aria-current',String(link===current));
    // On a narrow screen the contents are one row that scrolls; the part being read is kept in its view.
    const row=current.parentElement;if(row&&row.scrollWidth>row.clientWidth&&(current.offsetLeft<row.scrollLeft||current.offsetLeft+current.offsetWidth>row.scrollLeft+row.clientWidth))row.scrollTo({left:Math.max(0,current.offsetLeft-16),behavior:'smooth'});
  }
  const tocScroll=()=>{if(!tocFrame&&tocItems.length)tocFrame=requestAnimationFrame(markContents);};
  window.addEventListener('scroll',tocScroll,{passive:true});
  function render(){
    if(disposed)return;const token=++renderToken;
    for(const [mode,btn] of modeButtons){btn.replaceChildren(make('span','rb-mode-name',t(mode)),make('span','rb-mode-note',t(`${mode}Note`)));btn.title=t(`${mode}Note`);btn.setAttribute('aria-pressed',String(mode===state.mode));}
    modeCurrent.textContent=t(state.mode);modeCurrent.setAttribute('aria-label',`${t('mode')}: ${t(state.mode)}`);more.setAttribute('aria-label',t('more'));more.title=t('more');header.dataset.mode=state.mode;
    back.replaceChildren(make('span','rb-glyph','←'),make('span','rb-label',t('back')));forward.replaceChildren(make('span','rb-label',t('forward')),make('span','rb-glyph','→'));back.title=t('back');forward.title=t('forward');back.disabled=state.cursor<=0;forward.disabled=state.cursor>=state.stack.length-1;
    pin.textContent=t(state.pins.some(item=>item.id===state.id&&item.revision===state.revision)?'unpin':'pin');pin.disabled=!state.id;copy.textContent=t('copy');calculation.textContent=t('operations');calculation.setAttribute('aria-pressed',String(opsVisible));
    toolbar.setAttribute('aria-label',t('researchModes'));
    versionNotice.hidden=!(state.mode==='original'&&state.id&&state.revision!==index.revision);versionNotice.textContent=resolveLocale(locale())==='ru'?'Карта показывает текущую версию. Сохранённая версия предмета доступна в справочном режиме.':localeText('The map shows the current version. The retained item version is available in reference mode.',locale());
    space.root.classList.toggle('experience-space-inactive',state.mode!=='original');pane.hidden=state.mode==='original'&&!opsVisible;question.hidden=state.mode!=='reference';pane.classList.toggle('is-cognitive',state.mode==='cognitive');operationDetails.hidden=!opsVisible;body.hidden=state.mode!=='reference';comparison.hidden=state.mode!=='reference';searchWrap.hidden=state.mode!=='reference';history.hidden=state.mode!=='reference';cognitiveRoot.hidden=state.mode!=='cognitive';
    question.textContent=api?.getQuestion?.()?`${t('question')}: ${api.getQuestion()}`:'';search.placeholder=t('search');search.setAttribute('aria-label',t('search'));operationTitle.textContent=t('operations');historyTitle.textContent=t('history');recover.textContent=t('recover');
    body.replaceChildren();comparison.replaceChildren();historyBody.replaceChildren();renderSearch();
    if(opsVisible)void renderOperations();
    if(state.mode==='reference'){
      if(state.id){body.append(action(`← ${t('toContents')}`,()=>{rememberScroll();state.id=null;writeUrl(true);render();save();window.scrollTo(0,0);},'experience-back'));const toc=make('nav','experience-toc'),article=make('article','experience-article');toc.hidden=true;body.append(toc,article);void renderItem(article,state.id,state.revision).then(()=>{if(token!==renderToken)return;if(state.revision===index.revision)renderRefs(article,state.id);contentsOf(toc,article);space.markTerms?.(article);});}else if(!renderContents(body))body.append(make('p','experience-empty',t('empty')));
      if(state.pins.length){comparison.append(make('h2','',state.pins.length===2?t('compare'):t('pins')));const grid=make('div','experience-pin-grid');for(const entry of state.pins){const card=make('article','experience-pin');card.append(action(t('remove'),()=>{state.pins=state.pins.filter(item=>item!==entry);render();save();}));grid.append(card);void renderItem(card,entry.id,entry.revision);}comparison.append(grid);}
      if(!state.stack.length)historyBody.append(make('p','',t('noHistory')));for(const [i,entry] of state.stack.entries()){const item=index.get(entry.id),btn=action(`${title(item)||entry.id} · ${entry.revision===index.revision?t('current'):entry.revision.slice(0,12)}`,()=>{rememberScroll();state.cursor=i;open(entry.id,{revision:entry.revision,record:false,remember:false});requestAnimationFrame(()=>{if(state.mode==='reference')window.scrollTo(0,entry.scroll??0);});},'experience-result');if(i===state.cursor)btn.setAttribute('aria-current','true');historyBody.append(btn);}
    }
  }
  search.addEventListener('input',()=>{searchText=search.value;renderSearch();});
  operationDetails.addEventListener('toggle',()=>{if(!operationDetails.open&&opsVisible){opsVisible=false;render();}if(operationDetails.open&&opsVisible)void renderOperations();});
  function handleAction(detail){
    const {action:kind,recordId,cite}=detail,revision=detail.revision??index.revision;
    if(kind==='read'){
      if(cite?.w==='also-sprach-zarathustra'&&cite.c)void space.fragments.showChapter(cite.c,cite).catch(()=>{message.textContent=t('unavailable');});
      else if(cite)void versionContext(revision).then(context=>{const id=citationId(cite);if(context.index.get(id)){open(id,{revision});setMode('reference');}else message.textContent=t('missing');}).catch(()=>{message.textContent=t('unavailable');});
      else message.textContent=t('missing');
    }else if(kind==='compare'){if(recordId){open(recordId,{revision});if(!state.pins.some(item=>item.id===state.id&&item.revision===state.revision))togglePin();setMode('reference');}}
    else if(kind==='links'){if(recordId){open(recordId,{revision});setMode('reference');}}
    else if(kind==='continue'){setMode('original');space.openPersonal?.();}
    onCognitive({action:kind,id:recordId,revision,cite});
  }
  const selectionEvent=event=>{const selection=event.detail;if(suppressSelection||!selection?.id)return;open(selection.id,{show:false});};
  const openEvent=event=>{if(event.detail?.id)open(event.detail.id,{revision:event.detail.revision??index.revision});};
  const actionEvent=event=>handleAction(event.detail??{});
  const popEvent=()=>{const params=new URL(location.href).searchParams;state.mode=validMode(params.get('experience'));state.id=params.get('item');state.revision=params.get('revision')||index.revision;const at=state.stack.findLastIndex(entry=>entry.id===state.id&&entry.revision===state.revision);if(at>=0)state.cursor=at;if(state.id&&state.mode==='original'&&state.revision===index.revision){suppressSelection=true;try{space.openResearch?.(state.id);}finally{suppressSelection=false;}}render();save();};
  root.addEventListener('public-research-selection',selectionEvent);root.addEventListener('public-research-open',openEvent);root.addEventListener('public-research-action',actionEvent);window.addEventListener('popstate',popEvent);
  let scrollTimer;const scrollEvent=()=>{if(state.mode!=='reference'||state.cursor<0)return;clearTimeout(scrollTimer);scrollTimer=setTimeout(()=>{rememberScroll();save();},350);};window.addEventListener('scroll',scrollEvent,{passive:true});
  const unsubscribe=api?.subscribeQuestion?.(()=>{const value=api.getQuestion();if(value===state.question)return;state.question=value;render();save();});
  render();
  return {getState:()=>clone(snapshot()),versionContext,peekVersionContext,setMode,open,cognitiveRoot,refreshLocale(){render();ops?.refreshLocale();},destroy(){disposed=true;unsubscribe?.();ops?.destroy();root.removeEventListener('public-research-selection',selectionEvent);root.removeEventListener('public-research-open',openEvent);root.removeEventListener('public-research-action',actionEvent);window.removeEventListener('popstate',popEvent);window.removeEventListener('scroll',scrollEvent);window.removeEventListener('scroll',tocScroll);globalThis.cancelAnimationFrame?.(tocFrame);clearTimeout(scrollTimer);space.root.classList.remove('experience-space-inactive');document.removeEventListener?.('pointerdown',outside,true);document.removeEventListener?.('keydown',escape,true);clearTimeout(copiedTimer);toolbar.remove();tools.remove();more.remove();versionNotice.remove();pane.remove();}};
}
