// The trail of the stand's work: where it is open (the frontier), how a subject came to be what it is (its
// history), and what grew in the stand's core between a retained version and now. These are read-only views over
// the stand's own data, its retained versions and the person's act journal; a ticket filed from here is the
// person's own act, as anywhere else. Nothing here changes the stand, its bonsai or the Tree.
import './act-desk.css';
import './research-trail.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {canonicalJSON} from './research-index.mjs';
import witnessManifest from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};

export const SEEN_REVISION_KEY='tos.public-research.seen-revision.v1';
const WORK='also-sprach-zarathustra';
// Former states and first proposals keep the places they once cited; the frontier reads the map as it stands.
const PAST=/revision_history$|^proposed$|^statement_ids$/;
const REVISION=/^[a-f0-9]{64}$/;
export const GROWTH_COLLECTIONS=Object.freeze(['concepts','figures','links','proposed','chains','placeRelations']);

const labels={
  frontier:['Фронтир','Frontier'],
  frontierLead:['Где работа стенда открыта: главы, на которые не опирается ни одна связь мысли, связи на реконструкции, оставленные споры, отвергнутое и предложенное стендом.','Where the stand\'s work is open: chapters no link of thought rests on, links resting on a reconstruction, disputes left open, what review rejected and what the stand proposed.'],
  chapters:['Главы без связей мысли','Chapters without links of thought'],
  reconstructions:['Связи на реконструкции','Links resting on a reconstruction'],
  disputed:['Открытый спор','Disputes left open'],
  rejected:['Отвергнуто проверкой','Rejected by review'],
  proposed:['Предложено стендом','Proposed by the stand'],
  read:['Читать','Read'],
  open:['Открыть','Open'],
  ticket:['Тикет','Ticket'],
  yourTicket:['ваш тикет','your ticket'],
  history:['История предмета','History of the subject'],
  growth:['Что выросло','What grew'],
  growthSince:['Что выросло с прошлого визита','What grew since the last visit'],
  growthNotice:['Ядро стенда изменилось с вашего прошлого визита','The stand\'s core changed since your last visit'],
  compareWith:['Сравнить с версией','Compare with a version'],
  current:['текущая','current'],
  lastVisit:['прошлый визит','last visit'],
  noChange:['Без изменений','No changes'],
  added:['добавлено','added'],
  removed:['убрано','removed'],
  changed:['изменено','changed'],
  asItWas:['Как было','As it was'],
  yourActs:['Ваши действия','Your acts'],
  noActs:['Ваших действий здесь пока нет','You have no acts here yet'],
  made:['Сделано','Made by'],
  firstProposal:['Первое предложение','First proposal'],
  before:['Прежде','Before'],
  review:['Проверка','Review'],
  counter:['Контрпрочтение','Counter-reading'],
  admission:['Допуск','Admission'],
  onMap:['на карте','on the map'],
  touched:['касаются ваших тикетов','touch your tickets'],
  loading:['Загружаю версию…','Loading the version…'],
  unavailable:['Версия недоступна','The version is unavailable'],
  then:['тогда','then'],
  now:['теперь','now'],
  more:['ещё','more'],
  close:['Закрыть','Close'],
};
const statusLabels={open:['Открыт','Open'],'with-agent':['У агента','With an agent'],returned:['Вернулся','Returned'],reviewed:['Рассмотрен','Reviewed']};
const collectionLabels={concepts:['Понятия','Concepts'],figures:['Фигуры','Figures'],links:['Связи','Links'],proposed:['Предложения стенда','Stand proposals'],chains:['Цепи','Chains'],placeRelations:['Связи мест','Links of places']};
const fieldLabels={
  sense:['смысл','sense'],verb:['глагол','verb'],answer:['ответ','answer'],question:['вопрос','question'],assessment:['оценка','assessment'],
  grounds:['основания','grounds'],relation:['отношение','relation'],relation_family:['семейство связи','relation family'],title:['название','title'],
  conception:['замысел','conception'],development:['развитие','development'],essence:['суть','essence'],appearances:['появления','appearances'],
  questions:['вопросы','questions'],beyond:['за пределами книги','beyond the book'],lexicon:['словарь','lexicon'],dimensions:['измерения','dimensions'],
  reading:['прочтение','reading'],steps:['шаги','steps'],states:['состояния','states'],statement_ids:['адреса утверждений','statement addresses'],proposed:['предложения','proposals'],
  counter_voices:['встречные голоса','counter-voices'],voice:['голос','voice'],label:['подпись','label'],limit:['предел прочтения','limit of the reading'],
  independent_review:['независимая проверка','independent review'],relation_note:['пояснение к отношению','note on the relation'],sense_revision:['проверка смысла','review of the sense'],
  verb_revision_history:['прежние глаголы','former verbs'],sense_revision_history:['прежние смыслы','former senses'],title_revision_history:['прежние названия','former titles'],
  reading_revision_history:['прежние прочтения','former readings'],retired_steps:['снятые шаги','retired steps'],status:['статус','status'],admission:['допуск','admission'],
  card:['карточка','card'],maker:['создатель','maker'],uses:['употребления','uses'],occurrences:['вхождения','occurrences'],type:['тип','type'],wave:['волна','wave'],
};
const verdictLabels={confirmed:['подтверждено','confirmed'],revise:['исправлено по проверке','revised after review'],reconstruction:['реконструкция','reconstruction'],rejected:['отвергнуто','rejected']};
const kindLabels={ticket:['тикет','ticket'],proposal:['предложение','proposal'],review:['решение','decision'],note:['заметка','note'],reply:['ответ','reply'],assignment:['поручение','assignment'],transfer:['передача','handover'],withdrawal:['отзыв','withdrawal'],lease:['аренда места','lease'],release:['освобождение места','release'],'change-packet':['пакет изменений','change packet']};

