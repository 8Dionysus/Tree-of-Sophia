// The act desk: tickets on any subject or passage with checks on the device, the board of tickets, the review of
// returned proposals in groups, the change packet of what was accepted, handing anything over in the right
// format, and connecting an agent with its tier. Everything it writes is an act of the person's own journal;
// none of it changes the stand, its bonsai or the Tree. Accepted changes leave as a packet for the owner's route.
import './act-desk.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {citationId,citedPlaces} from './research-index.mjs';
import {referenceKey} from '../corpus-reader/model.mjs';
import {checkTicket,checksPass} from './ticket-checks.mjs';
import roles from '../../../contracts/agent-roles.v1.json' with {type:'json'};

export const AGENT_TIER_KEY='tos.public-research.agent-tier.v1';
const TIERS=['read','draft','propose'];
// The language a dictation is heard in, by interface language.
const SPEECH_LANGUAGES=new Map([['ru','ru-RU'],['en','en-US'],['es','es-ES'],['de','de-DE'],['zh-Hans','zh-CN']]);

const labels={
  ticket:['Завести тикет','File a ticket'],
  board:['Доска тикетов','Board of tickets'],
  review:['Рассмотрение','Review'],
  transfer:['Передать…','Hand over…'],
  connect:['Подключить агента','Connect an agent'],
  close:['Закрыть','Close'],
  category:['Категория','Category'],
  variant:['Вид','Kind'],
  observation:['Наблюдение','Observation'],
  observationHint:['Что вы заметили, своими словами','What you noticed, in your own words'],
  dictate:['Диктовать','Dictate'],
  dictating:['Слушаю…','Listening…'],
  noVoice:['Голосовой ввод здесь недоступен','Voice input is unavailable here'],
  propose:['Предложить исправление','Propose a correction'],
  from:['Было','Was'],
  to:['Стало','Becomes'],
  statement:['Предложение','Proposal'],
  pair:['Пара мест','Pair of places'],
  term:['Термин','Term'],
  noTerm:['Без термина','No term'],
  treeWord:['Слово Древа','The Tree\'s word'],
  field:['Где','Where'],
  checks:['Проверка на устройстве','Check on the device'],
  save:['Сохранить','Save'],
  saved:['Тикет сохранён','Ticket saved'],
  savedProposal:['Тикет и предложение сохранены','Ticket and proposal saved'],
  open:['Открыть','Open'],
  assign:['Поручить агенту','Assign to an agent'],
  assignee:['Кому','To whom'],
  withdraw:['Отозвать','Withdraw'],
  empty:['Пусто','Empty'],
  'status-open':['Открыт','Open'],
  'status-with-agent':['У агента','With an agent'],
  'status-returned':['Вернулся','Returned'],
  'status-reviewed':['Рассмотрен','Reviewed'],
  replies:['ответов','replies'],
  proposals:['предложений','proposals'],
  places:['мест','places'],
  independent:['независимых','independent'],
  reason:['Причина','Reason'],
  reasonHint:['Почему, у какого места','Why, at which place'],
  edit:['Правка','Edit'],
  accept:['Принять','Accept'],
  'accept-with-edit':['Принять с правкой','Accept with an edit'],
  reject:['Отклонить','Reject'],
  defer:['Отложить','Defer'],
  decided:['Решено','Decided'],
  nothingOpen:['Открытых предложений нет','No open proposals'],
  accepted:['Принято и ещё не в пакете','Accepted and not yet in a packet'],
  packet:['Собрать пакет правок','Gather a change packet'],
  packetLead:['Пакет ничего не меняет сам. Его применяет агент в репозитории маршрутом владельца: входы сборки, terms.json, проверка бонсая; свидетели остаются как есть.','The packet changes nothing by itself. An agent in the repository applies it through the owner\'s route: assembly inputs, terms.json, the bonsai check; the witnesses stay as they are.'],
  copyJson:['Копировать JSON','Copy JSON'],
  copyMarkdown:['Копировать Markdown','Copy Markdown'],
  copy:['Копировать','Copy'],
  formatMarkdown:['Markdown с точными ссылками','Markdown with exact links'],
  formatQuote:['Цитата с местом','Quotation with its place'],
  formatJson:['JSON для анализа','JSON for analysis'],
  formatAgent:['Задание агенту','Task for an agent'],
  tier:['Что агенту можно','What the agent may do'],
  'tier-read':['Читать','Read'],
  'tier-draft':['Готовить черновики','Prepare drafts'],
  'tier-propose':['Ставить предложения','Make proposals'],
  instruction:['Инструкция для агента','Instruction for the agent'],
  connectLead:['Агент подключается к этой странице через WebMCP или к родному tos mcp. Скопируйте инструкцию в задачу агента; роли и правила одни для любой модели.','The agent connects to this page through WebMCP or to the native tos mcp. Copy the instruction into the agent\'s task; the roles and rules are the same for any model.'],
  checkAddress:['Адрес предмета','Address of the subject'],
  checkObservation:['Наблюдение записано','Observation written'],
  checkGrounds:['Основания','Grounds'],
  checkChangeEmpty:['Заполнены «было» и «стало»','“Was” and “becomes” are filled'],
  checkChangeSame:['«Стало» отличается от «было»','“Becomes” differs from “was”'],
  checkFromFound:['«Было» есть в словах предмета','“Was” is in the subject\'s words'],
  checkTreeWord:['Совпадает со словом Древа','Matches the Tree\'s word'],
  checkRepeat:['Похожих открытых тикетов нет','No similar open tickets'],
  question:['Вопрос','Question'],
};
const categoryLabels={
  translation:['Перевод','Translation'],'tree-word':['Слово Древа','The Tree\'s word'],text:['Текст и транскрипция','Text and transcription'],
  alignment:['Выравнивание','Alignment'],relation:['Связь','Link'],reading:['Прочтение','Reading'],'source-dating':['Источник и датировка','Source and dating'],
  media:['Медиа','Media'],open:['Открытая','Open'],
};
const variantLabels={
  outdated:['устарел','outdated'],inaccurate:['неточен','inaccurate'],missing:['отсутствует','missing'],transcription:['транскрипция','transcription'],
  punctuation:['пунктуация','punctuation'],verb:['глагол','verb'],grounds:['основания','grounds'],superfluous:['лишняя','superfluous'],
  'counter-reading':['контрпрочтение','counter-reading'],scope:['область','scope'],date:['дата','date'],place:['место','place'],source:['источник','source'],
  caption:['подпись','caption'],rights:['права','rights'],region:['область изображения','image region'],
};
const fieldLabels={verb:['глагол связи','verb of the link'],sense:['смысл','sense'],question:['вопрос','question'],answer:['ответ','answer'],title:['название','title']};
const checkLabels={address:'checkAddress',observation:'checkObservation',grounds:'checkGrounds','change-empty':'checkChangeEmpty','change-same':'checkChangeSame','from-found':'checkFromFound','tree-word':'checkTreeWord',repeat:'checkRepeat'};