const list=value=>Array.isArray(value)?value:[];
const placeRelations=core=>list(core?.placeRelations?.relations);
const collection=(core,key)=>key==='placeRelations'?placeRelations(core):list(core?.[key]);

/** The book's chapters each record cites, read from the record as it stands now. */
function chaptersCited(value,out=new Set(),depth=0){
  if(depth>10||!value||typeof value!=='object')return out;
  if(Array.isArray(value)){for(const item of value)chaptersCited(item,out,depth+1);return out;}
  if(value.w===WORK&&typeof value.c==='string'){out.add(value.c);return out;}
  for(const [key,child] of Object.entries(value))if(!PAST.test(key))chaptersCited(child,out,depth+1);
  return out;
}
const firstChapter=record=>[...chaptersCited(record?.grounds??record)][0]??null;

/**
 * Where the stand's work is open. Each link stands in one group at most: rejected by review, resting on a
 * reconstruction, or a reading the review revised while keeping a counter-reading. Chapters count as open when no
 * link or chain step of thought rests on them, whatever figures and voices the map knows there.
 */
export function frontierOf({core,chapters=witnessManifest.chapters}={}){
  if(!core)return null;
  const thought=new Set();
  for(const link of list(core.links))chaptersCited(link,thought);
  for(const chain of list(core.chains))chaptersCited(chain.steps,thought);
  const groups={chapters:[],reconstructions:[],disputed:[],rejected:[],proposed:[]};
  for(const chapter of list(chapters))if(!thought.has(chapter.key))groups.chapters.push({id:chapter.key,chapter:chapter.key,titles:chapter.titles});
  for(const link of list(core.links)){
    const assessment=link.assessment??{},entry={id:link.id,chapter:firstChapter(link),record:link,reason:assessment.reason??null};
    if(link.admission==='rejected'||assessment.verdict==='rejected')groups.rejected.push(entry);
    else if(link.status==='reconstruction'||link.status==='hypothesis')groups.reconstructions.push(entry);
    else if(assessment.counter_reading&&assessment.verdict!=='confirmed')groups.disputed.push(entry);
  }
  for(const node of list(core.proposed))groups.proposed.push({id:node.id,chapter:firstChapter(node.reading),record:node,reason:null});
  const order=value=>{const [part,chapter]=String(value??'Z.99').split('.');return [part.length,part,Number(chapter)||0];};
  const byPlace=(a,b)=>{const x=order(a.chapter),y=order(b.chapter);return x[0]-y[0]||x[1].localeCompare(y[1])||x[2]-y[2]||String(a.id).localeCompare(String(b.id));};
  for(const rows of Object.values(groups))rows.sort(byPlace);
  return groups;
}