export function mountActDesk({shell,actionLayer,space,acts,registry=null,api,locale=()=> 'ru',profile='zarathustra',storage=globalThis.localStorage}){
  const ru=()=>resolveLocale(locale())==='ru';
  const pick=pair=>ru()?pair[0]:localeText(pair[1],locale());
  const t=key=>labels[key]?pick(labels[key]):key;
  const doc=shell.ownerDocument,view=doc.defaultView;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const option=(text,value)=>{const node=doc.createElement('option');node.textContent=text;node.value=value;return node;};
  const field=(label,control)=>{const wrap=el('label','ad-field');wrap.append(el('span','ad-label',label),control);return wrap;};
  const dialogs=[];

  function sheet(title){
    const dialog=el('dialog','ad-sheet'),head=el('div','ad-head'),heading=el('h2','',title),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});
    dialog.addEventListener('close',()=>dialog.remove());
    head.append(heading,close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);dialog.showModal();
    return {dialog,body};
  }

  // ------------------------------------------------------------ addresses and grounds
  const index=()=>space?.index;
  const titleOf=item=>{const title=item?.title;return typeof title==='string'?title:title?localeValue(title,locale()):null;};
  function addressOf(target){
    if(target.id)return target.id;
    if(target.passage?.reference){try{return referenceKey(target.passage.reference);}catch{return null;}}
    // A whole chapter is addressed without a unit; a passage names its unit and span.
    if(target.stand){const s=target.stand;return s.unitId?`place:${profile}:${s.language}:${s.chapter}:${s.unitId}:${s.start}-${s.end}`:`place:${profile}:${s.language}:${s.chapter}`;}
    return null;
  }
  function groundsOf(target){
    const address=addressOf(target);
    if(target.type==='passage')return address?[address]:[];
    const record=target.item?.record;if(!record)return [];
    const places=Array.isArray(record.grounds)&&record.grounds.length?record.grounds:citedPlaces(record);
    const out=[];for(const cite of places.slice(0,16)){try{out.push(citationId(cite));}catch{}}
    return [...new Set(out)];
  }
  const nameOfAddress=address=>titleOf(index()?.get?.(address))??address;

  // ------------------------------------------------------------ ticket (B2) with checks (B3)
  /** `preset` may choose the category and begin the observation, as the frontier does for its places. */
  async function openTicket(target,preset={}){
    const {dialog,body}=sheet(t('ticket'));
    const kinds=await acts.query({view:'kinds'});
    const subject=el('p','ad-subject',target.label||addressOf(target)||'');
    const category=el('select','ad-input');category.id='ad-category';
    for(const item of kinds.categories)category.append(option(categoryLabels[item.name]?pick(categoryLabels[item.name]):item.name,item.name));
    const variant=el('select','ad-input');variant.id='ad-variant';
    const observation=el('textarea','ad-input ad-text');observation.id='ad-observation';observation.rows=3;observation.placeholder=t('observationHint');
    const dictate=button(t('dictate'),'text-button ad-dictate');
    const proposeToggle=el('input');proposeToggle.type='checkbox';proposeToggle.id='ad-propose';
    const proposeRow=el('label','ad-check-row');proposeRow.append(proposeToggle,el('span','',t('propose')));
    const changeBox=el('div','ad-change');changeBox.hidden=true;
    const checks=el('ul','ad-checks');checks.setAttribute('aria-live','polite');
    const save=button(t('save'),'primary ad-save');
    const terms=space?.actions?.terms?.()??[];
    let inputs={},termSelect=null,lemma=null,fieldSelect=null;
    const categoryRule=()=>kinds.categories.find(item=>item.name===category.value)??{variants:[],change_required:[],group_by:[]};
    function paintVariants(){
      const rule=categoryRule();variant.replaceChildren(option('—',''));
      for(const name of rule.variants)variant.append(option(variantLabels[name]?pick(variantLabels[name]):name,name));
      variant.disabled=!rule.variants.length;
    }
    function paintChange(){
      const rule=categoryRule();changeBox.replaceChildren();inputs={};termSelect=lemma=fieldSelect=null;
      proposeRow.hidden=!rule.change_required.length&&category.value!=='open';
      if(proposeRow.hidden){proposeToggle.checked=false;}
      changeBox.hidden=!proposeToggle.checked;
      const names=rule.change_required.length?rule.change_required:['statement'];
      if(target.type==='link'||target.type==='star'){
        fieldSelect=el('select','ad-input');fieldSelect.id='ad-field';fieldSelect.append(option('—',''));
        for(const [name,pair] of Object.entries(fieldLabels))fieldSelect.append(option(pick(pair),name));
        changeBox.append(field(t('field'),fieldSelect));
      }
      for(const name of names){
        const input=el(name==='statement'?'textarea':'input','ad-input');input.id=`ad-change-${name}`;if(name==='statement')input.rows=2;
        inputs[name]=input;changeBox.append(field(t(name),input));input.addEventListener('input',paintChecks);
      }
      if(rule.group_by.includes('term')&&terms.length){
        termSelect=el('select','ad-input');termSelect.id='ad-term';termSelect.append(option(t('noTerm'),''));
        for(const term of terms)termSelect.append(option(`${term.original?.lemma??term.id} — ${term.renderings?.ru?.tos?.term??''}`,term.id));
        lemma=el('input','ad-input');lemma.id='ad-to-lemma';
        termSelect.addEventListener('change',()=>{const term=terms.find(item=>item.id===termSelect.value);lemma.value=term?.renderings?.ru?.tos?.term??'';paintChecks();});
        lemma.addEventListener('input',paintChecks);
        changeBox.append(field(t('term'),termSelect),field(t('treeWord'),lemma));
      }
    }
    function draft(){
      const change=proposeToggle.checked?Object.fromEntries(Object.entries(inputs).map(([name,input])=>[name,input.value.trim()])):null;
      if(change&&fieldSelect?.value)change.field=fieldSelect.value;
      if(change&&termSelect?.value){change.term=termSelect.value;change.to_lemma=lemma.value.trim();}
      return {category:category.value,variant:variant.value||null,observation:observation.value.trim(),change};
    }
    let similar=[];
    const address=addressOf(target),grounds=groundsOf(target);
    if(address){
      const found=await acts.query({view:'target',address,limit:100});
      similar=await Promise.all(found.acts.filter(act=>act.kind==='ticket').map(async act=>({...act,category:(await acts.record(act.id))?.body?.category})));
    }
    function paintChecks(){
      const value=draft(),term=terms.find(item=>item.id===value.change?.term);
      const result=checkTicket({target,draft:value,grounds,known:id=>Boolean(index()?.get?.(id)),similar,termWord:term?.renderings?.ru?.tos?.term??null});
      checks.replaceChildren(...result.map(check=>{const row=el('li','ad-check');row.dataset.level=check.level;
        row.append(el('span','ad-check-mark',check.level==='ok'?'✓':check.level==='warn'?'!':'×'),el('span','',`${t(checkLabels[check.id]??check.id)}${check.detail&&check.id!=='tree-word'?` · ${check.detail}`:''}${check.id==='tree-word'&&check.detail?` · ${check.detail}`:''}`));return row;}));
      save.disabled=!checksPass(result);
      return result;
    }
    category.addEventListener('change',()=>{paintVariants();paintChange();paintChecks();});
    proposeToggle.addEventListener('change',()=>{changeBox.hidden=!proposeToggle.checked;paintChecks();});
    observation.addEventListener('input',paintChecks);
    // Dictation uses the browser's own speech recognition where the page may use the microphone.
    const Recognition=view.SpeechRecognition??view.webkitSpeechRecognition;
    if(!Recognition)dictate.hidden=true;
    dictate.addEventListener('click',()=>{
      try{const recognition=new Recognition();recognition.lang=SPEECH_LANGUAGES.get(resolveLocale(locale()))??'ru-RU';recognition.interimResults=false;
        dictate.textContent=t('dictating');
        recognition.onresult=event=>{const text=[...event.results].map(result=>result[0]?.transcript??'').join(' ').trim();observation.value=[observation.value.trim(),text].filter(Boolean).join(' ');paintChecks();};
        recognition.onerror=()=>{actionLayer?.say?.(t('noVoice'));};recognition.onend=()=>{dictate.textContent=t('dictate');};recognition.start();}
      catch{actionLayer?.say?.(t('noVoice'));}
    });
    save.addEventListener('click',async()=>{
      const result=paintChecks();if(!checksPass(result))return;
      const value=draft();save.disabled=true;
      try{
        const firstLine=value.observation.split(/\n/)[0];
        const ticket=await acts.act({kind:'ticket',targets:[address],grounds,intent:firstLine,body:{category:value.category,...(value.variant?{variant:value.variant}:{}),observation:value.observation}});
        if(value.change){
          const intent=value.change.from?`«${value.change.from}» → «${value.change.to}»`:value.change.statement??firstLine;
          await acts.act({kind:'proposal',targets:[address],grounds,parents:[ticket.record.id],intent,body:{category:value.category,...(value.variant?{variant:value.variant}:{}),change:value.change}});
        }
        actionLayer?.say?.(t(value.change?'savedProposal':'saved'));dialog.close();
      }catch(error){actionLayer?.say?.(String(error?.message??error));save.disabled=false;}
    });
    const voiceRow=el('div','ad-row');voiceRow.append(dictate);
    body.append(subject,field(t('category'),category),field(t('variant'),variant),field(t('observation'),observation),voiceRow,proposeRow,changeBox,el('h3','ad-minor',t('checks')),checks,save);
    if(preset.category&&kinds.categories.some(item=>item.name===preset.category))category.value=preset.category;
    if(preset.observation)observation.value=preset.observation;
    paintVariants();paintChange();paintChecks();observation.focus();
  }

  // ------------------------------------------------------------ board (B4)
  async function openBoard(){
    const {body}=sheet(t('board'));
    async function render(){
      const board=await acts.query({view:'board',limit:200});
      const columns=el('div','ad-board');
      const records=new Map(await Promise.all(board.tickets.map(async ticket=>[ticket.id,await acts.record(ticket.id)])));
      for(const status of ['open','with-agent','returned','reviewed']){
        const column=el('section','ad-column');column.dataset.status=status;
        const items=board.tickets.filter(ticket=>ticket.status===status);
        column.append(el('h3','ad-minor',`${t(`status-${status}`)} · ${board.counts[status]??items.length}`));
        if(!items.length)column.append(el('p','ad-muted',t('empty')));
        for(const ticket of items){
          const record=records.get(ticket.id),card=el('article','ad-ticket');
          card.append(el('strong','',record?.intent??ticket.id),el('p','ad-muted',`${categoryLabels[ticket.category]?pick(categoryLabels[ticket.category]):ticket.category} · ${nameOfAddress(ticket.targets[0])}`));
          if(ticket.assignee)card.append(el('p','ad-muted',`${t('assignee')}: ${ticket.assignee}`));
          if(ticket.replies)card.append(el('p','ad-muted',`${ticket.replies} ${t('replies')}`));
          const row=el('div','ad-row');
          const open=button(t('open'),'text-button');open.addEventListener('click',()=>{const id=ticket.targets[0];if(index()?.get?.(id))void (registry?registry.invoke('tos.page.select',{item_id:id}):null);});
          row.append(open);
          if(status==='open'){
            const assign=button(t('assign'),'text-button');
            assign.addEventListener('click',async()=>{const assignee=el('input','ad-input');assignee.value='agent';const go=button(t('assign'),'primary');
              row.replaceChildren(assignee,go);assignee.focus();
              go.addEventListener('click',async()=>{const who=assignee.value.trim();if(!who)return;await acts.act({kind:'assignment',targets:[`act:${ticket.id}`],intent:`${t('assign')}: ${who}`,body:{assignee:who}});await render();});});
            row.append(assign);
          }
          const withdraw=button(t('withdraw'),'text-button');withdraw.addEventListener('click',async()=>{await acts.act({kind:'withdrawal',targets:[`act:${ticket.id}`],intent:t('withdraw'),body:{}});await render();});
          row.append(withdraw);card.append(row);column.append(card);
        }
        columns.append(column);
      }
      body.replaceChildren(columns);
    }
    await render();
  }

  // ------------------------------------------------------------ review (B8) and the change packet
  async function openReview(){
    const {body}=sheet(t('review'));
    async function render(){
      const review=await acts.query({view:'review',limit:50});
      const decided=await acts.query({view:'decided',limit:1000});
      const list=el('div','ad-groups');
      if(!review.groups.length)list.append(el('p','ad-muted',t('nothingOpen')));
      for(const group of review.groups){
        const box=el('article','ad-group');
        const sample=(await Promise.all(group.open.slice(0,6).map(id=>acts.record(id)))).filter(Boolean);
        const change=sample[0]?.body?.change??{};
        const termName=(space?.actions?.terms?.()??[]).find(term=>term.id===change.term)?.original?.lemma??change.term??'';
        box.append(el('h3','',`${categoryLabels[group.category]?pick(categoryLabels[group.category]):group.category}${change.to_lemma?` · ${termName} → ${change.to_lemma}`:''}`),
          el('p','ad-muted',`${group.open_count} ${t('proposals')} · ${group.targets_count} ${t('places')} · ${group.independent_count} ${t('independent')}${group.models.length?` (${group.models.join(', ')})`:''}`));
        const rows=el('ul','ad-sample');
        for(const record of sample){const item=el('li');item.append(el('span','ad-place',nameOfAddress(record.targets[0])),el('span','',record.body?.change?.from?`«${record.body.change.from}» → «${record.body.change.to}»`:record.intent));rows.append(item);}
        box.append(rows);
        const reason=el('input','ad-input');reason.placeholder=t('reasonHint');reason.id=`ad-reason-${group.group.slice(-8)}`;
        const edit=el('input','ad-input');edit.placeholder=t('edit');edit.hidden=true;edit.id=`ad-edit-${group.group.slice(-8)}`;
        const decide=el('div','ad-row');
        for(const decision of kinds.decisions){
          const go=button(t(decision),decision==='accept'?'primary':'text-button');
          go.addEventListener('click',async()=>{
            if(decision==='accept-with-edit'&&edit.hidden){edit.hidden=false;edit.focus();return;}
            if(!reason.value.trim()){reason.focus();reason.setAttribute('aria-invalid','true');return;}
            await acts.act({kind:'review',targets:[group.group],intent:`${t(decision)}: ${reason.value.trim()}`.slice(0,280),body:{decision,reason:reason.value.trim(),...(decision==='accept-with-edit'?{edit:edit.value.trim()}:{})}});
            await render();
          });
          decide.append(go);
        }
        box.append(field(t('reason'),reason),edit,decide);list.append(box);
      }
      const packed=el('section','ad-packet');
      const unpacked=decided.proposals.filter(row=>!row.packed);
      packed.append(el('h3','ad-minor',`${t('accepted')} · ${unpacked.length}`),el('p','ad-muted',t('packetLead')));
      if(unpacked.length){const gather=button(t('packet'),'primary');gather.addEventListener('click',async()=>{const packet=await gatherPacket(unpacked);showPacket(packet);await render();});packed.append(gather);}
      body.replaceChildren(list,packed);
    }
    const kinds=await acts.query({view:'kinds'});
    await render();
  }
  /** One change packet: up to 64 accepted proposals whose reviews fit in 32 parents. */
  async function gatherPacket(rows){
    const chosen=[],reviews=new Set();
    for(const row of rows){if(chosen.length>=64)break;if(!reviews.has(row.review)&&reviews.size>=32)continue;reviews.add(row.review);chosen.push(row);}
    const changes=[];
    for(const row of chosen){
      const [proposal,review]=await Promise.all([acts.record(row.proposal),acts.record(row.review)]);
      if(!proposal)continue;
      changes.push({proposal:row.proposal,review:row.review,decision:row.decision,...(review?.body?.edit?{edit:review.body.edit}:{}),reason:review?.body?.reason??'',
        category:proposal.body?.category,...(proposal.body?.variant?{variant:proposal.body.variant}:{}),target:proposal.targets[0],targets:proposal.targets,change:proposal.body?.change??{},grounds:proposal.grounds});
    }
    const stored=await acts.act({kind:'change-packet',targets:changes.map(change=>`act:${change.proposal}`),parents:[...reviews],intent:`${t('packet')}: ${changes.length}`,
      body:{format:'tos.change-packet.v1',stand:profile,changes}});
    return {schema:'tos.change-packet.v1',act:stored.record.id,digest:stored.record.digest,stand:profile,changes,
      instructions:'Apply each change through its owner\'s route (the bonsai assembly inputs or terms.json), keep the witness texts as they are, and run the bonsai check. Nothing in this packet is reviewed by the Tree.'};
  }
  function packetMarkdown(packet){
    const lines=[`# Change packet ${packet.act}`,'',`Stand: ${packet.stand}. Digest: ${packet.digest}.`,'',packet.instructions,''];
    for(const change of packet.changes){
      lines.push(`- **${change.category}${change.variant?` (${change.variant})`:''}** \`${change.target}\`${change.change.field?` · ${change.change.field}`:''}: ${change.change.from?`«${change.change.from}» → «${change.change.to}»`:change.change.statement??''}${change.edit?` · edit: ${change.edit}`:''}`);
      lines.push(`  - ${change.decision}: ${change.reason}`);
      if(change.grounds?.length)lines.push(`  - grounds: ${change.grounds.map(ground=>`\`${ground}\``).join(', ')}`);
    }
    return lines.join('\n');
  }
  function showPacket(packet){
    const {body}=sheet(t('packet'));
    const json=JSON.stringify(packet,null,2),markdown=packetMarkdown(packet);
    const row=el('div','ad-row'),copyJson=button(t('copyJson'),'primary'),copyMarkdown=button(t('copyMarkdown'),'text-button');
    copyJson.addEventListener('click',()=>void actionLayer?.copy?.(json));copyMarkdown.addEventListener('click',()=>void actionLayer?.copy?.(markdown));
    row.append(copyJson,copyMarkdown);
    body.append(el('p','ad-muted',t('packetLead')),row,el('pre','ad-pre',markdown));
  }

  // ------------------------------------------------------------ hand over (D1, D2)
  async function formatsFor(target){
    const address=addressOf(target),link=actionLayer?.linkTo?.(target)??'';
    const record=target.item?.record,ground=Array.isArray(record?.grounds)?record.grounds[0]:null;
    const quote=target.passage?.quote??target.quote??ground?.q??null;
    const place=target.passage?[target.passage.title,target.passage.versionLabel].filter(Boolean).join(', '):target.stand?target.stand.chapter:ground?`${ground.w} ${ground.c??''}`.trim():'';
    const grounds=groundsOf(target);
    const out=[];
    out.push({id:'markdown',label:'formatMarkdown',text:[`### ${target.label}`,'',`[${address}](${link})`,...(quote?['',`> ${quote}`,`> — ${place}`]:[]),...(grounds.length?['',...grounds.map(ground=>`- \`${ground}\``)]:[])].join('\n')});
    if(quote)out.push({id:'quote',label:'formatQuote',text:`«${String(quote).replace(/^[«"“]+|[»"”]+$/gu,'')}» — ${place}${link?`\n${link}`:''}`});
    let packet=null;
    if(target.id&&registry){try{packet=await registry.invoke('tos.page.research-item',{id:target.id});}catch{packet=null;}}
    out.push({id:'json',label:'formatJson',text:JSON.stringify(packet??{address,label:target.label,quote,place,grounds,...(target.passage?{reference:target.passage.reference}:{}),...(target.stand?{place:target.stand}:{})},null,2)});
    const question=api?.getQuestion?.()?.trim();
    out.push({id:'agent',label:'formatAgent',text:[
      `Subject: ${target.label} (${address})`,link?`Open: ${link}`:'',question?`Question: ${question}`:'',
      grounds.length?`Grounds to read first: ${grounds.join(', ')}`:'',
      '',...roles.rules.map(rule=>`- ${rule}`),'',
      'Return proposals as acts (tos.act.append) with exact grounds, or as text with exact places when you work without the page.',
    ].filter(line=>line!==undefined).join('\n').replace(/\n{3,}/g,'\n\n')});
    return {address,formats:out};
  }
  async function openTransfer(target){
    const {body}=sheet(`${t('transfer').replace('…','')} · ${target.label}`);
    const {address,formats}=await formatsFor(target);
    const tabs=el('div','ad-row'),pre=el('pre','ad-pre'),copy=button(t('copy'),'primary');
    let current=formats[0];
    const paint=()=>{pre.textContent=current.text;for(const tab of tabs.children)tab.setAttribute('aria-pressed',String(tab.dataset.format===current.id));};
    for(const format of formats){const tab=button(t(format.label),'text-button ad-tab');tab.dataset.format=format.id;tab.addEventListener('click',()=>{current=format;paint();});tabs.append(tab);}
    copy.addEventListener('click',async()=>{
      const ok=await actionLayer?.copy?.(current.text);
      if(ok&&address)void acts.act({kind:'transfer',targets:[address],intent:`${t(current.label)}`,body:{format:current.id,recipient:'clipboard',bytes:current.text.length}}).catch(()=>{});
    });
    body.append(tabs,pre,copy);paint();
  }

  // ------------------------------------------------------------ connect an agent (C1) and its tier (C8)
  const readTier=()=>{try{const value=storage?.getItem(AGENT_TIER_KEY);return TIERS.includes(value)?value:'propose';}catch{return 'propose';}};
  let tier=readTier();
  function openConnect(){
    const {body}=sheet(t('connect'));
    const group=el('div','ad-tiers');group.setAttribute('role','radiogroup');group.setAttribute('aria-label',t('tier'));
    for(const value of TIERS){
      const input=el('input');input.type='radio';input.name='ad-tier';input.value=value;input.id=`ad-tier-${value}`;input.checked=tier===value;
      input.addEventListener('change',()=>{tier=value;try{storage?.setItem(AGENT_TIER_KEY,value);}catch{}paint();});
      const label=el('label','ad-check-row');label.append(input,el('span','',t(`tier-${value}`)));group.append(label);
    }
    const pre=el('pre','ad-pre'),copy=button(t('copy'),'primary');
    const paint=()=>{pre.textContent=instruction();};
    copy.addEventListener('click',()=>void actionLayer?.copy?.(pre.textContent));
    body.append(el('p','ad-muted',t('connectLead')),el('h3','ad-minor',t('tier')),group,el('h3','ad-minor',t('instruction')),pre,copy);paint();
  }
  function instruction(){
    const page=view.location.href.split('#')[0];
    return [
      'You are the agent of the Tree of Sophia, working in the person\'s act journal.',
      `Page: ${page}`,
      `Tier: ${tier}.`,
      'On the page, use its WebMCP tools: read tos.act.manifest first, then tos.page.context and tos.bonsai.view-state; read places with tos.bonsai.read-source; append with tos.act.append.',
      'Without the page, use the native server: tos mcp, prompt tos-tree-agent.',
      '','Rules:',...roles.rules.map(rule=>`- ${rule}`),
      '','Roles:',...roles.roles.map(role=>`- ${role.name.en} (${role.id}): ${role.does} Appends: ${role.appends.join(', ')}.`),
    ].join('\n');
  }

  // ------------------------------------------------------------ the actions in the menu and palette
  const remove=actionLayer?.addActions?.([
    {id:'ticket',text:()=>t('ticket'),for:['star','link','passage'],run:target=>openTicket(target)},
    {id:'transfer',text:()=>t('transfer'),for:['star','link','passage'],run:target=>openTransfer(target)},
    {id:'board',text:()=>t('board'),for:['any'],run:()=>openBoard()},
    {id:'review',text:()=>t('review'),for:['any'],run:()=>openReview()},
    {id:'connect',text:()=>t('connect'),for:['any'],run:()=>openConnect()},
  ]);

  return {openTicket,openBoard,openReview,openTransfer,openConnect,tier:()=>tier,instruction,
    destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