/** What changed between two versions of the stand's core: per collection, what was added, removed and changed, by field. */
export function growthBetween(before,after){
  const out={collections:[],total:0,changedIds:new Set()};
  for(const key of GROWTH_COLLECTIONS){
    const old=new Map(collection(before,key).filter(item=>item?.id).map(item=>[item.id,item]));
    const now=new Map(collection(after,key).filter(item=>item?.id).map(item=>[item.id,item]));
    const added=[...now.keys()].filter(id=>!old.has(id)),removed=[...old.keys()].filter(id=>!now.has(id)),changed=[],fields={};
    for(const [id,item] of now){
      const prior=old.get(id);if(!prior)continue;
      const names=[...new Set([...Object.keys(prior),...Object.keys(item)])].filter(name=>canonicalJSON(prior[name]??null)!==canonicalJSON(item[name]??null)).sort();
      if(!names.length)continue;
      changed.push({id,fields:names});for(const name of names)fields[name]=(fields[name]??0)+1;
    }
    changed.sort((a,b)=>b.fields.length-a.fields.length||a.id.localeCompare(b.id));
    for(const id of [...added,...changed.map(row=>row.id)])out.changedIds.add(id);
    const size=added.length+removed.length+changed.length;out.total+=size;
    out.collections.push({key,added,removed,changed,fields:Object.entries(fields).sort((a,b)=>b[1]-a[1]).map(([name,n])=>({name,n}))});
  }
  return out;
}

/**
 * The stages a subject went through, as its record keeps them: who made it, its first proposal, each former
 * wording with its reviewer and reason, the review and its counter-reading, an independent review and admission.
 */
export function subjectStages(record){
  if(!record||typeof record!=='object')return [];
  const stages=[];
  const maker=record.maker??(list(record.proposed_by).join(', ')||null);
  if(maker)stages.push({kind:'made',who:maker});
  if(record.proposed&&!Array.isArray(record.proposed))stages.push({kind:'first',text:record.proposed.verb??record.proposed.sense??null,also:record.proposed.verb?record.proposed.sense??null:null});
  for(const [key,value] of Object.entries(record)){
    if(!/revision_history$/.test(key))continue;
    for(const entry of list(value)){
      const reviewer=entry.revision?.reviewer??entry.assessment?.reviewer??null;
      stages.push({kind:'before',field:key.replace(/_revision_history$/,''),text:entry.verb??entry.sense??entry.reading??(entry.ru||entry.en?entry:null),
        who:typeof reviewer==='string'?reviewer:reviewer?.id??null,model:typeof reviewer==='object'?reviewer?.model??null:null,date:typeof reviewer==='object'?reviewer?.date??null:null,
        reason:entry.revision?.reason??entry.assessment?.reason??null,verdict:entry.assessment?.verdict??null});
    }
  }
  if(record.sense_revision)stages.push({kind:'review',who:record.sense_revision.reviewer?.id??null,model:record.sense_revision.reviewer?.model??null,date:record.sense_revision.reviewer?.date??null,reason:record.sense_revision.reason??null,verdict:null});
  const assessment=record.assessment;
  if(assessment)stages.push({kind:'review',who:assessment.reviewer??null,verdict:assessment.verdict??null,reason:assessment.reason??null,counter:assessment.counter_reading??null});
  const independent=record.reading?.independent_review??record.independent_review;
  if(independent)stages.push({kind:'review',who:independent.reviewer??null,model:independent.model??null,date:independent.date??null,verdict:independent.verdict??null,reason:independent.reason??null});
  if(record.admission)stages.push({kind:'admission',state:record.admission});
  return stages;
}

/** The revision this visit sees and the different one the previous visit saw, kept in this browser. */
export function rememberRevision(storage,profile,revision){
  if(!REVISION.test(revision??''))return {current:revision??null,previous:null};
  let stored={};try{stored=JSON.parse(storage?.getItem(SEEN_REVISION_KEY)??'{}')??{};}catch{}
  const own=stored[profile]&&typeof stored[profile]==='object'?stored[profile]:{};
  const previous=own.current&&own.current!==revision?own.current:REVISION.test(own.previous??'')?own.previous:null;
  const next={...stored,[profile]:{current:revision,...(previous?{previous}:{})}};
  try{storage?.setItem(SEEN_REVISION_KEY,JSON.stringify(next));}catch{}
  return {current:revision,previous:previous&&REVISION.test(previous)?previous:null};
}

export function mountResearchTrail({shell,actionLayer,space,acts,actDesk=null,experience=null,registry=null,locale=()=> 'ru',profile='zarathustra',storage=globalThis.localStorage,versions=null}){
  const core=space?.actions?.core?.()??null;
  if(!core)return {frontier:()=>null,openFrontier:async()=>{},openHistory:async()=>{},openGrowth:async()=>{},destroy(){}};
  const ru=()=>resolveLocale(locale())==='ru';
  const pick=pair=>ru()?pair[0]:localeText(pair[1],locale());
  const t=key=>labels[key]?pick(labels[key]):key;
  const doc=shell.ownerDocument;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const dialogs=[];
  function sheet(title){
    const dialog=el('dialog','ad-sheet rt-sheet'),head=el('div','ad-head'),heading=el('h2','',title),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});
    dialog.addEventListener('close',()=>dialog.remove());
    head.append(heading,close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);dialog.showModal?.();
    return {dialog,body};
  }
  const index=()=>space?.index;
  const form=value=>value==null?'':typeof value==='string'?value:localeValue(value,locale())||value.ru||value.en||'';
  const nameOf=id=>form(index()?.get?.(id)?.title)||id;
  const sentenceOf=record=>record?.from&&record?.to?`${nameOf(record.from)} — ${form(record.verb)} — ${nameOf(record.to)}`:form(record?.title)||record?.id||'';
  const chapterTitle=chapter=>{const titles=witnessManifest.chapters.find(item=>item.key===chapter)?.titles??{};const language=resolveLocale(locale());return titles[language]??titles.en??chapter;};
  const verdict=value=>verdictLabels[value]?pick(verdictLabels[value]):value;
  const field=name=>fieldLabels[name]?pick(fieldLabels[name]):name;

  // The person's own tickets mark the places they already work on.
  async function ticketMarks(){
    const marks=new Map();
    try{const board=await acts.query({view:'board',limit:1000});for(const ticket of board.tickets)for(const target of ticket.targets??[])if(ticket.status!=='withdrawn'&&!marks.has(target))marks.set(target,ticket.status);}catch{}
    return marks;
  }
  const chapterAddress=chapter=>`place:${profile}:de:${chapter}`;
  const markOf=(marks,entry,group)=>marks.get(group==='chapters'?chapterAddress(entry.chapter):entry.id)??null;
  const targetOf=(entry,group)=>group==='chapters'
    ?{type:'passage',id:null,label:`${entry.chapter} · ${chapterTitle(entry.chapter)}`,stand:{language:'de',chapter:entry.chapter}}
    :{type:group==='proposed'?'star':'link',id:entry.id,label:sentenceOf(entry.record),item:index()?.get?.(entry.id)??null};
  const preset={chapters:{category:'reading'},reconstructions:{category:'relation'},disputed:{category:'reading'},rejected:{category:'relation'},proposed:{category:'reading'}};
  const select=id=>registry?registry.invoke('tos.page.select',{item_id:id}):experience?.open?.(id);

  // ------------------------------------------------------------ the frontier (A3)
  let groups=null;
  const frontier=()=>groups??=frontierOf({core});
  async function openFrontier(only=null){
    const {dialog,body}=sheet(t('frontier'));
    const marks=await ticketMarks();
    body.append(el('p','ad-muted',t('frontierLead')));
    const tabs=el('div','ad-row rt-tabs'),rows=el('ul','rt-list');
    let current=only??Object.entries(frontier()).find(([,items])=>items.length)?.[0]??'chapters';
    function paint(){
      tabs.replaceChildren();
      for(const [group,items] of Object.entries(frontier())){
        if(!items.length)continue;
        const tab=button(`${t(group)} · ${items.length}`,'text-button ad-tab');tab.dataset.group=group;tab.setAttribute('aria-pressed',String(group===current));
        tab.addEventListener('click',()=>{current=group;paint();});tabs.append(tab);
      }
      rows.replaceChildren();
      for(const entry of frontier()[current]??[]){
        const row=el('li','rt-entry'),head=el('div','rt-entry-head'),mark=markOf(marks,entry,current);
        head.append(el('span','rt-place',entry.chapter??''),el('span','rt-title',current==='chapters'?chapterTitle(entry.chapter):sentenceOf(entry.record)));
        if(mark){const chip=el('span','rt-mark',`${t('yourTicket')} · ${statusLabels[mark]?pick(statusLabels[mark]):mark}`);chip.dataset.status=mark;head.append(chip);}
        row.append(head);
        if(entry.reason)row.append(el('p','ad-muted rt-reason',form(entry.reason)));
        const actions=el('div','ad-row');
        if(current==='chapters'){const read=button(t('read'),'text-button');read.addEventListener('click',()=>{dialog.close();void space?.actions?.showChapter?.(entry.chapter);});actions.append(read);}
        else{const open=button(t('open'),'text-button');open.addEventListener('click',()=>{dialog.close();void select(entry.id);});actions.append(open);
          const history=button(t('history'),'text-button');history.addEventListener('click',()=>{dialog.close();void openHistory(targetOf(entry,current));});actions.append(history);}
        if(actDesk){const ticket=button(t('ticket'),'text-button');ticket.addEventListener('click',()=>{dialog.close();void actDesk.openTicket(targetOf(entry,current),preset[current]);});actions.append(ticket);}
        row.append(actions);rows.append(row);
      }
    }
    body.append(tabs,rows);paint();
  }
  // The measure panel shows the frontier as one block of rows; each row opens its places.
  const removeMeasure=space?.actions?.addMeasureBlock?.((panel,{bars})=>{
    const found=frontier();if(!found)return;
    bars(panel,t('frontier'),Object.entries(found).filter(([,items])=>items.length).map(([group,items])=>({label:t(group),n:items.length,detail:t('frontierLead'),action:()=>void openFrontier(group)})));
  });

  // ------------------------------------------------------------ history of a subject (A4)
  const available=()=>{const listed=versions?.list?.()??[];return listed.filter(revision=>REVISION.test(revision));};
  const seen=rememberRevision(storage,profile,index()?.revision);
  const versionLabel=revision=>`${revision.slice(0,12)}${revision===seen.current?` · ${t('current')}`:''}${revision===seen.previous?` · ${t('lastVisit')}`:''}`;
  function versionPicker(onPick,{exclude=seen.current}={}){
    const choose=el('select','ad-input rt-version'),blank=el('option','','—');blank.value='';choose.append(blank);
    const listed=available().filter(revision=>revision!==exclude).sort((a,b)=>(b===seen.previous)-(a===seen.previous)||a.localeCompare(b));
    for(const revision of listed){const option=el('option','',versionLabel(revision));option.value=revision;choose.append(option);}
    choose.addEventListener('change',()=>{if(choose.value)void onPick(choose.value);});
    choose.disabled=!listed.length;return choose;
  }
  const loaded=new Map();
  async function versionCore(revision){
    if(!loaded.has(revision))loaded.set(revision,Promise.resolve(versions?.load?.(revision)).then(packet=>packet?.status==='available'?packet.core:null).catch(()=>null));
    return loaded.get(revision);
  }
  function recordOf(target){
    const item=target.item??index()?.get?.(target.id);
    return item?.record??item??null;
  }
  async function openHistory(target){
    const record=recordOf(target);
    const {body}=sheet(`${t('history')} · ${record?.from&&record?.to?sentenceOf(record):target.label||nameOf(target.id)}`);
    const stages=subjectStages(record),timeline=el('ol','rt-stages');
    for(const stage of stages){
      const row=el('li','rt-stage');row.dataset.kind=stage.kind;
      const who=[stage.who,stage.model,stage.date].filter(Boolean).join(' · ');
      if(stage.kind==='made')row.append(el('strong','',t('made')),el('span','',stage.who));
      else if(stage.kind==='first'){row.append(el('strong','',t('firstProposal')),el('span','',form(stage.text)));if(stage.also)row.append(el('span','ad-muted',form(stage.also)));}
      else if(stage.kind==='before'){row.append(el('strong','',`${t('before')} · ${field(stage.field)}`),el('span','',form(stage.text)));if(who)row.append(el('span','ad-muted',who));if(stage.reason)row.append(el('span','ad-muted',form(stage.reason)));}
      else if(stage.kind==='review'){row.append(el('strong','',`${t('review')}${stage.verdict?` · ${verdict(stage.verdict)}`:''}`));if(who)row.append(el('span','ad-muted',who));if(stage.reason)row.append(el('span','',form(stage.reason)));if(stage.counter)row.append(el('span','rt-counter',`${t('counter')}: ${form(stage.counter)}`));}
      else if(stage.kind==='admission')row.append(el('strong','',t('admission')),el('span','',stage.state==='map'?t('onMap'):verdict(stage.state)));
      timeline.append(row);
    }
    body.append(timeline);
    // The person's own acts on this subject, newest first.
    body.append(el('h3','ad-minor',t('yourActs')));
    const own=el('ul','rt-acts');body.append(own);
    try{
      const found=target.id?await acts.query({view:'target',address:target.id,limit:100}):{acts:[]};
      if(!found.acts.length)own.append(el('li','ad-muted',t('noActs')));
      const records=await Promise.all(found.acts.map(act=>acts.record(act.id).catch(()=>null)));
      const language=resolveLocale(locale())==='zh-Hans'?'zh-CN':resolveLocale(locale());
      for(const [i,act] of found.acts.entries()){
        const row=el('li');if(act.withdrawn)row.dataset.withdrawn='true';
        row.append(el('span','rt-place',new Date(act.at_ms).toLocaleString(language)),el('span','',`${kindLabels[act.kind]?pick(kindLabels[act.kind]):act.kind} · ${records[i]?.intent??''}`));own.append(row);
      }
    }catch{own.append(el('li','ad-muted',t('noActs')));}
    // Any retained version can be set beside this one: what changed in this subject, and the subject as it was.
    if(target.id&&available().length){
      const result=el('div','rt-compare');
      const picker=versionPicker(async revision=>{
        result.replaceChildren(el('p','ad-muted',t('loading')));
        const old=await versionCore(revision);if(!old){result.replaceChildren(el('p','ad-muted',t('unavailable')));return;}
        const prior=GROWTH_COLLECTIONS.flatMap(key=>collection(old,key)).find(item=>item?.id===target.id);
        const names=prior?[...new Set([...Object.keys(prior),...Object.keys(record??{})])].filter(name=>canonicalJSON(prior[name]??null)!==canonicalJSON(record?.[name]??null)):null;
        result.replaceChildren();
        if(!prior){result.append(el('p','ad-muted',t('added')));return;}
        if(!names.length){result.append(el('p','ad-muted',t('noChange')));return;}
        for(const name of names.slice(0,12)){
          const box=el('section','rt-diff');box.append(el('strong','',field(name)));
          const words=value=>typeof value==='string'||value?.ru||value?.en?form(value):null;
          const then=words(prior[name]),now=words(record?.[name]);
          if(then!==null||now!==null){box.append(el('p','rt-then',`${t('then')}: ${then??'—'}`),el('p','rt-now',`${t('now')}: ${now??'—'}`));}
          else box.append(el('p','rt-then',t('changed')));
          result.append(box);
        }
        if(experience?.open){const asItWas=button(t('asItWas'),'text-button');asItWas.addEventListener('click',()=>{experience.open(target.id,{revision});experience.setMode?.('reference');});result.append(asItWas);}
      });
      body.append(el('h3','ad-minor',t('compareWith')),picker,result);
    }
  }

  // ------------------------------------------------------------ what grew (A4)
  async function openGrowth(revision=seen.previous){
    const {dialog,body}=sheet(revision&&revision===seen.previous?t('growthSince'):t('growth'));
    const result=el('div','rt-growth');
    const picker=versionPicker(value=>show(value));if(revision)picker.value=revision;
    body.append(field2(t('compareWith'),picker),result);
    async function show(value){
      result.replaceChildren(el('p','ad-muted',t('loading')));
      const old=await versionCore(value);if(!old){result.replaceChildren(el('p','ad-muted',t('unavailable')));return;}
      const growth=growthBetween(old,core),marks=await ticketMarks();
      result.replaceChildren();
      if(!growth.total){result.append(el('p','ad-muted',t('noChange')));return;}
      const touched=[...growth.changedIds].filter(id=>marks.has(id));
      if(touched.length)result.append(el('p','rt-touched',`${touched.length} ${t('touched')}`));
      for(const row of growth.collections){
        const size=row.added.length+row.removed.length+row.changed.length;if(!size)continue;
        const box=el('section','rt-collection');
        box.append(el('h3','ad-minor',`${pick(collectionLabels[row.key])} · ${[row.added.length?`${t('added')} ${row.added.length}`:'',row.removed.length?`${t('removed')} ${row.removed.length}`:'',row.changed.length?`${t('changed')} ${row.changed.length}`:''].filter(Boolean).join(' · ')}`));
        if(row.fields.length)box.append(el('p','ad-muted',row.fields.slice(0,8).map(item=>`${field(item.name)} ${item.n}`).join(' · ')));
        const items=el('ul','rt-list');
        const entries=[...row.added.map(id=>({id,fields:null})),...row.changed].sort((a,b)=>Number(marks.has(b.id))-Number(marks.has(a.id)));
        const shown=entries.slice(0,24);
        for(const entry of shown){
          const item=el('li','rt-entry'),head=el('div','rt-entry-head');
          const own=collection(core,row.key).find(value=>value?.id===entry.id);
          head.append(el('span','rt-title',sentenceOf(own)||nameOf(entry.id)));
          if(marks.has(entry.id)){const chip=el('span','rt-mark',t('yourTicket'));chip.dataset.status=marks.get(entry.id);head.append(chip);}
          item.append(head,el('p','ad-muted',entry.fields?entry.fields.map(field).join(' · '):t('added')));
          const actions=el('div','ad-row');
          const open=button(t('open'),'text-button');open.addEventListener('click',()=>{dialog.close();void select(entry.id);});actions.append(open);
          const history=button(t('history'),'text-button');history.addEventListener('click',()=>{dialog.close();void openHistory({type:row.key==='links'?'link':'star',id:entry.id,label:sentenceOf(own)});});actions.append(history);
          if(entry.fields&&experience?.open){const asItWas=button(t('asItWas'),'text-button');asItWas.addEventListener('click',()=>{dialog.close();experience.open(entry.id,{revision:value});experience.setMode?.('reference');});actions.append(asItWas);}
          item.append(actions);items.append(item);
        }
        if(entries.length>shown.length)items.append(el('li','ad-muted',`${t('more')} ${entries.length-shown.length}`));
        box.append(items);result.append(box);
      }
    }
    if(revision)await show(revision);
  }
  function field2(label,control){const wrap=el('label','ad-field');wrap.append(el('span','ad-label',label),control);return wrap;}

  const removeActions=actionLayer?.addActions?.([
    {id:'frontier',text:()=>t('frontier'),for:['any'],run:()=>openFrontier()},
    {id:'history',text:()=>t('history'),for:['star','link'],run:target=>openHistory(target)},
    {id:'growth',text:()=>seen.previous?t('growthSince'):t('growth'),for:['any'],when:()=>available().length>0,run:()=>openGrowth()},
  ]);
  if(seen.previous&&available().includes(seen.previous))actionLayer?.say?.(t('growthNotice'));

  return {frontier,openFrontier,openHistory,openGrowth,seen:()=>({...seen}),
    destroy(){removeActions?.();removeMeasure?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
